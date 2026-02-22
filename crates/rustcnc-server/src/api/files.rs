use std::path::Path as FsPath;
use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use tokio::io::AsyncWriteExt;
use tracing::{error, info};

use rustcnc_core::ws_protocol::{FileInfo, ServerMessage};
use rustcnc_planner::planner::PlannerCommand;

use crate::state::{AppState, StoredFile};

/// Broadcast updated file list to all WebSocket clients
fn broadcast_file_list(state: &AppState) {
    let files = state.files.read();
    let infos: Vec<FileInfo> = files
        .iter()
        .map(|f| FileInfo {
            id: f.id.to_string(),
            name: f.name.clone(),
            size_bytes: f.size_bytes,
            line_count: f.line_count,
            loaded_at: f.uploaded_at.to_rfc3339(),
        })
        .collect();
    let _ = state
        .ws_broadcast_tx
        .send(ServerMessage::FileListUpdated(infos));
}

/// GET /api/files
pub async fn list_files(State(state): State<Arc<AppState>>) -> Json<Vec<FileInfo>> {
    let files = state.files.read();
    let infos: Vec<FileInfo> = files
        .iter()
        .map(|f| FileInfo {
            id: f.id.to_string(),
            name: f.name.clone(),
            size_bytes: f.size_bytes,
            line_count: f.line_count,
            loaded_at: f.uploaded_at.to_rfc3339(),
        })
        .collect();
    Json(infos)
}

/// POST /api/files (multipart upload)
pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<FileInfo>, StatusCode> {
    let upload_dir = std::path::PathBuf::from(&state.config.files.upload_dir);
    let max_bytes = state
        .config
        .files
        .max_file_size_mb
        .saturating_mul(1024 * 1024);

    tokio::fs::create_dir_all(&upload_dir).await.map_err(|e| {
        error!(
            "Failed to create upload_dir {}: {}",
            upload_dir.display(),
            e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let Some(mut field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to read multipart field: {}", e);
        StatusCode::BAD_REQUEST
    })?
    else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let original_name = field.file_name().unwrap_or("unknown.gcode");
    let safe_name = FsPath::new(original_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown.gcode")
        .to_string();

    let id = uuid::Uuid::new_v4();
    let dest_path = upload_dir.join(format!("{}_{}", id, safe_name));

    let mut out = tokio::fs::File::create(&dest_path).await.map_err(|e| {
        error!("Failed to create file {}: {}", dest_path.display(), e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut size_bytes: u64 = 0;
    let mut newline_count: usize = 0;
    let mut last_byte: Option<u8> = None;

    while let Some(chunk) = field.chunk().await.map_err(|e| {
        error!("Failed to read upload chunk: {}", e);
        StatusCode::BAD_REQUEST
    })? {
        size_bytes = size_bytes.saturating_add(chunk.len() as u64);
        if size_bytes > max_bytes {
            let _ = tokio::fs::remove_file(&dest_path).await;
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }

        newline_count += chunk.iter().filter(|&&b| b == b'\n').count();
        last_byte = chunk.last().copied();

        out.write_all(&chunk).await.map_err(|e| {
            error!("Failed to write upload file {}: {}", dest_path.display(), e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    let line_count = if size_bytes == 0 {
        0
    } else {
        newline_count + usize::from(last_byte != Some(b'\n'))
    };

    info!(
        "Uploaded file: {} -> {} ({} bytes)",
        safe_name,
        dest_path.display(),
        size_bytes
    );

    let stored = StoredFile {
        id,
        name: safe_name.clone(),
        path: dest_path.clone(),
        size_bytes,
        line_count,
        uploaded_at: chrono::Utc::now(),
    };

    let file_info = FileInfo {
        id: stored.id.to_string(),
        name: stored.name.clone(),
        size_bytes: stored.size_bytes,
        line_count: stored.line_count,
        loaded_at: stored.uploaded_at.to_rfc3339(),
    };

    state.files.write().push(stored);
    broadcast_file_list(&state);

    // Parse once, in the planner, from disk (also generates GCodeLoaded for 3D viewer).
    let _ = state
        .planner_tx
        .send(PlannerCommand::LoadFile {
            path: dest_path.to_string_lossy().into_owned(),
        })
        .await;

    Ok(Json(file_info))
}

/// DELETE /api/files/:id
pub async fn delete_file(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> StatusCode {
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    let removed = {
        let mut files = state.files.write();
        let Some(idx) = files.iter().position(|f| f.id == id) else {
            return StatusCode::NOT_FOUND;
        };
        files.remove(idx)
    };

    if let Err(e) = tokio::fs::remove_file(&removed.path).await {
        warn_if_not_found(&removed.path, e);
    }
    broadcast_file_list(&state);
    StatusCode::NO_CONTENT
}

/// POST /api/files/:id/load
pub async fn load_file(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> StatusCode {
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    let path = {
        let files = state.files.read();
        if let Some(file) = files.iter().find(|f| f.id == id) {
            info!("Loading file for job: {}", file.name);
            file.path.to_string_lossy().into_owned()
        } else {
            return StatusCode::NOT_FOUND;
        }
    };

    // Send to planner so it becomes the active job
    let _ = state
        .planner_tx
        .send(PlannerCommand::LoadFile { path })
        .await;

    StatusCode::OK
}

fn warn_if_not_found(path: &std::path::Path, e: std::io::Error) {
    // `remove_file` can fail if the file was already removed; that's fine.
    if e.kind() != std::io::ErrorKind::NotFound {
        tracing::warn!("Failed to remove {}: {}", path.display(), e);
    }
}

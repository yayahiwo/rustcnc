use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use tracing::{error, info};

use rustcnc_core::ws_protocol::FileInfo;
use rustcnc_planner::planner::PlannerCommand;

use crate::state::AppState;

/// GET /api/files
pub async fn list_files(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<FileInfo>> {
    let files = state.files.read();
    let infos: Vec<FileInfo> = files
        .iter()
        .map(|f| FileInfo {
            id: f.id.to_string(),
            name: f.name.clone(),
            size_bytes: f.lines.iter().map(|l| l.byte_len as u64).sum(),
            line_count: f.total_lines,
            loaded_at: f.loaded_at.to_rfc3339(),
        })
        .collect();
    Json(infos)
}

/// POST /api/files (multipart upload)
pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<FileInfo>, StatusCode> {
    while let Ok(Some(field)) = multipart.next_field().await {
        let name: String = field
            .file_name()
            .unwrap_or("unknown.gcode")
            .to_string();

        let data: axum::body::Bytes = field.bytes().await.map_err(|e| {
            error!("Failed to read upload: {}", e);
            StatusCode::BAD_REQUEST
        })?;

        let content = String::from_utf8_lossy(&data).into_owned();
        info!("Uploaded file: {} ({} bytes)", name, content.len());

        // Send to planner to parse
        let _ = state
            .planner_tx
            .send(PlannerCommand::LoadContent {
                name: name.clone(),
                content,
            })
            .await;

        // Return basic info (full file info will come via WebSocket)
        return Ok(Json(FileInfo {
            id: "pending".into(),
            name,
            size_bytes: data.len() as u64,
            line_count: 0,
            loaded_at: chrono::Utc::now().to_rfc3339(),
        }));
    }

    Err(StatusCode::BAD_REQUEST)
}

/// DELETE /api/files/:id
pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> StatusCode {
    let mut files = state.files.write();
    let before = files.len();
    files.retain(|f| f.id.to_string() != id);
    if files.len() < before {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

/// POST /api/files/:id/load
pub async fn load_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> StatusCode {
    let files = state.files.read();
    if let Some(file) = files.iter().find(|f| f.id.to_string() == id) {
        // File is already loaded in memory; notify planner
        info!("Loading file for job: {}", file.name);
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use rustcnc_streamer::streamer::StreamerCommand;

use crate::state::AppState;

/// GET /api/settings
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::to_value(&state.config).unwrap_or_default())
}

/// POST /api/settings
pub async fn update_settings(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<serde_json::Value>,
) -> StatusCode {
    // Settings update would modify config in memory
    // For now, return OK
    StatusCode::OK
}

/// GET /api/settings/grbl
/// Request GRBL settings from the controller ($$)
pub async fn get_grbl_settings(
    State(state): State<Arc<AppState>>,
) -> StatusCode {
    let _ = state
        .streamer_cmd_tx
        .send(StreamerCommand::RawCommand("$$".into()));
    StatusCode::OK
}

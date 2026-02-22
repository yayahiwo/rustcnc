use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct SystemInfo {
    pub version: String,
    pub platform: String,
    pub uptime_secs: u64,
    pub connected: bool,
}

/// GET /api/system/info
pub async fn system_info(State(state): State<Arc<AppState>>) -> Json<SystemInfo> {
    Json(SystemInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        uptime_secs: 0, // TODO: track start time
        connected: state
            .machine_state
            .connected
            .load(std::sync::atomic::Ordering::Relaxed),
    })
}

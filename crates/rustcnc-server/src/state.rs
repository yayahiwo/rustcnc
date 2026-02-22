use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::{broadcast, mpsc};

use rustcnc_core::config::AppConfig;
use rustcnc_core::ws_protocol::{GCodeFileInfo, JobProgress, ServerMessage};
use rustcnc_planner::planner::PlannerCommand;
use rustcnc_streamer::streamer::{SharedMachineState, StreamerCommand};

#[derive(Debug, Clone)]
pub struct StoredFile {
    pub id: uuid::Uuid,
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub line_count: usize,
    pub uploaded_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub username: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Shared application state, passed to all Axum handlers via State extractor
pub struct AppState {
    /// Atomic machine state (updated by streamer, read by WS broadcaster)
    pub machine_state: Arc<SharedMachineState>,

    /// Channel to send commands to the planner
    pub planner_tx: mpsc::Sender<PlannerCommand>,

    /// Channel to send real-time commands directly to streamer
    pub streamer_cmd_tx: crossbeam_channel::Sender<StreamerCommand>,

    /// Broadcast channel for WebSocket clients
    pub ws_broadcast_tx: broadcast::Sender<ServerMessage>,

    /// Currently loaded files
    pub files: RwLock<Vec<StoredFile>>,

    /// Current active job state (shared with planner event handler)
    pub job_progress: Arc<RwLock<Option<JobProgress>>>,

    /// Currently loaded G-code file info (for 3D viewer on reconnect)
    pub loaded_gcode: Arc<RwLock<Option<GCodeFileInfo>>>,

    /// Application configuration
    pub config: AppConfig,

    /// Active login sessions (in-memory).
    pub sessions: RwLock<HashMap<uuid::Uuid, AuthSession>>,

    /// Connection info
    pub connection_port: Arc<RwLock<Option<String>>>,
}

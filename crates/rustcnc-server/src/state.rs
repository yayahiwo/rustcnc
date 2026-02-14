use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::{broadcast, mpsc};

use rustcnc_core::config::AppConfig;
use rustcnc_core::gcode::GCodeFile;
use rustcnc_core::ws_protocol::{JobProgress, ServerMessage};
use rustcnc_planner::planner::PlannerCommand;
use rustcnc_streamer::streamer::{SharedMachineState, StreamerCommand};

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
    pub files: RwLock<Vec<GCodeFile>>,

    /// Current active job state
    pub job_progress: RwLock<Option<JobProgress>>,

    /// Application configuration
    pub config: AppConfig,

    /// Connection info
    pub connection_port: RwLock<Option<String>>,
}

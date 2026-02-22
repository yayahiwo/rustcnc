use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use rustcnc_core::grbl::realtime::RealtimeCommand;
use rustcnc_planner::planner::PlannerCommand;
use rustcnc_streamer::streamer::StreamerCommand;

use crate::state::AppState;

/// POST /api/job/start
pub async fn start_job(State(state): State<Arc<AppState>>) -> StatusCode {
    if !state
        .machine_state
        .connected
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return StatusCode::CONFLICT;
    }
    let _ = state
        .planner_tx
        .send(PlannerCommand::StartJob {
            start_line: None,
            stop_line: None,
        })
        .await;
    StatusCode::OK
}

/// POST /api/job/pause
pub async fn pause_job(State(state): State<Arc<AppState>>) -> StatusCode {
    if !state
        .machine_state
        .connected
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return StatusCode::CONFLICT;
    }
    let _ = state.planner_tx.send(PlannerCommand::PauseJob).await;
    StatusCode::OK
}

/// POST /api/job/resume
pub async fn resume_job(State(state): State<Arc<AppState>>) -> StatusCode {
    if !state
        .machine_state
        .connected
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return StatusCode::CONFLICT;
    }
    let _ = state.planner_tx.send(PlannerCommand::ResumeJob).await;
    StatusCode::OK
}

/// POST /api/job/cancel
pub async fn cancel_job(State(state): State<Arc<AppState>>) -> StatusCode {
    let _ = state.planner_tx.send(PlannerCommand::CancelJob).await;
    StatusCode::OK
}

/// POST /api/machine/home
pub async fn home(State(state): State<Arc<AppState>>) -> StatusCode {
    if !state
        .machine_state
        .connected
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return StatusCode::CONFLICT;
    }
    let _ = state
        .streamer_cmd_tx
        .send(StreamerCommand::RawCommand("$H".into()));
    StatusCode::OK
}

/// POST /api/machine/unlock
pub async fn unlock(State(state): State<Arc<AppState>>) -> StatusCode {
    if !state
        .machine_state
        .connected
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return StatusCode::CONFLICT;
    }
    let _ = state
        .streamer_cmd_tx
        .send(StreamerCommand::RawCommand("$X".into()));
    StatusCode::OK
}

/// POST /api/machine/reset
pub async fn reset(State(state): State<Arc<AppState>>) -> StatusCode {
    if !state
        .machine_state
        .connected
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return StatusCode::CONFLICT;
    }
    let _ = state
        .streamer_cmd_tx
        .send(StreamerCommand::Realtime(RealtimeCommand::SoftReset));
    StatusCode::OK
}

#[derive(Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

/// POST /api/machine/command
pub async fn send_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CommandRequest>,
) -> StatusCode {
    if !state
        .machine_state
        .connected
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return StatusCode::CONFLICT;
    }
    let _ = state
        .planner_tx
        .send(PlannerCommand::SendCommand(req.command))
        .await;
    StatusCode::OK
}

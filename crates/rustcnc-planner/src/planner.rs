use std::time::Instant;

use crossbeam_channel::Sender;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use rustcnc_core::gcode::GCodeFile;
use rustcnc_core::grbl::realtime::RealtimeCommand;
use rustcnc_core::job::JobState;
use rustcnc_streamer::streamer::{StreamerCommand, StreamerEvent};

use crate::file_loader;

/// Commands from the web server to the planner
#[derive(Debug)]
pub enum PlannerCommand {
    /// Load and parse a G-code file from a path
    LoadFile { path: String },
    /// Load G-code from raw content (uploaded file)
    LoadContent { name: String, content: String },
    /// Start streaming the loaded file
    StartJob,
    /// Pause the current job (sends feed hold)
    PauseJob,
    /// Resume the current job (sends cycle start)
    ResumeJob,
    /// Cancel the current job (sends soft reset, flushes queue)
    CancelJob,
    /// Send a raw command through the streamer
    SendCommand(String),
    /// Send a real-time command
    SendRealtime(RealtimeCommand),
    /// Shutdown
    Shutdown,
}

/// Events from the planner to the web server
#[derive(Debug, Clone)]
pub enum PlannerEvent {
    FileLoaded {
        file: GCodeFile,
    },
    FileError(String),
    JobStateChanged(JobState),
    JobProgress {
        current_line: usize,
        total_lines: usize,
        elapsed_secs: f64,
    },
}

/// The planner task orchestrates file loading, validation,
/// and feeding lines to the streamer at the right pace.
pub async fn planner_task(
    mut cmd_rx: mpsc::Receiver<PlannerCommand>,
    event_tx: mpsc::Sender<PlannerEvent>,
    streamer_cmd_tx: Sender<StreamerCommand>,
    mut streamer_event_rx: mpsc::Receiver<StreamerEvent>,
) {
    let mut current_file: Option<GCodeFile> = None;
    let mut job_state = JobState::Idle;
    let mut current_line_idx: usize = 0;
    let mut lines_acked: usize = 0;
    let mut job_start_time: Option<Instant> = None;

    info!("Planner task started");

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    PlannerCommand::LoadFile { path } => {
                        match file_loader::load_gcode_file(&path).await {
                            Ok(file) => {
                                info!("Loaded file: {} ({} lines)", file.name, file.total_lines);
                                let _ = event_tx.send(PlannerEvent::FileLoaded { file: file.clone() }).await;
                                current_file = Some(file);
                            }
                            Err(e) => {
                                error!("Failed to load file: {}", e);
                                let _ = event_tx.send(PlannerEvent::FileError(e.to_string())).await;
                            }
                        }
                    }
                    PlannerCommand::LoadContent { name, content } => {
                        let file = file_loader::parse_gcode_content(name.clone(), &content);
                        info!("Loaded content: {} ({} lines)", name, file.total_lines);
                        let _ = event_tx.send(PlannerEvent::FileLoaded { file: file.clone() }).await;
                        current_file = Some(file);
                    }
                    PlannerCommand::StartJob => {
                        if let Some(ref file) = current_file {
                            if job_state == JobState::Idle || job_state.is_terminal() {
                                info!("Starting job: {} ({} lines)", file.name, file.total_lines);
                                job_state = JobState::Running;
                                current_line_idx = 0;
                                lines_acked = 0;
                                job_start_time = Some(Instant::now());
                                let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;

                                // Feed initial batch of lines to streamer
                                feed_lines(&current_file, &mut current_line_idx, &streamer_cmd_tx);
                            }
                        } else {
                            warn!("No file loaded, cannot start job");
                        }
                    }
                    PlannerCommand::PauseJob => {
                        if job_state == JobState::Running {
                            let _ = streamer_cmd_tx.send(StreamerCommand::Realtime(RealtimeCommand::FeedHold));
                            job_state = JobState::Paused;
                            let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                        }
                    }
                    PlannerCommand::ResumeJob => {
                        if job_state == JobState::Paused {
                            let _ = streamer_cmd_tx.send(StreamerCommand::Realtime(RealtimeCommand::CycleStart));
                            job_state = JobState::Running;
                            let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                        }
                    }
                    PlannerCommand::CancelJob => {
                        if job_state.is_active() {
                            let _ = streamer_cmd_tx.send(StreamerCommand::Realtime(RealtimeCommand::SoftReset));
                            let _ = streamer_cmd_tx.send(StreamerCommand::Flush);
                            job_state = JobState::Cancelled;
                            current_line_idx = 0;
                            lines_acked = 0;
                            let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                        }
                    }
                    PlannerCommand::SendCommand(text) => {
                        let _ = streamer_cmd_tx.send(StreamerCommand::RawCommand(text));
                    }
                    PlannerCommand::SendRealtime(cmd) => {
                        let _ = streamer_cmd_tx.send(StreamerCommand::Realtime(cmd));
                    }
                    PlannerCommand::Shutdown => {
                        info!("Planner shutting down");
                        let _ = streamer_cmd_tx.send(StreamerCommand::Shutdown);
                        break;
                    }
                }
            }
            Some(event) = streamer_event_rx.recv() => {
                match event {
                    StreamerEvent::LineAcknowledged { .. } => {
                        if job_state == JobState::Running {
                            lines_acked += 1;
                            let total = current_file.as_ref().map(|f| f.total_lines).unwrap_or(0);
                            let elapsed = job_start_time.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);

                            let _ = event_tx.send(PlannerEvent::JobProgress {
                                current_line: lines_acked,
                                total_lines: total,
                                elapsed_secs: elapsed,
                            }).await;

                            // Feed more lines as space frees up
                            feed_lines(&current_file, &mut current_line_idx, &streamer_cmd_tx);

                            // Check if job is complete
                            if lines_acked >= total {
                                job_state = JobState::Completed;
                                info!("Job completed: {} lines in {:.1}s", total, elapsed);
                                let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                            }
                        }
                    }
                    StreamerEvent::LineError { line_number, code, message } => {
                        error!("Error on line {}: error:{} - {}", line_number, code, message);
                        if job_state == JobState::Running {
                            lines_acked += 1;
                            // Continue or stop depending on error severity
                            // For now, continue
                            feed_lines(&current_file, &mut current_line_idx, &streamer_cmd_tx);
                        }
                    }
                    StreamerEvent::Alarm { code } => {
                        if job_state.is_active() {
                            job_state = JobState::Error;
                            let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                        }
                    }
                    StreamerEvent::Welcome { .. } => {
                        // Controller was reset -- if job was running, it's lost
                        if job_state.is_active() {
                            job_state = JobState::Error;
                            let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Feed lines from the current file to the streamer channel.
/// Sends lines in batches to keep the streamer's buffer topped up.
fn feed_lines(
    file: &Option<GCodeFile>,
    line_idx: &mut usize,
    tx: &Sender<StreamerCommand>,
) {
    let Some(file) = file else { return };

    // Send up to 4 lines at a time (conservative to reduce backlog;
    // the streamer re-queues lines that can't fit in the GRBL buffer)
    let batch_size = 4;
    let mut sent = 0;

    while *line_idx < file.lines.len() && sent < batch_size {
        let line = &file.lines[*line_idx];
        let cmd = StreamerCommand::GcodeLine {
            text: line.text.clone(),
            byte_len: line.byte_len,
            line_number: line.file_line,
        };
        if tx.send(cmd).is_err() {
            break;
        }
        *line_idx += 1;
        sent += 1;
    }
}

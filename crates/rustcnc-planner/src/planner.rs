use std::time::Instant;

use crossbeam_channel::Sender;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use rustcnc_core::gcode::{extract_modal_preamble, GCodeFile};
use rustcnc_core::grbl::realtime::RealtimeCommand;
use rustcnc_core::job::JobState;
use rustcnc_core::ws_protocol::PauseCondition;
use rustcnc_streamer::streamer::{StreamerCommand, StreamerEvent};

use crate::estimator::{self, EstimationParams};
use crate::file_loader;

/// Commands from the web server to the planner
#[derive(Debug)]
pub enum PlannerCommand {
    /// Load and parse a G-code file from a path
    LoadFile { path: String },
    /// Load G-code from raw content (uploaded file)
    LoadContent { name: String, content: String },
    /// Start streaming the loaded file (optionally from/to specific line numbers)
    StartJob {
        start_line: Option<usize>,
        stop_line: Option<usize>,
    },
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
    /// Schedule a pause at a specific condition (None = cancel)
    SchedulePause(Option<PauseCondition>),
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
        estimated_remaining_secs: Option<f64>,
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
    let mut pause_condition: Option<PauseCondition> = None;
    let mut pause_pending = false;
    let mut current_z: f64 = 0.0;
    let mut stop_line_idx: Option<usize> = None;
    let mut range_total: usize = 0;

    // Estimation state
    let mut line_cumulative_secs: Option<Vec<f64>> = None;
    let mut job_start_line_idx: usize = 0;
    let mut job_end_line_idx: usize = 0;

    info!("Planner task started");

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    PlannerCommand::LoadFile { path } => {
                        match file_loader::load_gcode_file(&path).await {
                            Ok(mut file) => {
                                let params = EstimationParams::default();
                                let cum = estimator::estimate_line_times(&file, &params);
                                let total_est = cum.last().copied().unwrap_or(0.0);
                                file.estimated_duration_secs = if total_est > 0.0 { Some(total_est) } else { None };
                                info!("Loaded file: {} ({} lines, est {:.1}s)", file.name, file.total_lines, total_est);
                                line_cumulative_secs = Some(cum);
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
                        let mut file = file_loader::parse_gcode_content(name.clone(), &content);
                        let params = EstimationParams::default();
                        let cum = estimator::estimate_line_times(&file, &params);
                        let total_est = cum.last().copied().unwrap_or(0.0);
                        file.estimated_duration_secs = if total_est > 0.0 { Some(total_est) } else { None };
                        info!("Loaded content: {} ({} lines, est {:.1}s)", name, file.total_lines, total_est);
                        line_cumulative_secs = Some(cum);
                        let _ = event_tx.send(PlannerEvent::FileLoaded { file: file.clone() }).await;
                        current_file = Some(file);
                    }
                    PlannerCommand::StartJob { start_line, stop_line } => {
                        if let Some(ref file) = current_file {
                            if job_state == JobState::Idle || job_state.is_terminal() {
                                // Convert user's 1-based file line numbers to indices in file.lines
                                let start_idx = if let Some(sl) = start_line {
                                    file.lines.iter().position(|l| l.file_line >= sl).unwrap_or(file.lines.len())
                                } else {
                                    0
                                };
                                let stop_idx = if let Some(sl) = stop_line {
                                    // First index where file_line > stop_line (i.e., include stop_line)
                                    file.lines.iter().position(|l| l.file_line > sl).unwrap_or(file.lines.len())
                                } else {
                                    file.lines.len()
                                };

                                let total = stop_idx.saturating_sub(start_idx);
                                info!("Starting job: {} (lines {}-{}, {} of {} total)",
                                    file.name,
                                    start_line.unwrap_or(1),
                                    stop_line.unwrap_or(file.lines.last().map(|l| l.file_line).unwrap_or(0)),
                                    total,
                                    file.total_lines,
                                );

                                job_state = JobState::Running;
                                current_line_idx = start_idx;
                                lines_acked = 0;
                                range_total = total;
                                stop_line_idx = if stop_line.is_some() { Some(stop_idx) } else { None };
                                job_start_time = Some(Instant::now());
                                job_start_line_idx = start_idx;
                                job_end_line_idx = stop_idx;
                                pause_condition = None;
                                pause_pending = false;
                                current_z = 0.0;
                                let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;

                                // Always set incremental arc distance mode before job starts.
                                info!("Sending G91.1 before job start (incremental arc distance mode)");
                                let _ = streamer_cmd_tx.send(StreamerCommand::RawCommand("G91.1".to_string()));

                                // If starting from a line other than the beginning, we may emit a modal preamble
                                // that includes G90/G91. Some firmwares implicitly reset arc distance mode when
                                // distance mode is set, so re-assert G91.1 after the preamble as well.
                                // Send modal preamble if starting from a line other than the beginning
                                if start_idx > 0 {
                                    let preamble = extract_modal_preamble(&file.lines, start_idx);
                                    for cmd in &preamble {
                                        info!("Preamble: {}", cmd);
                                        let _ = streamer_cmd_tx.send(StreamerCommand::RawCommand(cmd.clone()));
                                    }
                                    let _ =
                                        streamer_cmd_tx.send(StreamerCommand::RawCommand("G91.1".to_string()));
                                }

                                // If the very first G-code line sets G90/G91, send it first and then re-assert
                                // G91.1 so arc-center mode is correct for subsequent G2/G3 commands.
                                if start_idx == 0 {
                                    if let Some(first) = file.lines.first() {
                                        if line_sets_distance_mode(&first.text) {
                                            let _ = streamer_cmd_tx.send(StreamerCommand::GcodeLine {
                                                text: first.text.clone(),
                                                byte_len: first.byte_len,
                                                line_number: first.file_line,
                                            });
                                            if let Some(ref ep) = first.endpoint {
                                                if ep.len() > 2 {
                                                    current_z = ep[2];
                                                }
                                            }
                                            current_line_idx = 1;
                                            let _ = streamer_cmd_tx
                                                .send(StreamerCommand::RawCommand("G91.1".to_string()));
                                        }
                                    }
                                }

                                // Feed initial batch of lines to streamer
                                feed_lines(
                                    &current_file,
                                    job_start_line_idx,
                                    &mut current_line_idx,
                                    &streamer_cmd_tx,
                                    &pause_condition,
                                    &mut current_z,
                                    &mut pause_pending,
                                    lines_acked,
                                    stop_line_idx,
                                );
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
                            if lines_acked >= range_total {
                                // All lines were acked during pause — job is done
                                let elapsed = job_start_time.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
                                job_state = JobState::Completed;
                                info!("Job completed: {} lines in {:.1}s", range_total, elapsed);
                                let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                            } else {
                                let _ = streamer_cmd_tx.send(StreamerCommand::Realtime(RealtimeCommand::CycleStart));
                                job_state = JobState::Running;
                                let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                                // Feed lines after resume — needed when buffer was
                                // drained by a scheduled pause (no pending acks to
                                // trigger feeding otherwise)
                                feed_lines(
                                    &current_file,
                                    job_start_line_idx,
                                    &mut current_line_idx,
                                    &streamer_cmd_tx,
                                    &pause_condition,
                                    &mut current_z,
                                    &mut pause_pending,
                                    lines_acked,
                                    stop_line_idx,
                                );
                            }
                        }
                    }
                    PlannerCommand::SchedulePause(cond) => {
                        if job_state == JobState::Running {
                            if let Some(ref c) = cond {
                                info!("Scheduled pause set: {:?}", c);
                            } else {
                                info!("Scheduled pause cancelled");
                                pause_pending = false;
                            }
                            pause_condition = cond;
                        }
                    }
                    PlannerCommand::CancelJob => {
                        if job_state.is_active() {
                            let _ = streamer_cmd_tx.send(StreamerCommand::Realtime(RealtimeCommand::SoftReset));
                            let _ = streamer_cmd_tx.send(StreamerCommand::Flush);
                            job_state = JobState::Cancelled;
                            current_line_idx = 0;
                            lines_acked = 0;
                            range_total = 0;
                            stop_line_idx = None;
                            pause_condition = None;
                            pause_pending = false;
                            let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                            // $X unlock and G91.1 are handled by the streamer
                            // on Welcome after the soft reset completes.
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
                        if job_state == JobState::Running || job_state == JobState::Paused {
                            lines_acked += 1;
                            let elapsed = job_start_time.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);

                            // Compute estimated remaining time using adaptive correction
                            let est_remaining = if let Some(ref cum) = line_cumulative_secs {
                                // cur_idx: index of the line just acknowledged (0-based into cum[])
                                let cur_idx = job_start_line_idx + lines_acked;
                                let end_idx = job_end_line_idx;

                                if cur_idx > cum.len() || end_idx > cum.len() || end_idx == 0 {
                                    info!("Est: out of bounds cur_idx={} end_idx={} cum.len={}", cur_idx, end_idx, cum.len());
                                    None
                                } else {
                                    let start_cum = if job_start_line_idx > 0 {
                                        cum[job_start_line_idx - 1]
                                    } else {
                                        0.0
                                    };
                                    // cur_idx is 1-based count from start, so the 0-based index is cur_idx-1
                                    let cur_cum = if cur_idx > 0 { cum[cur_idx - 1] } else { 0.0 };
                                    let end_cum = cum[end_idx - 1];
                                    let est_elapsed = cur_cum - start_cum;
                                    let est_total_remaining = end_cum - cur_cum;

                                    if lines_acked <= 3 || lines_acked.is_multiple_of(100) {
                                        info!(
                                            "Est: acked={} start={} cur={} end={} est_elapsed={:.1} est_remain={:.1} actual_elapsed={:.1}",
                                            lines_acked, job_start_line_idx, cur_idx, end_idx,
                                            est_elapsed, est_total_remaining, elapsed
                                        );
                                    }

                                    if est_total_remaining <= 0.0 {
                                        Some(0.0)
                                    } else if est_elapsed > 1.0 {
                                        let scale = elapsed / est_elapsed;
                                        Some(est_total_remaining * scale)
                                    } else {
                                        Some(est_total_remaining)
                                    }
                                }
                            } else {
                                info!("Est: no cumulative times available");
                                None
                            };

                            let _ = event_tx.send(PlannerEvent::JobProgress {
                                current_line: lines_acked,
                                total_lines: range_total,
                                elapsed_secs: elapsed,
                                estimated_remaining_secs: est_remaining,
                            }).await;

                            // Scheduled pause: once all sent lines are acknowledged,
                            // transition to Paused WITHOUT FeedHold so GRBL finishes
                            // executing queued moves (completing the current layer).
                            let lines_sent = current_line_idx.saturating_sub(job_start_line_idx);
                            if pause_pending && lines_acked >= lines_sent {
                                info!("Scheduled pause — all lines acknowledged, GRBL will finish queued moves");
                                job_state = JobState::Paused;
                                pause_pending = false;
                                pause_condition = None;
                                let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                            } else if !pause_pending {
                                // Feed more lines as space frees up
                                feed_lines(
                                    &current_file,
                                    job_start_line_idx,
                                    &mut current_line_idx,
                                    &streamer_cmd_tx,
                                    &pause_condition,
                                    &mut current_z,
                                    &mut pause_pending,
                                    lines_acked,
                                    stop_line_idx,
                                );
                            }

                            // Check if job is complete (only when running, not paused)
                            if job_state == JobState::Running && lines_acked >= range_total {
                                job_state = JobState::Completed;
                                pause_condition = None;
                                pause_pending = false;
                                info!("Job completed: {} lines in {:.1}s", range_total, elapsed);
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
                            feed_lines(
                                &current_file,
                                job_start_line_idx,
                                &mut current_line_idx,
                                &streamer_cmd_tx,
                                &pause_condition,
                                &mut current_z,
                                &mut pause_pending,
                                lines_acked,
                                stop_line_idx,
                            );
                        }
                    }
                    StreamerEvent::Alarm { code: _ } => {
                        if job_state.is_active() {
                            job_state = JobState::Error;
                            let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                        }
                    }
                    StreamerEvent::Disconnected | StreamerEvent::Exited => {
                        if job_state.is_active() {
                            warn!("Controller disconnected during active job; cancelling");
                            job_state = JobState::Cancelled;
                            current_line_idx = 0;
                            lines_acked = 0;
                            range_total = 0;
                            stop_line_idx = None;
                            job_start_time = None;
                            pause_condition = None;
                            pause_pending = false;
                            current_z = 0.0;
                            job_start_line_idx = 0;
                            job_end_line_idx = 0;
                            let _ = event_tx.send(PlannerEvent::JobStateChanged(job_state)).await;
                        }
                    }
                    StreamerEvent::Welcome { .. } => {
                        // Controller was reset -- if job was running, it's lost
                        if job_state.is_active() {
                            job_state = JobState::Cancelled;
                            current_line_idx = 0;
                            lines_acked = 0;
                            range_total = 0;
                            stop_line_idx = None;
                            job_start_time = None;
                            pause_condition = None;
                            pause_pending = false;
                            current_z = 0.0;
                            job_start_line_idx = 0;
                            job_end_line_idx = 0;
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
/// Checks pause conditions before each line; sets `pause_pending` if triggered.
/// Limits in-flight lines (sent but unacked) so scheduled pause drain is fast.
/// Respects `stop_idx` to stop feeding at a specific line index.
#[allow(clippy::too_many_arguments)]
fn feed_lines(
    file: &Option<GCodeFile>,
    job_start_idx: usize,
    line_idx: &mut usize,
    tx: &Sender<StreamerCommand>,
    pause_condition: &Option<PauseCondition>,
    current_z: &mut f64,
    pause_pending: &mut bool,
    lines_acked: usize,
    stop_idx: Option<usize>,
) {
    let Some(file) = file else { return };
    if *pause_pending {
        return;
    }

    // Cap in-flight lines so scheduled pause drain completes in seconds,
    // not minutes. GRBL buffer holds ~6 lines; a few extra in the
    // streamer queue keeps it topped up without excessive pre-buffering.
    let max_in_flight: usize = 10;
    let lines_sent = line_idx.saturating_sub(job_start_idx);
    let in_flight = lines_sent.saturating_sub(lines_acked);
    if in_flight >= max_in_flight {
        return;
    }

    let end_idx = stop_idx.unwrap_or(file.lines.len());
    let batch_size = (max_in_flight - in_flight).min(4);
    let mut sent = 0;

    while *line_idx < end_idx && sent < batch_size {
        let line = &file.lines[*line_idx];

        // Check pause condition before sending this line
        if let Some(ref cond) = pause_condition {
            let triggered = match cond {
                PauseCondition::EndOfLayer => {
                    // Trigger just before a line whose Z differs from current_z
                    if let Some(ref ep) = line.endpoint {
                        ep.len() > 2 && (ep[2] - *current_z).abs() > 0.001
                    } else {
                        false
                    }
                }
                PauseCondition::AtZDepth { z: target_z } => {
                    // Trigger if next line's Z endpoint reaches or passes target depth
                    if let Some(ref ep) = line.endpoint {
                        ep.len() > 2 && ep[2] <= *target_z
                    } else {
                        false
                    }
                }
            };

            if triggered {
                info!(
                    "Pause condition triggered at line {}, waiting for buffer drain",
                    line.file_line
                );
                *pause_pending = true;
                return;
            }
        }

        let cmd = StreamerCommand::GcodeLine {
            text: line.text.clone(),
            byte_len: line.byte_len,
            line_number: line.file_line,
        };
        if tx.send(cmd).is_err() {
            break;
        }

        // Update current Z from endpoint
        if let Some(ref ep) = line.endpoint {
            if ep.len() > 2 {
                *current_z = ep[2];
            }
        }

        *line_idx += 1;
        sent += 1;
    }
}

fn line_sets_distance_mode(text: &str) -> bool {
    // Detect G90/G91, but ignore decimal arc-distance words (G90.1/G91.1).
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'G' {
            let num_start = i + 1;
            let mut j = num_start;
            while j < len && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > num_start {
                if j < len && bytes[j] == b'.' {
                    // Decimal G-code (e.g. G91.1) — ignore
                    i = (j + 1).max(i + 1);
                    continue;
                }
                if matches!(&text[num_start..j], "90" | "91") {
                    return true;
                }
                i = j.max(i + 1);
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    fn make_file(lines: usize) -> GCodeFile {
        let mut content = String::new();
        for i in 0..lines {
            content.push_str(&format!("G1 X{} F1000\n", i));
        }
        GCodeFile::parse("test.nc".into(), &content)
    }

    #[test]
    fn test_feed_lines_with_nonzero_start_idx_sends() {
        let file = make_file(50);
        let file_opt = Some(file);

        let (tx, rx) = unbounded::<StreamerCommand>();
        let mut line_idx = 10usize;
        let job_start_idx = 10usize;

        feed_lines(
            &file_opt,
            job_start_idx,
            &mut line_idx,
            &tx,
            &None,
            &mut 0.0,
            &mut false,
            0,
            None,
        );

        let sent: Vec<StreamerCommand> = rx.try_iter().collect();
        assert_eq!(sent.len(), 4);
        match &sent[0] {
            StreamerCommand::GcodeLine { line_number, .. } => assert_eq!(*line_number, 11),
            other => panic!("expected GcodeLine, got {:?}", other),
        }
        assert_eq!(line_idx, 14);
    }

    #[test]
    fn test_feed_lines_respects_in_flight_cap() {
        let file = make_file(50);
        let file_opt = Some(file);

        let (tx, rx) = unbounded::<StreamerCommand>();
        let mut line_idx = 20usize; // job_start_idx + 10 already "sent"
        let job_start_idx = 10usize;

        feed_lines(
            &file_opt,
            job_start_idx,
            &mut line_idx,
            &tx,
            &None,
            &mut 0.0,
            &mut false,
            0,
            None,
        );

        let sent: Vec<StreamerCommand> = rx.try_iter().collect();
        assert!(sent.is_empty());
        assert_eq!(line_idx, 20);
    }
}

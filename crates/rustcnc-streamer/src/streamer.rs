use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU8, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use tracing::{debug, error, info, trace, warn};

use rustcnc_core::grbl::realtime::RealtimeCommand;
use rustcnc_core::grbl::status::parse_status_report;
use rustcnc_core::machine::{MachineState, StatusReport};

use crate::buffer_tracker::BufferTracker;
use crate::response_parser::{GrblResponse, ResponseParser};
use crate::rt_config::apply_rt_config;
use crate::serial::SerialPort;

/// Commands that can be sent to the streamer thread
#[derive(Debug)]
pub enum StreamerCommand {
    /// A validated G-code line to send (text + byte length + file line number)
    GcodeLine {
        text: String,
        byte_len: usize,
        line_number: usize,
    },
    /// A real-time command (bypasses buffer)
    Realtime(RealtimeCommand),
    /// A raw command string (e.g. from console, $$, $H)
    RawCommand(String),
    /// Request status report (sends '?')
    PollStatus,
    /// Clear the send queue (on job cancel / soft reset)
    Flush,
    /// Shutdown the streamer thread
    Shutdown,
}

/// Events produced by the streamer thread
#[derive(Debug, Clone)]
pub enum StreamerEvent {
    /// A line was acknowledged (ok)
    LineAcknowledged { line_number: usize },
    /// GRBL returned an error for a line
    LineError {
        line_number: usize,
        code: u8,
        message: String,
    },
    /// A status report was received
    StatusReport(StatusReport),
    /// An alarm was triggered
    Alarm { code: u8 },
    /// GRBL welcome message (on connect or reset)
    Welcome { version: String },
    /// A message from GRBL [MSG:...]
    Message(String),
    /// Parser state [GC:...]
    ParserState(String),
    /// A GRBL setting ($N=value)
    Setting { key: String, value: String },
    /// Raw console output for display
    ConsoleOutput { text: String },
    /// Streamer thread has exited
    Exited,
}

/// Shared atomic state for zero-copy position reads from any thread.
/// The web server reads these atomically without locking the streamer.
pub struct SharedMachineState {
    // Positions stored as microns (i64) for atomic access
    // Multiply f64 by 1000.0 to convert to microns
    pub machine_x_um: AtomicI64,
    pub machine_y_um: AtomicI64,
    pub machine_z_um: AtomicI64,
    pub work_x_um: AtomicI64,
    pub work_y_um: AtomicI64,
    pub work_z_um: AtomicI64,
    // Feed and spindle (stored as value * 1000 for precision)
    pub feed_rate_x1000: AtomicI64,
    pub spindle_rpm_x1000: AtomicI64,
    // State as u8 (see MachineState::to_byte)
    pub state: AtomicU8,
    // Overrides
    pub feed_override: AtomicU8,
    pub rapid_override: AtomicU8,
    pub spindle_override: AtomicU8,
    // Line number
    pub line_number: AtomicU32,
    // Flags
    pub connected: AtomicBool,
}

impl SharedMachineState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            machine_x_um: AtomicI64::new(0),
            machine_y_um: AtomicI64::new(0),
            machine_z_um: AtomicI64::new(0),
            work_x_um: AtomicI64::new(0),
            work_y_um: AtomicI64::new(0),
            work_z_um: AtomicI64::new(0),
            feed_rate_x1000: AtomicI64::new(0),
            spindle_rpm_x1000: AtomicI64::new(0),
            state: AtomicU8::new(0),
            feed_override: AtomicU8::new(100),
            rapid_override: AtomicU8::new(100),
            spindle_override: AtomicU8::new(100),
            line_number: AtomicU32::new(0),
            connected: AtomicBool::new(false),
        })
    }

    /// Read machine position as f64 (converting from microns)
    pub fn machine_pos(&self) -> [f64; 3] {
        [
            self.machine_x_um.load(Ordering::Relaxed) as f64 / 1000.0,
            self.machine_y_um.load(Ordering::Relaxed) as f64 / 1000.0,
            self.machine_z_um.load(Ordering::Relaxed) as f64 / 1000.0,
        ]
    }

    /// Read work position as f64 (converting from microns)
    pub fn work_pos(&self) -> [f64; 3] {
        [
            self.work_x_um.load(Ordering::Relaxed) as f64 / 1000.0,
            self.work_y_um.load(Ordering::Relaxed) as f64 / 1000.0,
            self.work_z_um.load(Ordering::Relaxed) as f64 / 1000.0,
        ]
    }

    /// Update from a parsed status report
    pub fn update_from_report(&self, report: &StatusReport) {
        self.state
            .store(report.state.to_byte(), Ordering::Relaxed);

        if let Some(ref pos) = report.machine_position {
            self.machine_x_um
                .store((pos.x * 1000.0) as i64, Ordering::Relaxed);
            self.machine_y_um
                .store((pos.y * 1000.0) as i64, Ordering::Relaxed);
            self.machine_z_um
                .store((pos.z * 1000.0) as i64, Ordering::Relaxed);
        }

        if let Some(ref pos) = report.work_position {
            self.work_x_um
                .store((pos.x * 1000.0) as i64, Ordering::Relaxed);
            self.work_y_um
                .store((pos.y * 1000.0) as i64, Ordering::Relaxed);
            self.work_z_um
                .store((pos.z * 1000.0) as i64, Ordering::Relaxed);
        }

        if let Some(feed) = report.feed_rate {
            self.feed_rate_x1000
                .store((feed * 1000.0) as i64, Ordering::Relaxed);
        }

        if let Some(spindle) = report.spindle_speed {
            self.spindle_rpm_x1000
                .store((spindle * 1000.0) as i64, Ordering::Relaxed);
        }

        if let Some(ref ovr) = report.overrides {
            self.feed_override.store(ovr.feed, Ordering::Relaxed);
            self.rapid_override.store(ovr.rapids, Ordering::Relaxed);
            self.spindle_override
                .store(ovr.spindle, Ordering::Relaxed);
        }

        if let Some(ln) = report.line_number {
            self.line_number.store(ln, Ordering::Relaxed);
        }
    }
}

impl Default for SharedMachineState {
    fn default() -> Self {
        // Note: this returns Self, not Arc<Self>
        Self {
            machine_x_um: AtomicI64::new(0),
            machine_y_um: AtomicI64::new(0),
            machine_z_um: AtomicI64::new(0),
            work_x_um: AtomicI64::new(0),
            work_y_um: AtomicI64::new(0),
            work_z_um: AtomicI64::new(0),
            feed_rate_x1000: AtomicI64::new(0),
            spindle_rpm_x1000: AtomicI64::new(0),
            state: AtomicU8::new(0),
            feed_override: AtomicU8::new(100),
            rapid_override: AtomicU8::new(100),
            spindle_override: AtomicU8::new(100),
            line_number: AtomicU32::new(0),
            connected: AtomicBool::new(false),
        }
    }
}

/// Configuration for the streamer thread
pub struct StreamerConfig {
    pub rx_buffer_size: usize,
    pub cpu_pin_core: Option<usize>,
    pub rt_priority: Option<i32>,
    pub status_poll_interval: Duration,
}

/// The main streamer loop. Runs on a dedicated OS thread.
///
/// This function is the hot path -- it must never allocate on the heap,
/// never block on a mutex, and never do IO besides the serial port.
pub fn streamer_thread_main(
    mut serial: Box<dyn SerialPort>,
    cmd_rx: Receiver<StreamerCommand>,
    event_tx: Sender<StreamerEvent>,
    shared_state: Arc<SharedMachineState>,
    config: StreamerConfig,
) {
    // Apply RT scheduling and CPU pinning
    apply_rt_config(config.cpu_pin_core, config.rt_priority);

    shared_state.connected.store(true, Ordering::Relaxed);
    info!("Streamer thread started");

    let mut tracker = BufferTracker::new(config.rx_buffer_size);
    let mut parser = ResponseParser::new();
    let mut read_buf = [0u8; 512];

    // Track line numbers for correlating ok/error responses
    let mut pending_line_numbers: std::collections::VecDeque<usize> =
        std::collections::VecDeque::new();

    // Status polling
    let mut last_poll = Instant::now();

    'main: loop {
        // 1. Read any available data from serial port
        match serial.read(&mut read_buf) {
            Ok(n) if n > 0 => {
                let responses = parser.feed(&read_buf[..n]);
                for response in responses {
                    handle_response(
                        &response,
                        &mut tracker,
                        &mut pending_line_numbers,
                        &event_tx,
                        &shared_state,
                    );
                }
            }
            Ok(_) => {} // no data
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {} // timeout, normal
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {} // would block, normal
            Err(e) => {
                error!("Serial read error: {}", e);
                break 'main;
            }
        }

        // 2. Process commands from planner/server
        loop {
            match cmd_rx.try_recv() {
                Ok(StreamerCommand::GcodeLine {
                    text,
                    byte_len,
                    line_number,
                }) => {
                    if tracker.can_send(byte_len) {
                        let line_with_newline = format!("{}\n", text);
                        match serial.write_all(line_with_newline.as_bytes()) {
                            Ok(()) => {
                                tracker.line_sent(byte_len);
                                pending_line_numbers.push_back(line_number);
                                trace!("Sent line {}: {}", line_number, text);
                                let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                                    text: text.clone(),
                                });
                            }
                            Err(e) => {
                                error!("Serial write error: {}", e);
                                break 'main;
                            }
                        }
                    } else {
                        // Buffer full -- this shouldn't happen if planner respects flow control
                        // Re-queue by logging a warning
                        warn!(
                            "Buffer full, cannot send line {} (need {} bytes, have {} available)",
                            line_number,
                            byte_len,
                            tracker.available()
                        );
                    }
                }
                Ok(StreamerCommand::Realtime(cmd)) => {
                    let byte = cmd.to_byte();
                    if let Err(e) = serial.write_rt_command(byte) {
                        error!("Failed to send RT command: {}", e);
                    } else {
                        debug!("Sent RT command: {:?} (0x{:02X})", cmd, byte);
                    }
                }
                Ok(StreamerCommand::RawCommand(text)) => {
                    let line_with_newline = format!("{}\n", text);
                    if let Err(e) = serial.write_all(line_with_newline.as_bytes()) {
                        error!("Failed to send raw command: {}", e);
                    } else {
                        let _ = event_tx.send(StreamerEvent::ConsoleOutput { text });
                    }
                }
                Ok(StreamerCommand::PollStatus) => {
                    if let Err(e) = serial.write_rt_command(b'?') {
                        error!("Failed to send status query: {}", e);
                    }
                }
                Ok(StreamerCommand::Flush) => {
                    tracker.reset();
                    pending_line_numbers.clear();
                    parser.reset();
                    info!("Streamer flushed");
                }
                Ok(StreamerCommand::Shutdown) => {
                    info!("Streamer shutdown requested");
                    break 'main;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    info!("Command channel disconnected, shutting down streamer");
                    break 'main;
                }
            }
        }

        // 3. Periodic status polling
        if last_poll.elapsed() >= config.status_poll_interval {
            if let Err(e) = serial.write_rt_command(b'?') {
                error!("Failed to send periodic status query: {}", e);
            }
            last_poll = Instant::now();
        }

        // Small sleep to prevent busy-waiting while still being responsive
        std::thread::sleep(Duration::from_micros(100));
    }

    shared_state.connected.store(false, Ordering::Relaxed);
    let _ = event_tx.send(StreamerEvent::Exited);
    info!("Streamer thread exited");
}

/// Handle a parsed GRBL response
fn handle_response(
    response: &GrblResponse,
    tracker: &mut BufferTracker,
    pending_line_numbers: &mut std::collections::VecDeque<usize>,
    event_tx: &Sender<StreamerEvent>,
    shared_state: &Arc<SharedMachineState>,
) {
    match response {
        GrblResponse::Ok => {
            tracker.line_acknowledged();
            let line_number = pending_line_numbers.pop_front().unwrap_or(0);
            let _ = event_tx.send(StreamerEvent::LineAcknowledged { line_number });
            trace!("Line {} acknowledged", line_number);
        }
        GrblResponse::Error(code) => {
            tracker.line_acknowledged();
            let line_number = pending_line_numbers.pop_front().unwrap_or(0);
            let message = rustcnc_core::grbl::error_codes::GrblError::from_code(*code)
                .map(|e| e.message().to_string())
                .unwrap_or_else(|| format!("Unknown error {}", code));
            error!("GRBL error {} on line {}: {}", code, line_number, message);
            let _ = event_tx.send(StreamerEvent::LineError {
                line_number,
                code: *code,
                message,
            });
        }
        GrblResponse::Alarm(code) => {
            warn!("ALARM:{}", code);
            let _ = event_tx.send(StreamerEvent::Alarm { code: *code });
        }
        GrblResponse::StatusReport(raw) => {
            if let Some(report) = parse_status_report(raw) {
                shared_state.update_from_report(&report);
                let _ = event_tx.send(StreamerEvent::StatusReport(report));
            }
        }
        GrblResponse::Welcome(version) => {
            info!("GRBL welcome: {}", version);
            // Reset tracker on welcome (controller has been reset)
            tracker.reset();
            pending_line_numbers.clear();
            let _ = event_tx.send(StreamerEvent::Welcome {
                version: version.clone(),
            });
        }
        GrblResponse::Message(msg) => {
            info!("GRBL message: {}", msg);
            let _ = event_tx.send(StreamerEvent::Message(msg.clone()));
        }
        GrblResponse::ParserState(state) => {
            let _ = event_tx.send(StreamerEvent::ParserState(state.clone()));
        }
        GrblResponse::Setting(key, value) => {
            let _ = event_tx.send(StreamerEvent::Setting {
                key: key.clone(),
                value: value.clone(),
            });
        }
        GrblResponse::BuildInfo(info_str) => {
            let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                text: info_str.clone(),
            });
        }
        GrblResponse::Feedback(text) | GrblResponse::Unknown(text) => {
            let _ = event_tx.send(StreamerEvent::ConsoleOutput { text: text.clone() });
        }
        GrblResponse::StartupLine(text) => {
            let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                text: format!(">{}", text),
            });
        }
    }
}

use std::io::Write;

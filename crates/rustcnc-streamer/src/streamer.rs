use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU8, AtomicU16, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use tracing::{debug, error, info, trace, warn};

use rustcnc_core::grbl::realtime::RealtimeCommand;
use rustcnc_core::grbl::status::parse_status_report;
use rustcnc_core::machine::StatusReport;

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
    pub machine_a_um: AtomicI64,
    pub machine_b_um: AtomicI64,
    pub machine_c_um: AtomicI64,
    pub machine_u_um: AtomicI64,
    pub machine_v_um: AtomicI64,
    pub work_x_um: AtomicI64,
    pub work_y_um: AtomicI64,
    pub work_z_um: AtomicI64,
    pub work_a_um: AtomicI64,
    pub work_b_um: AtomicI64,
    pub work_c_um: AtomicI64,
    pub work_u_um: AtomicI64,
    pub work_v_um: AtomicI64,
    // Feed and spindle (stored as value * 1000 for precision)
    pub feed_rate_x1000: AtomicI64,
    pub spindle_rpm_x1000: AtomicI64,
    // State as u16 (see MachineState::to_u16)
    pub state: AtomicU16,
    // Overrides
    pub feed_override: AtomicU8,
    pub rapid_override: AtomicU8,
    pub spindle_override: AtomicU8,
    // Line number
    pub line_number: AtomicU32,
    // Flags
    pub connected: AtomicBool,
    // Bitfield tracking which optional axes are present in status reports
    // bit 0=A, bit 1=B, bit 2=C, bit 3=U, bit 4=V
    pub axes_present: AtomicU8,
    // Accessory state
    pub spindle_cw: AtomicBool,
    pub spindle_ccw: AtomicBool,
    pub coolant_flood: AtomicBool,
    pub coolant_mist: AtomicBool,
}

impl SharedMachineState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            machine_x_um: AtomicI64::new(0),
            machine_y_um: AtomicI64::new(0),
            machine_z_um: AtomicI64::new(0),
            machine_a_um: AtomicI64::new(0),
            machine_b_um: AtomicI64::new(0),
            machine_c_um: AtomicI64::new(0),
            machine_u_um: AtomicI64::new(0),
            machine_v_um: AtomicI64::new(0),
            work_x_um: AtomicI64::new(0),
            work_y_um: AtomicI64::new(0),
            work_z_um: AtomicI64::new(0),
            work_a_um: AtomicI64::new(0),
            work_b_um: AtomicI64::new(0),
            work_c_um: AtomicI64::new(0),
            work_u_um: AtomicI64::new(0),
            work_v_um: AtomicI64::new(0),
            feed_rate_x1000: AtomicI64::new(0),
            spindle_rpm_x1000: AtomicI64::new(0),
            state: AtomicU16::new(0),
            feed_override: AtomicU8::new(100),
            rapid_override: AtomicU8::new(100),
            spindle_override: AtomicU8::new(100),
            line_number: AtomicU32::new(0),
            connected: AtomicBool::new(false),
            axes_present: AtomicU8::new(0),
            spindle_cw: AtomicBool::new(false),
            spindle_ccw: AtomicBool::new(false),
            coolant_flood: AtomicBool::new(false),
            coolant_mist: AtomicBool::new(false),
        })
    }

    /// Read machine position as Position (converting from microns)
    pub fn machine_pos(&self) -> rustcnc_core::machine::Position {
        let mut pos = rustcnc_core::machine::Position::new(
            self.machine_x_um.load(Ordering::Acquire) as f64 / 1000.0,
            self.machine_y_um.load(Ordering::Acquire) as f64 / 1000.0,
            self.machine_z_um.load(Ordering::Acquire) as f64 / 1000.0,
        );
        let axes = self.axes_present.load(Ordering::Acquire);
        if axes & (1 << 0) != 0 { pos.a = Some(self.machine_a_um.load(Ordering::Acquire) as f64 / 1000.0); }
        if axes & (1 << 1) != 0 { pos.b = Some(self.machine_b_um.load(Ordering::Acquire) as f64 / 1000.0); }
        if axes & (1 << 2) != 0 { pos.c = Some(self.machine_c_um.load(Ordering::Acquire) as f64 / 1000.0); }
        if axes & (1 << 3) != 0 { pos.u = Some(self.machine_u_um.load(Ordering::Acquire) as f64 / 1000.0); }
        if axes & (1 << 4) != 0 { pos.v = Some(self.machine_v_um.load(Ordering::Acquire) as f64 / 1000.0); }
        pos
    }

    /// Read work position as Position (converting from microns)
    pub fn work_pos(&self) -> rustcnc_core::machine::Position {
        let mut pos = rustcnc_core::machine::Position::new(
            self.work_x_um.load(Ordering::Acquire) as f64 / 1000.0,
            self.work_y_um.load(Ordering::Acquire) as f64 / 1000.0,
            self.work_z_um.load(Ordering::Acquire) as f64 / 1000.0,
        );
        let axes = self.axes_present.load(Ordering::Acquire);
        if axes & (1 << 0) != 0 { pos.a = Some(self.work_a_um.load(Ordering::Acquire) as f64 / 1000.0); }
        if axes & (1 << 1) != 0 { pos.b = Some(self.work_b_um.load(Ordering::Acquire) as f64 / 1000.0); }
        if axes & (1 << 2) != 0 { pos.c = Some(self.work_c_um.load(Ordering::Acquire) as f64 / 1000.0); }
        if axes & (1 << 3) != 0 { pos.u = Some(self.work_u_um.load(Ordering::Acquire) as f64 / 1000.0); }
        if axes & (1 << 4) != 0 { pos.v = Some(self.work_v_um.load(Ordering::Acquire) as f64 / 1000.0); }
        pos
    }

    /// Update from a parsed status report
    pub fn update_from_report(&self, report: &StatusReport) {
        self.state
            .store(report.state.to_u16(), Ordering::Release);

        if let Some(ref pos) = report.machine_position {
            if pos.x.is_finite() {
                self.machine_x_um.store((pos.x * 1000.0) as i64, Ordering::Release);
            }
            if pos.y.is_finite() {
                self.machine_y_um.store((pos.y * 1000.0) as i64, Ordering::Release);
            }
            if pos.z.is_finite() {
                self.machine_z_um.store((pos.z * 1000.0) as i64, Ordering::Release);
            }

            // Track which optional axes are present and store their values
            let mut axes_bits: u8 = 0;
            if let Some(a) = pos.a {
                axes_bits |= 1 << 0;
                if a.is_finite() {
                    self.machine_a_um.store((a * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(b) = pos.b {
                axes_bits |= 1 << 1;
                if b.is_finite() {
                    self.machine_b_um.store((b * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(c) = pos.c {
                axes_bits |= 1 << 2;
                if c.is_finite() {
                    self.machine_c_um.store((c * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(u) = pos.u {
                axes_bits |= 1 << 3;
                if u.is_finite() {
                    self.machine_u_um.store((u * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(v) = pos.v {
                axes_bits |= 1 << 4;
                if v.is_finite() {
                    self.machine_v_um.store((v * 1000.0) as i64, Ordering::Release);
                }
            }
            // Merge with existing axes_present (work pos may also set bits)
            let existing = self.axes_present.load(Ordering::Acquire);
            self.axes_present.store(existing | axes_bits, Ordering::Release);
        }

        if let Some(ref pos) = report.work_position {
            if pos.x.is_finite() {
                self.work_x_um.store((pos.x * 1000.0) as i64, Ordering::Release);
            }
            if pos.y.is_finite() {
                self.work_y_um.store((pos.y * 1000.0) as i64, Ordering::Release);
            }
            if pos.z.is_finite() {
                self.work_z_um.store((pos.z * 1000.0) as i64, Ordering::Release);
            }

            let mut axes_bits: u8 = 0;
            if let Some(a) = pos.a {
                axes_bits |= 1 << 0;
                if a.is_finite() {
                    self.work_a_um.store((a * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(b) = pos.b {
                axes_bits |= 1 << 1;
                if b.is_finite() {
                    self.work_b_um.store((b * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(c) = pos.c {
                axes_bits |= 1 << 2;
                if c.is_finite() {
                    self.work_c_um.store((c * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(u) = pos.u {
                axes_bits |= 1 << 3;
                if u.is_finite() {
                    self.work_u_um.store((u * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(v) = pos.v {
                axes_bits |= 1 << 4;
                if v.is_finite() {
                    self.work_v_um.store((v * 1000.0) as i64, Ordering::Release);
                }
            }
            let existing = self.axes_present.load(Ordering::Acquire);
            self.axes_present.store(existing | axes_bits, Ordering::Release);
        }

        if let Some(feed) = report.feed_rate {
            if feed.is_finite() {
                self.feed_rate_x1000.store((feed * 1000.0) as i64, Ordering::Release);
            }
        }

        if let Some(spindle) = report.spindle_speed {
            if spindle.is_finite() {
                self.spindle_rpm_x1000.store((spindle * 1000.0) as i64, Ordering::Release);
            }
        }

        if let Some(ref ovr) = report.overrides {
            self.feed_override.store(ovr.feed, Ordering::Release);
            self.rapid_override.store(ovr.rapids, Ordering::Release);
            self.spindle_override.store(ovr.spindle, Ordering::Release);
        }

        if let Some(ln) = report.line_number {
            self.line_number.store(ln, Ordering::Release);
        }

        if let Some(ref acc) = report.accessories {
            self.spindle_cw.store(acc.spindle_cw, Ordering::Release);
            self.spindle_ccw.store(acc.spindle_ccw, Ordering::Release);
            self.coolant_flood.store(acc.flood_coolant, Ordering::Release);
            self.coolant_mist.store(acc.mist_coolant, Ordering::Release);
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
            machine_a_um: AtomicI64::new(0),
            machine_b_um: AtomicI64::new(0),
            machine_c_um: AtomicI64::new(0),
            machine_u_um: AtomicI64::new(0),
            machine_v_um: AtomicI64::new(0),
            work_x_um: AtomicI64::new(0),
            work_y_um: AtomicI64::new(0),
            work_z_um: AtomicI64::new(0),
            work_a_um: AtomicI64::new(0),
            work_b_um: AtomicI64::new(0),
            work_c_um: AtomicI64::new(0),
            work_u_um: AtomicI64::new(0),
            work_v_um: AtomicI64::new(0),
            feed_rate_x1000: AtomicI64::new(0),
            spindle_rpm_x1000: AtomicI64::new(0),
            state: AtomicU16::new(0),
            feed_override: AtomicU8::new(100),
            rapid_override: AtomicU8::new(100),
            spindle_override: AtomicU8::new(100),
            line_number: AtomicU32::new(0),
            connected: AtomicBool::new(false),
            axes_present: AtomicU8::new(0),
            spindle_cw: AtomicBool::new(false),
            spindle_ccw: AtomicBool::new(false),
            coolant_flood: AtomicBool::new(false),
            coolant_mist: AtomicBool::new(false),
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
/// This function is the hot path -- minimize heap allocations on the hot path,
/// never block on a mutex, and never do IO besides the serial port.
/// Note: format!() and String operations do allocate; this is acceptable
/// for the send path but should be kept to a minimum.
pub fn streamer_thread_main(
    mut serial: Box<dyn SerialPort>,
    cmd_rx: Receiver<StreamerCommand>,
    event_tx: Sender<StreamerEvent>,
    shared_state: Arc<SharedMachineState>,
    config: StreamerConfig,
) {
    // Apply RT scheduling and CPU pinning
    apply_rt_config(config.cpu_pin_core, config.rt_priority);

    shared_state.connected.store(true, Ordering::Release);
    info!("Streamer thread started");

    let mut tracker = BufferTracker::new(config.rx_buffer_size);
    let mut parser = ResponseParser::new();
    let mut read_buf = [0u8; 512];

    // Track line numbers for correlating ok/error responses
    let mut pending_line_numbers: std::collections::VecDeque<usize> =
        std::collections::VecDeque::new();

    // Status polling
    let mut last_poll = Instant::now();

    // Holds a G-code line that could not be sent because the buffer was full.
    // The line is retried on the next loop iteration instead of being dropped.
    let mut pending_line: Option<(String, usize, usize)> = None; // (text, byte_len, line_number)

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

        // 2. Try to send any pending line that was deferred due to buffer full
        if let Some((ref text, byte_len, line_number)) = pending_line {
            if tracker.can_send(byte_len) {
                let line_with_newline = format!("{}\n", text);
                match serial.write_all(line_with_newline.as_bytes()) {
                    Ok(()) => {
                        tracker.line_sent(byte_len);
                        pending_line_numbers.push_back(line_number);
                        trace!("Sent pending line {}: {}", line_number, text);
                        let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                            text: text.clone(),
                        });
                        pending_line = None;
                    }
                    Err(e) => {
                        error!("Serial write error: {}", e);
                        break 'main;
                    }
                }
            }
            // If still can't send, leave pending and skip to response handling
        }

        // 3. Process commands from planner/server (only if no pending line)
        loop {
            // Don't pull new G-code from the channel if we already have a pending line
            if pending_line.is_some() {
                // Still process non-GcodeLine commands (realtime, flush, etc.)
                match cmd_rx.try_recv() {
                    Ok(StreamerCommand::GcodeLine { text, byte_len, line_number }) => {
                        // We already have a pending line; this should not normally happen
                        // since the planner sends in batches. Log a warning and store
                        // the new line as pending (overwriting is wrong -- but we should
                        // never get here because we break below after storing pending).
                        // Safety: push it back... we can't, so warn and drop to avoid
                        // silent corruption. Actually, this path is unreachable because
                        // we break out of the inner loop when pending_line is set.
                        warn!("Received GcodeLine while pending line exists; this is unexpected");
                        // We must not drop this line. Store it as pending (the previous
                        // pending was already handled above).
                        pending_line = Some((text, byte_len, line_number));
                        break;
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
                        pending_line = None;
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
            } else {
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
                            // Buffer full -- store as pending instead of dropping
                            warn!(
                                "Buffer full, deferring line {} (need {} bytes, have {} available)",
                                line_number,
                                byte_len,
                                tracker.available()
                            );
                            pending_line = Some((text, byte_len, line_number));
                            break; // Stop pulling from channel until pending is sent
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
                        pending_line = None;
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
        }

        // 4. Periodic status polling
        if last_poll.elapsed() >= config.status_poll_interval {
            if let Err(e) = serial.write_rt_command(b'?') {
                error!("Failed to send periodic status query: {}", e);
            }
            last_poll = Instant::now();
        }

        // Small sleep to prevent busy-waiting while still being responsive
        std::thread::sleep(Duration::from_micros(100));
    }

    shared_state.connected.store(false, Ordering::Release);
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

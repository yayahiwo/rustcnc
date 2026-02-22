use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU16, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use tracing::{error, info, trace, warn};

use rustcnc_core::grbl::realtime::RealtimeCommand;
use rustcnc_core::grbl::status::parse_status_report;
use rustcnc_core::machine::{BufferState, FirmwareType, InputPins, StatusReport};

use crate::buffer_tracker::BufferTracker;
use crate::response_parser::{GrblResponse, ResponseParser};
use crate::rt_config::apply_rt_config;
use crate::serial::{HardwareSerialPort, SerialPort};

/// Commands that can be sent to the streamer thread
#[derive(Debug)]
pub enum StreamerCommand {
    /// Open a serial connection to a controller
    Connect { port: String, baud_rate: u32 },
    /// Close the current serial connection (if any)
    Disconnect,
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
#[allow(clippy::large_enum_variant)]
pub enum StreamerEvent {
    /// Serial connection established
    Connected { port: String, baud_rate: u32 },
    /// Serial connection closed or lost
    Disconnected,
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
    ConsoleOutput { text: String, is_tx: bool },
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
    // Work coordinate offset (WCO) cached from periodic GRBL reports
    // WPos = MPos - WCO
    pub wco_x_um: AtomicI64,
    pub wco_y_um: AtomicI64,
    pub wco_z_um: AtomicI64,
    pub wco_a_um: AtomicI64,
    pub wco_b_um: AtomicI64,
    pub wco_c_um: AtomicI64,
    pub wco_u_um: AtomicI64,
    pub wco_v_um: AtomicI64,
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
    // Buffer and input pins (from status reports)
    pub buffer_planner_blocks_available: AtomicU16,
    pub buffer_rx_bytes_available: AtomicU16,
    pub input_pins_mask: AtomicU16,
    // Firmware type detected from Welcome banner
    pub firmware_type: AtomicU8,
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
            wco_x_um: AtomicI64::new(0),
            wco_y_um: AtomicI64::new(0),
            wco_z_um: AtomicI64::new(0),
            wco_a_um: AtomicI64::new(0),
            wco_b_um: AtomicI64::new(0),
            wco_c_um: AtomicI64::new(0),
            wco_u_um: AtomicI64::new(0),
            wco_v_um: AtomicI64::new(0),
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
            buffer_planner_blocks_available: AtomicU16::new(0),
            buffer_rx_bytes_available: AtomicU16::new(0),
            input_pins_mask: AtomicU16::new(0),
            firmware_type: AtomicU8::new(0),
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
        if axes & (1 << 0) != 0 {
            pos.a = Some(self.machine_a_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
        if axes & (1 << 1) != 0 {
            pos.b = Some(self.machine_b_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
        if axes & (1 << 2) != 0 {
            pos.c = Some(self.machine_c_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
        if axes & (1 << 3) != 0 {
            pos.u = Some(self.machine_u_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
        if axes & (1 << 4) != 0 {
            pos.v = Some(self.machine_v_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
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
        if axes & (1 << 0) != 0 {
            pos.a = Some(self.work_a_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
        if axes & (1 << 1) != 0 {
            pos.b = Some(self.work_b_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
        if axes & (1 << 2) != 0 {
            pos.c = Some(self.work_c_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
        if axes & (1 << 3) != 0 {
            pos.u = Some(self.work_u_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
        if axes & (1 << 4) != 0 {
            pos.v = Some(self.work_v_um.load(Ordering::Acquire) as f64 / 1000.0);
        }
        pos
    }

    pub fn buffer_state(&self) -> BufferState {
        BufferState {
            planner_blocks_available: self.buffer_planner_blocks_available.load(Ordering::Acquire),
            rx_bytes_available: self.buffer_rx_bytes_available.load(Ordering::Acquire),
        }
    }

    pub fn input_pins(&self) -> InputPins {
        decode_input_pins(self.input_pins_mask.load(Ordering::Acquire))
    }

    pub fn firmware(&self) -> FirmwareType {
        firmware_from_u8(self.firmware_type.load(Ordering::Acquire))
    }

    pub fn set_firmware_type(&self, firmware: FirmwareType) {
        self.firmware_type
            .store(firmware_to_u8(firmware), Ordering::Release);
    }

    /// Update from a parsed status report.
    ///
    /// grblHAL reports either MPos or WPos (not both) in each status report.
    /// WCO (work coordinate offset) is sent periodically. We cache WCO and
    /// compute the missing position: WPos = MPos - WCO, MPos = WPos + WCO.
    pub fn update_from_report(&self, report: &StatusReport) {
        self.state.store(report.state.to_u16(), Ordering::Release);

        // Cache WCO when reported (sent every 10-30 status reports by grblHAL)
        if let Some(ref wco) = report.work_coordinate_offset {
            if wco.x.is_finite() {
                self.wco_x_um
                    .store((wco.x * 1000.0) as i64, Ordering::Release);
            }
            if wco.y.is_finite() {
                self.wco_y_um
                    .store((wco.y * 1000.0) as i64, Ordering::Release);
            }
            if wco.z.is_finite() {
                self.wco_z_um
                    .store((wco.z * 1000.0) as i64, Ordering::Release);
            }
            if let Some(a) = wco.a {
                if a.is_finite() {
                    self.wco_a_um.store((a * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(b) = wco.b {
                if b.is_finite() {
                    self.wco_b_um.store((b * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(c) = wco.c {
                if c.is_finite() {
                    self.wco_c_um.store((c * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(u) = wco.u {
                if u.is_finite() {
                    self.wco_u_um.store((u * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(v) = wco.v {
                if v.is_finite() {
                    self.wco_v_um.store((v * 1000.0) as i64, Ordering::Release);
                }
            }
        }

        if let Some(ref pos) = report.machine_position {
            let mx = (pos.x * 1000.0) as i64;
            let my = (pos.y * 1000.0) as i64;
            let mz = (pos.z * 1000.0) as i64;
            if pos.x.is_finite() {
                self.machine_x_um.store(mx, Ordering::Release);
            }
            if pos.y.is_finite() {
                self.machine_y_um.store(my, Ordering::Release);
            }
            if pos.z.is_finite() {
                self.machine_z_um.store(mz, Ordering::Release);
            }

            // Track which optional axes are present and store their values
            let mut axes_bits: u8 = 0;
            if let Some(a) = pos.a {
                axes_bits |= 1 << 0;
                if a.is_finite() {
                    self.machine_a_um
                        .store((a * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(b) = pos.b {
                axes_bits |= 1 << 1;
                if b.is_finite() {
                    self.machine_b_um
                        .store((b * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(c) = pos.c {
                axes_bits |= 1 << 2;
                if c.is_finite() {
                    self.machine_c_um
                        .store((c * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(u) = pos.u {
                axes_bits |= 1 << 3;
                if u.is_finite() {
                    self.machine_u_um
                        .store((u * 1000.0) as i64, Ordering::Release);
                }
            }
            if let Some(v) = pos.v {
                axes_bits |= 1 << 4;
                if v.is_finite() {
                    self.machine_v_um
                        .store((v * 1000.0) as i64, Ordering::Release);
                }
            }
            let existing = self.axes_present.load(Ordering::Acquire);
            self.axes_present
                .store(existing | axes_bits, Ordering::Release);

            // Compute WPos = MPos - WCO when WPos is not directly reported
            if report.work_position.is_none() {
                let wco_x = self.wco_x_um.load(Ordering::Acquire);
                let wco_y = self.wco_y_um.load(Ordering::Acquire);
                let wco_z = self.wco_z_um.load(Ordering::Acquire);
                if pos.x.is_finite() {
                    self.work_x_um.store(mx - wco_x, Ordering::Release);
                }
                if pos.y.is_finite() {
                    self.work_y_um.store(my - wco_y, Ordering::Release);
                }
                if pos.z.is_finite() {
                    self.work_z_um.store(mz - wco_z, Ordering::Release);
                }
                if let Some(a) = pos.a {
                    if a.is_finite() {
                        self.work_a_um.store(
                            (a * 1000.0) as i64 - self.wco_a_um.load(Ordering::Acquire),
                            Ordering::Release,
                        );
                    }
                }
                if let Some(b) = pos.b {
                    if b.is_finite() {
                        self.work_b_um.store(
                            (b * 1000.0) as i64 - self.wco_b_um.load(Ordering::Acquire),
                            Ordering::Release,
                        );
                    }
                }
                if let Some(c) = pos.c {
                    if c.is_finite() {
                        self.work_c_um.store(
                            (c * 1000.0) as i64 - self.wco_c_um.load(Ordering::Acquire),
                            Ordering::Release,
                        );
                    }
                }
                if let Some(u) = pos.u {
                    if u.is_finite() {
                        self.work_u_um.store(
                            (u * 1000.0) as i64 - self.wco_u_um.load(Ordering::Acquire),
                            Ordering::Release,
                        );
                    }
                }
                if let Some(v) = pos.v {
                    if v.is_finite() {
                        self.work_v_um.store(
                            (v * 1000.0) as i64 - self.wco_v_um.load(Ordering::Acquire),
                            Ordering::Release,
                        );
                    }
                }
            }
        }

        if let Some(ref pos) = report.work_position {
            let wx = (pos.x * 1000.0) as i64;
            let wy = (pos.y * 1000.0) as i64;
            let wz = (pos.z * 1000.0) as i64;
            if pos.x.is_finite() {
                self.work_x_um.store(wx, Ordering::Release);
            }
            if pos.y.is_finite() {
                self.work_y_um.store(wy, Ordering::Release);
            }
            if pos.z.is_finite() {
                self.work_z_um.store(wz, Ordering::Release);
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
            self.axes_present
                .store(existing | axes_bits, Ordering::Release);

            // Compute MPos = WPos + WCO when MPos is not directly reported
            if report.machine_position.is_none() {
                let wco_x = self.wco_x_um.load(Ordering::Acquire);
                let wco_y = self.wco_y_um.load(Ordering::Acquire);
                let wco_z = self.wco_z_um.load(Ordering::Acquire);
                if pos.x.is_finite() {
                    self.machine_x_um.store(wx + wco_x, Ordering::Release);
                }
                if pos.y.is_finite() {
                    self.machine_y_um.store(wy + wco_y, Ordering::Release);
                }
                if pos.z.is_finite() {
                    self.machine_z_um.store(wz + wco_z, Ordering::Release);
                }
            }
        }

        if let Some(feed) = report.feed_rate {
            if feed.is_finite() {
                self.feed_rate_x1000
                    .store((feed * 1000.0) as i64, Ordering::Release);
            }
        }

        if let Some(spindle) = report.spindle_speed {
            if spindle.is_finite() {
                self.spindle_rpm_x1000
                    .store((spindle * 1000.0) as i64, Ordering::Release);
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
            self.coolant_flood
                .store(acc.flood_coolant, Ordering::Release);
            self.coolant_mist.store(acc.mist_coolant, Ordering::Release);
        }

        if let Some(buf) = report.buffer {
            self.buffer_planner_blocks_available
                .store(buf.planner_blocks_available, Ordering::Release);
            self.buffer_rx_bytes_available
                .store(buf.rx_bytes_available, Ordering::Release);
        }

        if let Some(pins) = report.input_pins {
            self.input_pins_mask
                .store(encode_input_pins(&pins), Ordering::Release);
        }
    }
}

fn encode_input_pins(p: &InputPins) -> u16 {
    let mut mask = 0u16;
    if p.limit_x {
        mask |= 1 << 0;
    }
    if p.limit_y {
        mask |= 1 << 1;
    }
    if p.limit_z {
        mask |= 1 << 2;
    }
    if p.limit_a {
        mask |= 1 << 3;
    }
    if p.limit_b {
        mask |= 1 << 4;
    }
    if p.limit_c {
        mask |= 1 << 5;
    }
    if p.limit_u {
        mask |= 1 << 6;
    }
    if p.limit_v {
        mask |= 1 << 7;
    }
    if p.probe {
        mask |= 1 << 8;
    }
    if p.door {
        mask |= 1 << 9;
    }
    if p.hold {
        mask |= 1 << 10;
    }
    if p.soft_reset {
        mask |= 1 << 11;
    }
    if p.cycle_start {
        mask |= 1 << 12;
    }
    if p.estop {
        mask |= 1 << 13;
    }
    mask
}

fn decode_input_pins(mask: u16) -> InputPins {
    InputPins {
        limit_x: mask & (1 << 0) != 0,
        limit_y: mask & (1 << 1) != 0,
        limit_z: mask & (1 << 2) != 0,
        limit_a: mask & (1 << 3) != 0,
        limit_b: mask & (1 << 4) != 0,
        limit_c: mask & (1 << 5) != 0,
        limit_u: mask & (1 << 6) != 0,
        limit_v: mask & (1 << 7) != 0,
        probe: mask & (1 << 8) != 0,
        door: mask & (1 << 9) != 0,
        hold: mask & (1 << 10) != 0,
        soft_reset: mask & (1 << 11) != 0,
        cycle_start: mask & (1 << 12) != 0,
        estop: mask & (1 << 13) != 0,
    }
}

fn firmware_to_u8(f: FirmwareType) -> u8 {
    match f {
        FirmwareType::Unknown => 0,
        FirmwareType::Grbl => 1,
        FirmwareType::GrblHal => 2,
    }
}

fn firmware_from_u8(v: u8) -> FirmwareType {
    match v {
        1 => FirmwareType::Grbl,
        2 => FirmwareType::GrblHal,
        _ => FirmwareType::Unknown,
    }
}

fn detect_firmware_type(welcome: &str) -> FirmwareType {
    // grblHAL welcome examples often contain "grblHAL" or start with "GrblHAL".
    if welcome.contains("grblHAL") || welcome.contains("GrblHAL") || welcome.contains("GRBLHAL") {
        FirmwareType::GrblHal
    } else if welcome.starts_with("Grbl") || welcome.starts_with("GRBL") {
        FirmwareType::Grbl
    } else {
        FirmwareType::Unknown
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
            wco_x_um: AtomicI64::new(0),
            wco_y_um: AtomicI64::new(0),
            wco_z_um: AtomicI64::new(0),
            wco_a_um: AtomicI64::new(0),
            wco_b_um: AtomicI64::new(0),
            wco_c_um: AtomicI64::new(0),
            wco_u_um: AtomicI64::new(0),
            wco_v_um: AtomicI64::new(0),
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
            buffer_planner_blocks_available: AtomicU16::new(0),
            buffer_rx_bytes_available: AtomicU16::new(0),
            input_pins_mask: AtomicU16::new(0),
            firmware_type: AtomicU8::new(0),
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
    cmd_rx: Receiver<StreamerCommand>,
    event_tx: Sender<StreamerEvent>,
    shared_state: Arc<SharedMachineState>,
    config: StreamerConfig,
) {
    // Apply RT scheduling and CPU pinning
    apply_rt_config(config.cpu_pin_core, config.rt_priority);

    info!("Streamer thread started");
    shared_state.connected.store(false, Ordering::Release);

    let mut serial: Option<Box<dyn SerialPort>> = None;

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

    // After a soft reset, USB-serial stacks can still have bytes in flight.
    // Block motion sends briefly to avoid corrupted commands.
    let mut post_reset_block_until: Option<Instant> = None;

    // Schedule G91.1 (incremental arc distance mode) after any Welcome message.
    // grblHAL startup lines ($N0/$N1) can set G90.1 which breaks G2/G3 arcs.
    // We always restore G91.1 after the startup sequence completes.
    let mut g91_1_send_at: Option<Instant> = None;

    // After a Welcome/reset, drop any queued non-realtime commands (job lines, console
    // commands) for a short window while the controller is re-initializing. The planner
    // will cancel the active job on Welcome, but a few lines may already be queued.
    let mut drop_non_rt_until: Option<Instant> = None;

    // Buffer for non-realtime commands (GcodeLine, RawCommand) pulled from the
    // channel but not yet processed. This allows us to always drain RT commands
    // immediately while deferring G-code lines when the buffer is full.
    let mut non_rt_queue: std::collections::VecDeque<StreamerCommand> =
        std::collections::VecDeque::new();

    macro_rules! disconnect_serial {
        ($reason:expr) => {{
            if serial.take().is_some() {
                warn!("Serial disconnected: {}", $reason);
                shared_state.connected.store(false, Ordering::Release);
                shared_state
                    .buffer_planner_blocks_available
                    .store(0, Ordering::Release);
                shared_state
                    .buffer_rx_bytes_available
                    .store(0, Ordering::Release);
                shared_state.input_pins_mask.store(0, Ordering::Release);
                shared_state.firmware_type.store(0, Ordering::Release);
                tracker.reset();
                parser.reset();
                pending_line_numbers.clear();
                let _ = pending_line.take();
                let _ = post_reset_block_until.take();
                let _ = g91_1_send_at.take();
                let _ = drop_non_rt_until.take();
                non_rt_queue.clear();
                let _ = event_tx.send(StreamerEvent::Disconnected);
            }
        }};
    }

    'main: loop {
        // 1. Read any available data from serial port
        if serial.is_some() {
            let read_result = {
                let serial_port = serial.as_mut().expect("checked Some");
                serial_port.read(&mut read_buf)
            };
            match read_result {
                Ok(n) if n > 0 => {
                    let responses = parser.feed(&read_buf[..n]);
                    for response in responses {
                        // If this is a Welcome (controller reset), clear all queued
                        // commands, flush GRBL's parser with an empty line, and
                        // schedule $X unlock + G91.1 restore.
                        if matches!(response, GrblResponse::Welcome(_)) {
                            non_rt_queue.clear();
                            pending_line = None;
                            if let Some(serial_port) = serial.as_mut() {
                                // Drop any host-side buffered bytes that were destined for the
                                // pre-reset controller instance (prevents "garbled" commands).
                                let _ = serial_port.clear_output_buffer();
                                // Send empty line to flush any partial/garbled input
                                // sitting in GRBL's parser from stale USB-serial bytes
                                let _ = serial_port.write_all(b"\r\n");
                            }
                            // Schedule $X and G91.1 after USB buffers have drained
                            let deadline = Instant::now() + Duration::from_millis(800);
                            post_reset_block_until = Some(deadline);
                            g91_1_send_at = Some(deadline);
                            drop_non_rt_until = Some(deadline);
                        }
                        handle_response(
                            &response,
                            &mut tracker,
                            &mut pending_line_numbers,
                            &event_tx,
                            &shared_state,
                        );
                    }
                }
                Ok(_) => {}                                                    // no data
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}   // timeout, normal
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {} // would block, normal
                Err(e) => {
                    error!("Serial read error: {}", e);
                    disconnect_serial!("read error");
                }
            }
        }

        // 2. Try to send any pending line that was deferred due to buffer full
        if serial.is_some()
            && pending_line
                .as_ref()
                .is_some_and(|(_, byte_len, _)| tracker.can_send(*byte_len))
            && post_reset_block_until.is_none()
        {
            let (text, byte_len, line_number) = pending_line.take().expect("checked Some");
            let write_result = {
                let serial_port = serial.as_mut().expect("checked Some");
                serial_port
                    .write_all(text.as_bytes())
                    .and_then(|_| serial_port.write_all(b"\r\n"))
            };
            match write_result {
                Ok(()) => {
                    tracker.line_sent(byte_len);
                    pending_line_numbers.push_back(line_number);
                    trace!(
                        "TX line {}: {:?} ({} bytes) [retry]",
                        line_number,
                        text,
                        byte_len
                    );
                    let _ = event_tx.send(StreamerEvent::ConsoleOutput { text, is_tx: true });
                }
                Err(e) => {
                    error!("Serial write error: {}", e);
                    disconnect_serial!("write error");
                }
            }
        }

        // 3. Process commands from planner/server
        // Phase 1: Drain channel. Realtime commands (feed hold, cycle start,
        // status query) are sent to the serial port IMMEDIATELY — they are
        // single-byte commands that bypass GRBL's input buffer and must never
        // be blocked by a pending G-code line. Non-RT commands are queued.
        loop {
            match cmd_rx.try_recv() {
                Ok(StreamerCommand::Connect { port, baud_rate }) => {
                    // Reconnect: drop any existing connection first.
                    disconnect_serial!("reconnect requested");

                    match HardwareSerialPort::open(&port, baud_rate) {
                        Ok(port_handle) => {
                            serial = Some(Box::new(port_handle));
                            shared_state.connected.store(true, Ordering::Release);

                            tracker.reset();
                            parser.reset();
                            pending_line_numbers.clear();
                            pending_line = None;
                            non_rt_queue.clear();
                            last_poll = Instant::now();

                            // Schedule init ($X, G91.1, $I) after the controller banner settles.
                            let deadline = Instant::now() + Duration::from_millis(800);
                            post_reset_block_until = Some(deadline);
                            g91_1_send_at = Some(deadline);

                            let _ = event_tx.send(StreamerEvent::Connected {
                                port: port.clone(),
                                baud_rate,
                            });

                            // Send an empty line to wake up grblHAL (triggers welcome banner).
                            // Matches reference app behavior — do NOT send soft reset (0x18) on connect
                            // because it can trigger startup lines and alter machine state.
                            if let Some(serial_port) = serial.as_mut() {
                                if let Err(e) = serial_port.write_all(b"\r\n") {
                                    warn!("Failed to send startup probe: {}", e);
                                    disconnect_serial!("startup probe failed");
                                } else {
                                    info!("Connecting to serial port: {} @ {}", port, baud_rate);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to open serial port {} @ {}: {}", port, baud_rate, e);
                            let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                                text: format!("Connection failed: {} ({})", port, e),
                                is_tx: false,
                            });
                        }
                    }
                }
                Ok(StreamerCommand::Disconnect) => {
                    disconnect_serial!("disconnect requested");
                }
                Ok(StreamerCommand::Realtime(cmd)) => {
                    if serial.is_none() {
                        continue;
                    }
                    let byte = cmd.to_byte();
                    // Clear both serial buffers before soft reset to prevent stale
                    // bytes from corrupting the post-reset command stream.
                    if byte == 0x18 {
                        if let Some(serial_port) = serial.as_mut() {
                            let _ = serial_port.clear_output_buffer();
                            let _ = serial_port.clear_input_buffer();
                        }
                    }
                    let write_result = {
                        let serial_port = serial.as_mut().expect("checked Some");
                        serial_port.write_rt_command(byte)
                    };
                    if let Err(e) = write_result {
                        error!("Failed to send RT command: {}", e);
                        disconnect_serial!("rt write error");
                    } else {
                        info!("Sent RT command: {:?} (0x{:02X})", cmd, byte);
                        if byte == 0x18 {
                            // After soft reset, block motion sends until Welcome
                            // arrives and post-reset cleanup completes. 800ms is
                            // enough for USB-serial buffers to drain.
                            pending_line = None;
                            pending_line_numbers.clear();
                            tracker.reset();
                            parser.reset();
                            non_rt_queue.clear();
                            post_reset_block_until =
                                Some(Instant::now() + Duration::from_millis(800));
                        }
                    }
                }
                Ok(StreamerCommand::PollStatus) => {
                    if serial.is_some() {
                        let write_result = {
                            let serial_port = serial.as_mut().expect("checked Some");
                            serial_port.write_rt_command(b'?')
                        };
                        if let Err(e) = write_result {
                            error!("Failed to send status query: {}", e);
                            disconnect_serial!("status query write error");
                        }
                    }
                }
                Ok(StreamerCommand::Flush) => {
                    tracker.reset();
                    pending_line_numbers.clear();
                    pending_line = None;
                    non_rt_queue.clear();
                    parser.reset();
                    info!("Streamer flushed");
                }
                Ok(StreamerCommand::Shutdown) => {
                    info!("Streamer shutdown requested");
                    break 'main;
                }
                Ok(other) => {
                    if serial.is_some() {
                        // If we're recovering from a controller reset, drop any queued non-RT
                        // commands that arrived during the recovery window (these are almost
                        // always stale job lines that were already in flight).
                        if drop_non_rt_until.is_some_and(|deadline| Instant::now() < deadline) {
                            match &other {
                                StreamerCommand::GcodeLine { .. }
                                | StreamerCommand::RawCommand(_) => {
                                    continue;
                                }
                                _ => {}
                            }
                        }
                        non_rt_queue.push_back(other);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    info!("Command channel disconnected, shutting down streamer");
                    break 'main;
                }
            }
        }

        // Phase 2: Process queued non-RT commands (GcodeLine, RawCommand).
        // These require buffer space and are deferred when full.
        if serial.is_none() {
            non_rt_queue.clear();
            pending_line = None;
        }
        while let Some(cmd) = non_rt_queue.front() {
            match cmd {
                StreamerCommand::GcodeLine { .. } if pending_line.is_some() => {
                    break; // Can't send — leave in queue for next iteration
                }
                _ => {}
            }
            let cmd = non_rt_queue.pop_front().unwrap();
            match cmd {
                StreamerCommand::GcodeLine {
                    text,
                    byte_len,
                    line_number,
                } => {
                    if serial.is_none() {
                        continue;
                    }
                    if post_reset_block_until.is_some() {
                        // Defer until post-reset delay expires
                        pending_line = Some((text, byte_len, line_number));
                        break;
                    }
                    if tracker.can_send(byte_len) {
                        let write_result = {
                            let serial_port = serial.as_mut().expect("checked Some");
                            serial_port
                                .write_all(text.as_bytes())
                                .and_then(|_| serial_port.write_all(b"\r\n"))
                        };
                        match write_result {
                            Ok(()) => {
                                tracker.line_sent(byte_len);
                                pending_line_numbers.push_back(line_number);
                                trace!("TX line {}: {:?} ({} bytes)", line_number, text, byte_len);
                                let _ = event_tx
                                    .send(StreamerEvent::ConsoleOutput { text, is_tx: true });
                            }
                            Err(e) => {
                                error!("Serial write error: {}", e);
                                disconnect_serial!("gcode write error");
                                break;
                            }
                        }
                    } else {
                        pending_line = Some((text, byte_len, line_number));
                        break; // Stop processing until pending is sent
                    }
                }
                StreamerCommand::RawCommand(text) => {
                    if serial.is_none() {
                        continue;
                    }
                    let byte_len = text.len() + 2; // +2 for \r\n
                    if post_reset_block_until.is_some() || !tracker.can_send(byte_len) {
                        // Defer: re-queue at front so it's retried next iteration
                        non_rt_queue.push_front(StreamerCommand::RawCommand(text));
                        break;
                    }
                    let write_result = {
                        let serial_port = serial.as_mut().expect("checked Some");
                        serial_port
                            .write_all(text.as_bytes())
                            .and_then(|_| serial_port.write_all(b"\r\n"))
                    };
                    if let Err(e) = write_result {
                        error!("Failed to send raw command: {}", e);
                        disconnect_serial!("raw write error");
                    } else {
                        info!("TX raw: {:?} ({} bytes)", text, byte_len);
                        tracker.line_sent(byte_len);
                        pending_line_numbers.push_back(usize::MAX); // sentinel for raw commands
                        let _ = event_tx.send(StreamerEvent::ConsoleOutput { text, is_tx: true });
                    }
                }
                _ => {} // RT/Flush/Shutdown already handled in phase 1
            }
        }

        // 4. Expire post-reset motion block
        if post_reset_block_until.is_some_and(|deadline| Instant::now() >= deadline) {
            post_reset_block_until = None;
        }

        // 4b. After Welcome, send $X (unlock) then G91.1 (incremental arc mode)
        if serial.is_some() && g91_1_send_at.is_some_and(|t| Instant::now() >= t) {
            // Clear input buffer to discard any stale responses from USB drain
            if let Some(serial_port) = serial.as_mut() {
                let _ = serial_port.clear_input_buffer();
            }
            parser.reset();
            tracker.reset();
            pending_line_numbers.clear();

            // Send $X to unlock (safe even if not in alarm — just returns ok)
            let unlock = "$X\r\n";
            let unlock_result = {
                let Some(serial_port) = serial.as_mut() else {
                    continue;
                };
                serial_port.write_all(unlock.as_bytes())
            };
            if let Err(e) = unlock_result {
                error!("Failed to send $X: {}", e);
                disconnect_serial!("unlock write error");
            } else {
                info!("Sent $X (unlock after reset)");
                tracker.line_sent(unlock.len());
                pending_line_numbers.push_back(usize::MAX);
            }

            // Send G91.1 to restore incremental arc distance mode
            let g91 = "G91.1\r\n";
            let g91_result = {
                let Some(serial_port) = serial.as_mut() else {
                    continue;
                };
                serial_port.write_all(g91.as_bytes())
            };
            if let Err(e) = g91_result {
                error!("Failed to send G91.1: {}", e);
                disconnect_serial!("g91.1 write error");
            } else {
                info!("Sent G91.1 (incremental arc distance mode)");
                tracker.line_sent(g91.len());
                pending_line_numbers.push_back(usize::MAX);
            }

            // Send $I to query grblHAL build info (board, firmware, driver, etc.)
            let info_cmd = "$I\r\n";
            let info_result = {
                let Some(serial_port) = serial.as_mut() else {
                    continue;
                };
                serial_port.write_all(info_cmd.as_bytes())
            };
            if let Err(e) = info_result {
                error!("Failed to send $I: {}", e);
                disconnect_serial!("$I write error");
            } else {
                info!("Sent $I (grblHAL build info query)");
                tracker.line_sent(info_cmd.len());
                pending_line_numbers.push_back(usize::MAX);
            }

            // Release the post-reset block now that cleanup is done
            post_reset_block_until = None;
            g91_1_send_at = None;
            drop_non_rt_until = None;
        }

        // 5. Periodic status polling
        if serial.is_some() && last_poll.elapsed() >= config.status_poll_interval {
            let write_result = {
                let serial_port = serial.as_mut().expect("checked Some");
                serial_port.write_rt_command(b'?')
            };
            if let Err(e) = write_result {
                error!("Failed to send periodic status query: {}", e);
                disconnect_serial!("periodic status write error");
            } else {
                last_poll = Instant::now();
            }
        }

        // Small sleep to prevent busy-waiting while still being responsive
        std::thread::sleep(if serial.is_some() {
            Duration::from_micros(100)
        } else {
            Duration::from_millis(5)
        });
    }

    disconnect_serial!("streamer thread exiting");
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
            match pending_line_numbers.pop_front() {
                Some(usize::MAX) => {
                    // Raw console command — show "ok" in console
                    let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                        text: "ok".to_string(),
                        is_tx: false,
                    });
                }
                Some(line_number) => {
                    let _ = event_tx.send(StreamerEvent::LineAcknowledged { line_number });
                }
                None => {
                    // Untracked ok (typically from controller startup lines or stale input).
                    let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                        text: "ok".to_string(),
                        is_tx: false,
                    });
                }
            }
        }
        GrblResponse::Error(code) => {
            tracker.line_acknowledged();
            let message = rustcnc_core::grbl::error_codes::GrblError::from_code(*code)
                .map(|e| e.message().to_string())
                .unwrap_or_else(|| format!("Unknown error {}", code));
            match pending_line_numbers.pop_front() {
                Some(usize::MAX) => {
                    // Raw console command — show error in console
                    let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                        text: format!("error:{} ({})", code, message),
                        is_tx: false,
                    });
                }
                Some(line_number) => {
                    error!("RX error:{} on line {}: {}", code, line_number, message);
                    let _ = event_tx.send(StreamerEvent::LineError {
                        line_number,
                        code: *code,
                        message,
                    });
                }
                None => {
                    // Untracked error (startup line / stale bytes). Report to console
                    // but do not attribute it to "line 0".
                    let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                        text: format!("error:{} ({})", code, message),
                        is_tx: false,
                    });
                }
            }
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
            shared_state.set_firmware_type(detect_firmware_type(version));
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
                is_tx: false,
            });
        }
        GrblResponse::Feedback(text) | GrblResponse::Unknown(text) => {
            let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                text: text.clone(),
                is_tx: false,
            });
        }
        GrblResponse::StartupLine(text) => {
            let _ = event_tx.send(StreamerEvent::ConsoleOutput {
                text: format!(">{}", text),
                is_tx: false,
            });
        }
    }
}

use std::io::Write;

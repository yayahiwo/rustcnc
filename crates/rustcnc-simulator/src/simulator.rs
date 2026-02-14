use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use tracing::{debug, info, trace};

use rustcnc_core::grbl::realtime;
use crate::motion::{LinearMove, compute_target};
use crate::parser::{SimCommand, parse_sim_command};
use crate::state_machine::{GrblStateMachine, SimState};
use crate::virtual_serial::VirtualSerialPort;

/// Configuration for the simulator
pub struct SimulatorConfig {
    pub rx_buffer_size: usize,
    pub max_feed_rate: f64,
    pub max_rapid_rate: f64,
    pub motion_speed_factor: f64,
    pub startup_delay_ms: u64,
}

impl Default for SimulatorConfig {
    fn default() -> Self {
        Self {
            rx_buffer_size: 128,
            max_feed_rate: 8000.0,
            max_rapid_rate: 5000.0,
            motion_speed_factor: 10.0,
            startup_delay_ms: 100,
        }
    }
}

/// The GRBL simulator engine.
///
/// Creates a pair of channels that act like a serial port connection.
/// The streamer writes to one end, and the simulator reads from it,
/// processes commands, and writes responses back.
pub struct GrblSimulator {
    config: SimulatorConfig,
}

impl GrblSimulator {
    pub fn new(config: SimulatorConfig) -> Self {
        Self { config }
    }

    /// Start the simulator in a background thread.
    /// Returns a VirtualSerialPort that the streamer can use.
    pub fn start(self) -> VirtualSerialPort {
        // Host -> Controller channel (streamer writes, sim reads)
        let (host_tx, controller_rx) = unbounded::<Vec<u8>>();
        // Controller -> Host channel (sim writes, streamer reads)
        let (controller_tx, host_rx) = unbounded::<Vec<u8>>();

        let config = self.config;

        thread::spawn(move || {
            simulator_thread(controller_rx, controller_tx, config);
        });

        VirtualSerialPort::new(host_tx, host_rx)
    }
}

/// The simulator's main thread
fn simulator_thread(
    rx: Receiver<Vec<u8>>,
    tx: Sender<Vec<u8>>,
    config: SimulatorConfig,
) {
    info!("GRBL Simulator started");

    // Send startup delay
    thread::sleep(Duration::from_millis(config.startup_delay_ms));

    // Send welcome message
    let mut state = GrblStateMachine::new();
    let welcome = state.welcome_message();
    let _ = tx.send(welcome.into_bytes());

    let mut line_buffer = String::new();
    let mut last_status_time = Instant::now();

    loop {
        // Read incoming data
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(data) => {
                for &byte in &data {
                    // Check for real-time commands first (single bytes)
                    if handle_realtime_byte(byte, &mut state, &tx) {
                        continue;
                    }

                    // Accumulate regular characters
                    if byte == b'\n' || byte == b'\r' {
                        if !line_buffer.is_empty() {
                            let response = process_command(&line_buffer, &mut state, &config);
                            let _ = tx.send(response.into_bytes());
                            line_buffer.clear();
                        }
                    } else {
                        line_buffer.push(byte as char);
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                info!("Simulator: host disconnected, shutting down");
                break;
            }
        }
    }
}

/// Handle a single byte as a potential real-time command.
/// Returns true if it was a real-time command (consumed).
fn handle_realtime_byte(byte: u8, state: &mut GrblStateMachine, tx: &Sender<Vec<u8>>) -> bool {
    match byte {
        realtime::RT_STATUS_QUERY => {
            let report = state.status_report();
            let _ = tx.send(format!("{}\r\n", report).into_bytes());
            true
        }
        realtime::RT_FEED_HOLD => {
            state.feed_hold();
            true
        }
        realtime::RT_CYCLE_START => {
            state.cycle_start();
            true
        }
        realtime::RT_SOFT_RESET => {
            state.soft_reset();
            let welcome = state.welcome_message();
            let _ = tx.send(welcome.into_bytes());
            true
        }
        realtime::RT_JOG_CANCEL => {
            if state.state == SimState::Jog {
                state.state = SimState::Idle;
            }
            true
        }
        // Feed overrides
        realtime::RT_FEED_OVR_RESET => {
            state.feed_override = 100;
            true
        }
        realtime::RT_FEED_OVR_COARSE_PLUS => {
            state.feed_override = (state.feed_override as u16 + 10).min(200) as u8;
            true
        }
        realtime::RT_FEED_OVR_COARSE_MINUS => {
            state.feed_override = (state.feed_override as i16 - 10).max(10) as u8;
            true
        }
        realtime::RT_FEED_OVR_FINE_PLUS => {
            state.feed_override = (state.feed_override as u16 + 1).min(200) as u8;
            true
        }
        realtime::RT_FEED_OVR_FINE_MINUS => {
            state.feed_override = (state.feed_override as i16 - 1).max(10) as u8;
            true
        }
        // Rapid overrides
        realtime::RT_RAPID_OVR_RESET => {
            state.rapid_override = 100;
            true
        }
        realtime::RT_RAPID_OVR_MEDIUM => {
            state.rapid_override = 50;
            true
        }
        realtime::RT_RAPID_OVR_LOW => {
            state.rapid_override = 25;
            true
        }
        // Spindle overrides
        realtime::RT_SPINDLE_OVR_RESET => {
            state.spindle_override = 100;
            true
        }
        realtime::RT_SPINDLE_OVR_COARSE_PLUS => {
            state.spindle_override = (state.spindle_override as u16 + 10).min(200) as u8;
            true
        }
        realtime::RT_SPINDLE_OVR_COARSE_MINUS => {
            state.spindle_override = (state.spindle_override as i16 - 10).max(10) as u8;
            true
        }
        // Coolant toggles
        realtime::RT_COOLANT_FLOOD_TOGGLE => {
            state.flood_coolant = !state.flood_coolant;
            true
        }
        realtime::RT_COOLANT_MIST_TOGGLE => {
            state.mist_coolant = !state.mist_coolant;
            true
        }
        // Non-RT byte
        _ if byte >= 0x80 => true, // Consume unknown extended RT commands
        _ => false,
    }
}

/// Process a complete command line and return the response string
fn process_command(line: &str, state: &mut GrblStateMachine, config: &SimulatorConfig) -> String {
    let cmd = parse_sim_command(line);
    trace!("Simulator processing: {} -> {:?}", line, cmd);

    match cmd {
        SimCommand::RapidMove { axes } => {
            let target = compute_target(&state.position, &axes, state.absolute_mode);
            state.state = SimState::Run;
            // Instant move in simulator (simplified)
            state.position = target;
            state.state = SimState::Idle;
            "ok\r\n".to_string()
        }
        SimCommand::LinearMove { axes, f } => {
            if let Some(feed) = f {
                state.feed_rate = feed;
            }
            let target = compute_target(&state.position, &axes, state.absolute_mode);
            state.state = SimState::Run;
            state.position = target;
            state.state = SimState::Idle;
            "ok\r\n".to_string()
        }
        SimCommand::Home | SimCommand::HomingCycle => {
            state.state = SimState::Home;
            state.position = [0.0; 8];
            state.homed = true;
            state.state = SimState::Idle;
            "ok\r\n".to_string()
        }
        SimCommand::AbsoluteMode => {
            state.absolute_mode = true;
            "ok\r\n".to_string()
        }
        SimCommand::IncrementalMode => {
            state.absolute_mode = false;
            "ok\r\n".to_string()
        }
        SimCommand::MmMode => {
            state.units_mm = true;
            "ok\r\n".to_string()
        }
        SimCommand::InchMode => {
            state.units_mm = false;
            "ok\r\n".to_string()
        }
        SimCommand::SpindleCW { speed } => {
            state.spindle_on = true;
            state.spindle_cw = true;
            if let Some(s) = speed {
                state.spindle_speed = s;
            }
            "ok\r\n".to_string()
        }
        SimCommand::SpindleCCW { speed } => {
            state.spindle_on = true;
            state.spindle_cw = false;
            if let Some(s) = speed {
                state.spindle_speed = s;
            }
            "ok\r\n".to_string()
        }
        SimCommand::SpindleOff => {
            state.spindle_on = false;
            state.spindle_speed = 0.0;
            "ok\r\n".to_string()
        }
        SimCommand::FloodCoolantOn => {
            state.flood_coolant = true;
            "ok\r\n".to_string()
        }
        SimCommand::CoolantOff => {
            state.flood_coolant = false;
            state.mist_coolant = false;
            "ok\r\n".to_string()
        }
        SimCommand::ProgramEnd => {
            state.spindle_on = false;
            state.spindle_speed = 0.0;
            state.flood_coolant = false;
            state.mist_coolant = false;
            state.state = SimState::Idle;
            "ok\r\n".to_string()
        }
        SimCommand::Unlock => {
            state.unlock();
            "[MSG:Caution: Unlocked]\r\nok\r\n".to_string()
        }
        SimCommand::RequestSettings => {
            // Return a minimal set of GRBL settings
            let mut response = String::new();
            response.push_str("$0=10\r\n");
            response.push_str("$1=25\r\n");
            response.push_str("$10=1\r\n");
            response.push_str("$22=1\r\n");
            response.push_str("$100=250.000\r\n");
            response.push_str("$101=250.000\r\n");
            response.push_str("$102=250.000\r\n");
            response.push_str("$110=8000.000\r\n");
            response.push_str("$111=8000.000\r\n");
            response.push_str("$112=2000.000\r\n");
            response.push_str("$120=500.000\r\n");
            response.push_str("$121=500.000\r\n");
            response.push_str("$122=200.000\r\n");
            response.push_str("$130=800.000\r\n");
            response.push_str("$131=800.000\r\n");
            response.push_str("$132=300.000\r\n");
            response.push_str("ok\r\n");
            response
        }
        SimCommand::Jog { axes, f, incremental } => {
            let target = compute_target(&state.position, &axes, !incremental);
            state.state = SimState::Jog;
            state.feed_rate = f;
            state.position = target;
            state.state = SimState::Idle;
            "ok\r\n".to_string()
        }
        SimCommand::SetWorkOffset { axes } => {
            for i in 0..8 {
                if let Some(val) = axes[i] {
                    state.work_offset[i] = state.position[i] - val;
                }
            }
            "ok\r\n".to_string()
        }
        SimCommand::Unknown(_) => {
            "ok\r\n".to_string()
        }
    }
}

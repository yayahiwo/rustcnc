use serde::{Deserialize, Serialize};

use crate::overrides::Overrides;

/// Primary machine state reported by GRBL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MachineState {
    Idle,
    Run,
    Hold(u8),   // substate: 0=hold-complete, 1=hold-in-progress
    Jog,
    Alarm(u8),  // alarm code
    Door(u8),   // substate: 0=closed-holding, 1=closed-resuming, 2=opened, 3=closing
    Check,
    Home,
    Sleep,
    Tool, // grblHAL: tool change pending
}

impl Default for MachineState {
    fn default() -> Self {
        Self::Idle
    }
}

impl MachineState {
    /// Encode state to a u16 for atomic storage.
    /// High byte = state enum (0-9), low byte = substate.
    pub fn to_u16(self) -> u16 {
        match self {
            Self::Idle => 0 << 8,
            Self::Run => 1 << 8,
            Self::Hold(sub) => (2 << 8) | sub as u16,
            Self::Jog => 3 << 8,
            Self::Alarm(sub) => (4 << 8) | sub as u16,
            Self::Door(sub) => (5 << 8) | sub as u16,
            Self::Check => 6 << 8,
            Self::Home => 7 << 8,
            Self::Sleep => 8 << 8,
            Self::Tool => 9 << 8,
        }
    }

    /// Decode state from a u16 (high byte = state enum, low byte = substate).
    pub fn from_u16(v: u16) -> Self {
        let state = (v >> 8) as u8;
        let sub = (v & 0xFF) as u8;
        match state {
            0 => Self::Idle,
            1 => Self::Run,
            2 => Self::Hold(sub),
            3 => Self::Jog,
            4 => Self::Alarm(sub),
            5 => Self::Door(sub),
            6 => Self::Check,
            7 => Self::Home,
            8 => Self::Sleep,
            9 => Self::Tool,
            _ => Self::Idle,
        }
    }

    /// Returns true if the machine is in a state where G-code can be streamed
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Run | Self::Jog)
    }

    /// Returns true if the machine requires user intervention
    pub fn needs_attention(&self) -> bool {
        matches!(self, Self::Alarm(_) | Self::Door(_))
    }
}

/// 3D position in machine or work coordinates
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub u: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v: Option<f64>,
}

impl Position {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self {
            x,
            y,
            z,
            a: None,
            b: None,
            c: None,
            u: None,
            v: None,
        }
    }

    pub fn zero() -> Self {
        Self::default()
    }

    /// Distance to another position (all configured axes)
    pub fn distance_to(&self, other: &Position) -> f64 {
        let mut sum = (other.x - self.x).powi(2)
            + (other.y - self.y).powi(2)
            + (other.z - self.z).powi(2);
        if let (Some(a1), Some(a2)) = (self.a, other.a) {
            sum += (a2 - a1).powi(2);
        }
        if let (Some(b1), Some(b2)) = (self.b, other.b) {
            sum += (b2 - b1).powi(2);
        }
        if let (Some(c1), Some(c2)) = (self.c, other.c) {
            sum += (c2 - c1).powi(2);
        }
        if let (Some(u1), Some(u2)) = (self.u, other.u) {
            sum += (u2 - u1).powi(2);
        }
        if let (Some(v1), Some(v2)) = (self.v, other.v) {
            sum += (v2 - v1).powi(2);
        }
        sum.sqrt()
    }
}

/// Buffer state reported by GRBL
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BufferState {
    pub planner_blocks_available: u16,
    pub rx_bytes_available: u16,
}

/// Accessory state flags
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AccessoryState {
    pub spindle_cw: bool,
    pub spindle_ccw: bool,
    pub flood_coolant: bool,
    pub mist_coolant: bool,
}

/// Active input pin signals
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct InputPins {
    pub limit_x: bool,
    pub limit_y: bool,
    pub limit_z: bool,
    pub limit_a: bool,
    pub limit_b: bool,
    pub limit_c: bool,
    pub limit_u: bool,
    pub limit_v: bool,
    pub probe: bool,
    pub door: bool,
    pub hold: bool,
    pub soft_reset: bool,
    pub cycle_start: bool,
    pub estop: bool, // grblHAL
}

/// Complete parsed status report from GRBL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReport {
    pub state: MachineState,
    pub machine_position: Option<Position>,
    pub work_position: Option<Position>,
    pub work_coordinate_offset: Option<Position>,
    pub buffer: Option<BufferState>,
    pub line_number: Option<u32>,
    pub feed_rate: Option<f64>,
    pub spindle_speed: Option<f64>,
    pub input_pins: Option<InputPins>,
    pub overrides: Option<Overrides>,
    pub accessories: Option<AccessoryState>,
}

/// Firmware type detected from the controller
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FirmwareType {
    Grbl,
    GrblHal,
    Unknown,
}

impl Default for FirmwareType {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Connection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub port: String,
    pub baud_rate: u32,
    pub firmware: FirmwareType,
    pub version: String,
    pub options: Vec<String>,
}

/// Complete snapshot of machine state for UI consumption.
/// This is the structure broadcast over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineSnapshot {
    pub state: MachineState,
    pub machine_pos: Position,
    pub work_pos: Position,
    pub feed_rate: f64,
    pub spindle_speed: f64,
    pub overrides: Overrides,
    pub accessories: AccessoryState,
    pub input_pins: InputPins,
    pub buffer: BufferState,
    pub line_number: u32,
    pub connected: bool,
    pub firmware: FirmwareType,
}

impl Default for MachineSnapshot {
    fn default() -> Self {
        Self {
            state: MachineState::Idle,
            machine_pos: Position::zero(),
            work_pos: Position::zero(),
            feed_rate: 0.0,
            spindle_speed: 0.0,
            overrides: Overrides::default(),
            accessories: AccessoryState::default(),
            input_pins: InputPins::default(),
            buffer: BufferState::default(),
            line_number: 0,
            connected: false,
            firmware: FirmwareType::Unknown,
        }
    }
}

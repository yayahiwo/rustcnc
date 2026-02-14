use rustcnc_core::machine::MachineState;

/// GRBL state machine emulation for the simulator
#[derive(Debug, Clone)]
pub struct GrblStateMachine {
    pub state: SimState,
    pub position: [f64; 3],
    pub work_offset: [f64; 3],
    pub feed_rate: f64,
    pub spindle_speed: f64,
    pub spindle_on: bool,
    pub spindle_cw: bool,
    pub flood_coolant: bool,
    pub mist_coolant: bool,
    pub feed_override: u8,
    pub rapid_override: u8,
    pub spindle_override: u8,
    pub alarm_code: Option<u8>,
    pub homed: bool,
    pub units_mm: bool, // true = mm (G21), false = inches (G20)
    pub absolute_mode: bool, // true = G90, false = G91
    pub planner_buffer_used: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimState {
    Idle,
    Run,
    Hold,
    Jog,
    Alarm,
    Home,
    Check,
}

impl SimState {
    pub fn to_grbl_string(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Run => "Run",
            Self::Hold => "Hold:0",
            Self::Jog => "Jog",
            Self::Alarm => "Alarm",
            Self::Home => "Home",
            Self::Check => "Check",
        }
    }
}

impl Default for GrblStateMachine {
    fn default() -> Self {
        Self {
            state: SimState::Idle,
            position: [0.0; 3],
            work_offset: [0.0; 3],
            feed_rate: 0.0,
            spindle_speed: 0.0,
            spindle_on: false,
            spindle_cw: true,
            flood_coolant: false,
            mist_coolant: false,
            feed_override: 100,
            rapid_override: 100,
            spindle_override: 100,
            alarm_code: None,
            homed: false,
            units_mm: true,
            absolute_mode: true,
            planner_buffer_used: 0,
        }
    }
}

impl GrblStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a GRBL status report string
    pub fn status_report(&self) -> String {
        let state_str = self.state.to_grbl_string();
        let mpos = format!(
            "MPos:{:.3},{:.3},{:.3}",
            self.position[0], self.position[1], self.position[2]
        );
        let fs = format!("FS:{:.0},{:.0}", self.feed_rate, self.spindle_speed);
        let ov = format!(
            "Ov:{},{},{}",
            self.feed_override, self.rapid_override, self.spindle_override
        );
        let bf = format!("Bf:{},{}", 15 - self.planner_buffer_used, 128);

        format!("<{}|{}|{}|{}|{}>", state_str, mpos, fs, ov, bf)
    }

    /// Generate the welcome message
    pub fn welcome_message(&self) -> String {
        "Grbl 1.1h ['$' for help]\r\n".to_string()
    }

    /// Work position (machine position minus work offset)
    pub fn work_position(&self) -> [f64; 3] {
        [
            self.position[0] - self.work_offset[0],
            self.position[1] - self.work_offset[1],
            self.position[2] - self.work_offset[2],
        ]
    }

    /// Apply a feed hold
    pub fn feed_hold(&mut self) {
        if self.state == SimState::Run || self.state == SimState::Jog {
            self.state = SimState::Hold;
        }
    }

    /// Resume from hold
    pub fn cycle_start(&mut self) {
        if self.state == SimState::Hold {
            self.state = SimState::Run;
        }
    }

    /// Soft reset
    pub fn soft_reset(&mut self) {
        self.state = SimState::Idle;
        self.feed_rate = 0.0;
        self.spindle_speed = 0.0;
        self.spindle_on = false;
        self.flood_coolant = false;
        self.mist_coolant = false;
        self.feed_override = 100;
        self.rapid_override = 100;
        self.spindle_override = 100;
        self.planner_buffer_used = 0;
    }

    /// Trigger an alarm
    pub fn trigger_alarm(&mut self, code: u8) {
        self.state = SimState::Alarm;
        self.alarm_code = Some(code);
    }

    /// Unlock from alarm state
    pub fn unlock(&mut self) {
        if self.state == SimState::Alarm {
            self.state = SimState::Idle;
            self.alarm_code = None;
        }
    }
}

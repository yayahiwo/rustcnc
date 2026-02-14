use serde::{Deserialize, Serialize};

/// GRBL error codes (returned as `error:N`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum GrblError {
    ExpectedCommandLetter = 1,
    BadNumberFormat = 2,
    InvalidDollarCommand = 3,
    NegativeValue = 4,
    HomingNotEnabled = 5,
    MinStepPulse = 6,
    EepromReadFail = 7,
    IdleRequired = 8,
    AlarmLock = 9,
    SoftLimitNoHome = 10,
    LineOverflow = 11,
    MaxStepRateExceeded = 12,
    SafetyDoorOpen = 13,
    LineLengthExceeded = 14,
    TravelExceeded = 15,
    InvalidJogCommand = 16,
    LaserRequiresGrbl = 17,
    UnsupportedCommand = 20,
    ModalGroupViolation = 21,
    UndefinedFeedRate = 22,
    InvalidGcodeId = 23,
    AxisWordMissing = 24,
    LineNumberInvalid = 25,
    MissingPOrLValue = 26,
    DecimalMissing = 27,
    GCodeCommandInG43 = 28,
    G53RequiresG0OrG1 = 29,
    AxisNotConfigured = 30,
    GCodeConflict = 31,
    G2G3ArcsNotSupported = 32,
    MotionMissingTarget = 33,
    ArcRadiusError = 34,
    G2G3MissingOffset = 35,
    UnusedGCodeWord = 36,
    G43DynamicNotConfigured = 37,
    MaxToolNumberExceeded = 38,
}

impl GrblError {
    /// Get human-readable error message
    pub fn message(self) -> &'static str {
        match self {
            Self::ExpectedCommandLetter => "Expected command letter",
            Self::BadNumberFormat => "Bad number format",
            Self::InvalidDollarCommand => "Invalid $ command",
            Self::NegativeValue => "Negative value not allowed",
            Self::HomingNotEnabled => "Homing not enabled in settings",
            Self::MinStepPulse => "Min step pulse time exceeded",
            Self::EepromReadFail => "EEPROM read failed",
            Self::IdleRequired => "Command requires idle state",
            Self::AlarmLock => "G-code locked during alarm",
            Self::SoftLimitNoHome => "Soft limits require homing",
            Self::LineOverflow => "Line overflow",
            Self::MaxStepRateExceeded => "Max step rate exceeded",
            Self::SafetyDoorOpen => "Safety door open",
            Self::LineLengthExceeded => "Line length exceeded",
            Self::TravelExceeded => "Travel exceeded",
            Self::InvalidJogCommand => "Invalid jog command",
            Self::LaserRequiresGrbl => "Laser mode requires GRBL 1.1+",
            Self::UnsupportedCommand => "Unsupported or invalid command",
            Self::ModalGroupViolation => "Modal group violation",
            Self::UndefinedFeedRate => "Undefined feed rate",
            Self::InvalidGcodeId => "Invalid G-code command",
            Self::AxisWordMissing => "Required axis word missing",
            Self::LineNumberInvalid => "Invalid line number",
            Self::MissingPOrLValue => "Missing P or L value",
            Self::DecimalMissing => "Missing decimal value",
            Self::GCodeCommandInG43 => "G-code command not allowed in G43",
            Self::G53RequiresG0OrG1 => "G53 requires G0 or G1",
            Self::AxisNotConfigured => "Axis word in command that doesn't use it",
            Self::GCodeConflict => "G-code command conflict",
            Self::G2G3ArcsNotSupported => "G2/G3 arcs not supported in this plane",
            Self::MotionMissingTarget => "Motion command with no target",
            Self::ArcRadiusError => "Arc radius error",
            Self::G2G3MissingOffset => "G2/G3 missing I, J, or K offset",
            Self::UnusedGCodeWord => "Unused G-code word found",
            Self::G43DynamicNotConfigured => "G43.1 dynamic TLO not configured",
            Self::MaxToolNumberExceeded => "Max tool number exceeded",
        }
    }

    /// Parse from error code number
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::ExpectedCommandLetter),
            2 => Some(Self::BadNumberFormat),
            3 => Some(Self::InvalidDollarCommand),
            4 => Some(Self::NegativeValue),
            5 => Some(Self::HomingNotEnabled),
            6 => Some(Self::MinStepPulse),
            7 => Some(Self::EepromReadFail),
            8 => Some(Self::IdleRequired),
            9 => Some(Self::AlarmLock),
            10 => Some(Self::SoftLimitNoHome),
            11 => Some(Self::LineOverflow),
            12 => Some(Self::MaxStepRateExceeded),
            13 => Some(Self::SafetyDoorOpen),
            14 => Some(Self::LineLengthExceeded),
            15 => Some(Self::TravelExceeded),
            16 => Some(Self::InvalidJogCommand),
            17 => Some(Self::LaserRequiresGrbl),
            20 => Some(Self::UnsupportedCommand),
            21 => Some(Self::ModalGroupViolation),
            22 => Some(Self::UndefinedFeedRate),
            23 => Some(Self::InvalidGcodeId),
            24 => Some(Self::AxisWordMissing),
            25 => Some(Self::LineNumberInvalid),
            26 => Some(Self::MissingPOrLValue),
            27 => Some(Self::DecimalMissing),
            28 => Some(Self::GCodeCommandInG43),
            29 => Some(Self::G53RequiresG0OrG1),
            30 => Some(Self::AxisNotConfigured),
            31 => Some(Self::GCodeConflict),
            32 => Some(Self::G2G3ArcsNotSupported),
            33 => Some(Self::MotionMissingTarget),
            34 => Some(Self::ArcRadiusError),
            35 => Some(Self::G2G3MissingOffset),
            36 => Some(Self::UnusedGCodeWord),
            37 => Some(Self::G43DynamicNotConfigured),
            38 => Some(Self::MaxToolNumberExceeded),
            _ => None,
        }
    }
}

/// GRBL alarm codes (returned as `ALARM:N`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum GrblAlarm {
    HardLimit = 1,
    SoftLimit = 2,
    AbortCycle = 3,
    ProbeFailInitial = 4,
    ProbeFailContact = 5,
    HomingFailReset = 6,
    HomingFailDoor = 7,
    HomingFailPullOff = 8,
    HomingFailApproach = 9,
    EStop = 10,
    HomingRequired = 11,
}

impl GrblAlarm {
    pub fn message(self) -> &'static str {
        match self {
            Self::HardLimit => "Hard limit triggered",
            Self::SoftLimit => "Soft limit reached",
            Self::AbortCycle => "Abort during cycle",
            Self::ProbeFailInitial => "Probe fail: not in expected initial state",
            Self::ProbeFailContact => "Probe fail: no contact",
            Self::HomingFailReset => "Homing fail: reset during homing",
            Self::HomingFailDoor => "Homing fail: safety door opened",
            Self::HomingFailPullOff => "Homing fail: pull-off failed",
            Self::HomingFailApproach => "Homing fail: approach failed",
            Self::EStop => "Emergency stop activated",
            Self::HomingRequired => "Homing required",
        }
    }

    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::HardLimit),
            2 => Some(Self::SoftLimit),
            3 => Some(Self::AbortCycle),
            4 => Some(Self::ProbeFailInitial),
            5 => Some(Self::ProbeFailContact),
            6 => Some(Self::HomingFailReset),
            7 => Some(Self::HomingFailDoor),
            8 => Some(Self::HomingFailPullOff),
            9 => Some(Self::HomingFailApproach),
            10 => Some(Self::EStop),
            11 => Some(Self::HomingRequired),
            _ => None,
        }
    }
}

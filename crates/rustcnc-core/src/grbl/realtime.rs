use serde::{Deserialize, Serialize};

// ── ASCII real-time commands ──────────────────────────────────────

pub const RT_STATUS_QUERY: u8 = b'?';
pub const RT_CYCLE_START: u8 = b'~';
pub const RT_FEED_HOLD: u8 = b'!';
pub const RT_SOFT_RESET: u8 = 0x18; // Ctrl-X

// ── Extended real-time commands (grblHAL) ─────────────────────────

pub const RT_STOP: u8 = 0x19;
pub const RT_STATUS_QUERY_ALT: u8 = 0x80;
pub const RT_CYCLE_START_ALT: u8 = 0x81;
pub const RT_FEED_HOLD_ALT: u8 = 0x82;
pub const RT_PARSER_STATE_REPORT: u8 = 0x83;
pub const RT_SAFETY_DOOR: u8 = 0x84;
pub const RT_JOG_CANCEL: u8 = 0x85;
pub const RT_COMPLETE_STATUS: u8 = 0x87;
pub const RT_TOGGLE_OPTIONAL_STOP: u8 = 0x88;
pub const RT_TOGGLE_SINGLE_STEP: u8 = 0x89;

// ── Feed overrides ────────────────────────────────────────────────

pub const RT_FEED_OVR_RESET: u8 = 0x90; // 100%
pub const RT_FEED_OVR_COARSE_PLUS: u8 = 0x91; // +10%
pub const RT_FEED_OVR_COARSE_MINUS: u8 = 0x92; // -10%
pub const RT_FEED_OVR_FINE_PLUS: u8 = 0x93; // +1%
pub const RT_FEED_OVR_FINE_MINUS: u8 = 0x94; // -1%

// ── Rapid overrides ──────────────────────────────────────────────

pub const RT_RAPID_OVR_RESET: u8 = 0x95; // 100%
pub const RT_RAPID_OVR_MEDIUM: u8 = 0x96; // 50%
pub const RT_RAPID_OVR_LOW: u8 = 0x97; // 25%

// ── Spindle overrides ────────────────────────────────────────────

pub const RT_SPINDLE_OVR_RESET: u8 = 0x99; // 100%
pub const RT_SPINDLE_OVR_COARSE_PLUS: u8 = 0x9A; // +10%
pub const RT_SPINDLE_OVR_COARSE_MINUS: u8 = 0x9B; // -10%
pub const RT_SPINDLE_OVR_FINE_PLUS: u8 = 0x9C; // +1%
pub const RT_SPINDLE_OVR_FINE_MINUS: u8 = 0x9D; // -1%
pub const RT_SPINDLE_OVR_STOP: u8 = 0x9E;

// ── Coolant overrides ────────────────────────────────────────────

pub const RT_COOLANT_FLOOD_TOGGLE: u8 = 0xA0;
pub const RT_COOLANT_MIST_TOGGLE: u8 = 0xA1;

// ── grblHAL extras ───────────────────────────────────────────────

pub const RT_TOOL_ACK: u8 = 0xA3;
pub const RT_PROBE_TOGGLE: u8 = 0xA4;

/// Enumeration of all real-time commands with human-readable mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealtimeCommand {
    StatusQuery,
    CycleStart,
    FeedHold,
    SoftReset,
    SafetyDoor,
    JogCancel,
    FeedOverrideReset,
    FeedOverridePlus10,
    FeedOverrideMinus10,
    FeedOverridePlus1,
    FeedOverrideMinus1,
    RapidOverrideReset,
    RapidOverride50,
    RapidOverride25,
    SpindleOverrideReset,
    SpindleOverridePlus10,
    SpindleOverrideMinus10,
    SpindleOverridePlus1,
    SpindleOverrideMinus1,
    SpindleStop,
    CoolantFloodToggle,
    CoolantMistToggle,
    CompleteStatus,
}

impl RealtimeCommand {
    /// Convert to the byte(s) to send over serial
    pub fn to_byte(self) -> u8 {
        match self {
            Self::StatusQuery => RT_STATUS_QUERY,
            Self::CycleStart => RT_CYCLE_START,
            Self::FeedHold => RT_FEED_HOLD,
            Self::SoftReset => RT_SOFT_RESET,
            Self::SafetyDoor => RT_SAFETY_DOOR,
            Self::JogCancel => RT_JOG_CANCEL,
            Self::FeedOverrideReset => RT_FEED_OVR_RESET,
            Self::FeedOverridePlus10 => RT_FEED_OVR_COARSE_PLUS,
            Self::FeedOverrideMinus10 => RT_FEED_OVR_COARSE_MINUS,
            Self::FeedOverridePlus1 => RT_FEED_OVR_FINE_PLUS,
            Self::FeedOverrideMinus1 => RT_FEED_OVR_FINE_MINUS,
            Self::RapidOverrideReset => RT_RAPID_OVR_RESET,
            Self::RapidOverride50 => RT_RAPID_OVR_MEDIUM,
            Self::RapidOverride25 => RT_RAPID_OVR_LOW,
            Self::SpindleOverrideReset => RT_SPINDLE_OVR_RESET,
            Self::SpindleOverridePlus10 => RT_SPINDLE_OVR_COARSE_PLUS,
            Self::SpindleOverrideMinus10 => RT_SPINDLE_OVR_COARSE_MINUS,
            Self::SpindleOverridePlus1 => RT_SPINDLE_OVR_FINE_PLUS,
            Self::SpindleOverrideMinus1 => RT_SPINDLE_OVR_FINE_MINUS,
            Self::SpindleStop => RT_SPINDLE_OVR_STOP,
            Self::CoolantFloodToggle => RT_COOLANT_FLOOD_TOGGLE,
            Self::CoolantMistToggle => RT_COOLANT_MIST_TOGGLE,
            Self::CompleteStatus => RT_COMPLETE_STATUS,
        }
    }

    /// Parse from a string command name (used in WebSocket protocol)
    pub fn from_str_name(name: &str) -> Option<Self> {
        match name {
            "status_query" => Some(Self::StatusQuery),
            "cycle_start" => Some(Self::CycleStart),
            "feed_hold" => Some(Self::FeedHold),
            "soft_reset" => Some(Self::SoftReset),
            "safety_door" => Some(Self::SafetyDoor),
            "jog_cancel" => Some(Self::JogCancel),
            "feed_ovr_reset" => Some(Self::FeedOverrideReset),
            "feed_ovr_plus10" => Some(Self::FeedOverridePlus10),
            "feed_ovr_minus10" => Some(Self::FeedOverrideMinus10),
            "feed_ovr_plus1" => Some(Self::FeedOverridePlus1),
            "feed_ovr_minus1" => Some(Self::FeedOverrideMinus1),
            "rapid_ovr_reset" => Some(Self::RapidOverrideReset),
            "rapid_ovr_50" => Some(Self::RapidOverride50),
            "rapid_ovr_25" => Some(Self::RapidOverride25),
            "spindle_ovr_reset" => Some(Self::SpindleOverrideReset),
            "spindle_ovr_plus10" => Some(Self::SpindleOverridePlus10),
            "spindle_ovr_minus10" => Some(Self::SpindleOverrideMinus10),
            "spindle_ovr_plus1" => Some(Self::SpindleOverridePlus1),
            "spindle_ovr_minus1" => Some(Self::SpindleOverrideMinus1),
            "spindle_stop" => Some(Self::SpindleStop),
            "coolant_flood_toggle" => Some(Self::CoolantFloodToggle),
            "coolant_mist_toggle" => Some(Self::CoolantMistToggle),
            "complete_status" => Some(Self::CompleteStatus),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rt_command_roundtrip() {
        let cmd = RealtimeCommand::FeedHold;
        assert_eq!(cmd.to_byte(), RT_FEED_HOLD);
        assert_eq!(
            RealtimeCommand::from_str_name("feed_hold"),
            Some(RealtimeCommand::FeedHold)
        );
    }

    #[test]
    fn test_unknown_command() {
        assert_eq!(RealtimeCommand::from_str_name("bogus"), None);
    }
}

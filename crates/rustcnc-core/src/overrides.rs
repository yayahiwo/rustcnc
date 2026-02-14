use serde::{Deserialize, Serialize};

/// Override percentages (100 = nominal)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Overrides {
    /// Feed rate override: 10-200%
    pub feed: u8,
    /// Rapid override: 25, 50, or 100%
    pub rapids: u8,
    /// Spindle speed override: 10-200%
    pub spindle: u8,
}

impl Default for Overrides {
    fn default() -> Self {
        Self {
            feed: 100,
            rapids: 100,
            spindle: 100,
        }
    }
}

impl Overrides {
    /// Clamp feed override to valid range (10-200%)
    pub fn clamp_feed(value: u8) -> u8 {
        value.clamp(10, 200)
    }

    /// Validate rapid override (must be 25, 50, or 100)
    pub fn valid_rapid(value: u8) -> u8 {
        match value {
            0..=37 => 25,
            38..=74 => 50,
            _ => 100,
        }
    }

    /// Clamp spindle override to valid range (10-200%)
    pub fn clamp_spindle(value: u8) -> u8 {
        value.clamp(10, 200)
    }
}

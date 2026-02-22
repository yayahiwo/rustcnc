use serde::{Deserialize, Serialize};

/// Unit system for coordinates and distances
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UnitSystem {
    #[default]
    Metric, // G21 - millimeters
    Imperial, // G20 - inches
}

/// Distance mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DistanceMode {
    #[default]
    Absolute, // G90
    Incremental, // G91
}

/// Convert inches to millimeters
pub fn inches_to_mm(inches: f64) -> f64 {
    inches * 25.4
}

/// Convert millimeters to inches
pub fn mm_to_inches(mm: f64) -> f64 {
    mm / 25.4
}

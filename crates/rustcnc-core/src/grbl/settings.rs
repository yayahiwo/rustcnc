use serde::{Deserialize, Serialize};

/// A single GRBL setting ($N=value)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrblSetting {
    pub id: u16,
    pub value: String,
    pub description: &'static str,
    pub unit: &'static str,
}

/// Well-known GRBL settings
pub const SETTING_DESCRIPTIONS: &[(u16, &str, &str)] = &[
    (0, "Step pulse time", "microseconds"),
    (1, "Step idle delay", "milliseconds"),
    (2, "Step port invert mask", "mask"),
    (3, "Direction port invert mask", "mask"),
    (4, "Step enable invert", "boolean"),
    (5, "Limit pins invert", "boolean"),
    (6, "Probe pin invert", "boolean"),
    (10, "Status report mask", "mask"),
    (11, "Junction deviation", "mm"),
    (12, "Arc tolerance", "mm"),
    (13, "Report inches", "boolean"),
    (20, "Soft limits enable", "boolean"),
    (21, "Hard limits enable", "boolean"),
    (22, "Homing cycle enable", "boolean"),
    (23, "Homing direction invert mask", "mask"),
    (24, "Homing feed rate", "mm/min"),
    (25, "Homing seek rate", "mm/min"),
    (26, "Homing debounce delay", "milliseconds"),
    (27, "Homing pull-off distance", "mm"),
    (30, "Max spindle speed", "RPM"),
    (31, "Min spindle speed", "RPM"),
    (32, "Laser mode enable", "boolean"),
    (100, "X steps/mm", "steps/mm"),
    (101, "Y steps/mm", "steps/mm"),
    (102, "Z steps/mm", "steps/mm"),
    (110, "X max rate", "mm/min"),
    (111, "Y max rate", "mm/min"),
    (112, "Z max rate", "mm/min"),
    (120, "X acceleration", "mm/sec^2"),
    (121, "Y acceleration", "mm/sec^2"),
    (122, "Z acceleration", "mm/sec^2"),
    (130, "X max travel", "mm"),
    (131, "Y max travel", "mm"),
    (132, "Z max travel", "mm"),
];

/// Look up a setting description by its ID
pub fn setting_description(id: u16) -> (&'static str, &'static str) {
    SETTING_DESCRIPTIONS
        .iter()
        .find(|(sid, _, _)| *sid == id)
        .map(|(_, desc, unit)| (*desc, *unit))
        .unwrap_or(("Unknown setting", ""))
}

/// Parse GRBL settings response ($N=value format)
pub fn parse_setting(line: &str) -> Option<GrblSetting> {
    let line = line.trim();
    if !line.starts_with('$') {
        return None;
    }
    let rest = &line[1..];
    let mut parts = rest.splitn(2, '=');
    let id: u16 = parts.next()?.parse().ok()?;
    let value = parts.next()?.to_string();
    let (description, unit) = setting_description(id);

    Some(GrblSetting {
        id,
        value,
        description,
        unit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_setting() {
        let s = parse_setting("$110=8000.000").unwrap();
        assert_eq!(s.id, 110);
        assert_eq!(s.value, "8000.000");
        assert_eq!(s.description, "X max rate");
        assert_eq!(s.unit, "mm/min");
    }

    #[test]
    fn test_unknown_setting() {
        let s = parse_setting("$999=42").unwrap();
        assert_eq!(s.id, 999);
        assert_eq!(s.description, "Unknown setting");
    }

    #[test]
    fn test_invalid_setting() {
        assert!(parse_setting("not a setting").is_none());
        assert!(parse_setting("$abc=123").is_none());
    }
}

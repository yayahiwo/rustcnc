use serde::{Deserialize, Serialize};

/// A single GRBL setting ($N=value)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrblSetting {
    pub id: u16,
    pub value: String,
    pub description: &'static str,
    pub unit: &'static str,
}

/// Well-known GRBL/grblHAL settings
pub const SETTING_DESCRIPTIONS: &[(u16, &str, &str)] = &[
    // Core GRBL settings
    (0, "Step pulse time", "microseconds"),
    (1, "Step idle delay", "milliseconds"),
    (2, "Step pulse invert", "mask"),
    (3, "Step direction invert", "mask"),
    (4, "Invert step enable pin", "boolean"),
    (5, "Invert limit pins", "boolean"),
    (6, "Invert probe pin", "boolean"),
    (8, "Ganged direction invert", "mask"),
    (9, "Spindle PWM options", "mask"),
    (10, "Status report options", "mask"),
    (11, "Junction deviation", "mm"),
    (12, "Arc tolerance", "mm"),
    (13, "Report in inches", "boolean"),
    (14, "Invert control pins", "mask"),
    (15, "Invert coolant pins", "mask"),
    (16, "Invert spindle signals", "mask"),
    (17, "Pullup disable control pins", "mask"),
    (18, "Pullup disable limit pins", "mask"),
    (19, "Pullup disable probe pin", "boolean"),
    (20, "Soft limits enable", "boolean"),
    (21, "Hard limits enable", "boolean"),
    (22, "Homing cycle enable", "boolean"),
    (23, "Homing direction invert", "mask"),
    (24, "Homing locate feed rate", "mm/min"),
    (25, "Homing search seek rate", "mm/min"),
    (26, "Homing switch debounce delay", "milliseconds"),
    (27, "Homing switch pull-off distance", "mm"),
    (28, "G73 retract distance", "mm"),
    (29, "Step pulse delay", "microseconds"),
    (30, "Max spindle speed", "RPM"),
    (31, "Min spindle speed", "RPM"),
    (32, "Mode of operation", ""),
    (33, "Spindle PWM frequency", "Hz"),
    (34, "Spindle PWM off value", "%"),
    (35, "Spindle PWM min value", "%"),
    (36, "Spindle PWM max value", "%"),
    (37, "Steppers deenergize", "mask"),
    (38, "Spindle PPR", "pulses/rev"),
    (39, "Enable legacy RT commands", "boolean"),
    (40, "Limit jog commands", "boolean"),
    (41, "Parking cycle enable", "mask"),
    (42, "Parking axis", ""),
    (43, "Homing passes", ""),
    (44, "Axes homing, first pass", "mask"),
    (45, "Axes homing, second pass", "mask"),
    (46, "Axes homing, third pass", "mask"),
    (47, "Axes homing, fourth pass", "mask"),
    (48, "Axes homing, fifth pass", "mask"),
    (49, "Axes homing, sixth pass", "mask"),
    (50, "Step jog speed", "mm/min"),
    (51, "Slow jog speed", "mm/min"),
    (52, "Fast jog speed", "mm/min"),
    (53, "Step jog distance", "mm"),
    (54, "Slow jog distance", "mm"),
    (55, "Fast jog distance", "mm"),
    (56, "Parking pull-out distance", "mm"),
    (57, "Parking pull-out rate", "mm/min"),
    (58, "Parking target", "mm"),
    (59, "Parking fast rate", "mm/min"),
    (60, "Restore overrides", "boolean"),
    (61, "Ignore door when idle", "boolean"),
    (62, "Sleep enable", "boolean"),
    (63, "Feed hold actions", "mask"),
    (64, "Force init alarm", "boolean"),
    (65, "Probing feed override", "boolean"),
    // Network & communication
    (70, "Network services enable", "mask"),
    (71, "Bluetooth device name", ""),
    (72, "Bluetooth service name", ""),
    (73, "WiFi mode", ""),
    (74, "WiFi station SSID", ""),
    (75, "WiFi station password", ""),
    (76, "WiFi AP SSID", ""),
    (77, "WiFi AP password", ""),
    // Spindle PID
    (80, "Spindle P-gain", ""),
    (81, "Spindle I-gain", ""),
    (82, "Spindle D-gain", ""),
    (84, "Spindle PID max error", ""),
    (85, "Spindle PID max I error", ""),
    (90, "Spindle sync P-gain", ""),
    (91, "Spindle sync I-gain", ""),
    (92, "Spindle sync D-gain", ""),
    (95, "Spindle sync PID max I error", ""),
    // Axis: steps/mm
    (100, "X-axis travel resolution", "steps/mm"),
    (101, "Y-axis travel resolution", "steps/mm"),
    (102, "Z-axis travel resolution", "steps/mm"),
    (103, "A-axis travel resolution", "steps/deg"),
    (104, "B-axis travel resolution", "steps/deg"),
    (105, "C-axis travel resolution", "steps/deg"),
    (106, "U-axis travel resolution", "steps/mm"),
    (107, "V-axis travel resolution", "steps/mm"),
    // Axis: max rate
    (110, "X-axis maximum rate", "mm/min"),
    (111, "Y-axis maximum rate", "mm/min"),
    (112, "Z-axis maximum rate", "mm/min"),
    (113, "A-axis maximum rate", "deg/min"),
    (114, "B-axis maximum rate", "deg/min"),
    (115, "C-axis maximum rate", "deg/min"),
    (116, "U-axis maximum rate", "mm/min"),
    (117, "V-axis maximum rate", "mm/min"),
    // Axis: acceleration
    (120, "X-axis acceleration", "mm/sec^2"),
    (121, "Y-axis acceleration", "mm/sec^2"),
    (122, "Z-axis acceleration", "mm/sec^2"),
    (123, "A-axis acceleration", "deg/sec^2"),
    (124, "B-axis acceleration", "deg/sec^2"),
    (125, "C-axis acceleration", "deg/sec^2"),
    (126, "U-axis acceleration", "mm/sec^2"),
    (127, "V-axis acceleration", "mm/sec^2"),
    // Axis: max travel
    (130, "X-axis maximum travel", "mm"),
    (131, "Y-axis maximum travel", "mm"),
    (132, "Z-axis maximum travel", "mm"),
    (133, "A-axis maximum travel", "deg"),
    (134, "B-axis maximum travel", "deg"),
    (135, "C-axis maximum travel", "deg"),
    (136, "U-axis maximum travel", "mm"),
    (137, "V-axis maximum travel", "mm"),
    // Axis: motor current
    (140, "X-axis motor current", "mA"),
    (141, "Y-axis motor current", "mA"),
    (142, "Z-axis motor current", "mA"),
    (143, "A-axis motor current", "mA"),
    (144, "B-axis motor current", "mA"),
    (145, "C-axis motor current", "mA"),
    // Axis: microsteps
    (150, "X-axis microsteps", ""),
    (151, "Y-axis microsteps", ""),
    (152, "Z-axis microsteps", ""),
    (153, "A-axis microsteps", ""),
    (154, "B-axis microsteps", ""),
    (155, "C-axis microsteps", ""),
    // Axis: backlash compensation
    (160, "X-axis backlash compensation", "mm"),
    (161, "Y-axis backlash compensation", "mm"),
    (162, "Z-axis backlash compensation", "mm"),
    (163, "A-axis backlash compensation", "deg"),
    (164, "B-axis backlash compensation", "deg"),
    (165, "C-axis backlash compensation", "deg"),
    // Network interface
    (300, "Hostname", ""),
    (301, "IP mode", ""),
    (302, "IP address", ""),
    (303, "Gateway", ""),
    (304, "Netmask", ""),
    (305, "Telnet port", ""),
    (306, "HTTP port", ""),
    (307, "Websocket port", ""),
    // Trinamic & tools
    (338, "Trinamic driver enable", "mask"),
    (339, "Sensorless homing enable", "mask"),
    (340, "Spindle at speed tolerance", "%"),
    (341, "Tool change mode", ""),
    (342, "Tool change probing distance", "mm"),
    (343, "Tool change locate feed rate", "mm/min"),
    (344, "Tool change search seek rate", "mm/min"),
    (345, "Tool change probe pull-off rate", "mm/min"),
    // Plasma THC
    (350, "Plasma mode", ""),
    (351, "Plasma THC delay", "seconds"),
    (352, "Plasma THC threshold", "V"),
    (353, "Plasma THC P-gain", ""),
    (354, "Plasma THC I-gain", ""),
    (355, "Plasma THC D-gain", ""),
    (356, "Plasma THC VAD threshold", "%"),
    (357, "Plasma THC void override", "%"),
    (358, "Plasma arc fail timeout", "seconds"),
    (359, "Plasma arc retry delay", "seconds"),
    (360, "Plasma arc max retries", ""),
    (361, "Plasma arc voltage scale", ""),
    (362, "Plasma arc voltage offset", "V"),
    (363, "Plasma arc height per volt", "mm/V"),
    (364, "Plasma arc ok high volts", "V"),
    (365, "Plasma arc ok low volts", "V"),
    // I/O ports
    (370, "Invert I/O port inputs", "mask"),
    (371, "I/O port inputs pullup disable", "mask"),
    (372, "Invert I/O port outputs", "mask"),
    (373, "I/O port outputs open drain", "mask"),
    (374, "ModBus baud rate", "baud"),
    (375, "ModBus RX timeout", "milliseconds"),
    (376, "Rotary axes", "mask"),
    (384, "Disable G92 persistence", "boolean"),
    (392, "Door spindle on delay", "seconds"),
    (393, "Door coolant on delay", "seconds"),
    (394, "Spindle on delay", "seconds"),
    (395, "Spindle type", ""),
    (398, "Planner buffer blocks", ""),
    // Encoder
    (400, "Encoder mode", ""),
    (401, "Encoder CPR", "pulses/rev"),
    (402, "Encoder CPD", "pulses/det"),
    (403, "Encoder dbl-click window", "milliseconds"),
    // Extended
    (481, "Auto status report interval", "milliseconds"),
    (484, "Unlock after E-Stop", "boolean"),
    (485, "Tool number persistence", "boolean"),
    (538, "Rotary axis wrap", "mask"),
    (539, "Spindle off delay", "seconds"),
    (650, "Filing system options", "mask"),
    (671, "Invert home switch pins", "mask"),
    (673, "Coolant activation delay", "seconds"),
    (676, "Reset action options", "mask"),
    (680, "Stepper enable delay", "milliseconds"),
    (681, "ModBus stream format", ""),
    // Secondary spindle
    (716, "Spindle 2 invert mask", "mask"),
    (730, "Spindle 2 RPM max", "RPM"),
    (731, "Spindle 2 RPM min", "RPM"),
    (732, "Spindle 2 PWM frequency", "Hz"),
    (733, "Spindle 2 PWM off value", "%"),
    (734, "Spindle 2 PWM min value", "%"),
    (735, "Spindle 2 PWM max value", "%"),
    (736, "Spindle 2 PPR", "pulses/rev"),
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
        assert_eq!(s.description, "X-axis maximum rate");
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

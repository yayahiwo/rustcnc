use crate::machine::*;
use crate::overrides::Overrides;

/// Parse a GRBL status report string.
///
/// Format: `<State|Field1:value1|Field2:value2|...>`
///
/// Examples:
/// - `<Idle|MPos:0.000,0.000,0.000|FS:0,0>`
/// - `<Run|MPos:10.500,20.300,-5.000|FS:1500,12000|Ov:100,100,100|Bf:15,120>`
/// - `<Hold:0|WPos:0.000,0.000,0.000|FS:0,0|Pn:XYZ>`
pub fn parse_status_report(raw: &str) -> Option<StatusReport> {
    let raw = raw.trim();

    // Must be wrapped in < >
    if !raw.starts_with('<') || !raw.ends_with('>') {
        return None;
    }

    let inner = &raw[1..raw.len() - 1];
    let mut parts = inner.split('|');

    // First part is always the machine state
    let state_str = parts.next()?;
    let state = parse_machine_state(state_str)?;

    let mut report = StatusReport {
        state,
        machine_position: None,
        work_position: None,
        work_coordinate_offset: None,
        buffer: None,
        line_number: None,
        feed_rate: None,
        spindle_speed: None,
        input_pins: None,
        overrides: None,
        accessories: None,
    };

    // Parse remaining fields
    for field in parts {
        if let Some(value) = field.strip_prefix("MPos:") {
            report.machine_position = parse_position(value);
        } else if let Some(value) = field.strip_prefix("WPos:") {
            report.work_position = parse_position(value);
        } else if let Some(value) = field.strip_prefix("WCO:") {
            report.work_coordinate_offset = parse_position(value);
        } else if let Some(value) = field.strip_prefix("Bf:") {
            report.buffer = parse_buffer_state(value);
        } else if let Some(value) = field.strip_prefix("Ln:") {
            report.line_number = value.parse().ok();
        } else if let Some(value) = field.strip_prefix("FS:") {
            let parts: Vec<&str> = value.split(',').collect();
            if parts.len() >= 1 {
                report.feed_rate = parts[0].parse().ok();
            }
            if parts.len() >= 2 {
                report.spindle_speed = parts[1].parse().ok();
            }
        } else if let Some(value) = field.strip_prefix("F:") {
            report.feed_rate = value.parse().ok();
        } else if let Some(value) = field.strip_prefix("Pn:") {
            report.input_pins = Some(parse_input_pins(value));
        } else if let Some(value) = field.strip_prefix("Ov:") {
            report.overrides = parse_overrides(value);
        } else if let Some(value) = field.strip_prefix("A:") {
            report.accessories = Some(parse_accessories(value));
        }
        // Ignore unknown fields (per grblHAL spec: forward-compatible)
    }

    Some(report)
}

/// Parse machine state from string.
/// Handles substates: "Hold:0", "Door:1", "Alarm:3"
fn parse_machine_state(s: &str) -> Option<MachineState> {
    if let Some(sub) = s.strip_prefix("Hold:") {
        let code = sub.parse().unwrap_or(0);
        return Some(MachineState::Hold(code));
    }
    if let Some(sub) = s.strip_prefix("Door:") {
        let code = sub.parse().unwrap_or(0);
        return Some(MachineState::Door(code));
    }
    if let Some(sub) = s.strip_prefix("Alarm:") {
        let code = sub.parse().unwrap_or(0);
        return Some(MachineState::Alarm(code));
    }

    match s {
        "Idle" => Some(MachineState::Idle),
        "Run" => Some(MachineState::Run),
        "Hold" => Some(MachineState::Hold(0)),
        "Jog" => Some(MachineState::Jog),
        "Alarm" => Some(MachineState::Alarm(0)),
        "Door" => Some(MachineState::Door(0)),
        "Check" => Some(MachineState::Check),
        "Home" => Some(MachineState::Home),
        "Sleep" => Some(MachineState::Sleep),
        "Tool" => Some(MachineState::Tool),
        _ => None,
    }
}

/// Parse position from comma-separated values: "x,y,z" or "x,y,z,a,b,c"
fn parse_position(s: &str) -> Option<Position> {
    let parts: Vec<f64> = match s
        .split(',')
        .map(|p| p.parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(_) => return None,
    };

    if parts.len() < 3 {
        return None;
    }

    Some(Position {
        x: parts[0],
        y: parts[1],
        z: parts[2],
        a: parts.get(3).copied(),
        b: parts.get(4).copied(),
        c: parts.get(5).copied(),
        u: parts.get(6).copied(),
        v: parts.get(7).copied(),
    })
}

/// Parse buffer state from "planner_blocks,rx_bytes"
fn parse_buffer_state(s: &str) -> Option<BufferState> {
    let mut parts = s.split(',');
    let planner: u16 = parts.next()?.parse().ok()?;
    let rx: u16 = parts.next()?.parse().ok()?;
    Some(BufferState {
        planner_blocks_available: planner,
        rx_bytes_available: rx,
    })
}

/// Parse input pins from flag characters: "XYZPDHRS"
fn parse_input_pins(s: &str) -> InputPins {
    InputPins {
        limit_x: s.contains('X'),
        limit_y: s.contains('Y'),
        limit_z: s.contains('Z'),
        limit_a: s.contains('A'),
        limit_b: s.contains('B'),
        limit_c: s.contains('C'),
        limit_u: s.contains('U'),
        limit_v: s.contains('V'),
        probe: s.contains('P'),
        door: s.contains('D'),
        hold: s.contains('H'),
        soft_reset: s.contains('R'),
        cycle_start: s.contains('S'),
        estop: s.contains('E'),
    }
}

/// Parse overrides from "feed,rapid,spindle"
fn parse_overrides(s: &str) -> Option<Overrides> {
    let parts: Vec<u8> = s.split(',').filter_map(|p| p.parse().ok()).collect();
    if parts.len() < 3 {
        return None;
    }
    Some(Overrides {
        feed: parts[0],
        rapids: parts[1],
        spindle: parts[2],
    })
}

/// Parse accessory state from flag characters
fn parse_accessories(s: &str) -> AccessoryState {
    AccessoryState {
        spindle_cw: s.contains('S'),
        spindle_ccw: s.contains('C'),
        flood_coolant: s.contains('F'),
        mist_coolant: s.contains('M'),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_idle_status() {
        let report = parse_status_report("<Idle|MPos:0.000,0.000,0.000|FS:0,0>").unwrap();
        assert_eq!(report.state, MachineState::Idle);
        let pos = report.machine_position.unwrap();
        assert_eq!(pos.x, 0.0);
        assert_eq!(pos.y, 0.0);
        assert_eq!(pos.z, 0.0);
        assert_eq!(report.feed_rate, Some(0.0));
        assert_eq!(report.spindle_speed, Some(0.0));
    }

    #[test]
    fn test_parse_run_status() {
        let report =
            parse_status_report("<Run|MPos:10.500,20.300,-5.000|FS:1500,12000|Ov:100,100,100>")
                .unwrap();
        assert_eq!(report.state, MachineState::Run);
        let pos = report.machine_position.unwrap();
        assert_eq!(pos.x, 10.5);
        assert_eq!(pos.y, 20.3);
        assert_eq!(pos.z, -5.0);
        assert_eq!(report.feed_rate, Some(1500.0));
        assert_eq!(report.spindle_speed, Some(12000.0));
        let ovr = report.overrides.unwrap();
        assert_eq!(ovr.feed, 100);
        assert_eq!(ovr.rapids, 100);
        assert_eq!(ovr.spindle, 100);
    }

    #[test]
    fn test_parse_hold_with_substate() {
        let report = parse_status_report("<Hold:0|WPos:5.000,10.000,0.000|FS:0,0>").unwrap();
        assert_eq!(report.state, MachineState::Hold(0));
        assert!(report.work_position.is_some());
    }

    #[test]
    fn test_parse_alarm_with_code() {
        let report = parse_status_report("<Alarm:3|MPos:0.000,0.000,0.000|FS:0,0>").unwrap();
        assert_eq!(report.state, MachineState::Alarm(3));
    }

    #[test]
    fn test_parse_with_buffer() {
        let report =
            parse_status_report("<Idle|MPos:0.000,0.000,0.000|Bf:15,120|FS:0,0>").unwrap();
        let buf = report.buffer.unwrap();
        assert_eq!(buf.planner_blocks_available, 15);
        assert_eq!(buf.rx_bytes_available, 120);
    }

    #[test]
    fn test_parse_input_pins() {
        let report =
            parse_status_report("<Idle|MPos:0.000,0.000,0.000|FS:0,0|Pn:XZP>").unwrap();
        let pins = report.input_pins.unwrap();
        assert!(pins.limit_x);
        assert!(!pins.limit_y);
        assert!(pins.limit_z);
        assert!(pins.probe);
        assert!(!pins.door);
    }

    #[test]
    fn test_parse_accessories() {
        let report =
            parse_status_report("<Run|MPos:0.000,0.000,0.000|FS:1000,12000|A:SF>").unwrap();
        let acc = report.accessories.unwrap();
        assert!(acc.spindle_cw);
        assert!(acc.flood_coolant);
        assert!(!acc.mist_coolant);
    }

    #[test]
    fn test_parse_wco() {
        let report = parse_status_report(
            "<Idle|MPos:10.000,20.000,0.000|FS:0,0|WCO:100.000,200.000,50.000>",
        )
        .unwrap();
        let wco = report.work_coordinate_offset.unwrap();
        assert_eq!(wco.x, 100.0);
        assert_eq!(wco.y, 200.0);
        assert_eq!(wco.z, 50.0);
    }

    #[test]
    fn test_parse_line_number() {
        let report =
            parse_status_report("<Run|MPos:0.000,0.000,0.000|FS:1000,0|Ln:1542>").unwrap();
        assert_eq!(report.line_number, Some(1542));
    }

    #[test]
    fn test_parse_with_extra_axes() {
        let report =
            parse_status_report("<Idle|MPos:1.0,2.0,3.0,4.0,5.0,6.0|FS:0,0>").unwrap();
        let pos = report.machine_position.unwrap();
        assert_eq!(pos.a, Some(4.0));
        assert_eq!(pos.b, Some(5.0));
        assert_eq!(pos.c, Some(6.0));
    }

    #[test]
    fn test_parse_with_8_axes() {
        let report =
            parse_status_report("<Idle|MPos:1.0,2.0,3.0,4.0,5.0,6.0,7.0,8.0|FS:0,0>").unwrap();
        let pos = report.machine_position.unwrap();
        assert_eq!(pos.x, 1.0);
        assert_eq!(pos.y, 2.0);
        assert_eq!(pos.z, 3.0);
        assert_eq!(pos.a, Some(4.0));
        assert_eq!(pos.b, Some(5.0));
        assert_eq!(pos.c, Some(6.0));
        assert_eq!(pos.u, Some(7.0));
        assert_eq!(pos.v, Some(8.0));
    }

    #[test]
    fn test_invalid_status() {
        assert!(parse_status_report("not a status").is_none());
        assert!(parse_status_report("<>").is_none());
        assert!(parse_status_report("<|MPos:0,0,0>").is_none());
    }

    #[test]
    fn test_unknown_fields_ignored() {
        // grblHAL may add new fields -- we must not fail on unknown ones
        let report = parse_status_report(
            "<Idle|MPos:0.000,0.000,0.000|FS:0,0|FW:grblHAL|SD:50.0,test.gcode>",
        )
        .unwrap();
        assert_eq!(report.state, MachineState::Idle);
    }
}

/// Minimal G-code interpreter for the simulator.
/// Only needs to understand enough to move the virtual machine.

/// Parsed command from a G-code line
#[derive(Debug, Clone)]
pub enum SimCommand {
    /// G0 rapid move
    RapidMove {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
    /// G1 linear move
    LinearMove {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        f: Option<f64>,
    },
    /// G28 home
    Home,
    /// G90 absolute mode
    AbsoluteMode,
    /// G91 incremental mode
    IncrementalMode,
    /// G20 inches
    InchMode,
    /// G21 mm
    MmMode,
    /// M3 spindle CW
    SpindleCW { speed: Option<f64> },
    /// M4 spindle CCW
    SpindleCCW { speed: Option<f64> },
    /// M5 spindle off
    SpindleOff,
    /// M8 flood coolant on
    FloodCoolantOn,
    /// M9 coolant off
    CoolantOff,
    /// M2/M30 program end
    ProgramEnd,
    /// $H homing cycle
    HomingCycle,
    /// $X unlock
    Unlock,
    /// $$ request settings
    RequestSettings,
    /// G10 L20 P1 set work offset
    SetWorkOffset {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
    /// Jog command ($J=...)
    Jog {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        f: f64,
        incremental: bool,
    },
    /// Unknown/unsupported command (still returns ok)
    Unknown(String),
}

/// Parse a single G-code line into a SimCommand
pub fn parse_sim_command(line: &str) -> SimCommand {
    let line = line.trim().to_uppercase();

    if line.is_empty() {
        return SimCommand::Unknown(String::new());
    }

    // System commands
    if line == "$H" {
        return SimCommand::HomingCycle;
    }
    if line == "$X" {
        return SimCommand::Unlock;
    }
    if line == "$$" {
        return SimCommand::RequestSettings;
    }

    // Jog command
    if line.starts_with("$J=") {
        return parse_jog_command(&line[3..]);
    }

    // G-code commands
    if line.starts_with("G90") {
        return SimCommand::AbsoluteMode;
    }
    if line.starts_with("G91") {
        return SimCommand::IncrementalMode;
    }
    if line.starts_with("G20") {
        return SimCommand::InchMode;
    }
    if line.starts_with("G21") {
        return SimCommand::MmMode;
    }
    if line.starts_with("G28") {
        return SimCommand::Home;
    }

    // Motion commands
    if line.starts_with("G0") || line.starts_with("G00") {
        let (x, y, z, _) = extract_coords(&line);
        return SimCommand::RapidMove { x, y, z };
    }
    if line.starts_with("G1") || line.starts_with("G01") {
        let (x, y, z, f) = extract_coords(&line);
        return SimCommand::LinearMove { x, y, z, f };
    }

    // M-codes
    if line.starts_with("M3") || line.starts_with("M03") {
        let s = extract_word(&line, 'S');
        return SimCommand::SpindleCW { speed: s };
    }
    if line.starts_with("M4") || line.starts_with("M04") {
        let s = extract_word(&line, 'S');
        return SimCommand::SpindleCCW { speed: s };
    }
    if line.starts_with("M5") || line.starts_with("M05") {
        return SimCommand::SpindleOff;
    }
    if line.starts_with("M8") || line.starts_with("M08") {
        return SimCommand::FloodCoolantOn;
    }
    if line.starts_with("M9") || line.starts_with("M09") {
        return SimCommand::CoolantOff;
    }
    if line.starts_with("M2") || line.starts_with("M02") || line.starts_with("M30") {
        return SimCommand::ProgramEnd;
    }

    SimCommand::Unknown(line)
}

/// Extract X, Y, Z, F coordinate words from a G-code line
fn extract_coords(line: &str) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    (
        extract_word(line, 'X'),
        extract_word(line, 'Y'),
        extract_word(line, 'Z'),
        extract_word(line, 'F'),
    )
}

/// Extract a single word value (e.g., 'X' from "G0 X10.5 Y20")
fn extract_word(line: &str, letter: char) -> Option<f64> {
    let upper = letter.to_uppercase().next()?;
    let pos = line.find(upper)?;
    let rest = &line[pos + 1..];
    // Take characters until we hit a non-numeric character (except -, .)
    let num_str: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    num_str.parse().ok()
}

/// Parse a jog command: "$J=G91X10Y0F1000"
fn parse_jog_command(s: &str) -> SimCommand {
    let incremental = s.contains("G91");
    let (x, y, z, f) = extract_coords(s);
    SimCommand::Jog {
        x,
        y,
        z,
        f: f.unwrap_or(100.0),
        incremental,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rapid() {
        match parse_sim_command("G0 X10 Y20") {
            SimCommand::RapidMove { x, y, z } => {
                assert_eq!(x, Some(10.0));
                assert_eq!(y, Some(20.0));
                assert_eq!(z, None);
            }
            other => panic!("Expected RapidMove, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_linear() {
        match parse_sim_command("G1 X10 Y20 F1000") {
            SimCommand::LinearMove { x, y, z, f } => {
                assert_eq!(x, Some(10.0));
                assert_eq!(y, Some(20.0));
                assert_eq!(z, None);
                assert_eq!(f, Some(1000.0));
            }
            other => panic!("Expected LinearMove, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_spindle() {
        match parse_sim_command("M3 S12000") {
            SimCommand::SpindleCW { speed } => {
                assert_eq!(speed, Some(12000.0));
            }
            other => panic!("Expected SpindleCW, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_jog() {
        match parse_sim_command("$J=G91X10F1000") {
            SimCommand::Jog {
                x,
                y,
                z,
                f,
                incremental,
            } => {
                assert_eq!(x, Some(10.0));
                assert_eq!(y, None);
                assert_eq!(z, None);
                assert_eq!(f, 1000.0);
                assert!(incremental);
            }
            other => panic!("Expected Jog, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_system_commands() {
        assert!(matches!(parse_sim_command("$H"), SimCommand::HomingCycle));
        assert!(matches!(parse_sim_command("$X"), SimCommand::Unlock));
        assert!(matches!(
            parse_sim_command("$$"),
            SimCommand::RequestSettings
        ));
    }

    #[test]
    fn test_parse_negative_coords() {
        match parse_sim_command("G0 X-10.5 Z-2.0") {
            SimCommand::RapidMove { x, y, z } => {
                assert_eq!(x, Some(-10.5));
                assert_eq!(y, None);
                assert_eq!(z, Some(-2.0));
            }
            other => panic!("Expected RapidMove, got {:?}", other),
        }
    }
}

use serde::{Deserialize, Serialize};

/// Arc parameters for G2/G3 visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcData {
    pub i: f64,
    pub j: f64,
    pub k: f64,
    /// Active G-code plane: 17 (XY), 18 (ZX), 19 (YZ)
    pub plane: u8,
}

/// A parsed G-code line with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCodeLine {
    /// Original line number in file (1-based)
    pub file_line: usize,
    /// The raw text (stripped of comments, uppercased, trimmed)
    pub text: String,
    /// Byte length including newline (for character-counting protocol)
    pub byte_len: usize,
    /// Parsed move type for visualization
    pub move_type: Option<MoveType>,
    /// Endpoint position if this is a motion command (up to 8 axes: X,Y,Z,A,B,C,U,V)
    pub endpoint: Option<Vec<f64>>,
    /// Arc center offsets and plane for G2/G3
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc: Option<ArcData>,
}

impl GCodeLine {
    /// Create a new GCodeLine from raw text. Strips comments and trims.
    pub fn new(file_line: usize, raw: &str) -> Self {
        let text = strip_comments(raw).trim().to_uppercase();
        let byte_len = text.len() + 2; // +2 for \r\n (CRLF)
        let move_type = detect_move_type(&text);
        Self {
            file_line,
            text,
            byte_len,
            move_type,
            endpoint: None,
            arc: None,
        }
    }

    /// Returns true if the line is empty or comment-only
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Extract the numeric value for a given word letter (e.g., 'X', 'Y', 'F')
pub fn extract_word(text: &str, letter: char) -> Option<f64> {
    let upper = letter.to_ascii_uppercase();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.to_ascii_uppercase() == upper {
            let num_str: String = chars
                .by_ref()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                .collect();
            if let Ok(v) = num_str.parse::<f64>() {
                return Some(v);
            }
        }
    }
    None
}

const AXIS_LETTERS: [char; 8] = ['X', 'Y', 'Z', 'A', 'B', 'C', 'U', 'V'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DistanceMode {
    Absolute,
    Incremental,
}

/// Detect distance mode changes (G90/G91) in a line.
/// Ignores arc distance mode commands (G90.1/G91.1).
fn detect_distance_mode_change(text: &str) -> Option<DistanceMode> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut mode: Option<DistanceMode> = None;

    while i < len {
        if bytes[i] == b'G' {
            let num_start = i + 1;
            let mut j = num_start;
            while j < len && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > num_start {
                // If this is a decimal G-code like G91.1, ignore it for distance mode.
                if j < len && bytes[j] == b'.' {
                    let mut k = j + 1;
                    while k < len && bytes[k].is_ascii_digit() {
                        k += 1;
                    }
                    i = k.max(i + 1);
                    continue;
                }

                match &text[num_start..j] {
                    "90" => mode = Some(DistanceMode::Absolute),
                    "91" => mode = Some(DistanceMode::Incremental),
                    _ => {}
                }
                i = j.max(i + 1);
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    mode
}

/// Extract axis coordinates from a G-code line, updating modal position.
/// Returns the endpoint as a Vec (only X/Y/Z minimum, extended if higher axes present).
fn extract_endpoint(
    text: &str,
    modal_pos: &mut Vec<f64>,
    distance_mode: DistanceMode,
) -> Option<Vec<f64>> {
    let mut any_axis = false;
    for (i, &letter) in AXIS_LETTERS.iter().enumerate() {
        if let Some(val) = extract_word(text, letter) {
            // Grow modal_pos if needed
            while modal_pos.len() <= i {
                modal_pos.push(0.0);
            }
            match distance_mode {
                DistanceMode::Absolute => modal_pos[i] = val,
                DistanceMode::Incremental => modal_pos[i] += val,
            }
            any_axis = true;
        }
    }
    if any_axis {
        Some(modal_pos.clone())
    } else {
        None
    }
}

/// Check if a line has axis words (X/Y/Z/A/B/C/U/V) that indicate motion
fn has_axis_words(text: &str) -> bool {
    for &letter in &AXIS_LETTERS {
        if extract_word(text, letter).is_some() {
            return true;
        }
    }
    false
}

/// Detect plane change commands (G17/G18/G19) anywhere in the line
fn detect_plane_change(text: &str) -> Option<u8> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'G' {
            let num_start = i + 1;
            let mut j = num_start;
            while j < len && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > num_start {
                match &text[num_start..j] {
                    "17" => return Some(17),
                    "18" => return Some(18),
                    "19" => return Some(19),
                    _ => {}
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

/// Strip inline and block comments from a G-code line.
/// Removes everything between (...) and everything after ;
fn strip_comments(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut in_paren = false;

    for ch in line.chars() {
        match ch {
            '(' => in_paren = true,
            ')' => in_paren = false,
            ';' if !in_paren => break,
            _ if !in_paren => result.push(ch),
            _ => {}
        }
    }

    result
}

/// Type of move for toolpath visualization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveType {
    Rapid,       // G0
    Linear,      // G1
    ArcCW,       // G2
    ArcCCW,      // G3
    Probe,       // G38.x
    Dwell,       // G4
    Home,        // G28/G30
    CoordSystem, // G54-G59
    ToolChange,  // M6
    SpindleOn,   // M3/M4
    SpindleOff,  // M5
    CoolantOn,   // M7/M8
    CoolantOff,  // M9
    ProgramEnd,  // M2/M30
}

/// Detect the primary move type by scanning the entire line for G/M codes.
/// Motion commands (G0-G3, G38) take priority over non-motion commands.
/// Handles multi-command lines like "G17 G3 X0 Y-31.587 I-0.318 J0".
fn detect_move_type(text: &str) -> Option<MoveType> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let mut motion: Option<MoveType> = None;
    let mut non_motion: Option<MoveType> = None;

    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let is_g = bytes[i] == b'G';
        let is_m = bytes[i] == b'M';

        if is_g || is_m {
            let num_start = i + 1;
            let mut j = num_start;
            while j < len && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > num_start {
                let num = &text[num_start..j];
                if is_g {
                    match num {
                        "0" | "00" => {
                            motion = Some(MoveType::Rapid);
                        }
                        "1" | "01" => {
                            motion = Some(MoveType::Linear);
                        }
                        "2" | "02" => {
                            motion = Some(MoveType::ArcCW);
                        }
                        "3" | "03" => {
                            motion = Some(MoveType::ArcCCW);
                        }
                        "38" => {
                            motion = Some(MoveType::Probe);
                        }
                        "4" | "04" => {
                            non_motion = non_motion.or(Some(MoveType::Dwell));
                        }
                        "28" | "30" => {
                            non_motion = non_motion.or(Some(MoveType::Home));
                        }
                        "54" | "55" | "56" | "57" | "58" | "59" => {
                            non_motion = non_motion.or(Some(MoveType::CoordSystem));
                        }
                        _ => {}
                    }
                } else {
                    // M codes
                    match num {
                        "2" | "02" | "30" => {
                            non_motion = non_motion.or(Some(MoveType::ProgramEnd));
                        }
                        "3" | "03" | "4" | "04" => {
                            non_motion = non_motion.or(Some(MoveType::SpindleOn));
                        }
                        "5" | "05" => {
                            non_motion = non_motion.or(Some(MoveType::SpindleOff));
                        }
                        "6" | "06" => {
                            non_motion = non_motion.or(Some(MoveType::ToolChange));
                        }
                        "7" | "07" | "8" | "08" => {
                            non_motion = non_motion.or(Some(MoveType::CoolantOn));
                        }
                        "9" | "09" => {
                            non_motion = non_motion.or(Some(MoveType::CoolantOff));
                        }
                        _ => {}
                    }
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }

    // Motion commands take priority over non-motion
    motion.or(non_motion)
}

/// Extract a modal preamble from lines before `up_to_idx` so a job starting
/// mid-file begins with the correct machine state (distance mode, units, feed rate, etc.).
/// Only emits commands that differ from GRBL defaults (G90, G21, G17, G54, spindle off, coolant off).
pub fn extract_modal_preamble(lines: &[GCodeLine], up_to_idx: usize) -> Vec<String> {
    let mut distance_mode: Option<&str> = None; // G90/G91
    let mut units: Option<&str> = None; // G20/G21
    let mut plane: Option<&str> = None; // G17/G18/G19
    let mut coord_system: Option<&str> = None; // G54-G59
    let mut feed_rate: Option<f64> = None; // F word
    let mut spindle: Option<&str> = None; // M3/M4/M5
    let mut spindle_speed: Option<f64> = None; // S word
    let mut coolant: Option<&str> = None; // M7/M8/M9

    let limit = up_to_idx.min(lines.len());
    for line in &lines[..limit] {
        let text = &line.text;
        // Scan for G codes
        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        while i < len {
            let is_g = bytes[i] == b'G';
            let is_m = bytes[i] == b'M';
            if is_g || is_m {
                let num_start = i + 1;
                let mut j = num_start;
                while j < len && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > num_start {
                    // Ignore decimal G-codes like G90.1 / G91.1 for distance mode preamble.
                    if is_g && j < len && bytes[j] == b'.' {
                        let mut k = j + 1;
                        while k < len && bytes[k].is_ascii_digit() {
                            k += 1;
                        }
                        i = k.max(i + 1);
                        continue;
                    }
                    let num = &text[num_start..j];
                    if is_g {
                        match num {
                            "90" => distance_mode = Some("G90"),
                            "91" => distance_mode = Some("G91"),
                            "20" => units = Some("G20"),
                            "21" => units = Some("G21"),
                            "17" => plane = Some("G17"),
                            "18" => plane = Some("G18"),
                            "19" => plane = Some("G19"),
                            "54" => coord_system = Some("G54"),
                            "55" => coord_system = Some("G55"),
                            "56" => coord_system = Some("G56"),
                            "57" => coord_system = Some("G57"),
                            "58" => coord_system = Some("G58"),
                            "59" => coord_system = Some("G59"),
                            _ => {}
                        }
                    } else {
                        match num {
                            "3" | "03" => spindle = Some("M3"),
                            "4" | "04" => spindle = Some("M4"),
                            "5" | "05" => spindle = Some("M5"),
                            "7" | "07" => coolant = Some("M7"),
                            "8" | "08" => coolant = Some("M8"),
                            "9" | "09" => coolant = Some("M9"),
                            _ => {}
                        }
                    }
                }
                i = j.max(i + 1);
            } else {
                i += 1;
            }
        }
        // Extract F and S words
        if let Some(f) = extract_word(text, 'F') {
            feed_rate = Some(f);
        }
        if let Some(s) = extract_word(text, 'S') {
            spindle_speed = Some(s);
        }
    }

    let mut preamble = Vec::new();

    // Only emit non-default values
    // Defaults: G90, G21, G17, G54, spindle off (M5), coolant off (M9)
    if let Some(dm) = distance_mode {
        if dm != "G90" {
            preamble.push(dm.to_string());
        }
    }
    if let Some(u) = units {
        if u != "G21" {
            preamble.push(u.to_string());
        }
    }
    if let Some(p) = plane {
        if p != "G17" {
            preamble.push(p.to_string());
        }
    }
    if let Some(cs) = coord_system {
        if cs != "G54" {
            preamble.push(cs.to_string());
        }
    }
    if let Some(f) = feed_rate {
        preamble.push(format!("F{}", f));
    }
    if let Some(sp) = spindle {
        if sp != "M5" {
            let mut cmd = sp.to_string();
            if let Some(s) = spindle_speed {
                cmd.push_str(&format!(" S{}", s));
            }
            preamble.push(cmd);
        }
    }
    if let Some(c) = coolant {
        if c != "M9" {
            preamble.push(c.to_string());
        }
    }

    preamble
}

/// Represents a loaded and parsed G-code file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCodeFile {
    pub id: uuid::Uuid,
    pub name: String,
    pub lines: Vec<GCodeLine>,
    pub total_lines: usize,
    pub estimated_duration_secs: Option<f64>,
    pub bounding_box: Option<BoundingBox>,
    pub loaded_at: chrono::DateTime<chrono::Utc>,
}

impl GCodeFile {
    /// Parse raw G-code text into a GCodeFile
    pub fn parse(name: String, content: &str) -> Self {
        let mut lines: Vec<GCodeLine> = content
            .lines()
            .enumerate()
            .map(|(i, line)| GCodeLine::new(i + 1, line))
            .filter(|l| !l.is_empty())
            .collect();

        let total_lines = lines.len();

        // Track modal state
        let mut modal_motion: Option<MoveType> = None;
        let mut modal_plane: u8 = 17; // Default G17 (XY)
        let mut modal_pos: Vec<f64> = vec![0.0; 3];
        let mut distance_mode = DistanceMode::Absolute; // Default G90
        let mut bb = BoundingBox::new();
        let mut has_motion = false;

        for line in lines.iter_mut() {
            // Update distance mode (G90/G91) before evaluating motion words.
            if let Some(dm) = detect_distance_mode_change(&line.text) {
                distance_mode = dm;
            }

            // Check for plane changes (G17/G18/G19)
            if let Some(plane) = detect_plane_change(&line.text) {
                modal_plane = plane;
            }

            // Determine effective move type (explicit or modal)
            let effective_move = match line.move_type {
                Some(mt) => {
                    // Update modal state for motion commands
                    if matches!(
                        mt,
                        MoveType::Rapid
                            | MoveType::Linear
                            | MoveType::ArcCW
                            | MoveType::ArcCCW
                            | MoveType::Probe
                    ) {
                        modal_motion = Some(mt);
                    }
                    Some(mt)
                }
                None => {
                    // No explicit motion command — apply modal if line has axis words
                    if has_axis_words(&line.text) {
                        if let Some(modal) = modal_motion {
                            line.move_type = Some(modal);
                            Some(modal)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
            };

            // Extract endpoint and arc data for motion commands
            if let Some(mt) = effective_move {
                match mt {
                    MoveType::Rapid
                    | MoveType::Linear
                    | MoveType::ArcCW
                    | MoveType::ArcCCW
                    | MoveType::Probe => {
                        if let Some(endpoint) =
                            extract_endpoint(&line.text, &mut modal_pos, distance_mode)
                        {
                            bb.expand(&endpoint);
                            line.endpoint = Some(endpoint);
                            has_motion = true;
                        }

                        // For arcs, extract I/J/K center offsets and store plane
                        if mt == MoveType::ArcCW || mt == MoveType::ArcCCW {
                            let i_val = extract_word(&line.text, 'I').unwrap_or(0.0);
                            let j_val = extract_word(&line.text, 'J').unwrap_or(0.0);
                            let k_val = extract_word(&line.text, 'K').unwrap_or(0.0);
                            line.arc = Some(ArcData {
                                i: i_val,
                                j: j_val,
                                k: k_val,
                                plane: modal_plane,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        Self {
            id: uuid::Uuid::new_v4(),
            name,
            lines,
            total_lines,
            estimated_duration_secs: None,
            bounding_box: if has_motion { Some(bb) } else { None },
            loaded_at: chrono::Utc::now(),
        }
    }
}

/// Bounding box of a toolpath (supports up to 8 axes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: Vec<f64>,
    pub max: Vec<f64>,
}

impl BoundingBox {
    pub fn new() -> Self {
        Self {
            min: vec![f64::MAX; 3],
            max: vec![f64::MIN; 3],
        }
    }

    pub fn expand(&mut self, point: &[f64]) {
        // Grow the bounding box vectors if the point has more axes
        while self.min.len() < point.len() {
            self.min.push(f64::MAX);
            self.max.push(f64::MIN);
        }
        for (i, &coord) in point.iter().enumerate() {
            self.min[i] = self.min[i].min(coord);
            self.max[i] = self.max[i].max(coord);
        }
    }

    pub fn size(&self) -> Vec<f64> {
        self.min
            .iter()
            .zip(self.max.iter())
            .map(|(mn, mx)| mx - mn)
            .collect()
    }
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_comments() {
        assert_eq!(strip_comments("G0 X10 ; rapid"), "G0 X10 ");
        assert_eq!(strip_comments("G0 (comment) X10"), "G0  X10");
        assert_eq!(strip_comments("(full line comment)"), "");
        assert_eq!(strip_comments("G1 X10 Y20 F1000"), "G1 X10 Y20 F1000");
    }

    #[test]
    fn test_detect_move_type() {
        assert_eq!(detect_move_type("G0 X10"), Some(MoveType::Rapid));
        assert_eq!(detect_move_type("G00 X10"), Some(MoveType::Rapid));
        assert_eq!(detect_move_type("G1 X10 Y20 F1000"), Some(MoveType::Linear));
        assert_eq!(detect_move_type("G2 X10 Y20 I5 J5"), Some(MoveType::ArcCW));
        assert_eq!(detect_move_type("G3 X10 Y20 I5 J5"), Some(MoveType::ArcCCW));
        assert_eq!(detect_move_type("M3 S12000"), Some(MoveType::SpindleOn));
        assert_eq!(detect_move_type("M5"), Some(MoveType::SpindleOff));
        assert_eq!(detect_move_type("M30"), Some(MoveType::ProgramEnd));
        assert_eq!(detect_move_type(""), None);
    }

    #[test]
    fn test_detect_move_type_multi_command() {
        // Motion commands embedded after plane selects
        assert_eq!(
            detect_move_type("G17 G3 X0 Y-31.587 I-0.318 J0"),
            Some(MoveType::ArcCCW)
        );
        assert_eq!(
            detect_move_type("G19 G2 Y-32.54 Z-0.183 J0 K0.317"),
            Some(MoveType::ArcCW)
        );
        assert_eq!(
            detect_move_type("G19 G3 Y-32.222 Z-0.5 J0.318 K0"),
            Some(MoveType::ArcCCW)
        );
        // Non-motion multi-command
        assert_eq!(detect_move_type("G17 G90 G94"), None);
        assert_eq!(detect_move_type("G90 G94"), None);
        // Motion takes priority over non-motion on same line
        assert_eq!(detect_move_type("G54 G0 X0 Y0"), Some(MoveType::Rapid));
    }

    #[test]
    fn test_detect_move_type_no_false_matches() {
        assert_eq!(detect_move_type("G21"), None);
        assert_eq!(detect_move_type("G90"), None);
        assert_eq!(detect_move_type("G40"), None);
        assert_eq!(detect_move_type("G43 Z10 H1"), None);
        assert_eq!(detect_move_type("T1"), None);
        // Just axis words (no G/M code) — detected as None by detect_move_type
        // (modal tracking in parse() will assign the modal motion)
        assert_eq!(detect_move_type("Z7"), None);
        assert_eq!(detect_move_type("X0 Y-31.587 I0 J-31.587"), None);
    }

    #[test]
    fn test_gcode_line_byte_len() {
        let line = GCodeLine::new(1, "G0 X10 Y20");
        assert_eq!(line.text, "G0 X10 Y20");
        assert_eq!(line.byte_len, 12); // "G0 X10 Y20" + \r\n
    }

    #[test]
    fn test_gcode_file_parse() {
        let content = "G0 X10\nG1 X20 F1000\n; comment\n\nM30\n";
        let file = GCodeFile::parse("test.gcode".into(), content);
        assert_eq!(file.total_lines, 3); // empty/comment lines filtered
        assert_eq!(file.lines[0].text, "G0 X10");
        assert_eq!(file.lines[2].text, "M30");
    }

    #[test]
    fn test_bounding_box() {
        let mut bb = BoundingBox::new();
        bb.expand(&[10.0, 20.0, -5.0]);
        bb.expand(&[-5.0, 30.0, 0.0]);
        assert_eq!(bb.min, vec![-5.0, 20.0, -5.0]);
        assert_eq!(bb.max, vec![10.0, 30.0, 0.0]);
        assert_eq!(bb.size(), vec![15.0, 10.0, 5.0]);
    }

    #[test]
    fn test_extract_word() {
        assert_eq!(extract_word("G0 X10 Y20 Z5", 'X'), Some(10.0));
        assert_eq!(extract_word("G0 X10 Y20 Z5", 'Y'), Some(20.0));
        assert_eq!(extract_word("G0 X10 Y20 Z5", 'Z'), Some(5.0));
        assert_eq!(extract_word("G1 X50 Y0 F1000", 'F'), Some(1000.0));
        assert_eq!(extract_word("G1 X-5.5 Y0", 'X'), Some(-5.5));
        assert_eq!(extract_word("G0 X0", 'Y'), None);
    }

    #[test]
    fn test_endpoints_and_bounding_box_in_parse() {
        let content =
            "G21\nG90\nG0 X0 Y0 Z5\nG1 X50 Y0 F1000\nG1 X50 Y50\nG1 X0 Y50\nG1 X0 Y0\nG0 Z5\nM30\n";
        let file = GCodeFile::parse("test.nc".into(), content);

        let motion_lines: Vec<_> = file.lines.iter().filter(|l| l.endpoint.is_some()).collect();
        assert!(
            motion_lines.len() >= 5,
            "Expected at least 5 motion lines, got {}",
            motion_lines.len()
        );

        let first = motion_lines[0];
        assert_eq!(first.move_type, Some(MoveType::Rapid));
        assert_eq!(first.endpoint.as_ref().unwrap()[..3], [0.0, 0.0, 5.0]);

        let second = motion_lines[1];
        assert_eq!(second.move_type, Some(MoveType::Linear));
        assert_eq!(second.endpoint.as_ref().unwrap()[..3], [50.0, 0.0, 5.0]);

        let bb = file
            .bounding_box
            .as_ref()
            .expect("Should have bounding box");
        assert_eq!(bb.min[0], 0.0);
        assert_eq!(bb.min[1], 0.0);
        assert_eq!(bb.max[0], 50.0);
        assert_eq!(bb.max[1], 50.0);
    }

    #[test]
    fn test_modal_motion_tracking() {
        // Z7 after G0 should inherit G0 (Rapid)
        let content = "G0 X10 Y20\nZ5\nG1 X20 F1000\nZ3\n";
        let file = GCodeFile::parse("modal.nc".into(), content);

        // Line 0: G0 X10 Y20 → Rapid
        assert_eq!(file.lines[0].move_type, Some(MoveType::Rapid));
        assert!(file.lines[0].endpoint.is_some());

        // Line 1: Z5 → modal Rapid
        assert_eq!(file.lines[1].move_type, Some(MoveType::Rapid));
        assert_eq!(file.lines[1].endpoint.as_ref().unwrap()[2], 5.0);

        // Line 2: G1 X20 F1000 → Linear
        assert_eq!(file.lines[2].move_type, Some(MoveType::Linear));

        // Line 3: Z3 → modal Linear
        assert_eq!(file.lines[3].move_type, Some(MoveType::Linear));
        assert_eq!(file.lines[3].endpoint.as_ref().unwrap()[2], 3.0);
    }

    #[test]
    fn test_modal_arc_continuation() {
        // G2 full circle split into two semicircles, second line is modal
        let content = "G0 X0 Y-30\nG2 X0 Y30 I0 J30\nX0 Y-30 I0 J-30\n";
        let file = GCodeFile::parse("arc.nc".into(), content);

        // Line 0: G0 X0 Y-30
        assert_eq!(file.lines[0].move_type, Some(MoveType::Rapid));

        // Line 1: G2 X0 Y30 I0 J30 — explicit ArcCW
        assert_eq!(file.lines[1].move_type, Some(MoveType::ArcCW));
        assert!(file.lines[1].arc.is_some());
        let arc1 = file.lines[1].arc.as_ref().unwrap();
        assert_eq!(arc1.i, 0.0);
        assert_eq!(arc1.j, 30.0);
        assert_eq!(arc1.plane, 17);

        // Line 2: X0 Y-30 I0 J-30 — modal ArcCW continuation
        assert_eq!(file.lines[2].move_type, Some(MoveType::ArcCW));
        assert!(file.lines[2].arc.is_some());
        let arc2 = file.lines[2].arc.as_ref().unwrap();
        assert_eq!(arc2.j, -30.0);
    }

    #[test]
    fn test_plane_tracking_with_arcs() {
        let content = "G0 X0 Y-32\nG19 G3 Y-32 Z-0.5 J0.318 K0\nG17 G3 X0 Y-31 I-0.318 J0\n";
        let file = GCodeFile::parse("plane.nc".into(), content);

        // Line 1: G19 G3 — YZ plane arc (sent as-is to grblHAL)
        assert_eq!(file.lines[1].move_type, Some(MoveType::ArcCCW));
        let arc1 = file.lines[1].arc.as_ref().unwrap();
        assert_eq!(arc1.plane, 19);
        assert!(file.lines[1].text.starts_with("G19 G3"));

        // Line 2: G17 G3 — XY plane arc (kept as-is)
        assert_eq!(file.lines[2].move_type, Some(MoveType::ArcCCW));
        let arc2 = file.lines[2].arc.as_ref().unwrap();
        assert_eq!(arc2.plane, 17);
    }

    #[test]
    fn test_incremental_distance_mode_endpoints() {
        // In G91, axis words are deltas from the current position.
        let content = "G91\nG0 X1 Y1\nX1\nY-2\n";
        let file = GCodeFile::parse("inc.nc".into(), content);

        let motion_lines: Vec<_> = file.lines.iter().filter(|l| l.endpoint.is_some()).collect();
        assert_eq!(motion_lines.len(), 3);

        assert_eq!(
            motion_lines[0].endpoint.as_ref().unwrap()[..3],
            [1.0, 1.0, 0.0]
        );
        assert_eq!(
            motion_lines[1].endpoint.as_ref().unwrap()[..3],
            [2.0, 1.0, 0.0]
        );
        assert_eq!(
            motion_lines[2].endpoint.as_ref().unwrap()[..3],
            [2.0, -1.0, 0.0]
        );

        let bb = file
            .bounding_box
            .as_ref()
            .expect("Should have bounding box");
        assert_eq!(bb.min[0], 1.0);
        assert_eq!(bb.min[1], -1.0);
        assert_eq!(bb.max[0], 2.0);
        assert_eq!(bb.max[1], 1.0);
    }

    #[test]
    fn test_distance_mode_switching_absolute_after_incremental() {
        // Switching back to G90 should return to absolute coordinates.
        let content = "G91\nG0 X1\nG90\nX10\n";
        let file = GCodeFile::parse("switch.nc".into(), content);

        let motion_lines: Vec<_> = file.lines.iter().filter(|l| l.endpoint.is_some()).collect();
        assert_eq!(motion_lines.len(), 2);
        assert_eq!(motion_lines[0].endpoint.as_ref().unwrap()[0], 1.0);
        assert_eq!(motion_lines[1].endpoint.as_ref().unwrap()[0], 10.0);
    }

    #[test]
    fn test_arc_distance_mode_does_not_change_xyz_distance_mode() {
        // G91.1 is arc distance mode (IJK), not X/Y/Z distance mode.
        let content = "G91.1\nG0 X10\nX10\n";
        let file = GCodeFile::parse("arc_mode.nc".into(), content);

        let motion_lines: Vec<_> = file.lines.iter().filter(|l| l.endpoint.is_some()).collect();
        assert_eq!(motion_lines.len(), 2);
        // Absolute default: X10 then X10 again (not X20).
        assert_eq!(motion_lines[0].endpoint.as_ref().unwrap()[0], 10.0);
        assert_eq!(motion_lines[1].endpoint.as_ref().unwrap()[0], 10.0);

        let preamble = extract_modal_preamble(&file.lines, 1);
        assert!(!preamble.iter().any(|s| s == "G91"));
    }

    #[test]
    fn test_extract_modal_preamble_basic() {
        let content = "G21\nG90\nG54\nM3 S12000\nG1 X10 Y20 F1000\nG1 X50 Y50\nG0 Z5\nM30\n";
        let file = GCodeFile::parse("preamble.nc".into(), content);

        // Starting from line index 5 (G1 X50 Y50), preamble scans [0..5)
        let preamble = extract_modal_preamble(&file.lines, 5);
        // G21=default, G90=default, G54=default → not emitted
        // M3 S12000 → emitted (not default M5)
        // F1000 → emitted
        assert!(preamble.contains(&"F1000".to_string()));
        assert!(preamble.contains(&"M3 S12000".to_string()));
        assert!(!preamble.iter().any(|c| c == "G90"));
        assert!(!preamble.iter().any(|c| c == "G21"));
        assert!(!preamble.iter().any(|c| c == "G54"));
    }

    #[test]
    fn test_extract_modal_preamble_non_defaults() {
        let content = "G20\nG91\nG18\nG55\nM8\nG1 X10 F500\nG1 X20\n";
        let file = GCodeFile::parse("preamble2.nc".into(), content);

        // Starting from line index 6 (G1 X20), preamble scans [0..6)
        let preamble = extract_modal_preamble(&file.lines, 6);
        assert!(preamble.contains(&"G20".to_string()));
        assert!(preamble.contains(&"G91".to_string()));
        assert!(preamble.contains(&"G18".to_string()));
        assert!(preamble.contains(&"G55".to_string()));
        assert!(preamble.contains(&"M8".to_string()));
        assert!(preamble.contains(&"F500".to_string()));
    }

    #[test]
    fn test_extract_modal_preamble_empty_at_start() {
        let content = "G0 X10\nG1 X20 F1000\n";
        let file = GCodeFile::parse("start.nc".into(), content);
        let preamble = extract_modal_preamble(&file.lines, 0);
        assert!(preamble.is_empty());
    }
}

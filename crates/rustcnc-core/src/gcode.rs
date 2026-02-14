use serde::{Deserialize, Serialize};

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
}

impl GCodeLine {
    /// Create a new GCodeLine from raw text. Strips comments and trims.
    pub fn new(file_line: usize, raw: &str) -> Self {
        let text = strip_comments(raw).trim().to_uppercase();
        let byte_len = text.len() + 1; // +1 for \n
        let move_type = detect_move_type(&text);
        Self {
            file_line,
            text,
            byte_len,
            move_type,
            endpoint: None,
        }
    }

    /// Returns true if the line is empty or comment-only
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
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

/// Check if `text` starts with `code` and the next character (if any) is not an ASCII digit.
/// This prevents false prefix matches like M5 matching M50.
fn matches_gcode(text: &str, code: &str) -> bool {
    if !text.starts_with(code) {
        return false;
    }
    text[code.len()..]
        .chars()
        .next()
        .map_or(true, |c| !c.is_ascii_digit())
}

/// Detect the primary move type from a G-code line
fn detect_move_type(text: &str) -> Option<MoveType> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // Check for G commands
    if matches_gcode(text, "G0") || matches_gcode(text, "G00") {
        return Some(MoveType::Rapid);
    }
    if matches_gcode(text, "G1") || matches_gcode(text, "G01") {
        return Some(MoveType::Linear);
    }
    if matches_gcode(text, "G2") || matches_gcode(text, "G02") {
        return Some(MoveType::ArcCW);
    }
    if matches_gcode(text, "G3") || matches_gcode(text, "G03") {
        return Some(MoveType::ArcCCW);
    }
    if matches_gcode(text, "G38") {
        return Some(MoveType::Probe);
    }
    if matches_gcode(text, "G4") || matches_gcode(text, "G04") {
        return Some(MoveType::Dwell);
    }
    if matches_gcode(text, "G28") || matches_gcode(text, "G30") {
        return Some(MoveType::Home);
    }
    if matches_gcode(text, "G54")
        || matches_gcode(text, "G55")
        || matches_gcode(text, "G56")
        || matches_gcode(text, "G57")
        || matches_gcode(text, "G58")
        || matches_gcode(text, "G59")
    {
        return Some(MoveType::CoordSystem);
    }

    // Check for M commands
    if matches_gcode(text, "M6") || matches_gcode(text, "M06") {
        return Some(MoveType::ToolChange);
    }
    if matches_gcode(text, "M30") || matches_gcode(text, "M2") || matches_gcode(text, "M02") {
        return Some(MoveType::ProgramEnd);
    }
    if matches_gcode(text, "M3") || matches_gcode(text, "M03") || matches_gcode(text, "M4") || matches_gcode(text, "M04") {
        return Some(MoveType::SpindleOn);
    }
    if matches_gcode(text, "M5") || matches_gcode(text, "M05") {
        return Some(MoveType::SpindleOff);
    }
    if matches_gcode(text, "M7") || matches_gcode(text, "M07") || matches_gcode(text, "M8") || matches_gcode(text, "M08") {
        return Some(MoveType::CoolantOn);
    }
    if matches_gcode(text, "M9") || matches_gcode(text, "M09") {
        return Some(MoveType::CoolantOff);
    }

    None
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
        let lines: Vec<GCodeLine> = content
            .lines()
            .enumerate()
            .map(|(i, line)| GCodeLine::new(i + 1, line))
            .filter(|l| !l.is_empty())
            .collect();

        let total_lines = lines.len();

        Self {
            id: uuid::Uuid::new_v4(),
            name,
            lines,
            total_lines,
            estimated_duration_secs: None,
            bounding_box: None,
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
        for i in 0..point.len() {
            self.min[i] = self.min[i].min(point[i]);
            self.max[i] = self.max[i].max(point[i]);
        }
    }

    pub fn size(&self) -> Vec<f64> {
        self.min.iter().zip(self.max.iter())
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
    fn test_gcode_line_byte_len() {
        let line = GCodeLine::new(1, "G0 X10 Y20");
        assert_eq!(line.text, "G0 X10 Y20");
        assert_eq!(line.byte_len, 11); // "G0 X10 Y20" + \n
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
}

use tracing::warn;

/// Maximum line length before the buffer is discarded to prevent unbounded growth.
const MAX_LINE_LENGTH: usize = 1024;

/// Incremental parser for GRBL serial output.
///
/// Accumulates bytes until a newline is found, then parses the complete line
/// into a structured `GrblResponse`. This allows processing data byte-by-byte
/// as it arrives from the serial port.
pub struct ResponseParser {
    buffer: Vec<u8>,
}

/// Parsed GRBL response types
#[derive(Debug, Clone, PartialEq)]
pub enum GrblResponse {
    /// Command acknowledged successfully
    Ok,
    /// Command returned an error with code
    Error(u8),
    /// Alarm triggered
    Alarm(u8),
    /// Status report (raw `<...>` string for further parsing)
    StatusReport(String),
    /// Welcome/startup message ("Grbl X.Xx [...]" or "grblHAL ...")
    Welcome(String),
    /// Informational message [MSG:...]
    Message(String),
    /// Parser state report [GC:...]
    ParserState(String),
    /// GRBL setting ($key=value)
    Setting(String, String),
    /// Build info [VER:...] or [OPT:...]
    BuildInfo(String),
    /// Feedback message [...]
    Feedback(String),
    /// Startup line execution result (>line:ok)
    StartupLine(String),
    /// Unrecognized line
    Unknown(String),
}

impl ResponseParser {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(256),
        }
    }

    /// Feed raw bytes from serial port. Returns any complete parsed responses.
    pub fn feed(&mut self, data: &[u8]) -> Vec<GrblResponse> {
        let mut responses = Vec::new();

        for &byte in data {
            if byte == b'\n' || byte == b'\r' {
                if !self.buffer.is_empty() {
                    if let Some(resp) = self.parse_line() {
                        responses.push(resp);
                    }
                    self.buffer.clear();
                }
            } else {
                if self.buffer.len() >= MAX_LINE_LENGTH {
                    warn!("Response line exceeded {} bytes, discarding", MAX_LINE_LENGTH);
                    self.buffer.clear();
                }
                self.buffer.push(byte);
            }
        }

        responses
    }

    /// Reset the parser state (e.g. after soft reset)
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    fn parse_line(&self) -> Option<GrblResponse> {
        let line = std::str::from_utf8(&self.buffer).ok()?;
        let line = line.trim();

        if line.is_empty() {
            return None;
        }

        // "ok"
        if line == "ok" {
            return Some(GrblResponse::Ok);
        }

        // "error:N"
        if let Some(code_str) = line.strip_prefix("error:") {
            let code = code_str.parse().unwrap_or(0);
            return Some(GrblResponse::Error(code));
        }

        // "ALARM:N"
        if let Some(code_str) = line.strip_prefix("ALARM:") {
            let code = code_str.parse().unwrap_or(0);
            return Some(GrblResponse::Alarm(code));
        }

        // Status report: <...>
        if line.starts_with('<') && line.ends_with('>') {
            return Some(GrblResponse::StatusReport(line.to_string()));
        }

        // Welcome message
        if line.starts_with("Grbl") || line.starts_with("grblHAL") || line.starts_with("GrblHAL")
        {
            return Some(GrblResponse::Welcome(line.to_string()));
        }

        // [MSG:...]
        if let Some(rest) = line.strip_prefix("[MSG:") {
            let msg = rest.trim_end_matches(']');
            return Some(GrblResponse::Message(msg.to_string()));
        }

        // [GC:...]
        if let Some(rest) = line.strip_prefix("[GC:") {
            let state = rest.trim_end_matches(']');
            return Some(GrblResponse::ParserState(state.to_string()));
        }

        // [VER:...] or [OPT:...]
        if (line.starts_with("[VER:") || line.starts_with("[OPT:") || line.starts_with("[NEWOPT:"))
            && line.ends_with(']')
        {
            return Some(GrblResponse::BuildInfo(line.to_string()));
        }

        // Settings: $N=value
        if line.starts_with('$') && line.contains('=') {
            let rest = &line[1..];
            if let Some((key, val)) = rest.split_once('=') {
                return Some(GrblResponse::Setting(key.to_string(), val.to_string()));
            }
        }

        // Startup line execution: >line:ok
        if line.starts_with('>') {
            return Some(GrblResponse::StartupLine(line[1..].to_string()));
        }

        // Generic feedback: [...]
        if line.starts_with('[') && line.ends_with(']') {
            return Some(GrblResponse::Feedback(line.to_string()));
        }

        Some(GrblResponse::Unknown(line.to_string()))
    }
}

impl Default for ResponseParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ok() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"ok\n");
        assert_eq!(responses, vec![GrblResponse::Ok]);
    }

    #[test]
    fn test_parse_error() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"error:22\n");
        assert_eq!(responses, vec![GrblResponse::Error(22)]);
    }

    #[test]
    fn test_parse_alarm() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"ALARM:1\n");
        assert_eq!(responses, vec![GrblResponse::Alarm(1)]);
    }

    #[test]
    fn test_parse_status_report() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"<Idle|MPos:0.000,0.000,0.000|FS:0,0>\n");
        match &responses[0] {
            GrblResponse::StatusReport(s) => {
                assert!(s.starts_with("<Idle"));
            }
            other => panic!("Expected StatusReport, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_welcome_grbl() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"Grbl 1.1h ['$' for help]\n");
        match &responses[0] {
            GrblResponse::Welcome(s) => {
                assert!(s.starts_with("Grbl 1.1h"));
            }
            other => panic!("Expected Welcome, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_welcome_grblhal() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"grblHAL 1.1f ['$' for help]\n");
        match &responses[0] {
            GrblResponse::Welcome(s) => {
                assert!(s.starts_with("grblHAL"));
            }
            other => panic!("Expected Welcome, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_message() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"[MSG:Caution: Unlocked]\n");
        assert_eq!(
            responses,
            vec![GrblResponse::Message("Caution: Unlocked".to_string())]
        );
    }

    #[test]
    fn test_parse_parser_state() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]\n");
        match &responses[0] {
            GrblResponse::ParserState(s) => {
                assert!(s.contains("G0 G54"));
            }
            other => panic!("Expected ParserState, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_setting() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"$110=8000.000\n");
        assert_eq!(
            responses,
            vec![GrblResponse::Setting(
                "110".to_string(),
                "8000.000".to_string()
            )]
        );
    }

    #[test]
    fn test_parse_startup_line() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b">G54G20:ok\n");
        match &responses[0] {
            GrblResponse::StartupLine(s) => {
                assert_eq!(s, "G54G20:ok");
            }
            other => panic!("Expected StartupLine, got {:?}", other),
        }
    }

    #[test]
    fn test_partial_data() {
        let mut parser = ResponseParser::new();

        // Feed data in chunks
        let responses = parser.feed(b"ok");
        assert!(responses.is_empty()); // no newline yet

        let responses = parser.feed(b"\n");
        assert_eq!(responses, vec![GrblResponse::Ok]);
    }

    #[test]
    fn test_multiple_lines() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"ok\nok\nerror:22\n");
        assert_eq!(
            responses,
            vec![
                GrblResponse::Ok,
                GrblResponse::Ok,
                GrblResponse::Error(22)
            ]
        );
    }

    #[test]
    fn test_crlf() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"ok\r\n");
        assert_eq!(responses, vec![GrblResponse::Ok]);
    }

    #[test]
    fn test_empty_lines_skipped() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"\n\n\nok\n\n");
        assert_eq!(responses, vec![GrblResponse::Ok]);
    }

    #[test]
    fn test_unknown_line() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"some random text\n");
        assert_eq!(
            responses,
            vec![GrblResponse::Unknown("some random text".to_string())]
        );
    }

    #[test]
    fn test_build_info() {
        let mut parser = ResponseParser::new();
        let responses = parser.feed(b"[VER:1.1f.20170801:]\n");
        match &responses[0] {
            GrblResponse::BuildInfo(s) => {
                assert!(s.contains("VER:1.1f"));
            }
            other => panic!("Expected BuildInfo, got {:?}", other),
        }
    }

    #[test]
    fn test_reset_parser() {
        let mut parser = ResponseParser::new();
        parser.feed(b"partial");
        parser.reset();
        let responses = parser.feed(b"ok\n");
        assert_eq!(responses, vec![GrblResponse::Ok]);
    }
}

use serde::{Deserialize, Serialize};

use crate::job::JobState;
use crate::machine::{FirmwareType, MachineSnapshot, MachineState};
use crate::overrides::Overrides;

// ── Server → Client messages ─────────────────────────────────────

/// All messages from server to client (downstream)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    /// Machine state update (~100Hz when running, ~5Hz when idle)
    MachineState(MachineSnapshot),
    /// Job progress update (~2Hz)
    JobProgress(JobProgress),
    /// Console output (GRBL responses, messages)
    ConsoleOutput(ConsoleEntry),
    /// Connection state changed
    ConnectionChanged(ConnectionState),
    /// File list updated
    FileListUpdated(Vec<FileInfo>),
    /// Error notification
    Error(ErrorNotification),
    /// Full state sync (sent on connect/reconnect)
    StateSync(Box<FullStateSync>),
    /// Alarm notification
    Alarm(AlarmNotification),
    /// G-code file loaded (for 3D viewer)
    GCodeLoaded(GCodeFileInfo),
    /// Available serial ports
    PortList(Vec<PortInfo>),
    /// Pong (response to Ping)
    Pong,
}

// ── Client → Server messages ─────────────────────────────────────

/// All messages from client to server (upstream)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    /// Send a real-time command (feed hold, overrides, etc.)
    RealtimeCommand(RealtimeCommandMsg),
    /// Send a jog command
    Jog(JogCommand),
    /// Send a raw G-code/command line via console
    ConsoleSend(String),
    /// Request full state sync
    RequestSync,
    /// Heartbeat/keepalive
    Ping,
    /// Job control
    JobControl(JobControlAction),
    /// Connection control
    Connect(ConnectRequest),
    /// Disconnect from serial port
    Disconnect,
    /// Request list of available serial ports
    RequestPortList,
}

// ── Supporting types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeCommandMsg {
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JogCommand {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
    pub feed: f64,
    pub incremental: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
}

impl JogCommand {
    /// Build the GRBL $J= jog command string
    pub fn to_grbl_command(&self) -> String {
        let mut cmd = String::from("$J=G91");
        if !self.incremental {
            cmd = String::from("$J=G90");
        }
        if let Some(x) = self.x {
            cmd.push_str(&format!("X{:.3}", x));
        }
        if let Some(y) = self.y {
            cmd.push_str(&format!("Y{:.3}", y));
        }
        if let Some(z) = self.z {
            cmd.push_str(&format!("Z{:.3}", z));
        }
        cmd.push_str(&format!("F{:.0}", self.feed));
        cmd
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRequest {
    pub port: String,
    pub baud_rate: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum JobControlAction {
    Start,
    Pause,
    Resume,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub file_name: String,
    pub current_line: usize,
    pub total_lines: usize,
    pub percent_complete: f32,
    pub elapsed_secs: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_remaining_secs: Option<f64>,
    pub state: JobState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEntry {
    pub direction: ConsoleDirection,
    pub text: String,
    pub timestamp: i64, // unix millis
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ConsoleDirection {
    Sent,
    Received,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionState {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullStateSync {
    pub machine: MachineSnapshot,
    pub connection: ConnectionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<JobProgress>,
    pub files: Vec<FileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub line_count: usize,
    pub loaded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCodeFileInfo {
    pub id: String,
    pub name: String,
    pub lines: Vec<GCodeLineInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounding_box: Option<[[f64; 3]; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCodeLineInfo {
    pub line_num: usize,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub move_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorNotification {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u8>,
    pub message: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmNotification {
    pub code: u8,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_message_serialization() {
        let msg = ServerMessage::Pong;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"Pong\""));

        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, ServerMessage::Pong));
    }

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::RealtimeCommand(RealtimeCommandMsg {
            command: "feed_hold".to_string(),
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"RealtimeCommand\""));
        assert!(json.contains("feed_hold"));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ClientMessage::RealtimeCommand(cmd) => assert_eq!(cmd.command, "feed_hold"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_jog_command_to_grbl() {
        let jog = JogCommand {
            x: Some(10.0),
            y: None,
            z: Some(-5.0),
            feed: 1000.0,
            incremental: true,
            distance: None,
        };
        assert_eq!(jog.to_grbl_command(), "$J=G91X10.000Z-5.000F1000");
    }

    #[test]
    fn test_jog_command_absolute() {
        let jog = JogCommand {
            x: Some(100.0),
            y: Some(200.0),
            z: None,
            feed: 500.0,
            incremental: false,
            distance: None,
        };
        assert_eq!(jog.to_grbl_command(), "$J=G90X100.000Y200.000F500");
    }

    #[test]
    fn test_ping_pong_roundtrip() {
        let ping = ClientMessage::Ping;
        let json = serde_json::to_string(&ping).unwrap();
        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, ClientMessage::Ping));
    }

    #[test]
    fn test_job_progress_serialization() {
        let progress = JobProgress {
            file_name: "test.gcode".into(),
            current_line: 100,
            total_lines: 1000,
            percent_complete: 10.0,
            elapsed_secs: 60.0,
            estimated_remaining_secs: Some(540.0),
            state: JobState::Running,
        };
        let msg = ServerMessage::JobProgress(progress);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ServerMessage::JobProgress(p) => {
                assert_eq!(p.current_line, 100);
                assert_eq!(p.state, JobState::Running);
            }
            _ => panic!("wrong variant"),
        }
    }
}

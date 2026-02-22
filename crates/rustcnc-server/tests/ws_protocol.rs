//! Integration test: WebSocket protocol round-trip
//!
//! Tests that ServerMessage and ClientMessage serialize/deserialize correctly.

use rustcnc_core::machine::{
    AccessoryState, BufferState, FirmwareType, InputPins, MachineSnapshot, MachineState, Position,
};
use rustcnc_core::overrides::Overrides;
use rustcnc_core::ws_protocol::*;

fn default_snapshot() -> MachineSnapshot {
    MachineSnapshot {
        state: MachineState::Idle,
        machine_pos: Position {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            a: None,
            b: None,
            c: None,
            u: None,
            v: None,
        },
        work_pos: Position {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            a: None,
            b: None,
            c: None,
            u: None,
            v: None,
        },
        feed_rate: 0.0,
        spindle_speed: 0.0,
        overrides: Overrides {
            feed: 100,
            rapids: 100,
            spindle: 100,
        },
        accessories: AccessoryState {
            spindle_cw: false,
            spindle_ccw: false,
            flood_coolant: false,
            mist_coolant: false,
        },
        input_pins: InputPins {
            limit_x: false,
            limit_y: false,
            limit_z: false,
            limit_a: false,
            limit_b: false,
            limit_c: false,
            limit_u: false,
            limit_v: false,
            probe: false,
            door: false,
            hold: false,
            soft_reset: false,
            cycle_start: false,
            estop: false,
        },
        buffer: BufferState {
            planner_blocks_available: 15,
            rx_bytes_available: 128,
        },
        line_number: 0,
        connected: true,
        firmware: FirmwareType::Grbl,
    }
}

#[test]
fn test_server_message_roundtrip() {
    let mut snapshot = default_snapshot();
    snapshot.state = MachineState::Run;
    snapshot.machine_pos = Position {
        x: 100.5,
        y: 200.3,
        z: -10.0,
        a: None,
        b: None,
        c: None,
        u: None,
        v: None,
    };
    snapshot.feed_rate = 1500.0;
    snapshot.spindle_speed = 12000.0;
    snapshot.line_number = 1542;

    let messages: Vec<ServerMessage> = vec![
        ServerMessage::MachineState(snapshot),
        ServerMessage::JobProgress(JobProgress {
            file_name: "test.gcode".to_string(),
            current_line: 500,
            total_lines: 10000,
            percent_complete: 5.0,
            elapsed_secs: 120.0,
            estimated_remaining_secs: Some(2280.0),
            state: rustcnc_core::job::JobState::Running,
        }),
        ServerMessage::Error(ErrorNotification {
            code: Some(23),
            message: "Invalid gcode ID:23".to_string(),
            source: "grbl".to_string(),
        }),
        ServerMessage::Alarm(AlarmNotification {
            code: 1,
            message: "Hard limit triggered".to_string(),
        }),
        ServerMessage::Pong,
    ];

    for msg in &messages {
        let json = serde_json::to_string(msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&parsed).unwrap();
        assert_eq!(json, json2, "Round-trip failed for message");
    }
}

#[test]
fn test_client_message_roundtrip() {
    let messages: Vec<ClientMessage> = vec![
        ClientMessage::RealtimeCommand(RealtimeCommandMsg {
            command: "feed_hold".to_string(),
        }),
        ClientMessage::Jog(JogCommand {
            x: Some(10.0),
            y: None,
            z: Some(-5.0),
            a: None,
            b: None,
            c: None,
            u: None,
            v: None,
            feed: 1000.0,
            incremental: true,
            distance: None,
        }),
        ClientMessage::ConsoleSend("G28.1".to_string()),
        ClientMessage::RequestSync,
        ClientMessage::Ping,
        ClientMessage::JobControl(JobControlAction::Start {
            start_line: None,
            stop_line: None,
        }),
        ClientMessage::JobControl(JobControlAction::Pause),
        ClientMessage::JobControl(JobControlAction::Resume),
        ClientMessage::JobControl(JobControlAction::Stop),
        ClientMessage::Connect(ConnectRequest {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 115200,
        }),
        ClientMessage::Disconnect,
        ClientMessage::RequestPortList,
    ];

    for msg in &messages {
        let json = serde_json::to_string(msg).unwrap();
        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&parsed).unwrap();
        assert_eq!(json, json2, "Round-trip failed for message");
    }
}

#[test]
fn test_jog_command_grbl_format() {
    let jog = JogCommand {
        x: Some(10.0),
        y: Some(-5.0),
        z: None,
        a: None,
        b: None,
        c: None,
        u: None,
        v: None,
        feed: 1500.0,
        incremental: true,
        distance: None,
    };

    let grbl = jog.to_grbl_command();
    assert!(grbl.starts_with("$J=G91"));
    assert!(grbl.contains("X10.000"));
    assert!(grbl.contains("Y-5.000"));
    assert!(!grbl.contains("Z"));
    assert!(grbl.contains("F1500"));

    // Absolute jog
    let jog_abs = JogCommand {
        x: Some(100.0),
        y: Some(200.0),
        z: Some(0.0),
        a: None,
        b: None,
        c: None,
        u: None,
        v: None,
        feed: 3000.0,
        incremental: false,
        distance: None,
    };

    let grbl_abs = jog_abs.to_grbl_command();
    assert!(grbl_abs.starts_with("$J=G90"));
    assert!(grbl_abs.contains("X100.000"));
    assert!(grbl_abs.contains("Y200.000"));
    assert!(grbl_abs.contains("Z0.000"));
    assert!(grbl_abs.contains("F3000"));
}

#[test]
fn test_state_sync_message() {
    let sync = FullStateSync {
        machine: default_snapshot(),
        connection: ConnectionState {
            connected: true,
            port: Some("/dev/ttyACM0".to_string()),
            firmware: Some("grblHAL".to_string()),
            version: Some("1.1f".to_string()),
        },
        job: None,
        files: vec![FileInfo {
            id: "abc-123".to_string(),
            name: "test.gcode".to_string(),
            size_bytes: 15000,
            line_count: 500,
            loaded_at: "2024-01-01T00:00:00Z".to_string(),
        }],
    };

    let msg = ServerMessage::StateSync(Box::new(sync));
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ServerMessage = serde_json::from_str(&json).unwrap();

    if let ServerMessage::StateSync(sync) = parsed {
        assert_eq!(sync.files.len(), 1);
        assert_eq!(sync.files[0].name, "test.gcode");
        assert!(sync.connection.connected);
        assert_eq!(sync.connection.firmware, Some("grblHAL".to_string()));
    } else {
        panic!("Expected StateSync message");
    }
}

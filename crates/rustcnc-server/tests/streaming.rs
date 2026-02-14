//! Integration test: stream G-code through Streamer -> Simulator -> verify
//!
//! This test starts a GRBL simulator, connects a streamer thread,
//! sends G-code lines, and verifies they are all acknowledged.

use std::time::Duration;

use crossbeam_channel::{bounded, unbounded};

use rustcnc_simulator::simulator::{GrblSimulator, SimulatorConfig};
use rustcnc_streamer::streamer::{
    SharedMachineState, StreamerCommand, StreamerConfig, StreamerEvent, streamer_thread_main,
};

#[test]
fn test_stream_simple_gcode() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    // Start simulator
    let sim = GrblSimulator::new(SimulatorConfig {
        startup_delay_ms: 50,
        ..Default::default()
    });
    let serial = sim.start();

    // Create channels
    let (cmd_tx, cmd_rx) = bounded::<StreamerCommand>(256);
    let (event_tx, event_rx) = unbounded::<StreamerEvent>();
    let shared_state = SharedMachineState::new();

    let config = StreamerConfig {
        rx_buffer_size: 128,
        cpu_pin_core: None,
        rt_priority: None,
        status_poll_interval: Duration::from_millis(200),
    };

    // Spawn streamer thread
    let state_clone = shared_state.clone();
    let streamer_handle = std::thread::spawn(move || {
        streamer_thread_main(Box::new(serial), cmd_rx, event_tx, state_clone, config);
    });

    // Wait for welcome message
    let mut got_welcome = false;
    for _ in 0..50 {
        match event_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(StreamerEvent::Welcome { version }) => {
                println!("Got welcome: {}", version);
                got_welcome = true;
                break;
            }
            Ok(other) => {
                println!("Got event before welcome: {:?}", other);
            }
            Err(_) => {}
        }
    }
    assert!(got_welcome, "Did not receive GRBL welcome message");

    // Send G-code lines
    let gcode_lines = vec![
        "G21",
        "G90",
        "G0 Z5",
        "G0 X0 Y0",
        "G1 Z-1 F200",
        "G1 X10 F500",
        "G1 Y10",
        "G1 X0",
        "G1 Y0",
        "G0 Z5",
        "M30",
    ];

    let total_lines = gcode_lines.len();
    for (i, line) in gcode_lines.iter().enumerate() {
        let byte_len = line.len() + 1; // +1 for \n
        cmd_tx
            .send(StreamerCommand::GcodeLine {
                text: line.to_string(),
                byte_len,
                line_number: i + 1,
            })
            .unwrap();
    }

    // Wait for all acknowledgements
    let mut ack_count = 0;
    let timeout = Duration::from_secs(10);
    let start = std::time::Instant::now();

    while ack_count < total_lines && start.elapsed() < timeout {
        match event_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(StreamerEvent::LineAcknowledged { line_number }) => {
                println!("Line {} acknowledged", line_number);
                ack_count += 1;
            }
            Ok(StreamerEvent::LineError {
                line_number,
                code,
                message,
            }) => {
                panic!(
                    "Line {} returned error {}: {}",
                    line_number, code, message
                );
            }
            Ok(_) => {} // ignore other events
            Err(_) => {}
        }
    }

    assert_eq!(
        ack_count, total_lines,
        "Expected {} acks, got {}",
        total_lines, ack_count
    );

    // Poll status to verify final position
    cmd_tx.send(StreamerCommand::PollStatus).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Check shared state was updated
    let connected = shared_state.connected.load(std::sync::atomic::Ordering::Relaxed);
    assert!(connected, "Streamer should report connected");

    // Shutdown
    cmd_tx.send(StreamerCommand::Shutdown).unwrap();
    streamer_handle.join().unwrap();
}

#[test]
fn test_realtime_commands() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let sim = GrblSimulator::new(SimulatorConfig {
        startup_delay_ms: 50,
        ..Default::default()
    });
    let serial = sim.start();

    let (cmd_tx, cmd_rx) = bounded::<StreamerCommand>(256);
    let (event_tx, event_rx) = unbounded::<StreamerEvent>();
    let shared_state = SharedMachineState::new();

    let config = StreamerConfig {
        rx_buffer_size: 128,
        cpu_pin_core: None,
        rt_priority: None,
        status_poll_interval: Duration::from_secs(60), // disable auto-polling
    };

    let state_clone = shared_state.clone();
    let streamer_handle = std::thread::spawn(move || {
        streamer_thread_main(Box::new(serial), cmd_rx, event_tx, state_clone, config);
    });

    // Wait for welcome
    let mut got_welcome = false;
    for _ in 0..50 {
        match event_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(StreamerEvent::Welcome { .. }) => {
                got_welcome = true;
                break;
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }
    assert!(got_welcome);

    // Send a status poll
    cmd_tx.send(StreamerCommand::PollStatus).unwrap();

    // Expect a status report event
    let mut got_status = false;
    for _ in 0..20 {
        match event_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(StreamerEvent::StatusReport(report)) => {
                println!("Got status report: state={:?}", report.state);
                got_status = true;
                break;
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }
    assert!(got_status, "Expected a status report");

    // Send RT feed override
    use rustcnc_core::grbl::realtime::RealtimeCommand;
    cmd_tx
        .send(StreamerCommand::Realtime(RealtimeCommand::FeedOverridePlus10))
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Query status again to see override change
    cmd_tx.send(StreamerCommand::PollStatus).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Shutdown
    cmd_tx.send(StreamerCommand::Shutdown).unwrap();
    streamer_handle.join().unwrap();
}

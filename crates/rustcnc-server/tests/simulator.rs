//! Integration test: verify the GRBL simulator responds correctly
//!
//! Tests the simulator directly via its VirtualSerialPort interface.

use std::io::{Read, Write};
use std::time::Duration;

use rustcnc_simulator::simulator::{GrblSimulator, SimulatorConfig};
use rustcnc_streamer::serial::SerialPort;

fn read_response(serial: &mut impl SerialPort, timeout_ms: u64) -> String {
    let mut buf = [0u8; 1024];
    let mut result = String::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_millis(timeout_ms) {
        match serial.read(&mut buf) {
            Ok(n) if n > 0 => {
                result.push_str(&String::from_utf8_lossy(&buf[..n]));
                if result.contains("ok\r\n") || result.contains("error") {
                    break;
                }
            }
            _ => {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    result
}

#[test]
fn test_simulator_welcome() {
    let sim = GrblSimulator::new(SimulatorConfig {
        startup_delay_ms: 50,
        ..Default::default()
    });
    let mut serial = sim.start();

    // Read welcome message
    let mut buf = [0u8; 256];
    let mut welcome = String::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(2) {
        match serial.read(&mut buf) {
            Ok(n) if n > 0 => {
                welcome.push_str(&String::from_utf8_lossy(&buf[..n]));
                if welcome.contains("Grbl") {
                    break;
                }
            }
            _ => {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    assert!(
        welcome.contains("Grbl"),
        "Expected GRBL welcome, got: {:?}",
        welcome
    );
}

#[test]
fn test_simulator_gcode_execution() {
    let sim = GrblSimulator::new(SimulatorConfig {
        startup_delay_ms: 50,
        ..Default::default()
    });
    let mut serial = sim.start();

    // Wait for welcome
    std::thread::sleep(Duration::from_millis(200));
    let _ = read_response(&mut serial, 500); // consume welcome

    // Send home command
    serial.write_all(b"$H\n").unwrap();
    let resp = read_response(&mut serial, 1000);
    assert!(resp.contains("ok"), "Home should return ok, got: {:?}", resp);

    // Send G-code
    serial.write_all(b"G21\n").unwrap();
    let resp = read_response(&mut serial, 500);
    assert!(resp.contains("ok"), "G21 should return ok, got: {:?}", resp);

    serial.write_all(b"G90\n").unwrap();
    let resp = read_response(&mut serial, 500);
    assert!(resp.contains("ok"));

    serial.write_all(b"G0 X10 Y20 Z-5\n").unwrap();
    let resp = read_response(&mut serial, 500);
    assert!(resp.contains("ok"));

    // Query status
    serial.write_rt_command(b'?').unwrap();
    let status = read_response(&mut serial, 500);
    assert!(
        status.contains("MPos:10.000,20.000,-5.000"),
        "Expected position (10,20,-5), got: {:?}",
        status
    );
}

#[test]
fn test_simulator_status_query() {
    let sim = GrblSimulator::new(SimulatorConfig {
        startup_delay_ms: 50,
        ..Default::default()
    });
    let mut serial = sim.start();

    // Wait for welcome
    std::thread::sleep(Duration::from_millis(200));
    let _ = read_response(&mut serial, 500);

    // Query status
    serial.write_rt_command(b'?').unwrap();
    let status = read_response(&mut serial, 1000);

    assert!(
        status.contains("<") && status.contains(">"),
        "Expected status report format <...>, got: {:?}",
        status
    );
    assert!(
        status.contains("Idle"),
        "Expected Idle state, got: {:?}",
        status
    );
    assert!(
        status.contains("MPos:"),
        "Expected MPos field, got: {:?}",
        status
    );
}

#[test]
fn test_simulator_settings() {
    let sim = GrblSimulator::new(SimulatorConfig {
        startup_delay_ms: 50,
        ..Default::default()
    });
    let mut serial = sim.start();

    std::thread::sleep(Duration::from_millis(200));
    let _ = read_response(&mut serial, 500);

    serial.write_all(b"$$\n").unwrap();
    let resp = read_response(&mut serial, 1000);

    assert!(
        resp.contains("$100="),
        "Expected GRBL settings, got: {:?}",
        resp
    );
    assert!(resp.contains("ok"));
}

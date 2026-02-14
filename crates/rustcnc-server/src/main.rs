use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use parking_lot::RwLock;
use tokio::sync::{broadcast, mpsc};
use tracing::info;

use rustcnc_core::config::AppConfig;
use rustcnc_planner::planner::{PlannerCommand, PlannerEvent};
use rustcnc_simulator::simulator::{GrblSimulator, SimulatorConfig};
use rustcnc_streamer::streamer::{SharedMachineState, StreamerCommand, StreamerConfig, StreamerEvent};

mod api;
mod app;
mod cli;
mod logging;
mod shutdown;
mod state;
mod static_files;
mod ws;

use cli::Cli;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // 1. Initialize logging
    logging::init_logging(&cli.log_level);
    info!("RustCNC v{} starting", env!("CARGO_PKG_VERSION"));

    // 2. Load configuration
    let mut config = AppConfig::load_or_default(&cli.config);
    if cli.simulator {
        config.simulator.enabled = true;
    }
    if let Some(ref port) = cli.port {
        config.serial.default_port = Some(port.clone());
    }
    if let Some(baud) = cli.baud {
        config.serial.baud_rate = baud;
    }
    if let Some(ref host) = cli.host {
        config.server.host = host.clone();
    }
    if let Some(listen_port) = cli.listen_port {
        config.server.port = listen_port;
    }

    // 3. Create shared state
    let shared_state = SharedMachineState::new();

    // 4. Create channels between zones
    let (streamer_cmd_tx, streamer_cmd_rx) = crossbeam_channel::unbounded::<StreamerCommand>();
    let (streamer_event_tx, streamer_event_rx_raw) = crossbeam_channel::unbounded::<StreamerEvent>();
    let (planner_cmd_tx, planner_cmd_rx) = mpsc::channel::<PlannerCommand>(64);
    let (planner_event_tx, mut planner_event_rx) = mpsc::channel::<PlannerEvent>(64);
    let (ws_broadcast_tx, _) = broadcast::channel::<rustcnc_core::ws_protocol::ServerMessage>(256);

    // Bridge: crossbeam streamer events -> tokio mpsc for planner
    let (streamer_event_tokio_tx, streamer_event_tokio_rx) = mpsc::channel::<StreamerEvent>(256);
    let streamer_event_bridge_tx = streamer_event_tokio_tx.clone();
    let ws_tx_for_bridge = ws_broadcast_tx.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            match streamer_event_rx_raw.recv() {
                Ok(event) => {
                    // Forward console output and status events to WebSocket
                    match &event {
                        StreamerEvent::ConsoleOutput { text } => {
                            let _ = ws_tx_for_bridge.send(
                                rustcnc_core::ws_protocol::ServerMessage::ConsoleOutput(
                                    rustcnc_core::ws_protocol::ConsoleEntry {
                                        direction: rustcnc_core::ws_protocol::ConsoleDirection::Received,
                                        text: text.clone(),
                                        timestamp: chrono::Utc::now().timestamp_millis(),
                                    },
                                ),
                            );
                        }
                        StreamerEvent::Alarm { code } => {
                            let msg = rustcnc_core::grbl::error_codes::GrblAlarm::from_code(*code)
                                .map(|a| a.message().to_string())
                                .unwrap_or_else(|| format!("Unknown alarm {}", code));
                            let _ = ws_tx_for_bridge.send(
                                rustcnc_core::ws_protocol::ServerMessage::Alarm(
                                    rustcnc_core::ws_protocol::AlarmNotification {
                                        code: *code,
                                        message: msg,
                                    },
                                ),
                            );
                        }
                        _ => {}
                    }
                    // Forward to planner (blocking_send from non-async context)
                    let _ = streamer_event_bridge_tx.blocking_send(event);
                }
                Err(_) => break, // Channel closed
            }
        }
    });

    // 5. Create serial port (simulator or hardware)
    let serial: Box<dyn rustcnc_streamer::serial::SerialPort>;
    if config.simulator.enabled {
        info!("Starting GRBL simulator");
        let sim = GrblSimulator::new(SimulatorConfig {
            rx_buffer_size: config.streamer.rx_buffer_size,
            motion_speed_factor: config.simulator.motion_speed_factor,
            startup_delay_ms: config.simulator.startup_delay_ms,
            ..Default::default()
        });
        serial = Box::new(sim.start());
    } else if let Some(ref port) = config.serial.default_port {
        info!("Connecting to serial port: {} @ {}", port, config.serial.baud_rate);
        serial = Box::new(
            rustcnc_streamer::serial::HardwareSerialPort::open(port, config.serial.baud_rate)
                .map_err(|e| anyhow::anyhow!("Failed to open serial port: {}", e))?,
        );
    } else {
        info!("No serial port configured and simulator disabled");
        info!("Use --simulator flag or set serial.default_port in config");
        info!("Starting in disconnected mode");
        // Start with simulator anyway as a fallback
        let sim = GrblSimulator::new(SimulatorConfig::default());
        serial = Box::new(sim.start());
    }

    // 6. Spawn streamer thread (dedicated OS thread)
    let streamer_shared = shared_state.clone();
    let streamer_config = StreamerConfig {
        rx_buffer_size: config.streamer.rx_buffer_size,
        cpu_pin_core: config.streamer.cpu_pin_core,
        rt_priority: config.streamer.rt_priority,
        status_poll_interval: Duration::from_millis(
            1000 / config.serial.status_poll_rate_hz.max(1) as u64,
        ),
    };
    std::thread::Builder::new()
        .name("rustcnc-streamer".into())
        .spawn(move || {
            rustcnc_streamer::streamer::streamer_thread_main(
                serial,
                streamer_cmd_rx,
                streamer_event_tx,
                streamer_shared,
                streamer_config,
            );
        })?;

    // 7. Spawn planner task
    let planner_streamer_tx = streamer_cmd_tx.clone();
    tokio::spawn(async move {
        rustcnc_planner::planner::planner_task(
            planner_cmd_rx,
            planner_event_tx,
            planner_streamer_tx,
            streamer_event_tokio_rx,
        )
        .await;
    });

    // 8. Handle planner events (update shared state for web server)
    let ws_tx_for_planner = ws_broadcast_tx.clone();
    tokio::spawn(async move {
        while let Some(event) = planner_event_rx.recv().await {
            match event {
                PlannerEvent::JobProgress {
                    current_line,
                    total_lines,
                    elapsed_secs,
                } => {
                    let percent = if total_lines > 0 {
                        (current_line as f32 / total_lines as f32) * 100.0
                    } else {
                        0.0
                    };
                    let progress = rustcnc_core::ws_protocol::JobProgress {
                        file_name: String::new(),
                        current_line,
                        total_lines,
                        percent_complete: percent,
                        elapsed_secs,
                        estimated_remaining_secs: None,
                        state: rustcnc_core::job::JobState::Running,
                    };
                    let _ = ws_tx_for_planner.send(
                        rustcnc_core::ws_protocol::ServerMessage::JobProgress(progress),
                    );
                }
                PlannerEvent::JobStateChanged(state) => {
                    info!("Job state: {:?}", state);
                }
                PlannerEvent::FileLoaded { file } => {
                    info!("File ready: {} ({} lines)", file.name, file.total_lines);
                }
                PlannerEvent::FileError(msg) => {
                    let _ = ws_tx_for_planner.send(
                        rustcnc_core::ws_protocol::ServerMessage::Error(
                            rustcnc_core::ws_protocol::ErrorNotification {
                                code: None,
                                message: msg,
                                source: "planner".into(),
                            },
                        ),
                    );
                }
            }
        }
    });

    // 9. Spawn WebSocket broadcaster
    let broadcaster_shared = shared_state.clone();
    let broadcaster_tx = ws_broadcast_tx.clone();
    let ws_tick_rate_hz = config.server.ws_tick_rate_hz;
    let ws_idle_tick_rate_hz = config.server.ws_idle_tick_rate_hz;
    tokio::spawn(async move {
        ws::broadcaster::broadcaster_task(
            broadcaster_shared,
            broadcaster_tx,
            ws_tick_rate_hz,
            ws_idle_tick_rate_hz,
        )
        .await;
    });

    // 10. Build application state
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    let app_state = Arc::new(AppState {
        machine_state: shared_state,
        planner_tx: planner_cmd_tx,
        streamer_cmd_tx,
        ws_broadcast_tx,
        files: RwLock::new(Vec::new()),
        job_progress: RwLock::new(None),
        config,
        connection_port: RwLock::new(None),
    });

    // 11. Build Axum router and start server
    let router = app::build_router(app_state);

    info!("RustCNC listening on http://{}", addr);
    info!("WebSocket endpoint: ws://{}/ws", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown::shutdown_signal())
        .await?;

    info!("RustCNC shutdown complete");
    Ok(())
}

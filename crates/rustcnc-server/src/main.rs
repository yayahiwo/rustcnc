use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use parking_lot::RwLock;
use tokio::sync::{broadcast, mpsc};
use tracing::{info, warn};

use rustcnc_core::config::AppConfig;
use rustcnc_planner::planner::{PlannerCommand, PlannerEvent};
use rustcnc_streamer::streamer::{
    SharedMachineState, StreamerCommand, StreamerConfig, StreamerEvent,
};

mod api;
mod app;
mod auth;
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
    // Install panic hook that force-flushes to stderr so crash messages
    // are written to the log file even on sudden termination.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC: {}", info);
        if let Some(bt) = std::backtrace::Backtrace::force_capture()
            .to_string()
            .strip_prefix("")
        {
            eprintln!("{}", bt);
        } else {
            eprintln!("{}", std::backtrace::Backtrace::force_capture());
        }
        // Force flush
        use std::io::Write;
        let _ = std::io::stderr().flush();
    }));

    let cli = Cli::parse();

    if cli.hash_password_stdin {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        let password = buf.trim_end_matches(['\r', '\n']).to_string();
        let encoded = auth::hash_password(&password)?;
        println!("{}", encoded);
        return Ok(());
    }

    // 1. Load configuration (needed for logging config)
    let mut config = match AppConfig::load(&cli.config) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!(
                "Failed to load config from {}: {} (using defaults)",
                cli.config, e
            );
            AppConfig::default()
        }
    };
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

    // 2. Initialize logging
    let _logging_guards = logging::init_logging(&config.logging, cli.log_level.as_deref())?;

    if config.auth.enabled {
        let user_ok = config
            .auth
            .username
            .as_deref()
            .is_some_and(|u| !u.trim().is_empty());
        let hash_ok = config
            .auth
            .password_hash
            .as_deref()
            .is_some_and(|h| !h.trim().is_empty());
        if !user_ok || !hash_ok {
            anyhow::bail!(
                "auth.enabled=true but auth.username/auth.password_hash are not set. Run: printf '%s' 'PASSWORD' | rustcnc --hash-password-stdin"
            );
        }

        let clamped = config.auth.session_ttl_secs.clamp(60, 7 * 24 * 60 * 60);
        if clamped != config.auth.session_ttl_secs {
            warn!(
                "Clamping auth.session_ttl_secs from {} to {}",
                config.auth.session_ttl_secs, clamped
            );
            config.auth.session_ttl_secs = clamped;
        }
    }

    info!("RustCNC v{} starting", env!("CARGO_PKG_VERSION"));

    // 3. Create shared state
    let shared_state = SharedMachineState::new();

    // 4. Create channels between zones
    let (streamer_cmd_tx, streamer_cmd_rx) = crossbeam_channel::unbounded::<StreamerCommand>();
    let (streamer_event_tx, streamer_event_rx_raw) =
        crossbeam_channel::unbounded::<StreamerEvent>();
    let (planner_cmd_tx, planner_cmd_rx) = mpsc::channel::<PlannerCommand>(64);
    let (planner_event_tx, mut planner_event_rx) = mpsc::channel::<PlannerEvent>(1024);
    let (ws_broadcast_tx, _) = broadcast::channel::<rustcnc_core::ws_protocol::ServerMessage>(256);

    // Shared firmware welcome string (set by bridge on Welcome event)
    let firmware_welcome: Arc<parking_lot::RwLock<Option<String>>> =
        Arc::new(parking_lot::RwLock::new(None));
    let firmware_welcome_for_bridge = firmware_welcome.clone();
    let connection_started_at: Arc<parking_lot::RwLock<Option<std::time::Instant>>> =
        Arc::new(parking_lot::RwLock::new(None));
    let connection_started_at_for_bridge = connection_started_at.clone();

    let connection_port: Arc<parking_lot::RwLock<Option<String>>> =
        Arc::new(parking_lot::RwLock::new(None));
    let connection_port_for_bridge = connection_port.clone();

    // Shared grblHAL $I build info (populated by parsing bracket responses)
    let grbl_build_info: Arc<parking_lot::RwLock<HashMap<String, String>>> =
        Arc::new(parking_lot::RwLock::new(HashMap::new()));
    let grbl_info_for_bridge = grbl_build_info.clone();

    // Bridge: crossbeam streamer events -> tokio mpsc for planner
    let (streamer_event_tokio_tx, streamer_event_tokio_rx) = mpsc::channel::<StreamerEvent>(256);
    let streamer_event_bridge_tx = streamer_event_tokio_tx.clone();
    let ws_tx_for_bridge = ws_broadcast_tx.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(event) = streamer_event_rx_raw.recv() {
            // Track connection state
            match &event {
                StreamerEvent::Connected { port, .. } => {
                    *connection_port_for_bridge.write() = Some(port.clone());
                    *connection_started_at_for_bridge.write() = Some(std::time::Instant::now());
                    *firmware_welcome_for_bridge.write() = None;
                    grbl_info_for_bridge.write().clear();
                    let _ = ws_tx_for_bridge.send(
                        rustcnc_core::ws_protocol::ServerMessage::ConnectionChanged(
                            rustcnc_core::ws_protocol::ConnectionState {
                                connected: true,
                                port: Some(port.clone()),
                                firmware: None,
                                version: None,
                            },
                        ),
                    );
                }
                StreamerEvent::Disconnected => {
                    *connection_port_for_bridge.write() = None;
                    *connection_started_at_for_bridge.write() = None;
                    *firmware_welcome_for_bridge.write() = None;
                    grbl_info_for_bridge.write().clear();
                    let _ = ws_tx_for_bridge.send(
                        rustcnc_core::ws_protocol::ServerMessage::ConnectionChanged(
                            rustcnc_core::ws_protocol::ConnectionState {
                                connected: false,
                                port: None,
                                firmware: None,
                                version: None,
                            },
                        ),
                    );
                }
                _ => {}
            }

            // Capture firmware welcome string
            if let StreamerEvent::Welcome { ref version } = event {
                *firmware_welcome_for_bridge.write() = Some(version.clone());
                let port = connection_port_for_bridge.read().clone();
                let mut parts = version.split_whitespace();
                let firmware = parts.next().map(|s| s.to_string());
                let ver = parts.next().map(|s| s.to_string());
                let _ = ws_tx_for_bridge.send(
                    rustcnc_core::ws_protocol::ServerMessage::ConnectionChanged(
                        rustcnc_core::ws_protocol::ConnectionState {
                            connected: true,
                            port,
                            firmware,
                            version: ver,
                        },
                    ),
                );
            }
            // Parse bracket responses from $I query (e.g. [BOARD:T41U5XBB])
            if let StreamerEvent::ConsoleOutput { ref text, is_tx } = event {
                if !is_tx {
                    if let Some(inner) = text.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                        if let Some((key, value)) = inner.split_once(':') {
                            let key = key.trim().to_uppercase();
                            let value = value.trim().trim_end_matches(':').to_string();
                            if matches!(
                                key.as_str(),
                                "VER"
                                    | "OPT"
                                    | "NEWOPT"
                                    | "FIRMWARE"
                                    | "BOARD"
                                    | "DRIVER"
                                    | "DRIVER VERSION"
                                    | "DRIVER OPTIONS"
                                    | "NVS STORAGE"
                                    | "AUX INPUTS"
                                    | "AUX OUTPUTS"
                            ) {
                                info!("grblHAL info: {} = {}", key, value);
                                grbl_info_for_bridge.write().insert(key, value);
                            }
                        }
                    }
                }
            }
            // Forward console output and status events to WebSocket
            let console_entry = match &event {
                StreamerEvent::ConsoleOutput { text, is_tx } => Some((
                    if *is_tx {
                        rustcnc_core::ws_protocol::ConsoleDirection::Sent
                    } else {
                        rustcnc_core::ws_protocol::ConsoleDirection::Received
                    },
                    text.clone(),
                )),
                StreamerEvent::Setting { key, value } => {
                    let id: u16 = key.parse().unwrap_or(u16::MAX);
                    let (desc, unit) = rustcnc_core::grbl::settings::setting_description(id);
                    if unit.is_empty() {
                        Some((
                            rustcnc_core::ws_protocol::ConsoleDirection::Received,
                            format!("${}={} ({})", key, value, desc),
                        ))
                    } else {
                        Some((
                            rustcnc_core::ws_protocol::ConsoleDirection::Received,
                            format!("${}={} ({}, {})", key, value, desc, unit),
                        ))
                    }
                }
                StreamerEvent::Message(msg) => Some((
                    rustcnc_core::ws_protocol::ConsoleDirection::Received,
                    format!("[MSG:{}]", msg),
                )),
                StreamerEvent::ParserState(state) => Some((
                    rustcnc_core::ws_protocol::ConsoleDirection::Received,
                    format!("[GC:{}]", state),
                )),
                StreamerEvent::Welcome { version } => Some((
                    rustcnc_core::ws_protocol::ConsoleDirection::Received,
                    version.clone(),
                )),
                StreamerEvent::LineError {
                    line_number,
                    code,
                    message,
                } => Some((
                    rustcnc_core::ws_protocol::ConsoleDirection::Received,
                    format!("error:{} on line {} ({})", code, line_number, message),
                )),
                _ => None,
            };
            if let Some((direction, text)) = console_entry {
                let _ =
                    ws_tx_for_bridge.send(rustcnc_core::ws_protocol::ServerMessage::ConsoleOutput(
                        rustcnc_core::ws_protocol::ConsoleEntry {
                            direction,
                            text,
                            timestamp: chrono::Utc::now().timestamp_millis(),
                        },
                    ));
            }
            if let StreamerEvent::Alarm { code } = &event {
                let msg = rustcnc_core::grbl::error_codes::GrblAlarm::from_code(*code)
                    .map(|a| a.message().to_string())
                    .unwrap_or_else(|| format!("Unknown alarm {}", code));
                let _ = ws_tx_for_bridge.send(rustcnc_core::ws_protocol::ServerMessage::Alarm(
                    rustcnc_core::ws_protocol::AlarmNotification {
                        code: *code,
                        message: msg,
                    },
                ));
            }
            // Forward only job-relevant events to the planner to avoid
            // filling the channel with high-frequency status reports.
            let forward_to_planner = matches!(
                event,
                StreamerEvent::LineAcknowledged { .. }
                    | StreamerEvent::LineError { .. }
                    | StreamerEvent::Alarm { .. }
                    | StreamerEvent::Welcome { .. }
                    | StreamerEvent::Disconnected
                    | StreamerEvent::Exited
            );
            if forward_to_planner {
                // blocking_send from non-async context
                let _ = streamer_event_bridge_tx.blocking_send(event);
            }
        }
    });

    // 5. Spawn streamer thread (dedicated OS thread)
    let streamer_shared = shared_state.clone();
    let status_poll_rate_hz = config.serial.status_poll_rate_hz.clamp(1, 1000);
    if status_poll_rate_hz != config.serial.status_poll_rate_hz {
        warn!(
            "Clamping serial.status_poll_rate_hz from {} to {}",
            config.serial.status_poll_rate_hz, status_poll_rate_hz
        );
    }
    let streamer_config = StreamerConfig {
        rx_buffer_size: config.streamer.rx_buffer_size,
        cpu_pin_core: config.streamer.cpu_pin_core,
        rt_priority: config.streamer.rt_priority,
        status_poll_interval: Duration::from_micros(1_000_000 / status_poll_rate_hz as u64),
    };
    std::thread::Builder::new()
        .name("rustcnc-streamer".into())
        .spawn(move || {
            rustcnc_streamer::streamer::streamer_thread_main(
                streamer_cmd_rx,
                streamer_event_tx,
                streamer_shared,
                streamer_config,
            );
        })?;

    // Optional auto-connect on startup (if configured)
    if let Some(port) = config.serial.default_port.clone() {
        let _ = streamer_cmd_tx.send(StreamerCommand::Connect {
            port,
            baud_rate: config.serial.baud_rate,
        });
    }

    // 6. Spawn planner task
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

    // 7. Handle planner events (update shared state for web server)
    let loaded_gcode: Arc<parking_lot::RwLock<Option<rustcnc_core::ws_protocol::GCodeFileInfo>>> =
        Arc::new(parking_lot::RwLock::new(None));
    let loaded_gcode_for_planner = loaded_gcode.clone();
    let shared_job_progress: Arc<
        parking_lot::RwLock<Option<rustcnc_core::ws_protocol::JobProgress>>,
    > = Arc::new(parking_lot::RwLock::new(None));
    let job_progress_for_planner = shared_job_progress.clone();
    let ws_tx_for_planner = ws_broadcast_tx.clone();
    tokio::spawn(async move {
        let mut last_progress = rustcnc_core::ws_protocol::JobProgress {
            file_name: String::new(),
            current_line: 0,
            total_lines: 0,
            percent_complete: 0.0,
            elapsed_secs: 0.0,
            estimated_remaining_secs: None,
            state: rustcnc_core::job::JobState::Idle,
        };
        while let Some(event) = planner_event_rx.recv().await {
            match event {
                PlannerEvent::JobProgress {
                    current_line,
                    total_lines,
                    elapsed_secs,
                    estimated_remaining_secs,
                } => {
                    let percent = if total_lines > 0 {
                        (current_line as f32 / total_lines as f32) * 100.0
                    } else {
                        0.0
                    };
                    last_progress.current_line = current_line;
                    last_progress.total_lines = total_lines;
                    last_progress.percent_complete = percent;
                    last_progress.elapsed_secs = elapsed_secs;
                    last_progress.estimated_remaining_secs = estimated_remaining_secs;
                    *job_progress_for_planner.write() = Some(last_progress.clone());
                    let _ = ws_tx_for_planner.send(
                        rustcnc_core::ws_protocol::ServerMessage::JobProgress(
                            last_progress.clone(),
                        ),
                    );
                }
                PlannerEvent::JobStateChanged(state) => {
                    info!("Job state: {:?}", state);
                    last_progress.state = state;
                    // Reset progress counters when job reaches a terminal state
                    if state == rustcnc_core::job::JobState::Cancelled
                        || state == rustcnc_core::job::JobState::Error
                    {
                        last_progress.current_line = 0;
                        last_progress.total_lines = 0;
                        last_progress.percent_complete = 0.0;
                        last_progress.elapsed_secs = 0.0;
                        last_progress.estimated_remaining_secs = None;
                    }
                    *job_progress_for_planner.write() = Some(last_progress.clone());
                    let _ = ws_tx_for_planner.send(
                        rustcnc_core::ws_protocol::ServerMessage::JobProgress(
                            last_progress.clone(),
                        ),
                    );
                }
                PlannerEvent::FileLoaded { file } => {
                    info!("File ready: {} ({} lines)", file.name, file.total_lines);
                    last_progress.file_name = file.name.clone();
                    let gcode_info = rustcnc_core::ws_protocol::GCodeFileInfo {
                        id: file.id.to_string(),
                        name: file.name.clone(),
                        lines: file
                            .lines
                            .iter()
                            .map(|l| rustcnc_core::ws_protocol::GCodeLineInfo {
                                line_num: l.file_line,
                                text: l.text.clone(),
                                move_type: l.move_type.as_ref().map(|m| format!("{:?}", m)),
                                endpoint: l.endpoint.clone(),
                                arc: l.arc.as_ref().map(|a| {
                                    rustcnc_core::ws_protocol::ArcDataInfo {
                                        i: a.i,
                                        j: a.j,
                                        k: a.k,
                                        plane: a.plane,
                                    }
                                }),
                            })
                            .collect(),
                        bounding_box: file
                            .bounding_box
                            .as_ref()
                            .map(|bb| [bb.min.clone(), bb.max.clone()]),
                    };
                    info!(
                        "Cached GCodeLoaded for reconnect: {} ({} lines)",
                        gcode_info.name,
                        gcode_info.lines.len()
                    );
                    *loaded_gcode_for_planner.write() = Some(gcode_info.clone());
                    let _ = ws_tx_for_planner.send(
                        rustcnc_core::ws_protocol::ServerMessage::GCodeLoaded(gcode_info),
                    );
                }
                PlannerEvent::FileError(msg) => {
                    let _ =
                        ws_tx_for_planner.send(rustcnc_core::ws_protocol::ServerMessage::Error(
                            rustcnc_core::ws_protocol::ErrorNotification {
                                code: None,
                                message: msg,
                                source: "planner".into(),
                            },
                        ));
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

    // 10. Pi throttle/undervoltage monitor (reads sysfs every 5s, only sends on change)
    let throttle_tx = ws_broadcast_tx.clone();
    tokio::spawn(async move {
        let path = "/sys/devices/platform/soc/soc:firmware/get_throttled";
        let mut last_alert: Option<String> = None;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let alert = match tokio::fs::read_to_string(path).await {
                Ok(content) => {
                    let val = u32::from_str_radix(content.trim().trim_start_matches("0x"), 16)
                        .unwrap_or(0);
                    // Bit 0: under-voltage now, Bit 1: freq capped now, Bit 2: throttled now
                    let mut warnings = Vec::new();
                    if val & (1 << 0) != 0 {
                        warnings.push("Undervoltage");
                    }
                    if val & (1 << 2) != 0 {
                        warnings.push("Throttled");
                    }
                    if val & (1 << 1) != 0 {
                        warnings.push("Freq capped");
                    }
                    if warnings.is_empty() {
                        None
                    } else {
                        Some(warnings.join(" | "))
                    }
                }
                Err(_) => None, // Not a Pi or no permission — silently skip
            };
            if alert != last_alert {
                let _ = throttle_tx.send(rustcnc_core::ws_protocol::ServerMessage::SystemAlert(
                    alert.clone(),
                ));
                last_alert = alert;
            }
        }
    });

    // 11. System info broadcaster (reads Pi stats every 5s)
    let sysinfo_tx = ws_broadcast_tx.clone();
    let sysinfo_firmware = firmware_welcome.clone();
    let sysinfo_grbl = grbl_build_info.clone();
    let sysinfo_port = connection_port.clone();
    let sysinfo_started_at = connection_started_at.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let fw = sysinfo_firmware.read().clone();
            let grbl = sysinfo_grbl.read().clone();
            let conn_secs = sysinfo_started_at
                .read()
                .as_ref()
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);
            let port = sysinfo_port.read().clone();
            if let Some(mut info) = read_system_info().await {
                info.firmware_version = fw;
                info.serial_port = port;
                info.connection_uptime_secs = conn_secs;
                info.grbl_info = grbl;
                let _ = sysinfo_tx.send(rustcnc_core::ws_protocol::ServerMessage::SystemInfo(info));
            }
        }
    });

    // 12. Build application state
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    let app_state = Arc::new(AppState {
        machine_state: shared_state,
        planner_tx: planner_cmd_tx,
        streamer_cmd_tx,
        ws_broadcast_tx,
        files: RwLock::new(Vec::new()),
        job_progress: shared_job_progress,
        loaded_gcode,
        config,
        sessions: RwLock::new(HashMap::new()),
        connection_port: connection_port.clone(),
    });

    // 13. Build Axum router and start server
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

/// Read system information from /proc and sysfs (Linux/Pi).
/// Returns None on non-Linux platforms.
async fn read_system_info() -> Option<rustcnc_core::ws_protocol::SystemInfoData> {
    // CPU load averages from /proc/loadavg
    let cpu_load = match tokio::fs::read_to_string("/proc/loadavg").await {
        Ok(s) => {
            let parts: Vec<f32> = s
                .split_whitespace()
                .take(3)
                .filter_map(|p| p.parse().ok())
                .collect();
            if parts.len() == 3 {
                [parts[0], parts[1], parts[2]]
            } else {
                return None;
            }
        }
        Err(_) => return None, // Not Linux
    };

    // Memory from /proc/meminfo
    let (memory_total_mb, memory_used_mb) = match tokio::fs::read_to_string("/proc/meminfo").await {
        Ok(s) => {
            let mut total_kb: u64 = 0;
            let mut available_kb: u64 = 0;
            for line in s.lines() {
                if line.starts_with("MemTotal:") {
                    total_kb = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                } else if line.starts_with("MemAvailable:") {
                    available_kb = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                }
            }
            (
                total_kb / 1024,
                (total_kb.saturating_sub(available_kb)) / 1024,
            )
        }
        Err(_) => (0, 0),
    };

    // Temperature from thermal zone
    let temperature_c = tokio::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .await
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|t| t / 1000.0);

    // Uptime from /proc/uptime
    let uptime_secs = tokio::fs::read_to_string("/proc/uptime")
        .await
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .map(|u| u as u64)
        .unwrap_or(0);

    // Disk usage via libc::statvfs
    let (disk_total_gb, disk_used_gb) = {
        let path = std::ffi::CString::new("/").unwrap();
        unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(path.as_ptr(), &mut stat) == 0 {
                let total =
                    (stat.f_blocks as f64) * (stat.f_frsize as f64) / (1024.0 * 1024.0 * 1024.0);
                let free =
                    (stat.f_bavail as f64) * (stat.f_frsize as f64) / (1024.0 * 1024.0 * 1024.0);
                (total, total - free)
            } else {
                (0.0, 0.0)
            }
        }
    };

    Some(rustcnc_core::ws_protocol::SystemInfoData {
        cpu_load,
        memory_total_mb,
        memory_used_mb,
        disk_total_gb,
        disk_used_gb,
        temperature_c,
        uptime_secs,
        firmware_version: None,
        serial_port: None,
        connection_uptime_secs: 0,
        grbl_info: HashMap::new(),
    })
}

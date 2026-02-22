use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tracing::{debug, info, warn};

use rustcnc_core::grbl::realtime::RealtimeCommand;
use rustcnc_core::ws_protocol::{
    ClientMessage, ConnectionState, ConsoleDirection, ConsoleEntry, ErrorNotification,
    FullStateSync, ServerMessage,
};
use rustcnc_planner::planner::PlannerCommand;
use rustcnc_streamer::streamer::StreamerCommand;

use crate::state::AppState;
use crate::ws::codec;

/// Handle WebSocket upgrade request
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    info!("WebSocket client connecting");
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a connected WebSocket client
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Send initial state sync
    let sync_msg = build_state_sync(&state);
    if let Ok(json) = codec::encode_server_message(&sync_msg) {
        let _ = ws_tx.send(Message::Text(json.into())).await;
    }

    // Re-send loaded G-code file (for 3D viewer on reconnect)
    let loaded_gcode = state.loaded_gcode.read().clone();
    if let Some(gcode_info) = loaded_gcode {
        info!(
            "Sending cached GCodeLoaded on reconnect: {} ({} lines)",
            gcode_info.name,
            gcode_info.lines.len()
        );
        let msg = ServerMessage::GCodeLoaded(gcode_info);
        if let Ok(json) = codec::encode_server_message(&msg) {
            let _ = ws_tx.send(Message::Text(json.into())).await;
        }
    } else {
        info!("No cached GCodeLoaded to send on reconnect");
    }

    // Subscribe to broadcast channel
    let mut broadcast_rx = state.ws_broadcast_tx.subscribe();

    // Per-client direct message channel (for Ping/Pong, PortList, etc.)
    let (direct_tx, mut direct_rx) = tokio::sync::mpsc::channel::<ServerMessage>(32);

    // Task 1: Forward broadcast and direct messages to this WS client
    let mut send_task = tokio::spawn(async move {
        loop {
            let msg = tokio::select! {
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(msg) => msg,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            debug!("WebSocket client lagged by {} messages", n);
                            continue;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                Some(msg) = direct_rx.recv() => msg,
            };
            if let Ok(json) = codec::encode_server_message(&msg) {
                if ws_tx.send(Message::Text(json.into())).await.is_err() {
                    break; // Client disconnected
                }
            }
        }
    });

    // Task 2: Receive commands from this WS client
    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => match codec::decode_client_message(&text) {
                    Ok(client_msg) => {
                        handle_client_message(client_msg, &state_clone, &direct_tx).await;
                    }
                    Err(e) => {
                        debug!(
                            "Invalid WebSocket message: {} | raw: {}",
                            e,
                            &text[..text.len().min(200)]
                        );
                    }
                },
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish, then abort the other
    tokio::select! {
        _ = &mut send_task => { recv_task.abort(); }
        _ = &mut recv_task => { send_task.abort(); }
    }

    info!("WebSocket client disconnected");
}

/// Handle a decoded client message
async fn handle_client_message(
    msg: ClientMessage,
    state: &AppState,
    direct_tx: &tokio::sync::mpsc::Sender<ServerMessage>,
) {
    let is_connected = state
        .machine_state
        .connected
        .load(std::sync::atomic::Ordering::Acquire);

    match msg {
        ClientMessage::RealtimeCommand(cmd_msg) => {
            if !is_connected {
                let _ = direct_tx
                    .send(ServerMessage::Error(ErrorNotification {
                        code: None,
                        message: "Not connected to a controller".into(),
                        source: "ws".into(),
                    }))
                    .await;
                return;
            }
            if let Some(cmd) = RealtimeCommand::from_str_name(&cmd_msg.command) {
                info!("RT command received: {} -> {:?}", cmd_msg.command, cmd);
                let _ = state.streamer_cmd_tx.send(StreamerCommand::Realtime(cmd));
            } else {
                warn!("Unknown RT command: {}", cmd_msg.command);
            }
        }
        ClientMessage::Jog(jog) => {
            if !is_connected {
                let _ = direct_tx
                    .send(ServerMessage::Error(ErrorNotification {
                        code: None,
                        message: "Not connected to a controller".into(),
                        source: "ws".into(),
                    }))
                    .await;
                return;
            }
            let grbl_cmd = jog.to_grbl_command();
            let _ = state
                .streamer_cmd_tx
                .send(StreamerCommand::RawCommand(grbl_cmd));
        }
        ClientMessage::ConsoleSend(line) => {
            if !is_connected {
                let _ = direct_tx
                    .send(ServerMessage::Error(ErrorNotification {
                        code: None,
                        message: "Not connected to a controller".into(),
                        source: "ws".into(),
                    }))
                    .await;
                return;
            }
            let _ = state
                .planner_tx
                .send(PlannerCommand::SendCommand(line))
                .await;
        }
        ClientMessage::JobControl(action) => {
            use rustcnc_core::ws_protocol::JobControlAction;
            let cmd = match action {
                JobControlAction::Start {
                    start_line,
                    stop_line,
                } => {
                    if !is_connected {
                        let _ = direct_tx
                            .send(ServerMessage::Error(ErrorNotification {
                                code: None,
                                message: "Not connected to a controller".into(),
                                source: "ws".into(),
                            }))
                            .await;
                        return;
                    }
                    PlannerCommand::StartJob {
                        start_line,
                        stop_line,
                    }
                }
                JobControlAction::Pause => PlannerCommand::PauseJob,
                JobControlAction::Resume => PlannerCommand::ResumeJob,
                JobControlAction::Stop => PlannerCommand::CancelJob,
            };
            let _ = state.planner_tx.send(cmd).await;
        }
        ClientMessage::Connect(req) => {
            debug!("WS Connect request: {}@{}", req.port, req.baud_rate);
            let _ = direct_tx
                .send(ServerMessage::ConsoleOutput(ConsoleEntry {
                    direction: ConsoleDirection::System,
                    text: format!("Connecting to {} @ {}", req.port, req.baud_rate),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                }))
                .await;
            let _ = state.streamer_cmd_tx.send(StreamerCommand::Connect {
                port: req.port,
                baud_rate: req.baud_rate,
            });
        }
        ClientMessage::Disconnect => {
            info!("Disconnect requested via WS");
            let _ = direct_tx
                .send(ServerMessage::ConsoleOutput(ConsoleEntry {
                    direction: ConsoleDirection::System,
                    text: "Disconnecting...".into(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                }))
                .await;
            let _ = state.streamer_cmd_tx.send(StreamerCommand::Disconnect);
        }
        ClientMessage::RequestSync => {
            // State sync is already sent on connect
            // Could broadcast again for this specific client
        }
        ClientMessage::RequestPortList => {
            let ports = rustcnc_streamer::serial::list_ports();
            let port_infos: Vec<_> = ports
                .iter()
                .map(|p| rustcnc_core::ws_protocol::PortInfo {
                    path: p.port_name.clone(),
                    manufacturer: match &p.port_type {
                        serialport::SerialPortType::UsbPort(info) => info.manufacturer.clone(),
                        _ => None,
                    },
                    product: match &p.port_type {
                        serialport::SerialPortType::UsbPort(info) => info.product.clone(),
                        _ => None,
                    },
                })
                .collect();
            // Send directly to requesting client, not broadcast
            let _ = direct_tx.send(ServerMessage::PortList(port_infos)).await;
        }
        ClientMessage::Ping => {
            // Send Pong directly to requesting client, not broadcast
            let _ = direct_tx.send(ServerMessage::Pong).await;
        }
        ClientMessage::SchedulePause(cond) => {
            let _ = state
                .planner_tx
                .send(PlannerCommand::SchedulePause(cond))
                .await;
        }
    }
}

/// Build a full state sync message for new client connections
fn build_state_sync(state: &AppState) -> ServerMessage {
    let machine = crate::ws::broadcaster::read_snapshot_pub(&state.machine_state);
    let port = state.connection_port.read().clone();

    ServerMessage::StateSync(Box::new(FullStateSync {
        machine,
        connection: ConnectionState {
            connected: state
                .machine_state
                .connected
                .load(std::sync::atomic::Ordering::Relaxed),
            port,
            firmware: None,
            version: None,
        },
        job: state.job_progress.read().clone(),
        files: state
            .files
            .read()
            .iter()
            .map(|f| rustcnc_core::ws_protocol::FileInfo {
                id: f.id.to_string(),
                name: f.name.clone(),
                size_bytes: f.size_bytes,
                line_count: f.line_count,
                loaded_at: f.uploaded_at.to_rfc3339(),
            })
            .collect(),
    }))
}

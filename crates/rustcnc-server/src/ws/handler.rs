use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tracing::{debug, error, info, warn};

use rustcnc_core::grbl::realtime::RealtimeCommand;
use rustcnc_core::ws_protocol::{
    ClientMessage, ConnectionState, FullStateSync, ServerMessage,
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

    // Subscribe to broadcast channel
    let mut broadcast_rx = state.ws_broadcast_tx.subscribe();

    // Task 1: Forward broadcast messages to this WS client
    let send_task = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(msg) => {
                    if let Ok(json) = codec::encode_server_message(&msg) {
                        if ws_tx.send(Message::Text(json.into())).await.is_err() {
                            break; // Client disconnected
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    debug!("WebSocket client lagged by {} messages", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Task 2: Receive commands from this WS client
    let state_clone = state.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => {
                    match codec::decode_client_message(&text) {
                        Ok(client_msg) => {
                            handle_client_message(client_msg, &state_clone).await;
                        }
                        Err(e) => {
                            warn!("Invalid WebSocket message: {}", e);
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish (client disconnect)
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    info!("WebSocket client disconnected");
}

/// Handle a decoded client message
async fn handle_client_message(msg: ClientMessage, state: &AppState) {
    match msg {
        ClientMessage::RealtimeCommand(cmd_msg) => {
            if let Some(cmd) = RealtimeCommand::from_str_name(&cmd_msg.command) {
                let _ = state
                    .streamer_cmd_tx
                    .send(StreamerCommand::Realtime(cmd));
            } else {
                warn!("Unknown RT command: {}", cmd_msg.command);
            }
        }
        ClientMessage::Jog(jog) => {
            let grbl_cmd = jog.to_grbl_command();
            let _ = state
                .streamer_cmd_tx
                .send(StreamerCommand::RawCommand(grbl_cmd));
        }
        ClientMessage::ConsoleSend(line) => {
            let _ = state
                .planner_tx
                .send(PlannerCommand::SendCommand(line))
                .await;
        }
        ClientMessage::JobControl(action) => {
            use rustcnc_core::ws_protocol::JobControlAction;
            let cmd = match action {
                JobControlAction::Start => PlannerCommand::StartJob,
                JobControlAction::Pause => PlannerCommand::PauseJob,
                JobControlAction::Resume => PlannerCommand::ResumeJob,
                JobControlAction::Stop => PlannerCommand::CancelJob,
            };
            let _ = state.planner_tx.send(cmd).await;
        }
        ClientMessage::Connect(req) => {
            // Connection management handled via REST API for now
            debug!("WS Connect request: {}@{}", req.port, req.baud_rate);
        }
        ClientMessage::Disconnect => {
            debug!("WS Disconnect request");
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
                        serialport::SerialPortType::UsbPort(info) => {
                            info.manufacturer.clone()
                        }
                        _ => None,
                    },
                    product: match &p.port_type {
                        serialport::SerialPortType::UsbPort(info) => {
                            info.product.clone()
                        }
                        _ => None,
                    },
                })
                .collect();
            let _ = state
                .ws_broadcast_tx
                .send(ServerMessage::PortList(port_infos));
        }
        ClientMessage::Ping => {
            let _ = state.ws_broadcast_tx.send(ServerMessage::Pong);
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
                size_bytes: f.lines.iter().map(|l| l.byte_len as u64).sum(),
                line_count: f.total_lines,
                loaded_at: f.loaded_at.to_rfc3339(),
            })
            .collect(),
    }))
}

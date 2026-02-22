use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use tracing::warn;

use rustcnc_streamer::streamer::StreamerCommand;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct ConnectRequest {
    pub port: String,
    pub baud_rate: Option<u32>,
}

#[derive(Serialize)]
pub struct ConnectResponse {
    pub connected: bool,
    pub port: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct PortInfoResponse {
    pub path: String,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

/// POST /api/connect
///
/// TODO: Implement dynamic serial port connection. Requires sending a
/// connect command to the streamer thread so it can open a new serial port
/// at runtime, or restarting the streamer with the new port configuration.
pub async fn connect(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, StatusCode> {
    let baud_rate = req.baud_rate.unwrap_or(state.config.serial.baud_rate);
    let port = req.port.clone();
    let _ = state.streamer_cmd_tx.send(StreamerCommand::Connect {
        port: port.clone(),
        baud_rate,
    });
    warn!("Connect requested via REST: {} @ {}", port, baud_rate);
    Ok(Json(ConnectResponse {
        connected: state
            .machine_state
            .connected
            .load(std::sync::atomic::Ordering::Acquire),
        port,
        message: "Connect requested".into(),
    }))
}

/// POST /api/disconnect
///
/// TODO: Implement dynamic serial port disconnection. Requires sending a
/// disconnect command to the streamer thread so it can close the current
/// serial port gracefully.
pub async fn disconnect(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let _ = state.streamer_cmd_tx.send(StreamerCommand::Disconnect);
    warn!("Disconnect requested via REST");
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /api/ports
pub async fn list_ports(State(_state): State<Arc<AppState>>) -> Json<Vec<PortInfoResponse>> {
    let ports = rustcnc_streamer::serial::list_ports();
    let response: Vec<PortInfoResponse> = ports
        .iter()
        .map(|p| {
            let (manufacturer, product) = match &p.port_type {
                serialport::SerialPortType::UsbPort(info) => {
                    (info.manufacturer.clone(), info.product.clone())
                }
                _ => (None, None),
            };
            PortInfoResponse {
                path: p.port_name.clone(),
                manufacturer,
                product,
            }
        })
        .collect();
    Json(response)
}

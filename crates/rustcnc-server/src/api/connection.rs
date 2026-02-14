use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use tracing::warn;

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
    State(_state): State<Arc<AppState>>,
    Json(req): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, StatusCode> {
    warn!(
        "Connect endpoint not yet implemented - port: {} baud: {:?}",
        req.port, req.baud_rate
    );
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// POST /api/disconnect
///
/// TODO: Implement dynamic serial port disconnection. Requires sending a
/// disconnect command to the streamer thread so it can close the current
/// serial port gracefully.
pub async fn disconnect(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    warn!("Disconnect endpoint not yet implemented");
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// GET /api/ports
pub async fn list_ports(
    State(_state): State<Arc<AppState>>,
) -> Json<Vec<PortInfoResponse>> {
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

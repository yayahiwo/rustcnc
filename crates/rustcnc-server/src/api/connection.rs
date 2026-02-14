use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

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
pub async fn connect(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, StatusCode> {
    // Connection management will be implemented with dynamic serial port handling
    Ok(Json(ConnectResponse {
        connected: true,
        port: req.port,
        message: "Connection handling via WebSocket recommended".into(),
    }))
}

/// POST /api/disconnect
pub async fn disconnect(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({"disconnected": true})))
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

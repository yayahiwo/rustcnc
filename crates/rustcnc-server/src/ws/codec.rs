use rustcnc_core::ws_protocol::{ClientMessage, ServerMessage};

/// Encode a server message to JSON
pub fn encode_server_message(msg: &ServerMessage) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

/// Decode a client message from JSON
pub fn decode_client_message(text: &str) -> Result<ClientMessage, serde_json::Error> {
    serde_json::from_str(text)
}

use std::sync::Arc;

use axum::{
    routing::{delete, get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::api;
use crate::state::AppState;
use crate::static_files::static_handler;
use crate::ws::handler::ws_handler;

pub fn build_router(state: Arc<AppState>) -> Router {
    let api_routes = Router::new()
        // Connection management
        .route("/api/connect", post(api::connection::connect))
        .route("/api/disconnect", post(api::connection::disconnect))
        .route("/api/ports", get(api::connection::list_ports))
        // File management
        .route("/api/files", get(api::files::list_files))
        .route("/api/files", post(api::files::upload_file))
        .route("/api/files/{id}", delete(api::files::delete_file))
        .route("/api/files/{id}/load", post(api::files::load_file))
        // Job control
        .route("/api/job/start", post(api::machine::start_job))
        .route("/api/job/pause", post(api::machine::pause_job))
        .route("/api/job/resume", post(api::machine::resume_job))
        .route("/api/job/cancel", post(api::machine::cancel_job))
        // Machine commands
        .route("/api/machine/home", post(api::machine::home))
        .route("/api/machine/unlock", post(api::machine::unlock))
        .route("/api/machine/reset", post(api::machine::reset))
        .route("/api/machine/command", post(api::machine::send_command))
        // Settings
        .route("/api/settings", get(api::settings::get_settings))
        .route("/api/settings", post(api::settings::update_settings))
        .route("/api/settings/grbl", get(api::settings::get_grbl_settings))
        // System
        .route("/api/system/info", get(api::system::system_info));

    Router::new()
        // WebSocket endpoint
        .route("/ws", get(ws_handler))
        // API routes
        .merge(api_routes)
        // Static files (SolidJS frontend) -- fallback for SPA routing
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

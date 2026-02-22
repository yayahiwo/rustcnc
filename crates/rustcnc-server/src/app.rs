use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{header, uri::Authority, Method, Request, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Router,
};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::api;
use crate::auth;
use crate::state::AppState;
use crate::static_files::static_handler;
use crate::ws::handler::ws_handler;

fn is_default_port(scheme: Option<&str>, port: u16) -> bool {
    matches!((scheme, port), (Some("http"), 80) | (Some("https"), 443))
}

fn origin_allowed(origin: &str, host: &str, allowed_origins: &[String]) -> bool {
    if allowed_origins.iter().any(|o| o == origin) {
        return true;
    }

    let Ok(origin_uri) = origin.parse::<Uri>() else {
        return false;
    };
    let Some(origin_authority) = origin_uri.authority().map(|a| a.as_str()) else {
        return false;
    };

    let Ok(origin_auth) = origin_authority.parse::<Authority>() else {
        return origin_authority.eq_ignore_ascii_case(host);
    };
    let Ok(host_auth) = host.parse::<Authority>() else {
        return origin_authority.eq_ignore_ascii_case(host);
    };

    if !origin_auth.host().eq_ignore_ascii_case(host_auth.host()) {
        return false;
    }

    match (origin_auth.port_u16(), host_auth.port_u16()) {
        (Some(origin_port), Some(host_port)) => origin_port == host_port,
        (None, None) => true,
        (None, Some(host_port)) => is_default_port(origin_uri.scheme_str(), host_port),
        (Some(origin_port), None) => is_default_port(origin_uri.scheme_str(), origin_port),
    }
}

fn should_enforce_origin(req: &Request<Body>) -> bool {
    let path = req.uri().path();
    let is_ws = path == "/ws";
    let is_api = path.starts_with("/api/");
    let method = req.method();
    let is_unsafe = !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS);

    is_ws || (is_api && is_unsafe)
}

fn should_enforce_auth(req: &Request<Body>) -> bool {
    let path = req.uri().path();
    if path == "/ws" {
        return true;
    }
    if path.starts_with("/api/") {
        return !path.starts_with("/api/auth/");
    }
    false
}

fn get_cookie_value(req: &Request<Body>, name: &str) -> Option<String> {
    let header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    for part in header.split(';') {
        let part = part.trim();
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        if k == name {
            return Some(v.to_string());
        }
    }
    None
}

async fn enforce_same_origin(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !should_enforce_origin(&req) {
        return next.run(req).await;
    }

    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());

    let Some(origin) = origin else {
        return (
            StatusCode::FORBIDDEN,
            "Missing Origin header (cross-origin requests are not allowed)",
        )
            .into_response();
    };
    let Some(host) = host else {
        return (StatusCode::FORBIDDEN, "Missing Host header").into_response();
    };

    if origin_allowed(origin, host, &state.config.server.allowed_origins) {
        next.run(req).await
    } else {
        (
            StatusCode::FORBIDDEN,
            "Cross-origin request blocked (configure server.allowed_origins to override)",
        )
            .into_response()
    }
}

async fn enforce_auth(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !state.config.auth.enabled || !should_enforce_auth(&req) {
        return next.run(req).await;
    }

    if state.config.auth.username.is_none() || state.config.auth.password_hash.is_none() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Auth enabled but not configured (set auth.username and auth.password_hash)",
        )
            .into_response();
    }

    let Some(cookie_val) = get_cookie_value(&req, auth::SESSION_COOKIE_NAME) else {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };
    let Ok(session_id) = Uuid::parse_str(&cookie_val) else {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    };

    let ttl = state.config.auth.session_ttl_secs.min(7 * 24 * 60 * 60);
    let now = chrono::Utc::now();
    let ok = {
        let mut sessions = state.sessions.write();
        match sessions.get_mut(&session_id) {
            Some(sess) if sess.expires_at > now => {
                // Sliding expiration on activity
                sess.expires_at = now + chrono::Duration::seconds(ttl.try_into().unwrap_or(0));
                true
            }
            Some(_) => {
                sessions.remove(&session_id);
                false
            }
            None => false,
        }
    };

    if !ok {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    next.run(req).await
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let api_routes = Router::new()
        // Auth
        .route("/api/auth/status", get(api::auth::status))
        .route("/api/auth/login", post(api::auth::login))
        .route("/api/auth/logout", post(api::auth::logout))
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
        .layer(middleware::from_fn_with_state(state.clone(), enforce_auth))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            enforce_same_origin,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

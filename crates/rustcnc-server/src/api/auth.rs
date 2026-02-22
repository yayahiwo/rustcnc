use std::sync::Arc;

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use crate::{auth, state::AppState};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthStatusResponse {
    pub enabled: bool,
    pub authenticated: bool,
    pub username: Option<String>,
}

fn status_ok(enabled: bool, authenticated: bool, username: Option<String>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(AuthStatusResponse {
            enabled,
            authenticated,
            username,
        }),
    )
}

pub async fn status(State(state): State<Arc<AppState>>, jar: CookieJar) -> impl IntoResponse {
    if !state.config.auth.enabled {
        return status_ok(false, true, None);
    }

    let Some(cookie) = jar.get(auth::SESSION_COOKIE_NAME) else {
        return status_ok(true, false, None);
    };
    let Ok(session_id) = Uuid::parse_str(cookie.value()) else {
        return status_ok(true, false, None);
    };

    let now = chrono::Utc::now();
    let (authenticated, username) = {
        let mut sessions = state.sessions.write();
        match sessions.get(&session_id) {
            Some(sess) if sess.expires_at > now => (true, Some(sess.username.clone())),
            Some(_) => {
                sessions.remove(&session_id);
                (false, None)
            }
            None => (false, None),
        }
    };

    status_ok(true, authenticated, username)
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    if !state.config.auth.enabled {
        return (StatusCode::BAD_REQUEST, "Authentication is disabled").into_response();
    }
    let Some(expected_user) = state.config.auth.username.as_deref() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Auth enabled but auth.username is not set",
        )
            .into_response();
    };
    let Some(expected_hash) = state.config.auth.password_hash.as_deref() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Auth enabled but auth.password_hash is not set",
        )
            .into_response();
    };

    if req.username != expected_user {
        return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response();
    }

    match auth::verify_password(&req.password, expected_hash) {
        Ok(true) => {}
        Ok(false) => return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response(),
        Err(e) => {
            warn!("Failed to verify password hash: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Auth is misconfigured (invalid password hash)",
            )
                .into_response();
        }
    };

    let session_id = Uuid::new_v4();
    let ttl = state.config.auth.session_ttl_secs.min(7 * 24 * 60 * 60);
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl.try_into().unwrap_or(0));

    {
        let mut sessions = state.sessions.write();
        sessions.insert(
            session_id,
            crate::state::AuthSession {
                username: expected_user.to_string(),
                expires_at,
            },
        );
    }

    let cookie = Cookie::build((auth::SESSION_COOKIE_NAME, session_id.to_string()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .build();
    let jar = jar.add(cookie);

    (jar, status_ok(true, true, Some(expected_user.to_string()))).into_response()
}

pub async fn logout(State(state): State<Arc<AppState>>, jar: CookieJar) -> impl IntoResponse {
    if !state.config.auth.enabled {
        return status_ok(false, true, None).into_response();
    }

    let mut jar = jar;
    if let Some(cookie) = jar.get(auth::SESSION_COOKIE_NAME) {
        if let Ok(session_id) = Uuid::parse_str(cookie.value()) {
            state.sessions.write().remove(&session_id);
        }
    }

    jar = jar.remove(Cookie::from(auth::SESSION_COOKIE_NAME));
    (jar, status_ok(true, false, None)).into_response()
}

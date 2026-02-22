use axum::{
    http::{header, StatusCode, Uri},
    response::IntoResponse,
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../frontend/dist"]
struct FrontendAssets;

/// Serve embedded static files. Falls back to index.html for SPA routing.
pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try exact path first
    if !path.is_empty() {
        if let Some(file) = <FrontendAssets as Embed>::get(path) {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                file.data.into_owned(),
            )
                .into_response();
        }
    }

    // SPA fallback: serve index.html for any non-file path
    if let Some(file) = <FrontendAssets as Embed>::get("index.html") {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html".to_string())],
            file.data.into_owned(),
        )
            .into_response();
    }

    // No frontend built yet
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html".to_string())],
        b"<html><body><h1>RustCNC</h1><p>Frontend not built yet.</p></body></html>".to_vec(),
    )
        .into_response()
}

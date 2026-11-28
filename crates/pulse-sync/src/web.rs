//! Serves the web UI for every non-`/api` path.
//!
//! With the `embed-web` feature the built UI is compiled into the binary;
//! otherwise a short page points developers at the Vite dev server.

use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

#[cfg(feature = "embed-web")]
mod embedded {
    #[derive(rust_embed::RustEmbed)]
    #[folder = "../../web/dist"]
    pub struct Assets;
}

fn host_is_local(headers: &HeaderMap) -> bool {
    headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(pulse_api::is_local_host)
}

#[cfg(feature = "embed-web")]
pub async fn serve(
    uri: axum::http::Uri,
    headers: HeaderMap,
    token: std::sync::Arc<str>,
) -> Response {
    use embedded::Assets;

    if !host_is_local(&headers) {
        return (StatusCode::MISDIRECTED_REQUEST, "use http://localhost").into_response();
    }
    let path = uri.path().trim_start_matches('/');
    if let Some(file) = (!path.is_empty()).then(|| Assets::get(path)).flatten() {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        // Vite fingerprints everything under assets/, so it can be cached forever.
        let cache = if path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        return (
            [
                (header::CONTENT_TYPE, mime.as_ref().to_owned()),
                (header::CACHE_CONTROL, cache.to_owned()),
            ],
            file.data.into_owned(),
        )
            .into_response();
    }

    // SPA fallback: any unknown path gets index.html, plus the API token cookie.
    match Assets::get("index.html") {
        Some(index) => {
            let cookie = format!(
                "{}={token}; Path=/; HttpOnly; SameSite=Strict",
                pulse_api::TOKEN_COOKIE
            );
            (
                [
                    (header::CONTENT_TYPE, "text/html; charset=utf-8".to_owned()),
                    (header::CACHE_CONTROL, "no-cache".to_owned()),
                    (header::SET_COOKIE, cookie),
                ],
                index.data.into_owned(),
            )
                .into_response()
        }
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "web UI missing from build",
        )
            .into_response(),
    }
}

#[cfg(not(feature = "embed-web"))]
pub async fn serve(headers: HeaderMap) -> Response {
    if !host_is_local(&headers) {
        return (StatusCode::MISDIRECTED_REQUEST, "use http://localhost").into_response();
    }
    axum::response::Html(
        "<!doctype html><meta charset=utf-8><title>Pulse Sync</title>\
         <body style=\"font-family:system-ui;max-width:40rem;margin:4rem auto;line-height:1.5\">\
         <h1>Pulse Sync daemon is running</h1>\
         <p>This build doesn't embed the web UI. For development run <code>just dev</code> \
         and open <a href=\"http://localhost:5173\">http://localhost:5173</a>.</p>\
         <p>For a release build: <code>just build</code>.</p></body>",
    )
    .into_response()
}

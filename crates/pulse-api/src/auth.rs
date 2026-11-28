use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::error::ApiError;

pub const TOKEN_COOKIE: &str = "pulse_token";

pub async fn require_local_host(req: Request, next: Next) -> Response {
    let ok = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(is_local_host);
    if ok {
        next.run(req).await
    } else {
        ApiError::new(
            StatusCode::MISDIRECTED_REQUEST,
            "requests must use localhost",
        )
        .into_response()
    }
}

pub async fn require_token(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let expected = state.api_token.as_bytes();
    let presented = bearer(&req)
        .or_else(|| cookie(&req))
        .or_else(|| query_token(&req));
    if presented.is_some_and(|t| constant_time_eq(t.as_bytes(), expected)) {
        next.run(req).await
    } else {
        ApiError::new(StatusCode::UNAUTHORIZED, "missing or invalid API token").into_response()
    }
}

pub fn is_local_host(host: &str) -> bool {
    let name = if let Some(rest) = host.strip_prefix('[') {
        // [::1]:port
        rest.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map_or(host, |(name, _port)| name)
    };
    matches!(name, "localhost" | "127.0.0.1" | "::1")
}

fn bearer(req: &Request) -> Option<String> {
    let value = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ").map(str::to_owned)
}

fn cookie(req: &Request) -> Option<String> {
    req.headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|kv| {
            kv.trim()
                .strip_prefix(TOKEN_COOKIE)?
                .strip_prefix('=')
                .map(str::to_owned)
        })
}

/// Browsers can't set headers on WebSocket upgrades, so `/api/events` also
/// accepts `?token=`.
fn query_token(req: &Request) -> Option<String> {
    if req.uri().path() != "/api/events" {
        return None;
    }
    req.uri()
        .query()?
        .split('&')
        .find_map(|kv| kv.strip_prefix("token=").map(str::to_owned))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_hosts() {
        for h in [
            "localhost",
            "localhost:8417",
            "127.0.0.1:8417",
            "[::1]:8417",
            "[::1]",
        ] {
            assert!(is_local_host(h), "{h}");
        }
        for h in [
            "evil.com",
            "evil.com:8417",
            "127.0.0.1.evil.com",
            "192.168.1.5:8417",
        ] {
            assert!(!is_local_host(h), "{h}");
        }
    }

    #[test]
    fn constant_time_compare() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}

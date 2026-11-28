//! The daemon's local API: REST for commands, a WebSocket for live events.
//!
//! Only listens on loopback and requires the per-install API token (header,
//! cookie, or `?token=` for the WebSocket). The Host header is checked to
//! block DNS-rebinding attacks from web pages.

mod auth;
pub mod dto;
mod error;
mod routes;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::middleware;
use pulse_core::DeviceId;
use pulse_core::events::Event;
use pulse_fs::FolderManager;
use pulse_store::Store;
use tokio::sync::broadcast;

pub use auth::TOKEN_COOKIE;
pub use error::ApiError;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub manager: FolderManager,
    pub events: broadcast::Sender<Event>,
    pub device_id: DeviceId,
    pub api_token: Arc<str>,
    /// The daemon's own data directory; folders may not overlap it.
    pub data_dir: PathBuf,
}

/// All `/api/*` routes, with authentication applied.
pub fn router(state: AppState) -> Router {
    routes::routes()
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_token,
        ))
        .layer(middleware::from_fn(auth::require_local_host))
        .with_state(state)
}

/// Exposed for callers that serve the UI: does `host` name this machine?
pub fn is_local_host(host: &str) -> bool {
    auth::is_local_host(host)
}

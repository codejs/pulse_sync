mod events;
mod folders;
mod fs;
mod health;

use axum::Router;
use axum::routing::{get, post};

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/folders", get(folders::list).post(folders::create))
        .route(
            "/api/folders/{id}",
            get(folders::get)
                .patch(folders::update)
                .delete(folders::remove),
        )
        .route("/api/folders/{id}/rescan", post(folders::rescan))
        .route("/api/folders/{id}/files", get(folders::files))
        .route("/api/fs/browse", get(fs::browse))
        .route("/api/events", get(events::events))
}

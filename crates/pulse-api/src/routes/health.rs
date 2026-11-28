use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::dto::HealthResponse;

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        device_id: state.device_id.to_string(),
    })
}

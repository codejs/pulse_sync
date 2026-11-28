use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use pulse_store::StoreError;

use crate::dto::ErrorResponse;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    pub fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!(%err, "internal API error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal error; see the daemon log",
        )
    }
}

impl From<StoreError> for ApiError {
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::NotFound => ApiError::not_found("not found"),
            other => ApiError::internal(other),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

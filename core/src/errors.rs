use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Internal server error")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_type(&self) -> &str {
        match self {
            Self::Internal(_) => "INTERNAL_SERVER_ERROR",
            Self::NotFound(_) => "NOT_FOUND",
            Self::BadRequest(_) => "BAD_REQUEST",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = Json(ErrorResponse {
            error: self.error_type().to_string(),
            message: self.to_string(),
        });

        (status, body).into_response()
    }
}

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("not found")]
    NotFound,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("llm error: {0}")]
    Llm(String),

    #[error("jwt error: {0}")]
    Jwt(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let request_id = Uuid::new_v4().to_string();
        let status = match &self {
            AppError::Database(e) => {
                tracing::error!(
                    request_id = %request_id,
                    error = %e,
                    "database request failed"
                );
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::Internal(e) => {
                tracing::error!(
                    request_id = %request_id,
                    error = %e,
                    "internal request failed"
                );
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::Llm(e) => {
                tracing::error!(
                    request_id = %request_id,
                    error = %e,
                    "llm request failed"
                );
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Jwt(_) => StatusCode::UNAUTHORIZED,
        };

        let client_message = match &self {
            AppError::Database(_) | AppError::Internal(_) | AppError::Llm(_) => {
                "internal server error".to_string()
            }
            _ => self.to_string(),
        };

        let body = Json(json!({
            "error": client_message,
            "status": status.as_u16(),
            "request_id": request_id,
        }));

        (status, body).into_response()
    }
}

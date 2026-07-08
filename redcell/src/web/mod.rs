pub mod auth;
pub mod oidc;
pub mod routes;

use axum::response::{IntoResponse, Redirect, Response};
use serde::{Deserialize, Serialize};

pub const CURRENT_TOS_VERSION: &str = "v1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUser {
    pub user_id: String,
    pub email: String,
}

pub enum WebError {
    Unauthorized,
    Forbidden(&'static str),
    BadRequest(&'static str),
    Internal,
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        match self {
            WebError::Unauthorized => Redirect::to("/login").into_response(),
            WebError::Forbidden(msg) => (axum::http::StatusCode::FORBIDDEN, msg).into_response(),
            WebError::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, msg).into_response(),
            WebError::Internal => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            )
                .into_response(),
        }
    }
}

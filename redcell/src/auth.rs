use crate::AppState;
use crate::error::AppError;
use crate::models::User;
use axum::{extract::FromRequestParts, http::request::Parts};
use bcrypt::{DEFAULT_COST, hash, verify};
use rand::Rng;
use std::sync::Arc;

pub fn generate_api_key() -> (String, String, String) {
    let id = uuid::Uuid::new_v4().to_string();
    let secret: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let full_key = format!("rt_{}_{}", id, secret);
    let hash = hash(&full_key, DEFAULT_COST).expect("bcrypt hash should not fail");
    (id, full_key, hash)
}

pub struct ApiKeyAuth(pub User);

impl FromRequestParts<Arc<AppState>> for ApiKeyAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        let full_key = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized)?;

        let key_id = full_key
            .strip_prefix("rt_")
            .and_then(|rest| rest.split('_').next())
            .ok_or(AppError::Unauthorized)?;

        let api_key: crate::models::ApiKey =
            sqlx::query_as("SELECT * FROM api_keys WHERE id = ? AND revoked_at IS NULL")
                .bind(key_id)
                .fetch_one(&state.pool)
                .await
                .map_err(|_| AppError::Unauthorized)?;

        if !verify(full_key, &api_key.key_hash).map_err(|e| AppError::Internal(e.into()))? {
            return Err(AppError::Unauthorized);
        }

        let user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(&api_key.user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| AppError::Unauthorized)?;

        Ok(ApiKeyAuth(user))
    }
}

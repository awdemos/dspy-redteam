use crate::AppState;
use crate::error::AppError;
use crate::models::User;
use crate::web::{CURRENT_TOS_VERSION, SessionUser, WebError};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use tower_sessions::Session;

pub const SESSION_USER_KEY: &str = "user";

pub async fn set_user_session(session: &Session, user: &User) -> Result<(), AppError> {
    session
        .insert(
            SESSION_USER_KEY,
            SessionUser {
                user_id: user.id.clone(),
                email: user.email.clone(),
            },
        )
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))
}

pub async fn clear_user_session(session: &Session) -> Result<(), AppError> {
    session
        .remove::<()>(SESSION_USER_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    Ok(())
}

pub async fn get_session_user(session: &Session) -> Result<Option<SessionUser>, AppError> {
    session
        .get(SESSION_USER_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))
}

pub struct WebAuth(pub User);

impl FromRequestParts<AppState> for WebAuth {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| WebError::Unauthorized)?;
        let session_user: SessionUser = get_session_user(&session)
            .await
            .map_err(|_| WebError::Internal)?
            .ok_or(WebError::Unauthorized)?;

        let user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(&session_user.user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| WebError::Unauthorized)?;

        Ok(WebAuth(user))
    }
}

pub struct WebAuthWithTos(pub User);

impl FromRequestParts<AppState> for WebAuthWithTos {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let WebAuth(user) = WebAuth::from_request_parts(parts, state).await?;
        if user.accepted_tos_version.as_deref() != Some(CURRENT_TOS_VERSION) {
            return Err(WebError::Forbidden("terms of service not accepted"));
        }
        Ok(WebAuthWithTos(user))
    }
}

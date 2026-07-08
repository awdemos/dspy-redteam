use crate::AppState;
use crate::models::User;
use crate::web::WebError;
use crate::web::auth::{clear_user_session, set_user_session};
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tower_sessions::Session;

const OIDC_SCOPE: &str = "openid email profile";
const OIDC_STATE_TTL_SECONDS: i64 = 600; // 10 minutes

#[derive(Deserialize)]
pub struct AuthorizeQuery {
    pub code: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct AuthorizeErrorQuery {
    pub error: Option<String>,
    pub error_description: Option<String>,
}

fn random_base64_url(len: usize) -> String {
    let bytes: Vec<u8> = (0..len).map(|_| rand::random::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(&bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn oidc_error_redirect(msg: &str) -> Redirect {
    Redirect::to(&format!("/login?error={}", urlencoding::encode(msg)))
}

async fn store_oidc_state(
    state: &str,
    verifier: &str,
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> Result<(), WebError> {
    sqlx::query("INSERT INTO oidc_state (state, verifier, created_at) VALUES (?1, ?2, ?3)")
        .bind(state)
        .bind(verifier)
        .bind(Utc::now())
        .execute(pool)
        .await
        .map_err(|_| WebError::Internal)?;
    Ok(())
}

async fn consume_oidc_state(
    state: &str,
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> Result<Option<String>, WebError> {
    let row: Option<(String,)> =
        sqlx::query_as("DELETE FROM oidc_state WHERE state = ?1 RETURNING verifier")
            .bind(state)
            .fetch_optional(pool)
            .await
            .map_err(|_| WebError::Internal)?;
    Ok(row.map(|r| r.0))
}

async fn cleanup_expired_oidc_state(pool: &sqlx::Pool<sqlx::Sqlite>) -> Result<(), WebError> {
    let cutoff = Utc::now() - chrono::TimeDelta::seconds(OIDC_STATE_TTL_SECONDS);
    sqlx::query("DELETE FROM oidc_state WHERE created_at < ?1")
        .bind(cutoff)
        .execute(pool)
        .await
        .map_err(|_| WebError::Internal)?;
    Ok(())
}

async fn upsert_user_from_oidc(state: &AppState, userinfo: &Userinfo) -> Result<User, WebError> {
    let email = userinfo.email.as_deref().ok_or(WebError::BadRequest(
        "OIDC provider did not return an email address",
    ))?;

    let existing: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = ?1")
        .bind(email)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| WebError::Internal)?;

    if let Some(mut user) = existing {
        sqlx::query(
            "UPDATE users SET accepted_tos_version = COALESCE(accepted_tos_version, ?1), accepted_tos_at = COALESCE(accepted_tos_at, ?2), updated_at = ?3 WHERE id = ?4",
        )
        .bind(crate::web::CURRENT_TOS_VERSION)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(&user.id)
        .execute(&state.pool)
        .await
        .map_err(|_| WebError::Internal)?;
        user.accepted_tos_version = Some(crate::web::CURRENT_TOS_VERSION.to_string());
        user.accepted_tos_at = Some(Utc::now());
        return Ok(user);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, accepted_tos_version, accepted_tos_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(email)
    .bind(None::<String>)
    .bind(crate::web::CURRENT_TOS_VERSION)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(|_| WebError::Internal)?;

    Ok(User {
        id,
        email: email.to_string(),
        password_hash: None,
        accepted_tos_version: Some(crate::web::CURRENT_TOS_VERSION.to_string()),
        accepted_tos_at: Some(now),
        created_at: now,
    })
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct Userinfo {
    #[allow(dead_code)]
    sub: String,
    email: Option<String>,
    #[allow(dead_code)]
    preferred_username: Option<String>,
    #[allow(dead_code)]
    name: Option<String>,
    #[allow(dead_code)]
    email_verified: Option<bool>,
}

pub async fn oidc_login(
    session: Session,
    State(state): State<Arc<AppState>>,
) -> Result<Redirect, WebError> {
    if let Ok(Some(_)) = crate::web::auth::get_session_user(&session).await {
        return Ok(Redirect::to("/dashboard"));
    }

    let oidc = state.config.oidc.as_ref().ok_or(WebError::Internal)?;

    let state_param = random_base64_url(32);
    let verifier = random_base64_url(64);
    let challenge = pkce_challenge(&verifier);

    cleanup_expired_oidc_state(&state.pool).await.ok();
    store_oidc_state(&state_param, &verifier, &state.pool).await?;

    let authorize_url = format!(
        "{}/authorize?client_id={}&response_type=code&scope={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
        oidc.issuer_url.trim_end_matches('/'),
        urlencoding::encode(&oidc.client_id),
        urlencoding::encode(OIDC_SCOPE),
        urlencoding::encode(&oidc.redirect_uri),
        urlencoding::encode(&state_param),
        urlencoding::encode(&challenge)
    );

    Ok(Redirect::to(&authorize_url))
}

pub async fn oidc_callback(
    session: Session,
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuthorizeQuery>,
) -> Result<impl IntoResponse, WebError> {
    let oidc = state.config.oidc.as_ref().ok_or(WebError::Internal)?;

    let verifier = consume_oidc_state(&params.state, &state.pool)
        .await?
        .ok_or(WebError::BadRequest("invalid or expired OIDC state"))?;

    let mut token_body = vec![
        ("grant_type", "authorization_code"),
        ("code", params.code.as_str()),
        ("redirect_uri", oidc.redirect_uri.as_str()),
        ("client_id", oidc.client_id.as_str()),
        ("code_verifier", verifier.as_str()),
    ];

    if let Some(secret) = &oidc.client_secret {
        token_body.push(("client_secret", secret.as_str()));
    }

    let token_url = format!("{}/api/oidc/token", oidc.issuer_url.trim_end_matches('/'));

    let token_body_encoded = token_body
        .into_iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let token_res = state
        .http_client
        .post(&token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(token_body_encoded)
        .send()
        .await
        .map_err(|_| WebError::Internal)?;

    if !token_res.status().is_success() {
        let details = token_res.text().await.unwrap_or_default();
        tracing::error!("oidc token exchange failed: {}", details);
        return Ok(oidc_error_redirect("OIDC token exchange failed").into_response());
    }

    let tokens: TokenResponse = token_res.json().await.map_err(|_| WebError::Internal)?;

    let userinfo_url = format!(
        "{}/api/oidc/userinfo",
        oidc.issuer_url.trim_end_matches('/')
    );

    let userinfo_res = state
        .http_client
        .get(&userinfo_url)
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .map_err(|_| WebError::Internal)?;

    if !userinfo_res.status().is_success() {
        let details = userinfo_res.text().await.unwrap_or_default();
        tracing::error!("oidc userinfo failed: {}", details);
        return Ok(oidc_error_redirect("OIDC userinfo request failed").into_response());
    }

    let userinfo: Userinfo = userinfo_res.json().await.map_err(|_| WebError::Internal)?;

    let user = upsert_user_from_oidc(&state, &userinfo).await?;

    if set_user_session(&session, &user).await.is_err() {
        return Ok(oidc_error_redirect("session error").into_response());
    }

    Ok(Redirect::to("/dashboard").into_response())
}

pub async fn oidc_callback_error(Query(params): Query<AuthorizeErrorQuery>) -> impl IntoResponse {
    let msg = params
        .error_description
        .or(params.error)
        .unwrap_or_else(|| "oidc authentication error".to_string());
    oidc_error_redirect(&msg)
}

pub async fn oidc_logout(session: Session) -> Result<Redirect, WebError> {
    let _ = clear_user_session(&session).await;
    Ok(Redirect::to("/"))
}

use crate::AppState;
use crate::auth::generate_api_key;
use crate::models::{Job, Subscription};
use crate::web::auth::get_session_user;
use crate::web::oidc::{oidc_callback, oidc_callback_error, oidc_login, oidc_logout};
use crate::web::{SessionUser, WebError};
use askama::Template;
use axum::{
    Router,
    extract::{Form, Path, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};

use chrono::{DateTime, Datelike, Utc};
use serde::Deserialize;
use std::sync::Arc;
use tower_sessions::Session;

// ---------------------------------------------------------------------------
// Template structs
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub logged_in: bool,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub logged_in: bool,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub logged_in: bool,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "tos.html")]
pub struct TosTemplate {
    pub logged_in: bool,
}

#[derive(Template)]
#[template(path = "docs.html")]
pub struct DocsTemplate {
    pub logged_in: bool,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub logged_in: bool,
    pub user: SessionUser,
    pub subscription: Option<Subscription>,
    pub recent_jobs: Vec<Job>,
    pub api_key_count: i64,
    pub cost_str: String,
    pub jobs_used: i64,
    pub jobs_limit: i64,
}

#[derive(Template)]
#[template(path = "billing.html")]
pub struct BillingTemplate {
    pub logged_in: bool,
    pub subscription: Option<Subscription>,
    pub jobs_used: i64,
    pub jobs_limit: i64,
    pub jobs_percent: i64,
}

#[derive(Template)]
#[template(path = "api_keys.html")]
pub struct ApiKeysTemplate {
    pub logged_in: bool,
    pub keys: Vec<ApiKeyRow>,
    pub new_key: Option<String>,
    pub error: Option<String>,
}

pub struct ApiKeyRow {
    pub id: String,
    pub name: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ApiKeyForm {
    pub name: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the monthly job limit for a user based on subscription status.
fn job_limit(_sub: &Option<Subscription>) -> i64 {
    // For now every authenticated user gets the Pro limit.
    // Could derive tier from the subscription `price_id` in the future.
    100
}

/// Return a monthly-period-start for the current billing cycle.
fn current_month_period() -> DateTime<Utc> {
    let now = Utc::now();
    // Round down to the 1st of the current month at 00:00:00 UTC
    chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .unwrap_or(now)
}

/// Ensure a Stripe customer exists for the user, returning the customer id.
async fn get_or_create_customer(
    state: &AppState,
    user_id: &str,
    email: &str,
) -> Result<String, WebError> {
    let existing: Option<(String,)> =
        sqlx::query_as(
            "SELECT stripe_customer_id FROM subscriptions WHERE user_id = ?1 AND stripe_customer_id IS NOT NULL AND stripe_customer_id != '' LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| WebError::Internal)?;

    if let Some((id,)) = existing {
        return Ok(id);
    }

    let customer_id = state
        .billing
        .create_customer(email)
        .await
        .map_err(|_| WebError::Internal)?;

    let sub_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    sqlx::query(
            "INSERT INTO subscriptions (id, user_id, stripe_customer_id, status, created_at, updated_at) VALUES (?1, ?2, ?3, 'incomplete', ?4, ?4)",
        )
        .bind(&sub_id)
        .bind(user_id)
        .bind(&customer_id)
        .bind(now)
        .execute(&state.pool)
        .await
        .map_err(|_| WebError::Internal)?;

    Ok(customer_id)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

// -- Landing page ----------------------------------------------------------

async fn index_page(session: Session, State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let logged_in = get_session_user(&session)
        .await
        .map(|u| u.is_some())
        .unwrap_or(false);
    Html(IndexTemplate { logged_in }.to_string())
}

// -- Terms of service ------------------------------------------------------

async fn tos_page(session: Session) -> impl IntoResponse {
    let logged_in = get_session_user(&session)
        .await
        .map(|u| u.is_some())
        .unwrap_or(false);
    Html(TosTemplate { logged_in }.to_string())
}

async fn docs_page(session: Session) -> impl IntoResponse {
    let logged_in = get_session_user(&session)
        .await
        .map(|u| u.is_some())
        .unwrap_or(false);
    Html(DocsTemplate { logged_in }.to_string())
}

// -- Login -----------------------------------------------------------------

#[derive(Deserialize)]
pub struct LoginQuery {
    pub error: Option<String>,
}

async fn login_page(session: Session, Query(query): Query<LoginQuery>) -> impl IntoResponse {
    let user = get_session_user(&session).await.ok().flatten();
    if user.is_some() {
        return Redirect::to("/dashboard").into_response();
    }
    let html = LoginTemplate {
        logged_in: false,
        error: query.error,
    }
    .to_string();
    Html(html).into_response()
}

// -- Registration ----------------------------------------------------------

async fn register_page(session: Session) -> impl IntoResponse {
    let user = get_session_user(&session).await.ok().flatten();
    if user.is_some() {
        return Redirect::to("/dashboard").into_response();
    }
    Redirect::to("/login").into_response()
}

// -- Dashboard -------------------------------------------------------------

async fn dashboard_page(
    session: Session,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, WebError> {
    let session_user = get_session_user(&session)
        .await
        .map_err(|_| WebError::Internal)?
        .ok_or(WebError::Unauthorized)?;

    let user_id = &session_user.user_id;
    let pool = &state.pool;

    // Subscription
    let subscription: Option<Subscription> = sqlx::query_as(
        "SELECT * FROM subscriptions WHERE user_id = ?1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| WebError::Internal)?;

    // Recent jobs
    let recent_jobs: Vec<Job> =
        sqlx::query_as("SELECT * FROM jobs WHERE user_id = ?1 ORDER BY created_at DESC LIMIT 10")
            .bind(user_id)
            .fetch_all(pool)
            .await
            .map_err(|_| WebError::Internal)?;

    // API key count
    let api_key_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM api_keys WHERE user_id = ?1 AND revoked_at IS NULL")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .map_err(|_| WebError::Internal)?;

    // Monthly usage (jobs used this period)
    let period_start = current_month_period();
    let jobs_used: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(jobs_used), 0) FROM monthly_job_usage WHERE user_id = ?1 AND period_start >= ?2",
    )
    .bind(user_id)
    .bind(period_start)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    // Total cost MTD
    let cost: (Option<f64>,) = sqlx::query_as(
        "SELECT COALESCE(SUM(cost_estimate_usd), 0.0) FROM usage WHERE user_id = ?1 AND created_at >= ?2",
    )
    .bind(user_id)
    .bind(period_start)
    .fetch_one(pool)
    .await
    .unwrap_or((Some(0.0),));

    let limit = job_limit(&subscription);

    let tpl = DashboardTemplate {
        logged_in: true,
        user: session_user,
        subscription,
        recent_jobs,
        api_key_count: api_key_count.0,
        cost_str: format!("{:.2}", cost.0.unwrap_or(0.0)),
        jobs_used: jobs_used.0,
        jobs_limit: limit,
    };

    Ok(Html(tpl.to_string()))
}

// -- Billing ---------------------------------------------------------------

async fn billing_page(
    session: Session,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, WebError> {
    let session_user = get_session_user(&session)
        .await
        .map_err(|_| WebError::Internal)?
        .ok_or(WebError::Unauthorized)?;

    let subscription: Option<Subscription> = sqlx::query_as(
        "SELECT * FROM subscriptions WHERE user_id = ?1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&session_user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| WebError::Internal)?;

    let period_start = current_month_period();
    let jobs_used: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(jobs_used), 0) FROM monthly_job_usage WHERE user_id = ?1 AND period_start >= ?2",
    )
    .bind(&session_user.user_id)
    .bind(period_start)
    .fetch_one(&state.pool)
    .await
    .unwrap_or((0,));

    let limit = job_limit(&subscription);
    let pct = if limit > 0 {
        (jobs_used.0 * 100 / limit).min(100)
    } else {
        0
    };

    let tpl = BillingTemplate {
        logged_in: true,
        subscription,
        jobs_used: jobs_used.0,
        jobs_limit: limit,
        jobs_percent: pct,
    };

    Ok(Html(tpl.to_string()))
}

// -- Billing: create checkout session, redirect to Stripe ------------------

async fn billing_checkout(
    session: Session,
    State(state): State<Arc<AppState>>,
) -> Result<Redirect, WebError> {
    let session_user = get_session_user(&session)
        .await
        .map_err(|_| WebError::Internal)?
        .ok_or(WebError::Unauthorized)?;

    let customer_id =
        get_or_create_customer(&state, &session_user.user_id, &session_user.email).await?;

    let checkout_url = state
        .billing
        .create_checkout_session(&customer_id)
        .await
        .map_err(|_| WebError::Internal)?;

    Ok(Redirect::to(&checkout_url))
}

// -- Billing: redirect to Stripe customer portal ---------------------------

async fn billing_portal(
    session: Session,
    State(state): State<Arc<AppState>>,
) -> Result<Redirect, WebError> {
    let session_user = get_session_user(&session)
        .await
        .map_err(|_| WebError::Internal)?
        .ok_or(WebError::Unauthorized)?;

    let customer_id =
        get_or_create_customer(&state, &session_user.user_id, &session_user.email).await?;

    let customer_id_parsed: stripe::CustomerId =
        customer_id.parse().map_err(|_| WebError::Internal)?;

    let portal = stripe::BillingPortalSession::create(
        &state.billing.client,
        stripe::CreateBillingPortalSession::new(customer_id_parsed),
    )
    .await
    .map_err(|_| WebError::Internal)?;

    Ok(Redirect::to(&portal.url))
}

async fn api_keys_page(
    session: Session,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, WebError> {
    let session_user = get_session_user(&session)
        .await
        .map_err(|_| WebError::Internal)?
        .ok_or(WebError::Unauthorized)?;

    let keys = load_api_key_rows(&state.pool, &session_user.user_id).await?;

    let tpl = ApiKeysTemplate {
        logged_in: true,
        keys,
        new_key: None,
        error: None,
    };
    Ok(Html(tpl.to_string()))
}

async fn api_keys_create(
    session: Session,
    State(state): State<Arc<AppState>>,
    Form(form): Form<ApiKeyForm>,
) -> Result<Html<String>, WebError> {
    let session_user = get_session_user(&session)
        .await
        .map_err(|_| WebError::Internal)?
        .ok_or(WebError::Unauthorized)?;

    let name = form.name.trim().to_string();
    if name.is_empty() {
        let keys = load_api_key_rows(&state.pool, &session_user.user_id).await?;
        let tpl = ApiKeysTemplate {
            logged_in: true,
            keys,
            new_key: None,
            error: Some("Key name is required".to_string()),
        };
        return Ok(Html(tpl.to_string()));
    }

    let (id, key, hash) = generate_api_key();
    sqlx::query("INSERT INTO api_keys (id, user_id, key_hash, name) VALUES (?1, ?2, ?3, ?4)")
        .bind(&id)
        .bind(&session_user.user_id)
        .bind(&hash)
        .bind(&name)
        .execute(&state.pool)
        .await
        .map_err(|_| WebError::Internal)?;

    let keys = load_api_key_rows(&state.pool, &session_user.user_id).await?;
    let tpl = ApiKeysTemplate {
        logged_in: true,
        keys,
        new_key: Some(key),
        error: None,
    };
    Ok(Html(tpl.to_string()))
}

async fn api_keys_revoke(
    session: Session,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, WebError> {
    let session_user = get_session_user(&session)
        .await
        .map_err(|_| WebError::Internal)?
        .ok_or(WebError::Unauthorized)?;

    sqlx::query("UPDATE api_keys SET revoked_at = ?1 WHERE id = ?2 AND user_id = ?3")
        .bind(Utc::now())
        .bind(&id)
        .bind(&session_user.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| WebError::Internal)?;

    let keys = load_api_key_rows(&state.pool, &session_user.user_id).await?;
    let tpl = ApiKeysTemplate {
        logged_in: true,
        keys,
        new_key: None,
        error: None,
    };
    Ok(Html(tpl.to_string()))
}

type ApiKeyDbRow = (String, Option<String>, DateTime<Utc>, Option<DateTime<Utc>>);

async fn load_api_key_rows(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    user_id: &str,
) -> Result<Vec<ApiKeyRow>, WebError> {
    let rows: Vec<ApiKeyDbRow> = sqlx::query_as(
        "SELECT id, name, created_at, revoked_at FROM api_keys WHERE user_id = ?1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|_| WebError::Internal)?;

    Ok(rows
        .into_iter()
        .map(|(id, name, created_at, revoked_at)| ApiKeyRow {
            id,
            name,
            created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
            revoked_at: revoked_at.map(|d| d.format("%Y-%m-%d %H:%M").to_string()),
        })
        .collect())
}

// -- Stripe webhook --------------------------------------------------------

async fn stripe_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Result<(), WebError> {
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(WebError::BadRequest("missing stripe-signature header"))?;

    let event = stripe::Webhook::construct_event(&body, sig, &state.config.stripe.webhook_secret)
        .map_err(|_| WebError::BadRequest("invalid webhook signature"))?;

    match event.type_ {
        stripe::EventType::CheckoutSessionCompleted => {
            if let stripe::EventObject::CheckoutSession(session) = event.data.object {
                let customer_id = session.customer;
                let sub_id = session.subscription;

                if let (Some(cust), Some(sub)) = (&customer_id, &sub_id) {
                    let customer_str = cust.id().to_string();
                    let sub_str = sub.id().to_string();
                    let now = Utc::now();

                    // Upsert subscription record
                    sqlx::query(
                        "UPDATE subscriptions SET stripe_subscription_id = ?1, status = 'active', updated_at = ?2 WHERE stripe_customer_id = ?3",
                    )
                    .bind(&sub_str)
                    .bind(now)
                    .bind(&customer_str)
                    .execute(&state.pool)
                    .await
                    .map_err(|_| WebError::Internal)?;

                    tracing::info!(
                        "checkout.session.completed: customer={}, subscription={}",
                        customer_str,
                        sub_str
                    );
                }
            }
        }
        stripe::EventType::CustomerSubscriptionUpdated
        | stripe::EventType::CustomerSubscriptionCreated => {
            if let stripe::EventObject::Subscription(sub) = event.data.object {
                let customer_str = sub.customer.id().to_string();
                let sub_parsed = sub.id.to_string();
                let status = format!("{:?}", sub.status);
                let period_start: Option<chrono::DateTime<Utc>> = (sub.current_period_start != 0)
                    .then(|| DateTime::from_timestamp(sub.current_period_start, 0).unwrap());
                let period_end: Option<chrono::DateTime<Utc>> = (sub.current_period_end != 0)
                    .then(|| DateTime::from_timestamp(sub.current_period_end, 0).unwrap());
                let cancel_at_period = if sub.cancel_at_period_end { 1 } else { 0 };
                let now = Utc::now();

                sqlx::query(
                    "UPDATE subscriptions SET stripe_subscription_id = ?1, status = ?2, current_period_start = ?3, current_period_end = ?4, cancel_at_period_end = ?5, updated_at = ?6 WHERE stripe_customer_id = ?7",
                )
                .bind(&sub_parsed)
                .bind(&status)
                .bind(period_start)
                .bind(period_end)
                .bind(cancel_at_period)
                .bind(now)
                .bind(&customer_str)
                .execute(&state.pool)
                .await
                .map_err(|_| WebError::Internal)?;

                tracing::info!("customer.subscription.updated: customer={}", customer_str);
            }
        }
        stripe::EventType::CustomerSubscriptionDeleted => {
            if let stripe::EventObject::Subscription(sub) = event.data.object {
                let customer_str = sub.customer.id().to_string();
                let now = Utc::now();

                sqlx::query(
                    "UPDATE subscriptions SET status = 'canceled', updated_at = ?1 WHERE stripe_customer_id = ?2",
                )
                .bind(now)
                .bind(&customer_str)
                .execute(&state.pool)
                .await
                .map_err(|_| WebError::Internal)?;

                tracing::info!("customer.subscription.deleted: customer={}", customer_str);
            }
        }
        _ => {
            tracing::debug!("unhandled webhook event: {:?}", event.type_);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index_page))
        .route("/tos", get(tos_page))
        .route("/docs", get(docs_page))
        .route("/login", get(login_page))
        .route("/register", get(register_page))
        .route("/auth/login", get(oidc_login))
        .route("/auth/callback", get(oidc_callback))
        .route("/auth/error", get(oidc_callback_error))
        .route("/dashboard", get(dashboard_page))
        .route("/billing", get(billing_page))
        .route("/billing/checkout", get(billing_checkout))
        .route("/billing/portal", post(billing_portal))
        .route("/api-keys", get(api_keys_page).post(api_keys_create))
        .route("/api-keys/{id}/revoke", post(api_keys_revoke))
        .route("/logout", get(oidc_logout))
        .route("/stripe/webhook", post(stripe_webhook))
        .with_state(state)
}

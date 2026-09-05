use crate::AppState;
use crate::auth::{ApiKeyAuth, generate_api_key};
use crate::error::{AppError, AppResult};
use crate::models::{
    ApiKey, CreateApiKeyRequest, CreateApiKeyResponse, CreateJobRequest, Job, JobDetailResponse,
    JobResponse, JobResult, Usage,
};
use crate::rate_limit::{LimitConfig, RateLimitLayer};
use crate::validation::{validate_intent, validate_layers, validate_target_model};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, get, post},
};
use chrono::Utc;
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    let public = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .layer(RateLimitLayer::new(
            state.clone(),
            "public",
            LimitConfig::public(),
        ))
        .with_state(state.clone());

    let jobs_limited = Router::new()
        .route("/jobs", post(create_job))
        .layer(RateLimitLayer::new(
            state.clone(),
            "jobs",
            LimitConfig::jobs(),
        ))
        .with_state(state.clone());

    let auth = Router::new()
        .route("/api-keys", post(create_api_key).get(list_api_keys))
        .route("/api-keys/{id}", delete(revoke_api_key))
        .route("/jobs", get(list_jobs))
        .route("/jobs/{id}", get(get_job))
        .route("/usage", get(get_usage))
        .layer(RateLimitLayer::new(
            state.clone(),
            "auth",
            LimitConfig::auth(),
        ))
        .with_state(state.clone());

    public.merge(auth).merge(jobs_limited)
}

pub async fn health() -> &'static str {
    "ok"
}

pub async fn ready(State(state): State<Arc<AppState>>) -> AppResult<&'static str> {
    sqlx::query("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AppError::Internal(anyhow::anyhow!("database not ready")))?;
    Ok("ready")
}

async fn create_api_key(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Json(req): Json<CreateApiKeyRequest>,
) -> AppResult<Json<CreateApiKeyResponse>> {
    let (id, key, hash) = generate_api_key();

    sqlx::query("INSERT INTO api_keys (id, user_id, key_hash, name) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(&user.id)
        .bind(&hash)
        .bind(&req.name)
        .execute(&state.pool)
        .await?;

    Ok(Json(CreateApiKeyResponse {
        id,
        key,
        name: req.name,
    }))
}

async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
) -> AppResult<Json<Vec<ApiKey>>> {
    let keys: Vec<ApiKey> =
        sqlx::query_as("SELECT * FROM api_keys WHERE user_id = ? ORDER BY created_at DESC")
            .bind(&user.id)
            .fetch_all(&state.pool)
            .await?;

    Ok(Json(keys))
}

async fn revoke_api_key(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Path(id): Path<String>,
) -> AppResult<()> {
    let result = sqlx::query("UPDATE api_keys SET revoked_at = ? WHERE id = ? AND user_id = ?")
        .bind(Utc::now())
        .bind(&id)
        .bind(&user.id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(())
}

async fn create_job(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Json(req): Json<CreateJobRequest>,
) -> AppResult<Json<JobResponse>> {
    validate_intent(&req.intent)?;
    validate_target_model(&req.target_model)?;
    let layers = validate_layers(req.layers)?;

    let id = uuid::Uuid::new_v4().to_string();
    let run_at = req.run_at.unwrap_or_else(Utc::now);

    sqlx::query(
        "INSERT INTO jobs (id, user_id, intent, target_model, layers, status, max_attempts, run_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&req.intent)
    .bind(&req.target_model)
    .bind(layers)
    .bind("queued")
    .bind(req.max_attempts.clamp(1, 10))
    .bind(run_at)
    .execute(&state.pool)
    .await?;

    let job = Job {
        id,
        user_id: user.id,
        intent: req.intent,
        target_model: req.target_model,
        layers,
        status: "queued".to_string(),
        error_message: None,
        created_at: Utc::now(),
        completed_at: None,
        claimed_at: None,
        worker_id: None,
        attempts: 0,
        max_attempts: req.max_attempts.clamp(1, 10),
        run_at,
    };

    Ok(Json(job_to_response(job)))
}

async fn list_jobs(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
) -> AppResult<Json<Vec<JobResponse>>> {
    let jobs: Vec<Job> =
        sqlx::query_as("SELECT * FROM jobs WHERE user_id = ? ORDER BY created_at DESC")
            .bind(&user.id)
            .fetch_all(&state.pool)
            .await?;

    Ok(Json(jobs.into_iter().map(job_to_response).collect()))
}

async fn get_job(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Path(id): Path<String>,
) -> AppResult<Json<JobDetailResponse>> {
    let job: Job = sqlx::query_as("SELECT * FROM jobs WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AppError::NotFound)?;

    let results: Vec<JobResult> =
        sqlx::query_as("SELECT * FROM job_results WHERE job_id = ? ORDER BY layer ASC")
            .bind(&id)
            .fetch_all(&state.pool)
            .await?;

    Ok(Json(JobDetailResponse {
        job: job_to_response(job),
        results,
    }))
}

async fn get_usage(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
) -> AppResult<Json<Vec<Usage>>> {
    let usage: Vec<Usage> =
        sqlx::query_as("SELECT * FROM usage WHERE user_id = ? ORDER BY created_at DESC")
            .bind(&user.id)
            .fetch_all(&state.pool)
            .await?;

    Ok(Json(usage))
}

fn job_to_response(job: Job) -> JobResponse {
    JobResponse {
        id: job.id,
        intent: job.intent,
        target_model: job.target_model,
        layers: job.layers,
        status: job.status,
        error_message: job.error_message,
        created_at: job.created_at,
        completed_at: job.completed_at,
    }
}

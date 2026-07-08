use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub accepted_tos_version: Option<String>,
    pub accepted_tos_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Subscription {
    pub id: String,
    pub user_id: String,
    pub stripe_customer_id: String,
    pub stripe_subscription_id: Option<String>,
    pub status: String,
    pub price_id: Option<String>,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct TargetModelAccess {
    pub id: String,
    pub user_id: String,
    pub provider: String,
    pub encrypted_credential: String,
    pub model_id: Option<String>,
    pub file_path: Option<String>,
    pub ownership_proof_url: Option<String>,
    pub is_active: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct MonthlyJobUsage {
    pub user_id: String,
    pub period_start: DateTime<Utc>,
    pub jobs_used: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    #[serde(skip_serializing)]
    pub key_hash: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Job {
    pub id: String,
    pub user_id: String,
    pub intent: String,
    pub target_model: String,
    pub layers: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub worker_id: Option<String>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub run_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct JobResult {
    pub id: String,
    pub job_id: String,
    pub layer: i32,
    pub attack_prompt: String,
    pub target_response: Option<String>,
    pub score: Option<f64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Usage {
    pub id: String,
    pub user_id: String,
    pub job_id: Option<String>,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub cost_estimate_usd: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub accept_tos: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub key: String,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub intent: String,
    pub target_model: String,
    #[serde(default = "default_layers")]
    pub layers: i32,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: i32,
    pub run_at: Option<DateTime<Utc>>,
}

fn default_layers() -> i32 {
    5
}

fn default_max_attempts() -> i32 {
    3
}

#[derive(Debug, Serialize)]
pub struct JobResponse {
    pub id: String,
    pub intent: String,
    pub target_model: String,
    pub layers: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct JobDetailResponse {
    #[serde(flatten)]
    pub job: JobResponse,
    pub results: Vec<JobResult>,
}

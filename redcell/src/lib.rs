pub mod api;
pub mod auth;
pub mod billing;
pub mod config;
pub mod credentials;
pub mod db;
pub mod error;
pub mod gate;
pub mod judge;
pub mod llm;
pub mod models;
pub mod pipeline;
pub mod rate_limit;
pub mod redis;
pub mod validation;
pub mod web;
pub mod worker;

use crate::config::AppConfig;
use crate::llm::LlmClient;
use crate::rate_limit::RateLimiter;
use tokio_util::task::TaskTracker;

pub struct AppState {
    pub pool: sqlx::Pool<sqlx::Sqlite>,
    pub llm_client: LlmClient,
    pub config: AppConfig,
    pub task_tracker: TaskTracker,
    pub rate_limiter: RateLimiter,
    pub billing: crate::billing::BillingClient,
    pub credentials: Option<crate::credentials::CredentialEncryption>,
    pub http_client: reqwest::Client,
}

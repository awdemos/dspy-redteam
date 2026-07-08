use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_database_url")]
    pub url: String,
    #[serde(default = "default_pool_max_connections")]
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_attack_model")]
    pub attack_model: String,
    #[serde(default = "default_judge_model")]
    pub judge_model: String,
    pub target_api_key: Option<String>,
    #[serde(default = "default_base_url")]
    pub target_base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    #[serde(default = "default_redis_pool_size")]
    pub pool_size: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestConfig {
    #[serde(default = "default_request_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_max_body_size")]
    pub max_body_size_bytes: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CorsConfig {
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkerConfig {
    #[serde(default = "default_worker_poll_interval_seconds")]
    pub poll_interval_seconds: u64,
    #[serde(default = "default_worker_max_concurrent_jobs")]
    pub max_concurrent_jobs: usize,
    pub worker_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StripeConfig {
    pub secret_key: String,
    pub publishable_key: String,
    pub webhook_secret: String,
    pub price_id: String,
    #[serde(default = "default_stripe_success_url")]
    pub success_url: String,
    #[serde(default = "default_stripe_cancel_url")]
    pub cancel_url: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CredentialsConfig {
    pub master_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OidcConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub database: DatabaseConfig,
    pub llm: LlmConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub request: RequestConfig,
    #[serde(default)]
    pub cors: CorsConfig,
    #[serde(default)]
    pub redis: Option<RedisConfig>,
    #[serde(default = "default_env")]
    pub env: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub worker: WorkerConfig,
    #[serde(default)]
    pub stripe: StripeConfig,
    #[serde(default)]
    pub credentials: CredentialsConfig,
    #[serde(default)]
    pub oidc: Option<OidcConfig>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        Config::builder()
            .add_source(File::with_name(".env").required(false))
            .add_source(Environment::with_prefix("REDTEAM").separator("__"))
            .build()?
            .try_deserialize()
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.llm.api_key.is_empty() {
            return Err(ConfigError::Message(
                "REDTEAM_LLM__API_KEY must be set".to_string(),
            ));
        }
        if url::Url::parse(&self.llm.base_url).is_err() {
            return Err(ConfigError::Message(
                "REDTEAM_LLM__BASE_URL must be a valid URL".to_string(),
            ));
        }
        if self.is_production() {
            if self.oidc.is_none() {
                return Err(ConfigError::Message(
                    "REDTEAM_OIDC__ISSUER_URL, REDTEAM_OIDC__CLIENT_ID, and REDTEAM_OIDC__REDIRECT_URI must be set in production".to_string(),
                ));
            }
            let oidc = self.oidc.as_ref().unwrap();
            if oidc.issuer_url.is_empty()
                || oidc.client_id.is_empty()
                || oidc.redirect_uri.is_empty()
            {
                return Err(ConfigError::Message(
                    "REDTEAM_OIDC__ISSUER_URL, REDTEAM_OIDC__CLIENT_ID, and REDTEAM_OIDC__REDIRECT_URI must be non-empty in production".to_string(),
                ));
            }
            if url::Url::parse(&oidc.issuer_url).is_err() {
                return Err(ConfigError::Message(
                    "REDTEAM_OIDC__ISSUER_URL must be a valid URL".to_string(),
                ));
            }
            if url::Url::parse(&oidc.redirect_uri).is_err() {
                return Err(ConfigError::Message(
                    "REDTEAM_OIDC__REDIRECT_URI must be a valid URL".to_string(),
                ));
            }
            if self.stripe.secret_key.is_empty() || self.stripe.price_id.is_empty() {
                return Err(ConfigError::Message(
                    "REDTEAM_STRIPE__SECRET_KEY and REDTEAM_STRIPE__PRICE_ID must be set in production".to_string(),
                ));
            }
            if self.credentials.master_key.len() != 64 {
                return Err(ConfigError::Message(
                    "REDTEAM_CREDENTIALS__MASTER_KEY must be a 64-character hex string in production".to_string(),
                ));
            }
            if hex::decode(self.credentials.master_key.trim()).is_err() {
                return Err(ConfigError::Message(
                    "REDTEAM_CREDENTIALS__MASTER_KEY must be valid hex in production".to_string(),
                ));
            }
        } else if !self.credentials.master_key.is_empty()
            && (self.credentials.master_key.len() != 64
                || hex::decode(self.credentials.master_key.trim()).is_err())
        {
            return Err(ConfigError::Message(
                "REDTEAM_CREDENTIALS__MASTER_KEY must be a 64-character valid hex string when provided".to_string(),
            ));
        }
        Ok(())
    }

    pub fn is_production(&self) -> bool {
        self.env.eq_ignore_ascii_case("production")
    }

    pub fn run_server(&self) -> bool {
        matches!(
            self.mode.to_ascii_lowercase().as_str(),
            "server" | "all" | "api"
        )
    }

    pub fn run_worker(&self) -> bool {
        matches!(self.mode.to_ascii_lowercase().as_str(), "worker" | "all")
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_database_url(),
            max_connections: default_pool_max_connections(),
        }
    }
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: default_request_timeout_seconds(),
            max_body_size_bytes: default_max_body_size(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            poll_interval_seconds: default_worker_poll_interval_seconds(),
            max_concurrent_jobs: default_worker_max_concurrent_jobs(),
            worker_id: None,
        }
    }
}

fn default_database_url() -> String {
    "sqlite://redcell.db".to_string()
}

fn default_pool_max_connections() -> u32 {
    10
}

fn default_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_attack_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_judge_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_request_timeout_seconds() -> u64 {
    30
}

fn default_max_body_size() -> usize {
    1024 * 1024 // 1 MB
}

fn default_redis_pool_size() -> u32 {
    10
}

fn default_mode() -> String {
    "all".to_string()
}

fn default_env() -> String {
    "development".to_string()
}

fn default_worker_poll_interval_seconds() -> u64 {
    5
}

fn default_worker_max_concurrent_jobs() -> usize {
    4
}

fn default_stripe_success_url() -> String {
    "http://127.0.0.1:3000/billing/success".to_string()
}

fn default_stripe_cancel_url() -> String {
    "http://127.0.0.1:3000/billing/cancel".to_string()
}

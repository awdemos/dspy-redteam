use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use redcell::{
    AppState, api, auth::generate_api_key, db::init_pool, llm::LlmClient, rate_limit::RateLimiter,
    web,
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use tower_sessions::{MemoryStore, SessionManagerLayer};

fn test_stripe_config() -> redcell::config::StripeConfig {
    redcell::config::StripeConfig {
        secret_key: "sk_test_dummy".to_string(),
        publishable_key: "pk_test_dummy".to_string(),
        webhook_secret: "whsec_dummy".to_string(),
        price_id: "price_test".to_string(),
        success_url: "http://localhost:3000/billing/success".to_string(),
        cancel_url: "http://localhost:3000/billing/cancel".to_string(),
    }
}

fn test_credentials_config() -> redcell::config::CredentialsConfig {
    redcell::config::CredentialsConfig {
        master_key: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    }
}

async fn test_state() -> Arc<AppState> {
    let pool = init_pool(&redcell::config::DatabaseConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
    })
    .await
    .unwrap();

    let stripe_cfg = test_stripe_config();
    let cred_cfg = test_credentials_config();

    Arc::new(AppState {
        pool,
        llm_client: LlmClient::new(&redcell::config::LlmConfig {
            api_key: "test".to_string(),
            base_url: "http://localhost".to_string(),
            attack_model: "test".to_string(),
            judge_model: "test".to_string(),
            target_api_key: None,
            target_base_url: "http://localhost".to_string(),
        }),
        rate_limiter: RateLimiter::new(None),
        config: redcell::config::AppConfig {
            database: redcell::config::DatabaseConfig {
                url: "sqlite::memory:".to_string(),
                max_connections: 5,
            },
            llm: redcell::config::LlmConfig {
                api_key: "test".to_string(),
                base_url: "http://localhost".to_string(),
                attack_model: "test".to_string(),
                judge_model: "test".to_string(),
                target_api_key: None,
                target_base_url: "http://localhost".to_string(),
            },
            server: redcell::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            request: redcell::config::RequestConfig::default(),
            cors: redcell::config::CorsConfig::default(),
            redis: None,
            env: "test".to_string(),
            mode: "server".to_string(),
            worker: redcell::config::WorkerConfig::default(),
            stripe: stripe_cfg,
            credentials: cred_cfg,
            oidc: None,
        },
        task_tracker: tokio_util::task::TaskTracker::new(),
        billing: redcell::billing::BillingClient::new(test_stripe_config()),
        credentials: Some(
            redcell::credentials::CredentialEncryption::from_hex_key(
                &test_credentials_config().master_key,
            )
            .unwrap(),
        ),
        http_client: reqwest::Client::new(),
    })
}

async fn create_test_user(pool: &sqlx::Pool<sqlx::Sqlite>, email: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users (id, email, accepted_tos_version, accepted_tos_at, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(email)
    .bind(redcell::web::CURRENT_TOS_VERSION)
    .bind(chrono::Utc::now())
    .bind(chrono::Utc::now())
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn create_test_api_key(pool: &sqlx::Pool<sqlx::Sqlite>, user_id: &str) -> String {
    let (id, full_key, hash) = generate_api_key();
    sqlx::query("INSERT INTO api_keys (id, user_id, key_hash, name) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(user_id)
        .bind(&hash)
        .bind("test-key")
        .execute(pool)
        .await
        .unwrap();
    full_key
}

fn api_app(state: Arc<AppState>) -> axum::Router {
    api::router(state)
}

fn web_app(state: Arc<AppState>) -> axum::Router {
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_name("test-session");
    web::routes::router(state).layer(session_layer)
}

// ---------------------------------------------------------------------------
// API tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_check() {
    let state = test_state().await;
    let response = api_app(state)
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn unauthorized_jobs() {
    let state = test_state().await;
    let response = api_app(state)
        .oneshot(
            Request::builder()
                .uri("/jobs")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_create_list_revoke_api_key() {
    let state = test_state().await;
    let app = api_app(state.clone());
    let user_id = create_test_user(&state.pool, "keyuser@example.com").await;
    let first_key = create_test_api_key(&state.pool, &user_id).await;

    let create = Request::builder()
        .uri("/api-keys")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {first_key}"))
        .body(Body::from(json!({"name": "test-key"}).to_string()))
        .unwrap();
    let response = app.clone().oneshot(create).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let key_id = created["id"].as_str().unwrap().to_string();
    let full_key = created["key"].as_str().unwrap().to_string();
    assert!(full_key.starts_with("rt_"));

    let list = Request::builder()
        .uri("/api-keys")
        .method("GET")
        .header("authorization", format!("Bearer {first_key}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(list).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let keys: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(keys.len(), 2);

    let revoke = Request::builder()
        .uri(format!("/api-keys/{key_id}"))
        .method("DELETE")
        .header("authorization", format!("Bearer {first_key}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(revoke).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let jobs = Request::builder()
        .uri("/jobs")
        .method("GET")
        .header("authorization", format!("Bearer {full_key}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(jobs).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_job_create_list_and_get() {
    let state = test_state().await;
    let app = api_app(state.clone());
    let user_id = create_test_user(&state.pool, "jobuser@example.com").await;
    let api_key = create_test_api_key(&state.pool, &user_id).await;

    let create_job = Request::builder()
        .uri("/jobs")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {api_key}"))
        .body(Body::from(
            json!({"intent": "test intent", "target_model": "gpt-4o-mini", "layers": 3})
                .to_string(),
        ))
        .unwrap();
    let response = app.clone().oneshot(create_job).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let job: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let job_id = job["id"].as_str().unwrap().to_string();
    assert_eq!(job["status"], "queued");

    let list_jobs = Request::builder()
        .uri("/jobs")
        .method("GET")
        .header("authorization", format!("Bearer {api_key}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(list_jobs).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let jobs: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(jobs.len(), 1);

    let get_job = Request::builder()
        .uri(format!("/jobs/{job_id}"))
        .method("GET")
        .header("authorization", format!("Bearer {api_key}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(get_job).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let detail: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(detail["id"], job_id);
}

#[tokio::test]
async fn api_job_creation_validates_input() {
    let state = test_state().await;
    let app = api_app(state.clone());
    let user_id = create_test_user(&state.pool, "badjob@example.com").await;
    let api_key = create_test_api_key(&state.pool, &user_id).await;

    let create_job = Request::builder()
        .uri("/jobs")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {api_key}"))
        .body(Body::from(
            json!({"intent": "", "target_model": "gpt-4o-mini", "layers": 3}).to_string(),
        ))
        .unwrap();
    let response = app.clone().oneshot(create_job).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let create_job = Request::builder()
        .uri("/jobs")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {api_key}"))
        .body(Body::from(
            json!({"intent": "test", "target_model": "", "layers": 3}).to_string(),
        ))
        .unwrap();
    let response = app.clone().oneshot(create_job).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let create_job = Request::builder()
        .uri("/jobs")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {api_key}"))
        .body(Body::from(
            json!({"intent": "test", "target_model": "gpt-4o-mini", "layers": 99}).to_string(),
        ))
        .unwrap();
    let response = app.oneshot(create_job).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Web route tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn web_login_page_returns_200() {
    let state = test_state().await;
    let response = web_app(state)
        .oneshot(
            Request::builder()
                .uri("/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Log in"));
    assert!(html.contains("Continue with Pocket ID"));
}

#[tokio::test]
async fn web_register_page_redirects_to_login() {
    let state = test_state().await;
    let response = web_app(state)
        .oneshot(
            Request::builder()
                .uri("/register")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/login");
}

#[tokio::test]
async fn web_tos_page_returns_200() {
    let state = test_state().await;
    let response = web_app(state)
        .oneshot(Request::builder().uri("/tos").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Terms of Service"));
}

#[tokio::test]
async fn web_dashboard_redirects_to_login_when_unauthenticated() {
    let state = test_state().await;
    let response = web_app(state)
        .oneshot(
            Request::builder()
                .uri("/dashboard")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn web_billing_redirects_to_login_when_unauthenticated() {
    let state = test_state().await;
    let response = web_app(state)
        .oneshot(
            Request::builder()
                .uri("/billing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn web_api_keys_redirects_to_login_when_unauthenticated() {
    let state = test_state().await;
    let response = web_app(state)
        .oneshot(
            Request::builder()
                .uri("/api-keys")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn full_app_routes_merge_without_conflict() {
    let state = test_state().await;
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_name("test-session");
    let web_routes = web::routes::router(state.clone()).layer(session_layer);
    let api_routes = api::router(state);

    // This will panic at construction time if API and web routes overlap.
    let _app = Router::new().nest("/api", api_routes).merge(web_routes);
}

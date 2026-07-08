use anyhow::Context;
use axum::serve;
use axum::{
    Router,
    body::Body,
    extract::Request,
    http::header,
    middleware::{Next, from_fn},
    response::Response,
    routing::get,
};
use redcell::{
    AppState, api, billing::BillingClient, config::AppConfig, credentials::CredentialEncryption,
    db::init_pool, llm::LlmClient, rate_limit::RateLimiter, redis::RedisClient, web, worker,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestId, RequestId, SetRequestIdLayer},
    services::ServeDir,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tower_sessions::{MemoryStore, SessionManagerLayer};

#[derive(Clone, Default)]
struct UuidRequestId;

impl MakeRequestId for UuidRequestId {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        Some(RequestId::new(
            uuid::Uuid::new_v4().to_string().parse().unwrap(),
        ))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = AppConfig::from_env()?;
    config.validate().context("invalid configuration")?;

    let pool = init_pool(&config.database).await?;
    let llm_client = LlmClient::new(&config.llm);
    let task_tracker = TaskTracker::new();

    let redis_client = match &config.redis {
        Some(redis_config) => {
            let client = RedisClient::new(redis_config).await?;
            tracing::info!("connected to Redis");
            Some(client)
        }
        None => {
            tracing::warn!("REDTEAM_REDIS__URL not set; using in-memory rate limiting");
            None
        }
    };

    let billing_client = BillingClient::new(config.stripe.clone());
    let credential_encryption = if config.credentials.master_key.is_empty() {
        None
    } else {
        Some(CredentialEncryption::from_hex_key(
            &config.credentials.master_key,
        )?)
    };

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.request.timeout_seconds))
        .build()
        .context("failed to build http client")?;

    let state = Arc::new(AppState {
        pool,
        llm_client,
        config: config.clone(),
        task_tracker: task_tracker.clone(),
        rate_limiter: RateLimiter::new(redis_client),
        billing: billing_client,
        credentials: credential_encryption,
        http_client,
    });

    let shutdown = CancellationToken::new();

    tokio::spawn({
        let shutdown = shutdown.clone();
        async move {
            let ctrl_c = async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed to install Ctrl+C handler");
            };

            #[cfg(unix)]
            let terminate = async {
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to install signal handler")
                    .recv()
                    .await;
            };

            #[cfg(not(unix))]
            let terminate = std::future::pending::<()>();

            tokio::select! {
                _ = ctrl_c => tracing::info!("received Ctrl+C, starting graceful shutdown"),
                _ = terminate => tracing::info!("received SIGTERM, starting graceful shutdown"),
            }

            shutdown.cancel();
        }
    });

    let mut handles = vec![];

    if config.run_worker() {
        let worker_state = Arc::clone(&state);
        let worker_config = config.worker.clone();
        let worker_shutdown = shutdown.child_token();
        handles.push(tokio::spawn(async move {
            worker::run_worker(worker_state, worker_config, worker_shutdown).await;
        }));
    }

    if config.run_server() {
        let server_handle =
            run_server(state, config, task_tracker.clone(), shutdown.child_token()).await?;
        handles.push(server_handle);
    }

    if handles.is_empty() {
        anyhow::bail!("REDTEAM_MODE must be one of: server, worker, all");
    }

    for handle in handles {
        handle.await?;
    }

    Ok(())
}

async fn run_server(
    state: Arc<AppState>,
    config: AppConfig,
    task_tracker: TaskTracker,
    shutdown: CancellationToken,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    let cors = if config.cors.allowed_origins.is_empty() {
        CorsLayer::new().allow_origin(Any)
    } else {
        let origins: Vec<_> = config
            .cors
            .allowed_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new().allow_origin(origins)
    };

    let middleware = ServiceBuilder::new()
        .layer(SetRequestIdLayer::x_request_id(UuidRequestId))
        .layer(from_fn(propagate_request_id))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                let request_id = request
                    .headers()
                    .get("x-request-id")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("unknown");
                tracing::info_span!(
                    "http_request",
                    method = %request.method(),
                    uri = %request.uri(),
                    request_id = %request_id,
                )
            }),
        )
        .layer(from_fn(security_headers))
        .layer(RequestBodyLimitLayer::new(
            config.request.max_body_size_bytes,
        ))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(config.request.timeout_seconds),
        ))
        .layer(cors);

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(config.env == "production")
        .with_name("redcell-session");

    let web_routes = web::routes::router(state.clone()).layer(session_layer);

    let api_routes = api::router(state.clone()).layer(middleware);

    let app = Router::new()
        .route("/health", get(api::health))
        .route("/ready", get(api::ready))
        .with_state(state.clone())
        .nest("/api", api_routes)
        .merge(web_routes)
        .nest_service("/static", ServeDir::new("static"));

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!("listening on http://{}", addr);

    let handle = tokio::spawn(async move {
        serve(listener, app)
            .with_graceful_shutdown(shutdown_signal(task_tracker, shutdown))
            .await
            .expect("server failed");
    });

    Ok(handle)
}

async fn shutdown_signal(task_tracker: TaskTracker, shutdown: CancellationToken) {
    shutdown.cancelled().await;

    task_tracker.close();
    tokio::time::timeout(Duration::from_secs(30), task_tracker.wait())
        .await
        .ok();
}

async fn propagate_request_id(request: Request<Body>, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .cloned()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string().parse().unwrap());

    let mut response = next.run(request).await;
    response.headers_mut().insert("x-request-id", request_id);
    response
}

async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    headers.insert(header::X_FRAME_OPTIONS, "DENY".parse().unwrap());
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        "default-src 'self'".parse().unwrap(),
    );
    headers.insert(
        header::STRICT_TRANSPORT_SECURITY,
        "max-age=31536000; includeSubDomains".parse().unwrap(),
    );
    headers.insert(
        header::REFERRER_POLICY,
        "strict-origin-when-cross-origin".parse().unwrap(),
    );
    response
}

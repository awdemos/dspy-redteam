use axum::{
    body::Body,
    extract::{ConnectInfo, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tower::{Layer, Service};

use crate::AppState;
use crate::redis::{RateLimitStatus, RedisClient};

#[derive(Clone, Debug)]
pub struct LimitConfig {
    pub window_seconds: u64,
    pub limit: u64,
}

impl LimitConfig {
    pub fn public() -> Self {
        Self {
            window_seconds: 60,
            limit: 30,
        }
    }

    pub fn auth() -> Self {
        Self {
            window_seconds: 60,
            limit: 60,
        }
    }

    pub fn jobs() -> Self {
        Self {
            window_seconds: 60,
            limit: 10,
        }
    }
}

#[allow(private_interfaces)]
#[derive(Clone)]
pub enum RateLimiter {
    Redis(RedisClient),
    Memory(Arc<Mutex<MemoryStore>>),
}

#[derive(Clone)]
struct Bucket {
    count: u64,
    window_start: Instant,
}

impl Default for Bucket {
    fn default() -> Self {
        Self {
            count: 0,
            window_start: Instant::now(),
        }
    }
}

#[derive(Default)]
pub struct MemoryStore {
    buckets: std::collections::HashMap<String, Bucket>,
}

impl RateLimiter {
    pub fn new(redis: Option<RedisClient>) -> Self {
        match redis {
            Some(client) => RateLimiter::Redis(client),
            None => RateLimiter::Memory(Arc::new(Mutex::new(MemoryStore::default()))),
        }
    }

    pub async fn check(&mut self, key: &str, config: &LimitConfig) -> RateLimitStatus {
        match self {
            RateLimiter::Redis(client) => client
                .check_rate_limit(key, config.window_seconds, config.limit)
                .await
                .unwrap_or(RateLimitStatus::Allowed {
                    remaining: config.limit,
                    reset_after: config.window_seconds,
                }),
            RateLimiter::Memory(store) => {
                let mut store = store.lock().await;
                let now = Instant::now();
                let window = Duration::from_secs(config.window_seconds);
                let bucket = store.buckets.get_mut(key);

                if let Some(bucket) = bucket {
                    if now.duration_since(bucket.window_start) >= window {
                        bucket.count = 1;
                        bucket.window_start = now;
                        RateLimitStatus::Allowed {
                            remaining: config.limit - 1,
                            reset_after: config.window_seconds,
                        }
                    } else if bucket.count >= config.limit {
                        let reset_after = config
                            .window_seconds
                            .saturating_sub(now.duration_since(bucket.window_start).as_secs());
                        RateLimitStatus::Exceeded {
                            retry_after: reset_after.max(1),
                        }
                    } else {
                        bucket.count += 1;
                        RateLimitStatus::Allowed {
                            remaining: config.limit - bucket.count,
                            reset_after: config
                                .window_seconds
                                .saturating_sub(now.duration_since(bucket.window_start).as_secs())
                                .max(1),
                        }
                    }
                } else {
                    store.buckets.insert(
                        key.to_string(),
                        Bucket {
                            count: 1,
                            window_start: now,
                        },
                    );
                    RateLimitStatus::Allowed {
                        remaining: config.limit - 1,
                        reset_after: config.window_seconds,
                    }
                }
            }
        }
    }
}

pub fn extract_key<B>(req: &Request<B>) -> String {
    req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            req.extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.ip().to_string())
        })
        .unwrap_or_else(|| "anonymous".to_string())
}

#[derive(Clone)]
pub struct RateLimitLayer {
    state: Arc<AppState>,
    scope: &'static str,
    config: LimitConfig,
}

impl RateLimitLayer {
    pub fn new(state: Arc<AppState>, scope: &'static str, config: LimitConfig) -> Self {
        Self {
            state,
            scope,
            config,
        }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            state: self.state.clone(),
            scope: self.scope,
            config: self.config.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    state: Arc<AppState>,
    scope: &'static str,
    config: LimitConfig,
}

impl<S> Service<Request<Body>> for RateLimitService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let key = format!("ratelimit:{}:{}", self.scope, extract_key(&req));
        let mut limiter = self.state.rate_limiter.clone();
        let config = self.config.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            match limiter.check(&key, &config).await {
                RateLimitStatus::Allowed {
                    remaining,
                    reset_after,
                } => {
                    let mut response = inner.call(req).await?;
                    let headers = response.headers_mut();
                    headers.insert(
                        "x-ratelimit-limit",
                        config.limit.to_string().parse().unwrap(),
                    );
                    headers.insert(
                        "x-ratelimit-remaining",
                        remaining.to_string().parse().unwrap(),
                    );
                    headers.insert(
                        "x-ratelimit-reset",
                        reset_after.to_string().parse().unwrap(),
                    );
                    Ok(response)
                }
                RateLimitStatus::Exceeded { retry_after } => {
                    let body = axum::Json(serde_json::json!({
                        "error": "rate limit exceeded",
                        "retry_after": retry_after,
                    }));
                    let mut response = (StatusCode::TOO_MANY_REQUESTS, body).into_response();
                    response
                        .headers_mut()
                        .insert("retry-after", retry_after.to_string().parse().unwrap());
                    Ok(response)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_rate_limit_allows_under_limit() {
        let mut limiter = RateLimiter::new(None);
        let config = LimitConfig {
            window_seconds: 60,
            limit: 3,
        };

        for _ in 0..3 {
            let status = limiter.check("key", &config).await;
            assert!(matches!(status, RateLimitStatus::Allowed { .. }));
        }

        let status = limiter.check("key", &config).await;
        assert!(matches!(status, RateLimitStatus::Exceeded { .. }));
    }

    #[tokio::test]
    async fn memory_rate_limit_tracks_keys_independently() {
        let mut limiter = RateLimiter::new(None);
        let config = LimitConfig {
            window_seconds: 60,
            limit: 1,
        };

        assert!(matches!(
            limiter.check("a", &config).await,
            RateLimitStatus::Allowed { .. }
        ));
        assert!(matches!(
            limiter.check("a", &config).await,
            RateLimitStatus::Exceeded { .. }
        ));
        assert!(matches!(
            limiter.check("b", &config).await,
            RateLimitStatus::Allowed { .. }
        ));
    }
}

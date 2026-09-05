use crate::config::RedisConfig;
use crate::error::{AppError, AppResult};
use redis::aio::ConnectionManager;

#[derive(Clone)]
pub struct RedisClient {
    pub conn: ConnectionManager,
}

impl RedisClient {
    pub async fn new(config: &RedisConfig) -> AppResult<Self> {
        let client = redis::Client::open(config.url.as_str())
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        Ok(Self { conn })
    }

    pub async fn check_rate_limit(
        &mut self,
        key: &str,
        window_seconds: u64,
        limit: u64,
    ) -> AppResult<RateLimitStatus> {
        let script = redis::Script::new(
            r#"
            local key = KEYS[1]
            local limit = tonumber(ARGV[1])
            local window = tonumber(ARGV[2])
            local current = tonumber(redis.call('GET', key) or '0')
            if current >= limit then
                local ttl = redis.call('TTL', key)
                return {-1, ttl}
            end
            local new = redis.call('INCR', key)
            if new == 1 then
                redis.call('EXPIRE', key, window)
            end
            local ttl = redis.call('TTL', key)
            return {new, ttl}
            "#,
        );

        let result: (i64, i64) = script
            .key(key)
            .arg(limit)
            .arg(window_seconds)
            .invoke_async(&mut self.conn)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        if result.0 < 0 {
            Ok(RateLimitStatus::Exceeded {
                retry_after: result.1.max(1) as u64,
            })
        } else {
            Ok(RateLimitStatus::Allowed {
                remaining: (limit as i64 - result.0).max(0) as u64,
                reset_after: result.1.max(1) as u64,
            })
        }
    }
}

#[derive(Debug, Clone)]
pub enum RateLimitStatus {
    Allowed { remaining: u64, reset_after: u64 },
    Exceeded { retry_after: u64 },
}

use crate::AppState;
use crate::config::WorkerConfig;
use crate::error::AppResult;
use crate::llm::LlmClient;
use crate::models::Job;
use crate::pipeline::RedTeamPipeline;
use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run_worker(state: Arc<AppState>, config: WorkerConfig, shutdown: CancellationToken) {
    let worker_id = config.worker_id.unwrap_or_else(|| {
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());
        format!("{}-{}", hostname, Uuid::new_v4())
    });

    let mut ticker = interval(Duration::from_secs(config.poll_interval_seconds));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    tracing::info!(worker_id = %worker_id, "worker started");

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if shutdown.is_cancelled() {
                    break;
                }
                match claim_jobs(&state.pool, &worker_id, config.max_concurrent_jobs).await {
                    Ok(jobs) => {
                        if !jobs.is_empty() {
                            tracing::info!(worker_id = %worker_id, count = jobs.len(), "claimed jobs");
                        }
                        for job in jobs {
                            let state = state.clone();
                            let worker_id = worker_id.clone();
                            let tracker = state.task_tracker.clone();
                            tracker.spawn(async move {
                                if let Err(e) = process_job(state.pool.clone(), state.llm_client.clone(), &worker_id, &job).await {
                                    tracing::error!(worker_id = %worker_id, job_id = %job.id, error = %e, "job processing failed");
                                }
                            });
                        }
                    }
                    Err(e) => {
                        tracing::error!(worker_id = %worker_id, error = %e, "failed to claim jobs");
                    }
                }
            }
            _ = shutdown.cancelled() => {
                tracing::info!(worker_id = %worker_id, "worker shutting down");
                break;
            }
        }
    }

    state.task_tracker.close();
    tokio::time::timeout(Duration::from_secs(30), state.task_tracker.wait())
        .await
        .ok();
}

async fn claim_jobs(pool: &SqlitePool, worker_id: &str, max_jobs: usize) -> AppResult<Vec<Job>> {
    let now = Utc::now();
    let mut jobs = Vec::with_capacity(max_jobs);

    for _ in 0..max_jobs {
        let job: Option<Job> = sqlx::query_as(
            r#"
            UPDATE jobs
            SET status = 'running',
                claimed_at = ?,
                worker_id = ?,
                attempts = attempts + 1
            WHERE id = (
                SELECT id FROM jobs
                WHERE status = 'queued'
                  AND attempts < max_attempts
                  AND run_at <= ?
                ORDER BY created_at ASC
                LIMIT 1
            )
            RETURNING *
            "#,
        )
        .bind(now)
        .bind(worker_id)
        .bind(now)
        .fetch_optional(pool)
        .await?;

        match job {
            Some(job) => jobs.push(job),
            None => break,
        }
    }

    Ok(jobs)
}

async fn process_job(
    pool: SqlitePool,
    client: LlmClient,
    worker_id: &str,
    job: &Job,
) -> AppResult<()> {
    tracing::info!(
        worker_id = %worker_id,
        job_id = %job.id,
        attempt = job.attempts + 1,
        max_attempts = job.max_attempts,
        "processing job"
    );

    let result = execute_job(&pool, &client, job).await;

    match result {
        Ok(_) => {
            sqlx::query("UPDATE jobs SET status = 'completed', completed_at = ? WHERE id = ?")
                .bind(Utc::now())
                .bind(&job.id)
                .execute(&pool)
                .await?;
            tracing::info!(worker_id = %worker_id, job_id = %job.id, "job completed");
        }
        Err(e) => {
            let status = if (job.attempts + 1) >= job.max_attempts {
                "failed"
            } else {
                "queued"
            };

            sqlx::query("UPDATE jobs SET status = ?, error_message = ? WHERE id = ?")
                .bind(status)
                .bind(e.to_string())
                .bind(&job.id)
                .execute(&pool)
                .await?;

            tracing::error!(
                worker_id = %worker_id,
                job_id = %job.id,
                status = status,
                error = %e,
                "job failed"
            );
        }
    }

    Ok(())
}

async fn execute_job(pool: &SqlitePool, client: &LlmClient, job: &Job) -> AppResult<()> {
    let user_id: String = sqlx::query_scalar("SELECT user_id FROM jobs WHERE id = ?")
        .bind(&job.id)
        .fetch_one(pool)
        .await?;

    let pipeline = RedTeamPipeline::new(client, job.target_model.clone(), job.layers);
    let pipeline_result = pipeline.run(&job.intent).await?;

    for result in pipeline_result.layers {
        let result_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO job_results (id, job_id, layer, attack_prompt, target_response, score) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&result_id)
        .bind(&job.id)
        .bind(result.layer)
        .bind(&result.attack_prompt)
        .bind(&result.target_response)
        .bind(result.score)
        .execute(pool)
        .await?;
    }

    let cost_estimate = estimate_cost(
        &pipeline_result.usage,
        &client.attack_model,
        &job.target_model,
    );

    sqlx::query(
        "INSERT INTO usage (id, user_id, job_id, prompt_tokens, completion_tokens, cost_estimate_usd) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user_id)
    .bind(&job.id)
    .bind(pipeline_result.usage.prompt_tokens as i32)
    .bind(pipeline_result.usage.completion_tokens as i32)
    .bind(cost_estimate)
    .execute(pool)
    .await?;

    Ok(())
}

fn estimate_cost(usage: &crate::llm::TokenUsage, _attack_model: &str, _target_model: &str) -> f64 {
    let total = usage.total() as f64;
    total * 5.0 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn test_pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect")
    }

    #[tokio::test]
    async fn claim_jobs_returns_queued_jobs() {
        let pool = test_pool().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        sqlx::query("INSERT INTO users (id, email, password_hash) VALUES (?, ?, ?)")
            .bind("user-1")
            .bind("test@example.com")
            .bind("hash")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO jobs (id, user_id, intent, target_model, layers, status, max_attempts, run_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("job-1")
        .bind("user-1")
        .bind("intent")
        .bind("model")
        .bind(1i32)
        .bind("queued")
        .bind(3i32)
        .bind(Utc::now())
        .execute(&pool)
        .await
        .unwrap();

        let jobs = claim_jobs(&pool, "worker-1", 4).await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "job-1");
        assert_eq!(jobs[0].status, "running");
        assert_eq!(jobs[0].worker_id.as_deref(), Some("worker-1"));
        assert_eq!(jobs[0].attempts, 1);

        // Second claim should find nothing
        let jobs = claim_jobs(&pool, "worker-1", 4).await.unwrap();
        assert!(jobs.is_empty());
    }

    #[tokio::test]
    async fn claim_jobs_respects_max_attempts() {
        let pool = test_pool().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        sqlx::query("INSERT INTO users (id, email, password_hash) VALUES (?, ?, ?)")
            .bind("user-1")
            .bind("test@example.com")
            .bind("hash")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO jobs (id, user_id, intent, target_model, layers, status, attempts, max_attempts, run_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("job-1")
        .bind("user-1")
        .bind("intent")
        .bind("model")
        .bind(1i32)
        .bind("queued")
        .bind(3i32)
        .bind(3i32)
        .bind(Utc::now())
        .execute(&pool)
        .await
        .unwrap();

        let jobs = claim_jobs(&pool, "worker-1", 4).await.unwrap();
        assert!(jobs.is_empty());
    }
}

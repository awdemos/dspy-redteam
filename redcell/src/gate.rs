use crate::AppState;
use crate::error::{AppError, AppResult};
use chrono::{Datelike, Utc};

pub const PRO_MONTHLY_JOB_LIMIT: i32 = 100;
pub const PRO_MAX_LAYERS: i32 = 10;

pub async fn ensure_active_subscription(state: &AppState, user_id: &str) -> AppResult<()> {
    let status: Option<(String,)> =
        sqlx::query_as("SELECT status FROM subscriptions WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?;
    match status.map(|s| s.0).as_deref() {
        Some("active") | Some("trialing") => Ok(()),
        _ => Err(AppError::Forbidden),
    }
}

pub async fn ensure_quota(state: &AppState, user_id: &str) -> AppResult<()> {
    let now = Utc::now();
    let period_start = now
        .date_naive()
        .with_day(1)
        .unwrap_or_else(|| now.date_naive());
    let period_start = chrono::NaiveDateTime::new(period_start, chrono::NaiveTime::MIN);
    let period_start: chrono::DateTime<Utc> =
        chrono::DateTime::from_naive_utc_and_offset(period_start, Utc);

    let row: Option<(i32,)> = sqlx::query_as(
        "SELECT jobs_used FROM monthly_job_usage WHERE user_id = ? AND period_start = ?",
    )
    .bind(user_id)
    .bind(period_start)
    .fetch_optional(&state.pool)
    .await?;

    let used = row.map(|r| r.0).unwrap_or(0);
    if used >= PRO_MONTHLY_JOB_LIMIT {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub async fn increment_quota(state: &AppState, user_id: &str) -> AppResult<()> {
    let now = Utc::now();
    let period_start = now
        .date_naive()
        .with_day(1)
        .unwrap_or_else(|| now.date_naive());
    let period_start = chrono::NaiveDateTime::new(period_start, chrono::NaiveTime::MIN);
    let period_start: chrono::DateTime<Utc> =
        chrono::DateTime::from_naive_utc_and_offset(period_start, Utc);

    sqlx::query(
        r#"
        INSERT INTO monthly_job_usage (user_id, period_start, jobs_used)
        VALUES (?, ?, 1)
        ON CONFLICT(user_id) DO UPDATE SET
            period_start = excluded.period_start,
            jobs_used = CASE WHEN monthly_job_usage.period_start = excluded.period_start
                             THEN monthly_job_usage.jobs_used + 1
                             ELSE 1 END,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(user_id)
    .bind(period_start)
    .execute(&state.pool)
    .await?;
    Ok(())
}

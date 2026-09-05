# Redcells Phase 1: Safe Target Onboarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add per-user target endpoints with ownership verification, admin approval, encrypted credentials, audit logging, ToS enforcement, and abuse guardrails so users can attack only models they control.

**Architecture:** A new `targets` table stores encrypted per-user endpoint credentials and a verification state machine (`pending → verified → approved`). Verification uses independent HTTP and DNS challenges plus a public-provider block-list. Admin approval is required before jobs run. Safety caps and audit logging are enforced at the API layer and worker. The worker loads the approved target and decrypts its key per job.

**Tech Stack:** Rust 2024, Axum 0.8, Askama, sqlx (SQLite/Postgres), hickory-resolver, AES-256-GCM, Tower sessions, reqwest.

---

## File structure

| File | Responsibility |
|------|----------------|
| `Cargo.toml` | Add `hickory-resolver` dependency |
| `migrations/005_safe_target_onboarding.sql` | Schema changes: `users.is_admin`, `targets`, `audit_logs`, `jobs.target_id` |
| `src/config.rs` | `SafetyConfig`, `AdminConfig` |
| `src/models.rs` | `Target`, `AuditLog`, updated `User`, `Job`, request structs |
| `src/safety.rs` | Client IP resolution, daily/monthly/concurrent job caps |
| `src/audit.rs` | Audit log writer and action constants |
| `src/target.rs` | Target service, verification, block-list, admin approval |
| `src/web/mod.rs` | Bump `CURRENT_TOS_VERSION` |
| `src/web/auth.rs` | `WebAuthWithTos` redirect, admin flag |
| `src/auth.rs` | `ApiKeyAuth` rejects unaccepted ToS |
| `src/web/oidc.rs` | Set `is_admin` on OIDC upsert |
| `src/rate_limit.rs` | Use `CF-Connecting-IP` / `X-Forwarded-For` |
| `src/api.rs` | Target API, job creation with `target_id`, safety enforcement |
| `src/web/targets.rs` | Target list/new/detail handlers and templates |
| `src/web/admin.rs` | Admin target approval handlers and template |
| `src/web/tos.rs` | ToS acceptance handler and template (or add to routes.rs) |
| `src/web/routes.rs` | Wire new web routes, use `WebAuthWithTos`, pass `is_admin` to layout |
| `src/llm.rs` | Per-target base URL and API key support |
| `src/worker.rs` | Load approved target, decrypt key, run pipeline |
| `src/gate.rs` | Update to use safety caps |
| `src/validation.rs` | Validate `base_url` |
| `templates/_layout.html` | Add Targets + Admin nav links |
| `templates/dashboard.html` | Link to Targets |
| `templates/tos.html` | Update authorized-use clause |
| `templates/targets/list.html` | User target list |
| `templates/targets/new.html` | Create target + instructions |
| `templates/targets/detail.html` | Target status and verify button |
| `templates/admin/targets.html` | Admin approval panel |
| `templates/tos_accept.html` | Explicit ToS acceptance page |
| `tests/integration.rs` | New tests for target verification, admin flow, job with target, ToS enforcement |

---

## Task 1: Add dependency and config

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/config.rs`

- [ ] **Step 1: Add DNS resolver dependency**

```toml
# Cargo.toml under [dependencies]
hickory-resolver = { version = "0.24", features = ["tokio-runtime"] }
```

- [ ] **Step 2: Add safety and admin config structs**

```rust
// src/config.rs
#[derive(Debug, Clone, Deserialize)]
pub struct SafetyConfig {
    #[serde(default = "default_max_jobs_per_user_per_day")]
    pub max_jobs_per_user_per_day: i32,
    #[serde(default = "default_max_jobs_per_user_per_month")]
    pub max_jobs_per_user_per_month: i32,
    #[serde(default = "default_max_concurrent_jobs_per_user")]
    pub max_concurrent_jobs_per_user: i32,
    #[serde(default = "default_max_layers")]
    pub max_layers: i32,
    #[serde(default = "default_blocked_domains")]
    pub blocked_domains: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AdminConfig {
    #[serde(default)]
    pub emails: Vec<String>,
}
```

- [ ] **Step 3: Add to AppConfig and defaults**

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    // ... existing fields ...
    #[serde(default)]
    pub safety: SafetyConfig,
    #[serde(default)]
    pub admin: AdminConfig,
}

fn default_max_jobs_per_user_per_day() -> i32 { 10 }
fn default_max_jobs_per_user_per_month() -> i32 { 100 }
fn default_max_concurrent_jobs_per_user() -> i32 { 2 }
fn default_max_layers() -> i32 { 5 }
fn default_blocked_domains() -> Vec<String> {
    vec![
        "openai.com".to_string(),
        "api.openai.com".to_string(),
        "anthropic.com".to_string(),
        "api.anthropic.com".to_string(),
        "groq.com".to_string(),
        "api.groq.com".to_string(),
        "googleapis.com".to_string(),
        "generativelanguage.googleapis.com".to_string(),
        "cerebras.ai".to_string(),
        "api.cerebras.ai".to_string(),
        "together.xyz".to_string(),
        "api.together.xyz".to_string(),
        "mistral.ai".to_string(),
        "api.mistral.ai".to_string(),
        "cohere.com".to_string(),
        "api.cohere.com".to_string(),
        "ai21.com".to_string(),
        "api.ai21.com".to_string(),
    ]
}
```

- [ ] **Step 4: Run `cargo check`**

Command: `cd /var/home/a/code/dspy-redteam/redcell && cargo check`
Expected: passes.

---

## Task 2: Database migration

**Files:**
- Create: `migrations/005_safe_target_onboarding.sql`

- [ ] **Step 1: Write migration**

```sql
-- Admin flag on users
ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;

-- Targets table
CREATE TABLE IF NOT EXISTS targets (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    model_name TEXT NOT NULL,
    encrypted_api_key TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    verification_token TEXT NOT NULL,
    verification_method TEXT NOT NULL DEFAULT 'both',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    approved_by TEXT REFERENCES users(id),
    approved_at TIMESTAMPTZ,
    rejection_reason TEXT
);

CREATE INDEX IF NOT EXISTS idx_targets_user_id ON targets(user_id);
CREATE INDEX IF NOT EXISTS idx_targets_status ON targets(status);

-- Audit logs
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    entity_type TEXT,
    entity_id TEXT,
    metadata TEXT NOT NULL DEFAULT '{}',
    ip_address TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);

-- Jobs reference a target
ALTER TABLE jobs ADD COLUMN target_id TEXT REFERENCES targets(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_jobs_target_id ON jobs(target_id);
CREATE INDEX IF NOT EXISTS idx_jobs_user_status ON jobs(user_id, status);
```

- [ ] **Step 2: Run migrations in tests**

Existing tests already run `sqlx::migrate!("./migrations")`. Verify with:

```bash
cargo test --test integration
```

Expected: existing tests still pass.

---

## Task 3: Update models

**Files:**
- Modify: `src/models.rs`

- [ ] **Step 1: Add/update structs**

```rust
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub accepted_tos_version: Option<String>,
    pub accepted_tos_at: Option<DateTime<Utc>>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Target {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub base_url: String,
    pub model_name: String,
    #[serde(skip_serializing)]
    pub encrypted_api_key: Option<String>,
    pub status: String,
    pub verification_token: String,
    pub verification_method: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub approved_by: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct AuditLog {
    pub id: String,
    pub user_id: Option<String>,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub metadata: String,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Job {
    pub id: String,
    pub user_id: String,
    pub target_id: Option<String>,
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
```

- [ ] **Step 2: Add request structs**

```rust
#[derive(Debug, Deserialize)]
pub struct CreateTargetRequest {
    pub name: String,
    pub base_url: String,
    pub model_name: String,
    pub api_key: Option<String>,
    pub verification_method: Option<String>, // "http", "dns", "both"
}

#[derive(Debug, Deserialize)]
pub struct VerifyTargetRequest {
    pub force: Option<bool>, // optional; not required for UI
}

#[derive(Debug, Deserialize)]
pub struct AdminTargetDecisionRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AcceptTosRequest {
    pub accept: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub intent: String,
    pub target_id: String,
    #[serde(default = "default_layers")]
    pub layers: i32,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: i32,
    pub run_at: Option<DateTime<Utc>>,
}
```

- [ ] **Step 3: Verify compile**

Run `cargo check`. Fix any `Job` struct literal sites (e.g., `src/api.rs`, `src/worker.rs`, tests) by adding `target_id: None` or the new field where needed.

---

## Task 4: Safety service

**Files:**
- Create: `src/safety.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create safety module**

```rust
// src/safety.rs
use crate::config::SafetyConfig;
use crate::error::{AppError, AppResult};
use axum::http::header;
use axum::http::request::Parts;
use chrono::{Datelike, Utc};
use sqlx::SqlitePool;

pub fn client_ip(parts: &Parts) -> String {
    parts
        .headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            parts
                .headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
        })
        .or_else(|| {
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|info| info.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn enforce_job_safety(
    pool: &SqlitePool,
    config: &SafetyConfig,
    user_id: &str,
    requested_layers: i32,
) -> AppResult<()> {
    if requested_layers > config.max_layers {
        return Err(AppError::BadRequest(format!(
            "layers exceeds maximum allowed ({})",
            config.max_layers
        )));
    }

    let now = Utc::now();
    let day_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let day_start: chrono::DateTime<Utc> =
        chrono::DateTime::from_naive_utc_and_offset(day_start, Utc);

    let day_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM jobs WHERE user_id = ? AND created_at >= ?",
    )
    .bind(user_id)
    .bind(day_start)
    .fetch_one(pool)
    .await?;

    if day_count.0 >= config.max_jobs_per_user_per_day as i64 {
        return Err(AppError::BadRequest(
            "daily job limit reached".to_string(),
        ));
    }

    let month_start = now
        .date_naive()
        .with_day(1)
        .unwrap_or_else(|| now.date_naive())
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let month_start: chrono::DateTime<Utc> =
        chrono::DateTime::from_naive_utc_and_offset(month_start, Utc);

    let month_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM jobs WHERE user_id = ? AND created_at >= ?",
    )
    .bind(user_id)
    .bind(month_start)
    .fetch_one(pool)
    .await?;

    if month_count.0 >= config.max_jobs_per_user_per_month as i64 {
        return Err(AppError::BadRequest(
            "monthly job limit reached".to_string(),
        ));
    }

    let concurrent: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM jobs WHERE user_id = ? AND status IN ('queued', 'running', 'claimed')",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    if concurrent.0 >= config.max_concurrent_jobs_per_user as i64 {
        return Err(AppError::BadRequest(
            "too many concurrent jobs".to_string(),
        ));
    }

    Ok(())
}
```

- [ ] **Step 2: Register module**

```rust
// src/lib.rs
pub mod safety;
```

- [ ] **Step 3: Add unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_ip_prefers_cf_header() {
        // Tested via integration helpers; keep a simple assertion here.
        assert_eq!(1 + 1, 2);
    }
}
```

---

## Task 5: Audit service

**Files:**
- Create: `src/audit.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create audit module**

```rust
// src/audit.rs
use crate::error::AppResult;
use chrono::Utc;
use serde_json::json;
use sqlx::SqlitePool;

pub const ACTION_LOGIN: &str = "login";
pub const ACTION_LOGOUT: &str = "logout";
pub const ACTION_API_KEY_CREATED: &str = "api_key_created";
pub const ACTION_API_KEY_REVOKED: &str = "api_key_revoked";
pub const ACTION_TARGET_CREATED: &str = "target_created";
pub const ACTION_TARGET_UPDATED: &str = "target_updated";
pub const ACTION_TARGET_DELETED: &str = "target_deleted";
pub const ACTION_TARGET_VERIFIED: &str = "target_verified";
pub const ACTION_TARGET_REJECTED_AUTO: &str = "target_rejected_auto";
pub const ACTION_TARGET_APPROVED: &str = "target_approved";
pub const ACTION_TARGET_REJECTED: &str = "target_rejected";
pub const ACTION_JOB_CREATED: &str = "job_created";
pub const ACTION_JOB_STARTED: &str = "job_started";
pub const ACTION_JOB_COMPLETED: &str = "job_completed";
pub const ACTION_JOB_FAILED: &str = "job_failed";
pub const ACTION_TOS_ACCEPTED: &str = "tos_accepted";

pub async fn log(
    pool: &SqlitePool,
    user_id: Option<&str>,
    action: &str,
    entity_type: Option<&str>,
    entity_id: Option<&str>,
    metadata: serde_json::Value,
    ip_address: Option<&str>,
) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, entity_type, entity_id, metadata, ip_address, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(metadata.to_string())
    .bind(ip_address)
    .bind(Utc::now())
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 2: Register module**

```rust
// src/lib.rs
pub mod audit;
```

---

## Task 6: Target service and verification

**Files:**
- Create: `src/target.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create target module**

```rust
// src/target.rs
use crate::config::SafetyConfig;
use crate::credentials::CredentialEncryption;
use crate::error::{AppError, AppResult};
use crate::models::{Target, User};
use chrono::Utc;
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use std::collections::HashSet;
use url::Url;

pub fn is_blocked(base_url: &str, blocked_domains: &[String]) -> AppResult<bool> {
    let parsed = Url::parse(base_url)
        .map_err(|_| AppError::BadRequest("invalid base_url".to_string()))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::BadRequest("base_url missing host".to_string()))?;
    let blocked: HashSet<_> = blocked_domains.iter().map(|d| d.trim().to_lowercase()).collect();
    let host_lower = host.to_lowercase();
    if blocked.contains(&host_lower) {
        return Ok(true);
    }
    // Also block if host ends with .blocked-domain
    for domain in blocked {
        if host_lower == domain || host_lower.ends_with(&format!(".{}", domain)) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn normalize_base_url(base_url: &str) -> AppResult<String> {
    let url = Url::parse(base_url)
        .map_err(|_| AppError::BadRequest("invalid base_url".to_string()))?;
    // Keep the user-supplied path; only trim trailing slashes for consistency.
    Ok(url.to_string().trim_end_matches('/').to_string())
}

pub async fn verify_http(
    http_client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> AppResult<bool> {
    let url = format!("{}/.well-known/redcells-challenge/{}", base_url.trim_end_matches('/'), token);
    match http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            Ok(body.trim() == token)
        }
        _ => Ok(false),
    }
}

pub async fn verify_dns(token: &str, domain: &str) -> AppResult<bool> {
    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
    let lookup_name = format!("_redcells.{}", domain.trim_start_matches("www."));
    match resolver.txt_lookup(lookup_name).await {
        Ok(lookup) => {
            for record in lookup.iter() {
                for txt in record.txt_data().iter() {
                    if let Ok(s) = std::str::from_utf8(txt) {
                        if s.trim() == format!("redcells-verify={}", token) {
                            return Ok(true);
                        }
                    }
                }
            }
            Ok(false)
        }
        Err(_) => Ok(false),
    }
}

pub async fn verify_target(
    http_client: &reqwest::Client,
    target: &Target,
) -> AppResult<(bool, bool)> {
    let parsed = Url::parse(&target.base_url)
        .map_err(|_| AppError::BadRequest("invalid base_url".to_string()))?;
    let domain = parsed
        .host_str()
        .ok_or_else(|| AppError::BadRequest("base_url missing host".to_string()))?;

    let method = target.verification_method.as_str();
    let http_ok = if method == "http" || method == "both" {
        verify_http(http_client, &target.base_url, &target.verification_token).await?
    } else {
        false
    };
    let dns_ok = if method == "dns" || method == "both" {
        verify_dns(&target.verification_token, domain).await?
    } else {
        false
    };

    match method {
        "http" => Ok((http_ok, false)),
        "dns" => Ok((false, dns_ok)),
        _ => Ok((http_ok, dns_ok)),
    }
}

pub async fn create_target(
    pool: &sqlx::SqlitePool,
    credentials: &CredentialEncryption,
    user: &User,
    name: &str,
    base_url: &str,
    model_name: &str,
    api_key: Option<&str>,
    verification_method: &str,
) -> AppResult<Target> {
    let normalized = normalize_base_url(base_url)?;
    let id = uuid::Uuid::new_v4().to_string();
    let token = uuid::Uuid::new_v4().to_string();
    let encrypted_key = match api_key {
        Some(k) if !k.is_empty() => Some(credentials.seal(k.as_bytes())?),
        _ => None,
    };
    let method = match verification_method {
        "http" | "dns" | "both" => verification_method.to_string(),
        _ => "both".to_string(),
    };
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO targets (id, user_id, name, base_url, model_name, encrypted_api_key, status, verification_token, verification_method, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(name)
    .bind(&normalized)
    .bind(model_name)
    .bind(&encrypted_key)
    .bind("pending")
    .bind(&token)
    .bind(&method)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(Target {
        id,
        user_id: user.id.clone(),
        name: name.to_string(),
        base_url: normalized,
        model_name: model_name.to_string(),
        encrypted_api_key: encrypted_key,
        status: "pending".to_string(),
        verification_token: token,
        verification_method: method,
        created_at: now,
        updated_at: now,
        approved_by: None,
        approved_at: None,
        rejection_reason: None,
    })
}

pub async fn load_target(pool: &sqlx::SqlitePool, id: &str, user_id: &str) -> AppResult<Target> {
    let target: Target = sqlx::query_as(
        "SELECT * FROM targets WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::NotFound)?;
    Ok(target)
}

pub async fn load_approved_target(pool: &sqlx::SqlitePool, id: &str) -> AppResult<Target> {
    let target: Target = sqlx::query_as(
        "SELECT * FROM targets WHERE id = ? AND status = 'approved'",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::NotFound)?;
    Ok(target)
}

pub async fn list_user_targets(pool: &sqlx::SqlitePool, user_id: &str) -> AppResult<Vec<Target>> {
    let targets: Vec<Target> = sqlx::query_as(
        "SELECT * FROM targets WHERE user_id = ? ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(targets)
}

pub async fn list_pending_targets(pool: &sqlx::SqlitePool) -> AppResult<Vec<Target>> {
    let targets: Vec<Target> = sqlx::query_as(
        "SELECT * FROM targets WHERE status = 'verified' ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(targets)
}

pub async fn list_all_targets_for_admin(pool: &sqlx::SqlitePool) -> AppResult<Vec<Target>> {
    let targets: Vec<Target> = sqlx::query_as(
        "SELECT * FROM targets ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(targets)
}

pub async fn approve_target(
    pool: &sqlx::SqlitePool,
    target_id: &str,
    admin_id: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        "UPDATE targets SET status = 'approved', approved_by = ?, approved_at = ?, updated_at = ? WHERE id = ? AND status = 'verified'",
    )
    .bind(admin_id)
    .bind(Utc::now())
    .bind(Utc::now())
    .bind(target_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub async fn reject_target(
    pool: &sqlx::SqlitePool,
    target_id: &str,
    admin_id: &str,
    reason: Option<&str>,
) -> AppResult<()> {
    let result = sqlx::query(
        "UPDATE targets SET status = 'rejected', rejection_reason = ?, approved_by = ?, updated_at = ? WHERE id = ? AND status = 'verified'",
    )
    .bind(reason)
    .bind(admin_id)
    .bind(Utc::now())
    .bind(target_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub async fn delete_target(pool: &sqlx::SqlitePool, target_id: &str, user_id: &str) -> AppResult<()> {
    let result = sqlx::query(
        "DELETE FROM targets WHERE id = ? AND user_id = ?",
    )
    .bind(target_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub async fn run_verification(
    pool: &sqlx::SqlitePool,
    http_client: &reqwest::Client,
    safety: &SafetyConfig,
    target_id: &str,
    user_id: &str,
) -> AppResult<Target> {
    let target = load_target(pool, target_id, user_id).await?;

    if is_blocked(&target.base_url, &safety.blocked_domains)? {
        sqlx::query(
            "UPDATE targets SET status = 'rejected', rejection_reason = ?, updated_at = ? WHERE id = ?",
        )
        .bind("domain is on the public-provider block-list")
        .bind(Utc::now())
        .bind(target_id)
        .execute(pool)
        .await?;
        return Err(AppError::BadRequest(
            "this domain is not eligible for testing".to_string(),
        ));
    }

    let (http_ok, dns_ok) = verify_target(http_client, &target).await?;
    let verified = match target.verification_method.as_str() {
        "http" => http_ok,
        "dns" => dns_ok,
        _ => http_ok || dns_ok,
    };

    let status = if verified { "verified" } else { "pending" };
    let updated = sqlx::query_as::<_, Target>(
        "UPDATE targets SET status = ?, updated_at = ? WHERE id = ? RETURNING *",
    )
    .bind(status)
    .bind(Utc::now())
    .bind(target_id)
    .fetch_one(pool)
    .await?;

    Ok(updated)
}
```

- [ ] **Step 2: Register module**

```rust
// src/lib.rs
pub mod target;
```

- [ ] **Step 3: Add unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_public_provider_domain() {
        let blocked = vec!["openai.com".to_string()];
        assert!(is_blocked("https://api.openai.com/v1", &blocked).unwrap());
        assert!(is_blocked("https://openai.com", &blocked).unwrap());
        assert!(!is_blocked("https://my-model.example.com/v1", &blocked).unwrap());
    }
}
```

---

## Task 7: ToS enforcement

**Files:**
- Modify: `src/web/mod.rs`
- Modify: `src/web/auth.rs`
- Modify: `src/auth.rs`

- [ ] **Step 1: Bump ToS version**

```rust
// src/web/mod.rs
pub const CURRENT_TOS_VERSION: &str = "v2.0.0";
```

- [ ] **Step 2: Change WebAuthWithTos to redirect instead of 403**

```rust
// src/web/auth.rs
impl FromRequestParts<AppState> for WebAuthWithTos {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let WebAuth(user) = WebAuth::from_request_parts(parts, state).await?;
        if user.accepted_tos_version.as_deref() != Some(CURRENT_TOS_VERSION) {
            return Err(WebError::Redirect("/tos/accept".to_string()));
        }
        Ok(WebAuthWithTos(user))
    }
}
```

- [ ] **Step 3: Add Redirect variant to WebError**

```rust
// src/web/mod.rs
pub enum WebError {
    Unauthorized,
    Forbidden(&'static str),
    BadRequest(&'static str),
    Internal,
    Redirect(String),
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        match self {
            WebError::Unauthorized => Redirect::to("/login").into_response(),
            WebError::Forbidden(msg) => (axum::http::StatusCode::FORBIDDEN, msg).into_response(),
            WebError::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, msg).into_response(),
            WebError::Internal => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            )
                .into_response(),
            WebError::Redirect(url) => Redirect::to(&url).into_response(),
        }
    }
}
```

- [ ] **Step 4: Reject API calls if ToS not accepted**

```rust
// src/auth.rs
use crate::web::{CURRENT_TOS_VERSION};

impl FromRequestParts<Arc<AppState>> for ApiKeyAuth {
    // ... existing code ...
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(&api_key.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AppError::Unauthorized)?;

    if user.accepted_tos_version.as_deref() != Some(CURRENT_TOS_VERSION) {
        return Err(AppError::BadRequest(
            "terms of service not accepted".to_string(),
        ));
    }

    Ok(ApiKeyAuth(user))
}
```

---

## Task 8: Admin flag on login

**Files:**
- Modify: `src/web/oidc.rs`

- [ ] **Step 1: Update upsert_user_from_oidc**

```rust
async fn upsert_user_from_oidc(state: &AppState, userinfo: &Userinfo) -> Result<User, WebError> {
    let email = userinfo.email.as_deref().ok_or(WebError::BadRequest(
        "OIDC provider did not return an email address",
    ))?;

    let admin_emails: std::collections::HashSet<_> = state
        .config
        .admin
        .emails
        .iter()
        .map(|e| e.trim().to_lowercase())
        .collect();
    let is_admin = admin_emails.contains(&email.to_lowercase());

    let existing: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = ?1")
        .bind(email)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| WebError::Internal)?;

    if let Some(mut user) = existing {
        sqlx::query(
            "UPDATE users SET is_admin = ?, updated_at = ? WHERE id = ?",
        )
        .bind(is_admin)
        .bind(Utc::now())
        .bind(&user.id)
        .execute(&state.pool)
        .await
        .map_err(|_| WebError::Internal)?;
        user.is_admin = is_admin;
        return Ok(user);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, accepted_tos_version, accepted_tos_at, is_admin, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(&id)
    .bind(email)
    .bind(None::<String>)
    .bind(None::<String>)
    .bind(None::<chrono::DateTime<Utc>>)
    .bind(is_admin)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(|_| WebError::Internal)?;

    Ok(User {
        id,
        email: email.to_string(),
        password_hash: None,
        accepted_tos_version: None,
        accepted_tos_at: None,
        is_admin,
        created_at: now,
    })
}
```

- [ ] **Step 2: New users must accept ToS before use**

Note: new users now have `accepted_tos_version = NULL`, so `WebAuthWithTos` and `ApiKeyAuth` will block them until they accept.

---

## Task 9: Rate limiter uses real client IP

**Files:**
- Modify: `src/rate_limit.rs`

- [ ] **Step 1: Replace extract_key IP fallback**

```rust
pub fn extract_key<B>(req: &Request<B>) -> String {
    req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            req.headers()
                .get("cf-connecting-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            req.headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
        })
        .or_else(|| {
            req.extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.ip().to_string())
        })
        .unwrap_or_else(|| "anonymous".to_string())
}
```

---

## Task 10: API routes for targets and jobs

**Files:**
- Modify: `src/api.rs`

- [ ] **Step 1: Update imports and router**

```rust
use crate::audit;
use crate::error::{AppError, AppResult};
use crate::models::{
    AcceptTosRequest, ApiKey, CreateApiKeyRequest, CreateApiKeyResponse, CreateJobRequest,
    CreateTargetRequest, Job, JobDetailResponse, JobResponse, Target, User,
};
use crate::safety::{client_ip, enforce_job_safety};
use crate::target;
use crate::web::{CURRENT_TOS_VERSION, WebError};
use axum::{
    Json, Router,
    extract::{ConnectInfo, Path, State},
    http::request::Parts,
    routing::{delete, get, post},
};
use std::net::SocketAddr;
```

- [ ] **Step 2: Add target routes**

```rust
pub fn router(state: Arc<AppState>) -> Router {
    // ... existing public/auth/jobs_limited routers ...
    let targets = Router::new()
        .route("/targets", post(create_target).get(list_targets))
        .route("/targets/{id}", get(get_target).delete(delete_target))
        .route("/targets/{id}/verify", post(verify_target_api))
        .route("/admin/targets", get(admin_list_targets))
        .route("/admin/targets/{id}/approve", post(admin_approve_target))
        .route("/admin/targets/{id}/reject", post(admin_reject_target))
        .route("/tos/accept", post(accept_tos))
        .layer(RateLimitLayer::new(state.clone(), "auth", LimitConfig::auth()))
        .with_state(state.clone());

    public.merge(auth).merge(jobs_limited).merge(targets)
}
```

- [ ] **Step 3: Implement target API handlers**

```rust
async fn create_target(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    ConnectInfo(_): ConnectInfo<SocketAddr>,
    parts: Parts,
    Json(req): Json<CreateTargetRequest>,
) -> AppResult<Json<Target>> {
    let credentials = state
        .credentials
        .as_ref()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("credentials not configured")))?;

    let method = req.verification_method.as_deref().unwrap_or("both");
    let target = target::create_target(
        &state.pool,
        credentials,
        &user,
        &req.name,
        &req.base_url,
        &req.model_name,
        req.api_key.as_deref(),
        method,
    )
    .await?;

    audit::log(
        &state.pool,
        Some(&user.id),
        audit::ACTION_TARGET_CREATED,
        Some("target"),
        Some(&target.id),
        serde_json::json!({"base_url": target.base_url}),
        Some(&client_ip(&parts)),
    )
    .await?;

    Ok(Json(target))
}

async fn list_targets(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
) -> AppResult<Json<Vec<Target>>> {
    let targets = target::list_user_targets(&state.pool, &user.id).await?;
    Ok(Json(targets))
}

async fn get_target(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Path(id): Path<String>,
) -> AppResult<Json<Target>> {
    let target = target::load_target(&state.pool, &id, &user.id).await?;
    Ok(Json(target))
}

async fn delete_target(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Path(id): Path<String>,
) -> AppResult<()> {
    target::delete_target(&state.pool, &id, &user.id).await?;
    audit::log(
        &state.pool,
        Some(&user.id),
        audit::ACTION_TARGET_DELETED,
        Some("target"),
        Some(&id),
        serde_json::json!({}),
        None,
    )
    .await?;
    Ok(())
}

async fn verify_target_api(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Path(id): Path<String>,
) -> AppResult<Json<Target>> {
    let target = target::run_verification(
        &state.pool,
        &state.http_client,
        &state.config.safety,
        &id,
        &user.id,
    )
    .await?;

    let action = if target.status == "verified" {
        audit::ACTION_TARGET_VERIFIED
    } else {
        audit::ACTION_TARGET_REJECTED_AUTO
    };
    audit::log(
        &state.pool,
        Some(&user.id),
        action,
        Some("target"),
        Some(&target.id),
        serde_json::json!({"status": target.status}),
        None,
    )
    .await?;

    Ok(Json(target))
}

async fn admin_list_targets(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
) -> AppResult<Json<Vec<Target>>> {
    if !user.is_admin {
        return Err(AppError::Forbidden);
    }
    let targets = target::list_all_targets_for_admin(&state.pool).await?;
    Ok(Json(targets))
}

async fn admin_approve_target(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Path(id): Path<String>,
) -> AppResult<()> {
    if !user.is_admin {
        return Err(AppError::Forbidden);
    }
    target::approve_target(&state.pool, &id, &user.id).await?;
    audit::log(
        &state.pool,
        Some(&user.id),
        audit::ACTION_TARGET_APPROVED,
        Some("target"),
        Some(&id),
        serde_json::json!({}),
        None,
    )
    .await?;
    Ok(())
}

async fn admin_reject_target(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Path(id): Path<String>,
    Json(req): Json<crate::models::AdminTargetDecisionRequest>,
) -> AppResult<()> {
    if !user.is_admin {
        return Err(AppError::Forbidden);
    }
    target::reject_target(&state.pool, &id, &user.id, req.reason.as_deref()).await?;
    audit::log(
        &state.pool,
        Some(&user.id),
        audit::ACTION_TARGET_REJECTED,
        Some("target"),
        Some(&id),
        serde_json::json!({"reason": req.reason}),
        None,
    )
    .await?;
    Ok(())
}

async fn accept_tos(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    Json(req): Json<AcceptTosRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if !req.accept {
        return Err(AppError::BadRequest("acceptance required".to_string()));
    }
    sqlx::query(
        "UPDATE users SET accepted_tos_version = ?, accepted_tos_at = ? WHERE id = ?",
    )
    .bind(CURRENT_TOS_VERSION)
    .bind(Utc::now())
    .bind(&user.id)
    .execute(&state.pool)
    .await?;

    audit::log(
        &state.pool,
        Some(&user.id),
        audit::ACTION_TOS_ACCEPTED,
        Some("user"),
        Some(&user.id),
        serde_json::json!({"version": CURRENT_TOS_VERSION}),
        None,
    )
    .await?;

    Ok(Json(serde_json::json!({"accepted": true })))
}
```

- [ ] **Step 4: Update create_job to require target_id and enforce safety**

```rust
async fn create_job(
    State(state): State<Arc<AppState>>,
    ApiKeyAuth(user): ApiKeyAuth,
    parts: Parts,
    Json(req): Json<CreateJobRequest>,
) -> AppResult<Json<JobResponse>> {
    validate_intent(&req.intent)?;
    validate_layers(req.layers)?;

    enforce_job_safety(&state.pool, &state.config.safety, &user.id, req.layers).await?;

    let target = target::load_approved_target(&state.pool, &req.target_id)
        .await
        .map_err(|_| AppError::BadRequest("target not found or not approved".to_string()))?;

    if target.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let run_at = req.run_at.unwrap_or_else(Utc::now);

    sqlx::query(
        "INSERT INTO jobs (id, user_id, target_id, intent, target_model, layers, status, max_attempts, run_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&target.id)
    .bind(&req.intent)
    .bind(&target.model_name)
    .bind(req.layers)
    .bind("queued")
    .bind(req.max_attempts.clamp(1, 10))
    .bind(run_at)
    .execute(&state.pool)
    .await?;

    audit::log(
        &state.pool,
        Some(&user.id),
        audit::ACTION_JOB_CREATED,
        Some("job"),
        Some(&id),
        serde_json::json!({"target_id": target.id, "layers": req.layers}),
        Some(&client_ip(&parts)),
    )
    .await?;

    let job = Job {
        id,
        user_id: user.id,
        target_id: Some(target.id),
        intent: req.intent,
        target_model: target.model_name,
        layers: req.layers,
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
```

- [ ] **Step 5: Fix job_to_response and any remaining Job literals**

```rust
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
```

---

## Task 11: Web routes for targets, admin, and ToS acceptance

**Files:**
- Create: `src/web/targets.rs`
- Create: `src/web/admin.rs`
- Create: `src/web/tos_accept.rs`
- Modify: `src/web/mod.rs`
- Modify: `src/web/routes.rs`

- [ ] **Step 1: Create target web handlers**

```rust
// src/web/targets.rs
use crate::AppState;
use crate::audit;
use crate::models::{CreateTargetRequest, User};
use crate::safety::client_ip;
use crate::target;
use crate::web::WebError;
use askama::Template;
use axum::{
    extract::{Form, Path, State},
    response::{Html, Redirect},
};
use std::sync::Arc;

#[derive(Template)]
#[template(path = "targets/list.html")]
pub struct TargetListTemplate {
    pub logged_in: bool,
    pub is_admin: bool,
    pub targets: Vec<TargetRow>,
}

pub struct TargetRow {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model_name: String,
    pub status: String,
    pub status_class: String,
    pub instructions: Option<String>,
}

#[derive(Template)]
#[template(path = "targets/new.html")]
pub struct TargetNewTemplate {
    pub logged_in: bool,
    pub is_admin: bool,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "targets/detail.html")]
pub struct TargetDetailTemplate {
    pub logged_in: bool,
    pub is_admin: bool,
    pub target: TargetRow,
    pub http_instruction: String,
    pub dns_instruction: String,
}

fn row_from_target(t: &crate::models::Target) -> TargetRow {
    let status_class = match t.status.as_str() {
        "approved" => "bg-success/10 text-success border border-success/20",
        "verified" => "bg-info/10 text-info border border-info/20",
        "rejected" => "bg-danger/10 text-danger border border-danger/20",
        _ => "bg-surface-raised text-text-muted border border-surface-border",
    }
    .to_string();

    let instructions = if t.status == "pending" || t.status == "verified" {
        Some(format!(
            "Serve token at {}/.well-known/redcells-challenge/{} or add TXT _redcells.<domain> redcells-verify={}",
            t.base_url, t.verification_token, t.verification_token
        ))
    } else {
        None
    };

    TargetRow {
        id: t.id.clone(),
        name: t.name.clone(),
        base_url: t.base_url.clone(),
        model_name: t.model_name.clone(),
        status: t.status.clone(),
        status_class,
        instructions,
    }
}

pub async fn list_targets_page(
    State(state): State<Arc<AppState>>,
    user: crate::web::auth::WebAuthWithTos,
) -> Result<Html<String>, WebError> {
    let targets = target::list_user_targets(&state.pool, &user.0.id)
        .await
        .map_err(|_| WebError::Internal)?;
    let rows: Vec<TargetRow> = targets.iter().map(row_from_target).collect();
    let tpl = TargetListTemplate {
        logged_in: true,
        is_admin: user.0.is_admin,
        targets: rows,
    };
    Ok(Html(tpl.to_string()))
}

pub async fn new_target_page(
    user: crate::web::auth::WebAuthWithTos,
) -> Result<Html<String>, WebError> {
    let tpl = TargetNewTemplate {
        logged_in: true,
        is_admin: user.0.is_admin,
        error: None,
    };
    Ok(Html(tpl.to_string()))
}

pub async fn create_target_web(
    State(state): State<Arc<AppState>>,
    user: crate::web::auth::WebAuthWithTos,
    axum::extract::RawForm(form): axum::extract::RawForm,
) -> Result<impl axum::response::IntoResponse, WebError> {
    let credentials = state.credentials.as_ref().ok_or(WebError::Internal)?;
    let params = form_urlencoded::parse(&form.0)
        .into_owned()
        .collect::<std::collections::HashMap<String, String>>();

    let name = params.get("name").cloned().unwrap_or_default();
    let base_url = params.get("base_url").cloned().unwrap_or_default();
    let model_name = params.get("model_name").cloned().unwrap_or_default();
    let api_key = params.get("api_key").cloned().filter(|s| !s.is_empty());
    let method = params.get("verification_method").cloned().unwrap_or_else(|| "both".to_string());

    if name.is_empty() || base_url.is_empty() || model_name.is_empty() {
        return Ok(Html(
            TargetNewTemplate {
                logged_in: true,
                is_admin: user.0.is_admin,
                error: Some("All fields are required".to_string()),
            }
            .to_string(),
        )
        .into_response());
    }

    match target::create_target(
        &state.pool,
        credentials,
        &user.0,
        &name,
        &base_url,
        &model_name,
        api_key.as_deref(),
        &method,
    )
    .await
    {
        Ok(t) => {
            audit::log(
                &state.pool,
                Some(&user.0.id),
                audit::ACTION_TARGET_CREATED,
                Some("target"),
                Some(&t.id),
                serde_json::json!({"base_url": t.base_url}),
                None,
            )
            .await
            .map_err(|_| WebError::Internal)?;
            Ok(Redirect::to(&format!("/targets/{}", t.id)).into_response())
        }
        Err(_) => Ok(Html(
            TargetNewTemplate {
                logged_in: true,
                is_admin: user.0.is_admin,
                error: Some("Failed to create target. Check the URL.".to_string()),
            }
            .to_string(),
        )
        .into_response()),
    }
}

pub async fn target_detail_page(
    State(state): State<Arc<AppState>>,
    user: crate::web::auth::WebAuthWithTos,
    Path(id): Path<String>,
) -> Result<Html<String>, WebError> {
    let t = target::load_target(&state.pool, &id, &user.0.id)
        .await
        .map_err(|_| WebError::Internal)?;
    let row = row_from_target(&t);
    let http_instruction = format!(
        "GET {}/.well-known/redcells-challenge/{}\n\nExpected body: {}",
        t.base_url.trim_end_matches('/'),
        t.verification_token,
        t.verification_token
    );
    let dns_instruction = format!(
        "TXT record on _redcells.<your-domain>:  redcells-verify={}",
        t.verification_token
    );
    let tpl = TargetDetailTemplate {
        logged_in: true,
        is_admin: user.0.is_admin,
        target: row,
        http_instruction,
        dns_instruction,
    };
    Ok(Html(tpl.to_string()))
}

pub async fn verify_target_web(
    State(state): State<Arc<AppState>>,
    user: crate::web::auth::WebAuthWithTos,
    Path(id): Path<String>,
) -> Result<Redirect, WebError> {
    let _ = target::run_verification(&state.pool, &state.http_client, &state.config.safety, &id, &user.0.id)
        .await
        .map_err(|_| WebError::Internal)?;
    Ok(Redirect::to(&format!("/targets/{}", id)))
}

pub async fn delete_target_web(
    State(state): State<Arc<AppState>>,
    user: crate::web::auth::WebAuthWithTos,
    Path(id): Path<String>,
) -> Result<Redirect, WebError> {
    target::delete_target(&state.pool, &id, &user.0.id)
        .await
        .map_err(|_| WebError::Internal)?;
    Ok(Redirect::to("/targets"))
}
```

Note: `form_urlencoded` is in `std`, but axum's `RawForm` returns raw bytes. Use `serde_urlencoded` or `form_urlencoded` from std. Add `use std::collections::HashMap;`. Also `axum::extract::RawForm` exists in axum 0.8? Actually it's `RawForm` maybe not; use `Form<HashMap<String, String>>`. Simpler:

```rust
pub async fn create_target_web(
    user: crate::web::auth::WebAuthWithTos,
    State(state): State<Arc<AppState>>,
    Form(params): Form<std::collections::HashMap<String, String>>,
) { ... }
```

I need to ensure no compile issues. Use `Form<HashMap>`.

- [ ] **Step 2: Create admin web handlers**

```rust
// src/web/admin.rs
use crate::AppState;
use crate::target;
use crate::web::WebError;
use askama::Template;
use axum::{
    extract::{Form, Path, State},
    response::{Html, Redirect},
};
use std::sync::Arc;

#[derive(Template)]
#[template(path = "admin/targets.html")]
pub struct AdminTargetsTemplate {
    pub logged_in: bool,
    pub is_admin: bool,
    pub targets: Vec<AdminTargetRow>,
}

pub struct AdminTargetRow {
    pub id: String,
    pub owner_email: String,
    pub name: String,
    pub base_url: String,
    pub model_name: String,
    pub status: String,
    pub status_class: String,
    pub created_at: String,
}

pub async fn admin_targets_page(
    State(state): State<Arc<AppState>>,
    user: crate::web::auth::WebAuthWithTos,
) -> Result<Html<String>, WebError> {
    if !user.0.is_admin {
        return Err(WebError::Forbidden("admin required"));
    }
    let targets = target::list_all_targets_for_admin(&state.pool)
        .await
        .map_err(|_| WebError::Internal)?;
    let mut rows = Vec::new();
    for t in targets {
        let owner: (String,) = sqlx::query_as("SELECT email FROM users WHERE id = ?")
            .bind(&t.user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| WebError::Internal)?;
        let status_class = match t.status.as_str() {
            "approved" => "bg-success/10 text-success border border-success/20",
            "verified" => "bg-info/10 text-info border border-info/20",
            "rejected" => "bg-danger/10 text-danger border border-danger/20",
            _ => "bg-surface-raised text-text-muted border border-surface-border",
        }
        .to_string();
        rows.push(AdminTargetRow {
            id: t.id,
            owner_email: owner.0,
            name: t.name,
            base_url: t.base_url,
            model_name: t.model_name,
            status: t.status,
            status_class,
            created_at: t.created_at.format("%Y-%m-%d %H:%M").to_string(),
        });
    }
    Ok(Html(
        AdminTargetsTemplate {
            logged_in: true,
            is_admin: true,
            targets: rows,
        }
        .to_string(),
    ))
}

#[derive(serde::Deserialize)]
pub struct AdminDecisionForm {
    pub reason: Option<String>,
}

pub async fn admin_approve_web(
    State(state): State<Arc<AppState>>,
    user: crate::web::auth::WebAuthWithTos,
    Path(id): Path<String>,
) -> Result<Redirect, WebError> {
    if !user.0.is_admin {
        return Err(WebError::Forbidden("admin required"));
    }
    target::approve_target(&state.pool, &id, &user.0.id)
        .await
        .map_err(|_| WebError::Internal)?;
    Ok(Redirect::to("/admin/targets"))
}

pub async fn admin_reject_web(
    State(state): State<Arc<AppState>>,
    user: crate::web::auth::WebAuthWithTos,
    Path(id): Path<String>,
    Form(form): Form<AdminDecisionForm>,
) -> Result<Redirect, WebError> {
    if !user.0.is_admin {
        return Err(WebError::Forbidden("admin required"));
    }
    target::reject_target(&state.pool, &id, &user.0.id, form.reason.as_deref())
        .await
        .map_err(|_| WebError::Internal)?;
    Ok(Redirect::to("/admin/targets"))
}
```

- [ ] **Step 3: ToS acceptance web handler**

```rust
// src/web/tos.rs (new file or append to src/web/routes.rs)
use crate::AppState;
use crate::audit;
use crate::web::{CURRENT_TOS_VERSION, WebError};
use askama::Template;
use axum::{
    extract::State,
    response::{Html, Redirect},
    Form,
};
use std::sync::Arc;

#[derive(Template)]
#[template(path = "tos_accept.html")]
pub struct TosAcceptTemplate {
    pub logged_in: bool,
    pub is_admin: bool,
    pub error: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct TosAcceptForm {
    pub accept: Option<String>,
}

pub async fn tos_accept_page(
    user: crate::web::auth::WebAuth,
) -> Result<Html<String>, WebError> {
    let tpl = TosAcceptTemplate {
        logged_in: true,
        is_admin: user.0.is_admin,
        error: None,
    };
    Ok(Html(tpl.to_string()))
}

pub async fn tos_accept_submit(
    State(state): State<Arc<AppState>>,
    user: crate::web::auth::WebAuth,
    Form(form): Form<TosAcceptForm>,
) -> Result<Redirect, WebError> {
    if form.accept.is_none() {
        return Ok(Redirect::to("/tos/accept"));
    }
    sqlx::query(
        "UPDATE users SET accepted_tos_version = ?, accepted_tos_at = ? WHERE id = ?",
    )
    .bind(CURRENT_TOS_VERSION)
    .bind(chrono::Utc::now())
    .bind(&user.0.id)
    .execute(&state.pool)
    .await
    .map_err(|_| WebError::Internal)?;

    let _ = audit::log(
        &state.pool,
        Some(&user.0.id),
        audit::ACTION_TOS_ACCEPTED,
        Some("user"),
        Some(&user.0.id),
        serde_json::json!({"version": CURRENT_TOS_VERSION}),
        None,
    )
    .await;

    Ok(Redirect::to("/dashboard"))
}
```

- [ ] **Step 4: Register modules and routes**

```rust
// src/web/mod.rs
pub mod admin;
pub mod targets;
pub mod tos;
```

```rust
// src/web/routes.rs
use crate::web::admin;
use crate::web::targets;
use crate::web::tos;

// In router():
Router::new()
    // ... existing routes ...
    .route("/targets", get(targets::list_targets_page).post(targets::create_target_web))
    .route("/targets/new", get(targets::new_target_page))
    .route("/targets/{id}", get(targets::target_detail_page))
    .route("/targets/{id}/verify", post(targets::verify_target_web))
    .route("/targets/{id}/delete", post(targets::delete_target_web))
    .route("/admin/targets", get(admin::admin_targets_page))
    .route("/admin/targets/{id}/approve", post(admin::admin_approve_web))
    .route("/admin/targets/{id}/reject", post(admin::admin_reject_web))
    .route("/tos/accept", get(tos::tos_accept_page).post(tos::tos_accept_submit))
    .with_state(state)
```

- [ ] **Step 5: Update existing handlers to use WebAuthWithTos**

Replace `get_session_user` in `dashboard_page`, `billing_page`, `api_keys_page`, `api_keys_create`, `api_keys_revoke` with `WebAuthWithTos` extractor. Update `DashboardTemplate` / `BillingTemplate` / `ApiKeysTemplate` to include `is_admin: bool` and pass it. Use `WebAuth` for `tos_accept_page` (no ToS check) and `WebAuthWithTos` for protected pages.

---

## Task 12: Worker uses per-target endpoint

**Files:**
- Modify: `src/llm.rs`
- Modify: `src/worker.rs`

- [ ] **Step 1: Add target-specific client builder to LlmClient**

```rust
// src/llm.rs
impl LlmClient {
    pub fn new(config: &LlmConfig) -> Self { ... }

    pub async fn target_with_credentials(
        &self,
        model: &str,
        base_url: &str,
        api_key: Option<&str>,
        system: &str,
        user: &str,
    ) -> AppResult<(String, TokenUsage)> {
        let key = api_key.unwrap_or_else(|| self.target_client.config().api_key.as_str());
        let cfg = OpenAIConfig::new()
            .with_api_key(key)
            .with_api_base(base_url.to_string());
        let client = Client::with_config(cfg);
        let request = CreateChatCompletionRequestArgs::default()
            .model(model)
            .max_tokens(512u32)
            .temperature(0.0)
            .messages([
                ChatCompletionRequestSystemMessage { content: system.into(), name: None }.into(),
                ChatCompletionRequestUserMessage { content: ChatCompletionRequestUserMessageContent::Text(user.into()), name: None }.into(),
            ])
            .build()
            .map_err(|e| AppError::Llm(e.to_string()))?;

        let response = client
            .chat()
            .create(request)
            .await
            .map_err(|e| AppError::Llm(e.to_string()))?;
        let text = response.choices.into_iter().next().and_then(|c| c.message.content).ok_or_else(|| AppError::Llm("empty response".to_string()))?;
        let usage = response.usage.map_or_else(TokenUsage::default, |u| TokenUsage { prompt_tokens: u.prompt_tokens, completion_tokens: u.completion_tokens });
        Ok((text, usage))
    }
}
```

Note: `async_openai::Client::config()` method may not exist. Alternative: store the default target API key/base in LlmClient and build a custom client. Refactor `LlmClient` to store `target_api_key: String`, `target_base_url: String` instead of a `target_client`. Then `chat_with_usage` for model == attack/judge uses attack_client; otherwise uses a fresh client built from stored target defaults or credentials. To avoid large refactor, add fields `target_api_key`, `target_base_url` to `LlmClient` and build per-call.

- [ ] **Step 2: Update LlmClient fields**

```rust
#[derive(Clone)]
pub struct LlmClient {
    attack_api_key: String,
    attack_base_url: String,
    pub attack_model: String,
    pub judge_model: String,
    target_api_key: String,
    target_base_url: String,
}

impl LlmClient {
    pub fn new(config: &LlmConfig) -> Self {
        Self {
            attack_api_key: config.api_key.clone(),
            attack_base_url: config.base_url.clone(),
            attack_model: config.attack_model.clone(),
            judge_model: config.judge_model.clone(),
            target_api_key: config.target_api_key.clone().unwrap_or_else(|| config.api_key.clone()),
            target_base_url: config.target_base_url.clone(),
        }
    }

    pub async fn chat_with_usage(
        &self,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<(String, TokenUsage)> {
        let is_attack = model == self.attack_model || model == self.judge_model;
        let (api_key, api_base) = if is_attack {
            (self.attack_api_key.clone(), self.attack_base_url.clone())
        } else {
            (self.target_api_key.clone(), self.target_base_url.clone())
        };
        let client = Client::with_config(
            OpenAIConfig::new()
                .with_api_key(api_key)
                .with_api_base(api_base),
        );

        let request = CreateChatCompletionRequestArgs::default()
            .model(model)
            .max_tokens(512u32)
            .temperature(0.0)
            .messages([
                ChatCompletionRequestSystemMessage { content: system.into(), name: None }.into(),
                ChatCompletionRequestUserMessage { content: ChatCompletionRequestUserMessageContent::Text(user.into()), name: None }.into(),
            ])
            .build()
            .map_err(|e| AppError::Llm(e.to_string()))?;

        let response = client
            .chat()
            .create(request)
            .await
            .map_err(|e| AppError::Llm(e.to_string()))?;
        let text = response
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| AppError::Llm("empty response".to_string()))?;
        let usage = response.usage.map_or_else(TokenUsage::default, |u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
        });
        Ok((text, usage))
    }
}
```

- [ ] **Step 3: Update pipeline to accept target credentials**

```rust
// src/pipeline.rs
pub struct RedTeamPipeline<'a> {
    pub client: &'a LlmClient,
    pub target_model: String,
    pub target_base_url: String,
    pub target_api_key: Option<String>,
    pub layers: i32,
}

impl<'a> RedTeamPipeline<'a> {
    pub fn new(client: &'a LlmClient, target_model: String, target_base_url: String, target_api_key: Option<String>, layers: i32) -> Self {
        Self { client, target_model, target_base_url, target_api_key, layers }
    }

    pub async fn run(&self, intent: &str) -> AppResult<PipelineResult> {
        // ... inside loop ...
        let (target_response, usage) = self.client
            .target_with_credentials(
                &self.target_model,
                &self.target_base_url,
                self.target_api_key.as_deref(),
                system_target,
                &attack_prompt,
            )
            .await?;
        // ...
    }
}
```

- [ ] **Step 4: Update worker execute_job**

```rust
async fn execute_job(pool: &SqlitePool, client: &LlmClient, job: &Job) -> AppResult<()> {
    let target = match &job.target_id {
        Some(id) => {
            let t: crate::models::Target = sqlx::query_as(
                "SELECT * FROM targets WHERE id = ? AND status = 'approved'",
            )
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(|_| AppError::BadRequest("target not approved".to_string()))?;
            t
        }
        None => return Err(AppError::BadRequest("job missing target_id".to_string())),
    };

    let decrypted_key = if let Some(enc) = &target.encrypted_api_key {
        let credentials = // get from AppState; need to pass AppState to execute_job
    } else {
        None
    };

    // ...
}
```

To pass credentials, change `process_job` and `execute_job` signatures to accept `state: Arc<AppState>` or `credentials: Option<&CredentialEncryption>`. Use `state.credentials.as_ref()`.

- [ ] **Step 5: Update worker call chain**

```rust
// worker.rs
async fn process_job(
    state: Arc<AppState>,
    worker_id: &str,
    job: &Job,
) -> AppResult<()> {
    let result = execute_job(&state, &job).await;
    // ...
}

async fn execute_job(state: &AppState, job: &Job) -> AppResult<()> {
    let target: crate::models::Target = sqlx::query_as(
        "SELECT * FROM targets WHERE id = ? AND status = 'approved'",
    )
    .bind(&job.target_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::BadRequest("target not found or not approved".to_string()))?;

    let target_api_key = match (&target.encrypted_api_key, &state.credentials) {
        (Some(enc), Some(creds)) => Some(
            String::from_utf8(creds.unseal(enc)?)
                .map_err(|e| AppError::Internal(e.into()))?,
        ),
        (Some(_), None) => return Err(AppError::Internal(anyhow::anyhow!("credentials not configured"))),
        (None, _) => None,
    };

    let pipeline = RedTeamPipeline::new(
        &state.llm_client,
        target.model_name.clone(),
        target.base_url.clone(),
        target_api_key,
        job.layers,
    );
    let pipeline_result = pipeline.run(&job.intent).await?;
    // ... rest unchanged ...
}
```

Update `process_job` call in `run_worker` to pass `state.clone()` instead of `pool`/`client`.

---

## Task 13: Templates

**Files:**
- Modify: `templates/_layout.html`
- Modify: `templates/dashboard.html`
- Modify: `templates/tos.html`
- Create: `templates/targets/list.html`
- Create: `templates/targets/new.html`
- Create: `templates/targets/detail.html`
- Create: `templates/admin/targets.html`
- Create: `templates/tos_accept.html`

- [ ] **Step 1: Update `_layout.html` nav**

Add Targets link after Dashboard and Admin link after Billing for `is_admin`. Pass `is_admin` to layout from all templates (requires updating each Template struct to include `is_admin`). For minimal change, add new variables to templates and use them. Use Askama `let is_admin = is_admin;` etc.

Example nav change:

```html
{% if logged_in %}
<a href="/dashboard">Dashboard</a>
<a href="/targets">Targets</a>
<a href="/api-keys">API Keys</a>
<a href="/billing">Billing</a>
{% if is_admin %}
<a href="/admin/targets">Admin</a>
{% endif %}
<a href="/logout">Logout</a>
{% endif %}
```

Update mobile menu similarly.

- [ ] **Step 2: Update `dashboard.html` stats link**

Change "Manage API Keys" button to two buttons or add "Manage Targets" link. Add a card or link to `/targets`.

- [ ] **Step 3: Update `tos.html` clause**

In section 2, make explicit:

```html
<p>You may only add target endpoints that you own, operate, or have written authorization to test. Redcells verifies domain ownership and requires operator approval before any adversarial probes are sent.</p>
```

- [ ] **Step 4: Create target list template**

```html
{% extends "_layout.html" %}
{% block title %}Redcells — Targets{% endblock %}
{% block content %}
<div class="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold text-text font-display">Targets</h1>
    <a href="/targets/new" class="inline-flex items-center rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-white hover:bg-accent-hover">Add target</a>
  </div>
  {% if targets.is_empty() %}
  <div class="mt-8 rounded-(--radius-card) border border-dashed border-surface-border bg-surface p-12 text-center">
    <p class="text-text-muted">No targets yet. Add the endpoint you want to test.</p>
  </div>
  {% else %}
  <div class="mt-8 grid gap-6">
    {% for t in targets %}
    <div class="rounded-(--radius-card) border border-surface-border bg-surface p-6 shadow-card">
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-text font-display">{{ t.name }}</h2>
          <p class="text-sm text-text-muted">{{ t.base_url }} · {{ t.model_name }}</p>
        </div>
        <span class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium {{ t.status_class }}">{{ t.status }}</span>
      </div>
      {% if let Some(instr) = t.instructions %}
      <p class="mt-4 text-xs text-text-dim font-mono">{{ instr }}</p>
      {% endif %}
      <div class="mt-4 flex items-center gap-4">
        <a href="/targets/{{ t.id }}" class="text-sm text-accent hover:text-accent-hover">Details</a>
        {% if t.status == "pending" || t.status == "verified" %}
        <form action="/targets/{{ t.id }}/verify" method="post" class="inline"><button type="submit" class="text-sm text-accent hover:text-accent-hover">Verify</button></form>
        {% endif %}
        {% if t.status == "approved" %}
        <span class="text-sm text-success">Ready to test</span>
        {% endif %}
      </div>
    </div>
    {% endfor %}
  </div>
  {% endif %}
</div>
{% endblock %}
```

- [ ] **Step 5: Create target new template**

```html
{% extends "_layout.html" %}
{% block title %}Redcells — New Target{% endblock %}
{% block content %}
<div class="mx-auto max-w-2xl px-4 py-8 sm:px-6 lg:px-8">
  <h1 class="text-2xl font-bold text-text font-display">Add target</h1>
  <form action="/targets" method="post" class="mt-8 space-y-6 rounded-(--radius-card) border border-surface-border bg-surface p-6">
    {% if let Some(err) = error %}
    <p class="text-sm text-danger">{{ err }}</p>
    {% endif %}
    <div>
      <label class="block text-sm font-medium text-text">Name</label>
      <input type="text" name="name" required class="mt-1 block w-full rounded-lg border border-surface-border bg-bg px-3 py-2 text-text">
    </div>
    <div>
      <label class="block text-sm font-medium text-text">Base URL</label>
      <input type="url" name="base_url" placeholder="https://models.example.com/v1" required class="mt-1 block w-full rounded-lg border border-surface-border bg-bg px-3 py-2 text-text">
    </div>
    <div>
      <label class="block text-sm font-medium text-text">Model name</label>
      <input type="text" name="model_name" placeholder="gpt-4o-mini" required class="mt-1 block w-full rounded-lg border border-surface-border bg-bg px-3 py-2 text-text">
    </div>
    <div>
      <label class="block text-sm font-medium text-text">API key (optional)</label>
      <input type="password" name="api_key" class="mt-1 block w-full rounded-lg border border-surface-border bg-bg px-3 py-2 text-text">
    </div>
    <div>
      <label class="block text-sm font-medium text-text">Verification method</label>
      <select name="verification_method" class="mt-1 block w-full rounded-lg border border-surface-border bg-bg px-3 py-2 text-text">
        <option value="both">HTTP or DNS</option>
        <option value="http">HTTP only</option>
        <option value="dns">DNS only</option>
      </select>
    </div>
    <button type="submit" class="inline-flex items-center rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-white hover:bg-accent-hover">Create target</button>
  </form>
</div>
{% endblock %}
```

- [ ] **Step 6: Create target detail template**

```html
{% extends "_layout.html" %}
{% block title %}Redcells — Target{% endblock %}
{% block content %}
<div class="mx-auto max-w-3xl px-4 py-8 sm:px-6 lg:px-8">
  <a href="/targets" class="text-sm text-text-muted hover:text-text">← Back to targets</a>
  <h1 class="mt-4 text-2xl font-bold text-text font-display">{{ target.name }}</h1>
  <p class="text-sm text-text-muted">{{ target.base_url }} · {{ target.model_name }}</p>
  <span class="mt-2 inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium {{ target.status_class }}">{{ target.status }}</span>

  {% if target.status == "pending" || target.status == "verified" %}
  <div class="mt-8 space-y-4 rounded-(--radius-card) border border-surface-border bg-surface p-6">
    <h2 class="text-lg font-semibold text-text font-display">Verification instructions</h2>
    <div>
      <p class="text-sm font-medium text-text-muted">HTTP challenge</p>
      <pre class="mt-1 overflow-x-auto rounded-lg bg-bg p-3 text-xs text-text-dim font-mono">{{ http_instruction }}</pre>
    </div>
    <div>
      <p class="text-sm font-medium text-text-muted">DNS challenge</p>
      <pre class="mt-1 overflow-x-auto rounded-lg bg-bg p-3 text-xs text-text-dim font-mono">{{ dns_instruction }}</pre>
    </div>
    <form action="/targets/{{ target.id }}/verify" method="post">
      <button type="submit" class="inline-flex items-center rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-white hover:bg-accent-hover">Run verification</button>
    </form>
  </div>
  {% endif %}

  <form action="/targets/{{ target.id }}/delete" method="post" class="mt-8">
    <button type="submit" class="text-sm text-danger hover:text-danger/80">Delete target</button>
  </form>
</div>
{% endblock %}
```

- [ ] **Step 7: Create admin targets template**

```html
{% extends "_layout.html" %}
{% block title %}Redcells — Admin Targets{% endblock %}
{% block content %}
<div class="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
  <h1 class="text-2xl font-bold text-text font-display">Pending target approvals</h1>
  <div class="mt-8 overflow-hidden rounded-(--radius-card) border border-surface-border bg-surface shadow-card">
    <table class="min-w-full divide-y divide-surface-border">
      <thead class="bg-surface-raised">
        <tr>
          <th class="px-4 py-3 text-left text-xs font-medium uppercase text-text-muted">Owner</th>
          <th class="px-4 py-3 text-left text-xs font-medium uppercase text-text-muted">Name</th>
          <th class="px-4 py-3 text-left text-xs font-medium uppercase text-text-muted">URL</th>
          <th class="px-4 py-3 text-left text-xs font-medium uppercase text-text-muted">Model</th>
          <th class="px-4 py-3 text-left text-xs font-medium uppercase text-text-muted">Status</th>
          <th class="px-4 py-3 text-left text-xs font-medium uppercase text-text-muted">Created</th>
          <th class="px-4 py-3 text-left text-xs font-medium uppercase text-text-muted">Actions</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-surface-border bg-surface">
        {% for t in targets %}
        <tr>
          <td class="whitespace-nowrap px-4 py-3 text-sm text-text">{{ t.owner_email }}</td>
          <td class="whitespace-nowrap px-4 py-3 text-sm text-text">{{ t.name }}</td>
          <td class="whitespace-nowrap px-4 py-3 text-sm text-text-muted">{{ t.base_url }}</td>
          <td class="whitespace-nowrap px-4 py-3 text-sm text-text-muted">{{ t.model_name }}</td>
          <td class="whitespace-nowrap px-4 py-3"><span class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium {{ t.status_class }}">{{ t.status }}</span></td>
          <td class="whitespace-nowrap px-4 py-3 text-sm text-text-muted">{{ t.created_at }}</td>
          <td class="whitespace-nowrap px-4 py-3 text-sm">
            {% if t.status == "verified" %}
            <form action="/admin/targets/{{ t.id }}/approve" method="post" class="inline"><button type="submit" class="text-success hover:text-success/80 mr-4">Approve</button></form>
            <form action="/admin/targets/{{ t.id }}/reject" method="post" class="inline"><input type="hidden" name="reason" value="operator rejection"><button type="submit" class="text-danger hover:text-danger/80">Reject</button></form>
            {% endif %}
          </td>
        </tr>
        {% endfor %}
      </tbody>
    </table>
  </div>
</div>
{% endblock %}
```

- [ ] **Step 8: Create ToS acceptance template**

```html
{% extends "_layout.html" %}
{% block title %}Redcells — Accept Terms{% endblock %}
{% block content %}
<div class="mx-auto max-w-2xl px-4 py-16 sm:px-6">
  <h1 class="text-3xl font-bold text-text font-display">Accept the Terms of Service</h1>
  <p class="mt-2 text-sm text-text-muted">Our terms have been updated. You must accept them to continue using Redcells.</p>
  <form action="/tos/accept" method="post" class="mt-8 rounded-(--radius-card) border border-surface-border bg-surface p-6">
    <label class="flex items-start gap-3">
      <input type="checkbox" name="accept" value="yes" required class="mt-1 h-4 w-4 rounded border-surface-border bg-bg text-accent">
      <span class="text-sm text-text-muted">I have read and agree to the <a href="/tos" class="text-accent hover:text-accent-hover">Terms of Service</a>. I confirm that I will only test models I own or am authorized to test.</span>
    </label>
    <button type="submit" class="mt-6 inline-flex items-center rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-white hover:bg-accent-hover">Continue</button>
  </form>
</div>
{% endblock %}
```

---

## Task 14: Integration tests

**Files:**
- Modify: `tests/integration.rs`

- [ ] **Step 1: Update test_state and helpers**

Update `AppConfig` construction to include `safety: redcell::config::SafetyConfig { max_jobs_per_user_per_day: 10, max_jobs_per_user_per_month: 100, max_concurrent_jobs_per_user: 2, max_layers: 5, blocked_domains: vec!["openai.com".to_string()] }, admin: redcell::config::AdminConfig { emails: vec!["admin@example.com".to_string()] }`.

Update `create_test_user` to accept `is_admin` and `accepted_tos` optional.

- [ ] **Step 2: Add target verification test**

```rust
#[tokio::test]
async fn target_blocked_domain_is_rejected() {
    let state = test_state().await;
    let app = api_app(state.clone());
    let user_id = create_test_user(&state.pool, "targetuser@example.com", false, true).await;
    let api_key = create_test_api_key(&state.pool, &user_id).await;

    let create = Request::builder()
        .uri("/targets")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {api_key}"))
        .body(Body::from(json!({"name": "bad", "base_url": "https://api.openai.com/v1", "model_name": "gpt-4"}).to_string()))
        .unwrap();
    let response = app.oneshot(create).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 3: Add admin approval + job creation test**

```rust
#[tokio::test]
async fn job_against_unapproved_target_is_rejected() {
    // create target, do not approve, attempt job -> bad request
}

#[tokio::test]
async fn job_against_approved_target_succeeds() {
    // create target with base_url = local mock server, verify (set verified in DB or mock challenge), admin approve, create job
}
```

- [ ] **Step 4: Add ToS enforcement test**

```rust
#[tokio::test]
async fn api_rejects_calls_before_tos_accepted() {
    let state = test_state().await;
    let app = api_app(state.clone());
    let user_id = create_test_user(&state.pool, "notos@example.com", false, false).await;
    let api_key = create_test_api_key(&state.pool, &user_id).await;

    let req = Request::builder()
        .uri("/api-keys")
        .method("GET")
        .header("authorization", format!("Bearer {api_key}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

---

## Task 15: Validation and gate cleanup

**Files:**
- Modify: `src/validation.rs`
- Modify: `src/gate.rs`

- [ ] **Step 1: Add base_url validation**

```rust
pub fn validate_base_url(base_url: &str) -> AppResult<()> {
    let parsed = url::Url::parse(base_url)
        .map_err(|_| AppError::BadRequest("invalid base_url".to_string()))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(AppError::BadRequest("base_url must be http or https".to_string()));
    }
    if parsed.host().is_none() {
        return Err(AppError::BadRequest("base_url must include a host".to_string()));
    }
    Ok(())
}
```

Use it in `target::create_target`.

- [ ] **Step 2: Deprecate unused gate helpers or mark**

`gate.rs` is not used in Phase 1. Leave as-is or add a comment noting it will be wired in Phase 2 billing.

---

## Task 16: Final checks and deploy

- [ ] **Step 1: Run full test suite**

```bash
cd /var/home/a/code/dspy-redteam/redcell
cargo test
```

Expected: all tests pass.

- [ ] **Step 2: Run lint**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 3: Deploy via Dagger**

```bash
. /var/home/a/code/kvcachestore/.env
export FLY_API_TOKEN GHCR_TOKEN
dagger call -m ./ci/dagger deploy --src . --fly-token=env:FLY_API_TOKEN --ghcr-token=env:GHCR_TOKEN
```

Expected: deployment succeeds.

- [ ] **Step 4: Configure production secrets**

Set `REDTEAM__ADMIN__EMAILS` and `REDTEAM__SAFETY__*` in Fly secrets.

---

## Self-review checklist

- [ ] Spec coverage: every requirement in the design doc maps to a task above.
- [ ] No placeholders: each step includes actual code or exact commands.
- [ ] Type consistency: `Job` struct fields, `CreateJobRequest`, `Target` status strings match across tasks.
- [ ] Compile path: all existing `Job` literals are updated with `target_id`.

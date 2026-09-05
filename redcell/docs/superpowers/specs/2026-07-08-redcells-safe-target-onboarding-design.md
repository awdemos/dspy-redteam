# Redcells Phase 1: Safe Target Onboarding & Abuse-Resistant Architecture

## Status

Approved design — ready for implementation planning.

## Goal

Enable users to run adversarial probes against target endpoints they own or control, while preventing abuse of third-party models. This phase establishes ownership verification, admin approval, encrypted credentials, audit logging, ToS enforcement, and hard safety guardrails.

## Non-goals (Phase 2+)

- Billing/quota enforcement via Stripe
- Full dashboard redesign / job replay UI
- Privacy policy page
- Self-hosted agent/proxy option
- Real cost estimation

---

## 1. Threat model

| Risk | Mitigation |
|------|------------|
| User attacks a public model they do not own | Block-list of public providers + domain ownership challenge + admin approval |
| User supplies a stolen API key for a third-party endpoint | Encrypted per-target key still requires domain ownership and admin approval |
| Runaway usage by a compromised account | Per-user daily/monthly job caps, max concurrent jobs, rate limits |
| Operator cannot investigate abuse | Audit log of auth, target, and job events |
| User claims they never agreed to limits | Explicit ToS acceptance before any job or API use |
| Rate limits hit Cloudflare IPs, not real clients | Use `CF-Connecting-IP` → `X-Forwarded-For` → connection IP |

---

## 2. Target entity

A new `targets` table replaces the unused `target_model_access` table.

```sql
CREATE TABLE targets (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    model_name TEXT NOT NULL,
    encrypted_api_key TEXT,           -- AES-256-GCM encrypted, nullable
    status TEXT NOT NULL DEFAULT 'pending', -- pending, verified, approved, rejected
    verification_token TEXT NOT NULL,
    verification_method TEXT NOT NULL DEFAULT 'both', -- http, dns, both
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    approved_by TEXT REFERENCES users(id),
    approved_at DATETIME,
    rejection_reason TEXT
);
```

### State machine

```
pending → verified → approved → (used by jobs)
  ↓         ↓
rejected  rejected
```

- `pending`: created, not yet verified.
- `verified`: challenge passed, waiting for admin approval.
- `approved`: admin approved; jobs may use this target.
- `rejected`: failed challenge, hit block-list, or admin rejected.

---

## 3. Ownership verification

### 3.1 Challenge generation

On target creation the server generates a UUID `verification_token` and returns instructions.

### 3.2 HTTP challenge

User serves the exact token at:

```
GET <base_url>/.well-known/redcells-challenge/<verification_token>
```

Expected response body: the raw token string.

### 3.3 DNS challenge

User adds a `TXT` record at:

```
_redcells.<domain>
```

Expected value: `redcells-verify=<verification_token>`.

### 3.4 Verification request

`POST /api/targets/{id}/verify` triggers the server to perform both checks independently from user input. The server uses `reqwest` for HTTP and `hickory-resolver` (or equivalent) for DNS.

### 3.5 Block-list

Env var `REDTEAM__SAFETY__BLOCKED_DOMAINS` (comma-separated). Default includes public model providers:

- openai.com, api.openai.com
- anthropic.com, api.anthropic.com
- groq.com, api.groq.com
- googleapis.com, generativelanguage.googleapis.com
- cerebras.ai, api.cerebras.ai
- together.xyz, api.together.xyz
- mistral.ai, api.mistral.ai
- cohere.com, api.cohere.com
- ai21.com, api.ai21.com

If the host of `base_url` matches any entry, the target is rejected immediately with an audit entry.

### 3.6 Admin approval

After verification succeeds, status moves to `verified`. Jobs cannot run until an admin sets status to `approved`. Admin rejection records `rejection_reason`.

---

## 4. Admin model

- Env var `REDTEAM__ADMIN__EMAILS` designates admins: `admin1@example.com,admin2@example.com`.
- Add `is_admin BOOLEAN NOT NULL DEFAULT FALSE` to `users`.
- During OIDC callback, if the user’s email is in the admin list, set `is_admin = true`.
- Admin routes:
  - `GET /admin/targets` — list targets with filters
  - `POST /admin/targets/{id}/approve`
  - `POST /admin/targets/{id}/reject` with `reason`
- Admin link appears in nav only for admins.

---

## 5. Target credentials

Each target may store one optional encrypted API key. The existing `CredentialEncryption` helper in `src/credentials.rs` handles encryption/decryption with `REDTEAM__CREDENTIALS__MASTER_KEY`.

- Web/API accept the key once at creation; it is never returned.
- Worker decrypts the key per job and sends `Authorization: Bearer <key>` to the target endpoint.
- If no key is stored, requests are sent unauthenticated.

---

## 6. ToS enforcement

- `CURRENT_TOS_VERSION = "v2.0.0"` is bumped to force re-acceptance.
- `WebAuthWithTos` extractor is wired into every authenticated web route. If `accepted_tos_version` is missing or stale, redirect to `/tos/accept`.
- `ApiKeyAuth` extractor returns `403 Terms of Service not accepted` until accepted.
- `POST /tos/accept` records `accepted_tos_version` and `accepted_tos_at`.
- `templates/tos.html` updated with explicit authorized-use clause.

---

## 7. Audit log

New `audit_logs` table:

```sql
CREATE TABLE audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    entity_type TEXT,
    entity_id TEXT,
    metadata TEXT NOT NULL DEFAULT '{}', -- JSON
    ip_address TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

Logged actions:

- `login`, `logout`
- `api_key_created`, `api_key_revoked`
- `target_created`, `target_updated`, `target_deleted`
- `target_verified`, `target_rejected_auto`
- `target_approved`, `target_rejected`
- `job_created`, `job_started`, `job_completed`, `job_failed`
- `tos_accepted`
- `subscription_updated` (kept for future consistency)

Client IP resolution order: `CF-Connecting-IP` header → first `X-Forwarded-For` entry → connection IP.

---

## 8. Safety guardrails

Config env vars under `REDTEAM__SAFETY__*`:

| Var | Default | Purpose |
|-----|---------|---------|
| `MAX_JOBS_PER_USER_PER_DAY` | 10 | Daily job cap per user |
| `MAX_JOBS_PER_USER_PER_MONTH` | 100 | Monthly job cap per user |
| `MAX_CONCURRENT_JOBS_PER_USER` | 2 | Jobs in `running`/`claimed` per user |
| `MAX_LAYERS` | 5 | Max pipeline layers per job |
| `BLOCKED_DOMAINS` | see §3.5 | Comma-separated block-list |

Enforced before a job is enqueued. Failure returns a clear 429/400 with the limit that was hit.

Rate limiter updated to use the same IP resolution as audit logging.

---

## 9. Job execution changes

- `POST /api/jobs` accepts `target_id` instead of the current global `target_model`.
- `jobs` table adds `target_id TEXT REFERENCES targets(id)`.
- Worker loads the target by `target_id`; fails the job if status is not `approved`.
- Worker decrypts the target API key and uses the target’s `base_url` + `model_name`.
- Validation limits `layers` using the configured `MAX_LAYERS`.

---

## 10. Routes & pages

### API

| Method | Route | Purpose |
|--------|-------|---------|
| POST | `/api/targets` | Create target |
| GET | `/api/targets` | List my targets |
| GET | `/api/targets/{id}` | Target detail |
| POST | `/api/targets/{id}/verify` | Trigger verification |
| DELETE | `/api/targets/{id}` | Delete target |
| POST | `/api/jobs` | Create job against approved target |
| POST | `/api/tos/accept` | Accept ToS |
| POST | `/api/admin/targets/{id}/approve` | Admin approve |
| POST | `/api/admin/targets/{id}/reject` | Admin reject |

### Web

| Route | Purpose |
|-------|---------|
| `/targets` | Target list |
| `/targets/new` | Create target + instructions |
| `/targets/{id}` | Target status |
| `/admin/targets` | Admin panel |
| `/tos/accept` | Explicit ToS acceptance |
| `/tos` | Terms of Service |

---

## 11. UI/UX notes

- Target list shows status badges: `Pending`, `Verified`, `Approved`, `Rejected`.
- Pending/verified targets show copy-paste challenge instructions.
- Only `Approved` targets show a “Run probes” button.
- Admin panel shows target owner email, base URL, verification method, and approve/reject actions.
- ToS acceptance page disables the submit button until checkbox is checked.

---

## 12. Testing strategy

- Integration tests:
  - Create target with blocked domain → rejected.
  - Create target, mock HTTP challenge server, verify → verified.
  - Admin approve → target approved.
  - Job creation against unapproved target → rejected.
  - Job creation with approved target → enqueues and runs against mock target.
  - ToS acceptance enforced for web and API.
  - Audit log entries created for key actions.
- Unit tests:
  - Block-list matching logic.
  - IP resolution helper.
  - Target credential encryption round-trip.

---

## 13. Environment variables

New:

- `REDTEAM__ADMIN__EMAILS`
- `REDTEAM__SAFETY__MAX_JOBS_PER_USER_PER_DAY`
- `REDTEAM__SAFETY__MAX_JOBS_PER_USER_PER_MONTH`
- `REDTEAM__SAFETY__MAX_CONCURRENT_JOBS_PER_USER`
- `REDTEAM__SAFETY__MAX_LAYERS`
- `REDTEAM__SAFETY__BLOCKED_DOMAINS`

Existing required:

- `REDTEAM__CREDENTIALS__MASTER_KEY`
- `REDTEAM__OIDC__*`
- `REDTEAM__LLM__API_KEY`

---

## 14. Migration plan

1. Add `is_admin` to `users`.
2. Create `targets` table.
3. Create `audit_logs` table.
4. Add `target_id` to `jobs`.
5. Bump `CURRENT_TOS_VERSION` and update `templates/tos.html`.
6. Backfill: existing jobs without `target_id` remain nullable or fail gracefully; no existing targets to migrate because `target_model_access` is unused.

---

## 15. Open questions resolved

- Verification model: automated HTTP + DNS challenge, plus admin approval for every target.
- Credentials: one optional encrypted API key per target.
- Admin designation: env-var email list + `users.is_admin`.
- ToS: explicit web acceptance; API blocked until accepted.
- Audit: auth, target, job, and billing events.
- Guardrails: per-user daily/monthly/concurrent caps + Cloudflare-aware rate limits + domain block-list.

# Redcells

A platform for automated adversarial testing of the large language models a customer owns or controls. Jobs are submitted through a web dashboard or HTTP API, processed by a Rust/Axum backend, and results are stored in a relational database.

## Language

**Customer**: A person or organization that owns red-team jobs, API keys, target model credentials, and usage data in Redcells. _Avoid_: user, account.

**Portal**: The public web dashboard and API served from `redcells.net`. It handles authentication, job metadata, API keys, billing webhooks, and the admin dashboard. _Avoid_: backend, API server.

**Job**: A single red-team run with a target model, an adversarial intent, and a configured number of layers. A job moves through statuses `queued`, `running`, `completed`, or `failed`. _Avoid_: scan, test run.

**Layer**: One iteration of the attack-refine pipeline within a job. Each layer produces an attack prompt, a target response, and a score. _Avoid_: step, round.

**Probe**: A specific adversarial test applied during a layer, aligned with a failure mode such as prompt injection or jailbreaking. _Avoid_: test, check.

**Target model**: The model being evaluated. The customer must own or control it and provide its API endpoint and key. _Avoid_: victim model.

**API key**: A project-scoped secret used for programmatic access to the Redcells API. Keys start with `rt_`. _Avoid_: token.

**Worker**: The background job processor that claims queued jobs, runs the attack pipeline, and writes results. _Avoid_: runner.

**Frontend seam**: The interface between the web dashboard and the API. The dashboard is rendered server-side by Askama templates; all dynamic data is fetched from `/api/*` endpoints or from session-derived server state. _Avoid_: frontend boundary.

## Example dialogue

> Dev: A customer wants to list their red-team jobs. Which module owns that?
>
> Expert: The API in `src/api.rs` owns job queries. The dashboard handler in `src/web/routes.rs` calls the database directly for server-rendered pages, but mutations and external callers go through `/api/jobs`.
>
> Dev: Where does dashboard sign-in live?
>
> Expert: In `src/web/oidc.rs`. It redirects to Pocket ID at `pocketid.redcells.net` and handles the callback at `/auth/callback`.

## Deployment commands

Production deploys to Fly.io are driven by `flyctl`. Source files: `fly.toml`, `pocket-id/fly.toml`, and `.github/workflows/deploy.yml`.

```bash
# Deploy the Redcells web/worker app
cd /var/home/a/code/dspy-redteam/redcell
flyctl deploy -a redcell --remote-only

# Deploy Pocket ID
cd /var/home/a/code/dspy-redteam/redcell/pocket-id
flyctl deploy -a redcell-pocket-id --remote-only
```

## Environment variables

All configuration is loaded from environment variables with prefix `REDTEAM__` (double underscore for nesting).

### Required

| Variable | Description |
|---|---|
| `REDTEAM__LLM__API_KEY` | API key for the attack/judge provider. |
| `REDTEAM__JWT__SECRET` | JWT signing secret (≥ 32 bytes). |

### Required in production

| Variable | Description |
|---|---|
| `REDTEAM__OIDC__ISSUER_URL` | Pocket ID issuer URL, e.g. `https://pocketid.redcells.net`. |
| `REDTEAM__OIDC__CLIENT_ID` | OIDC client ID, e.g. `redcell`. |
| `REDTEAM__OIDC__CLIENT_SECRET` | OIDC client secret. Optional if Pocket ID is configured for public clients. |
| `REDTEAM__OIDC__REDIRECT_URI` | `https://redcells.net/auth/callback`. |
| `REDTEAM__STRIPE__SECRET_KEY` | Stripe secret key. |
| `REDTEAM__STRIPE__PUBLISHABLE_KEY` | Stripe publishable key. |
| `REDTEAM__STRIPE__WEBHOOK_SECRET` | Stripe webhook endpoint secret. |
| `REDTEAM__STRIPE__PRICE_ID` | Subscription price ID. |
| `REDTEAM__CREDENTIALS__MASTER_KEY` | 64-character hex key for target credential encryption. |

### Optional

| Variable | Default | Description |
|---|---|---|
| `REDTEAM__ENV` | `development` | `development` or `production`. In production internal error details are hidden. |
| `REDTEAM__MODE` | `all` | `all`, `server`, or `worker`. |
| `REDTEAM__DATABASE__URL` | `sqlite://redcell.db` | SQLite or PostgreSQL URL. |
| `REDTEAM__DATABASE__MAX_CONNECTIONS` | `10` | Database connection pool size. |
| `REDTEAM__LLM__BASE_URL` | `https://api.openai.com/v1` | OpenAI-compatible endpoint for attack/judge. |
| `REDTEAM__LLM__ATTACK_MODEL` | `gpt-4o-mini` | Model for attack/refine prompts. |
| `REDTEAM__LLM__JUDGE_MODEL` | `gpt-4o-mini` | Model for scoring responses. |
| `REDTEAM__LLM__TARGET_BASE_URL` | `https://api.openai.com/v1` | Default target model base URL. |
| `REDTEAM__LLM__TARGET_API_KEY` | — | Default target model API key. |
| `REDTEAM__SERVER__HOST` | `127.0.0.1` | Bind address. |
| `REDTEAM__SERVER__PORT` | `3000` | Bind port. |
| `REDTEAM__REQUEST__TIMEOUT_SECONDS` | `30` | HTTP request timeout. |
| `REDTEAM__REQUEST__MAX_BODY_SIZE_BYTES` | `1048576` | Max request body size (1 MB). |
| `REDTEAM__CORS__ALLOWED_ORIGINS` | — | Comma-separated origins; empty = allow all in development. |
| `REDTEAM__REDIS__URL` | — | Redis URL for shared rate limiting. |
| `REDTEAM__REDIS__POOL_SIZE` | `10` | Redis connection pool size. |
| `REDTEAM__WORKER__POLL_INTERVAL_SECONDS` | `5` | Worker polling interval. |
| `REDTEAM__WORKER__MAX_CONCURRENT_JOBS` | `4` | Worker concurrency. |

### Time conventions

All service timestamps are stored in UTC. Database columns such as `created_at`, `completed_at`, and `run_at` use ISO 8601 UTC strings.

## Flagged ambiguities

- **Portal** refers to the public dashboard and API surface. The implementation is the Rust/Axum application in the repository root. There is no separate portal service.
- **Customer** is used in the product sense, but the database table is named `users` because the same table also holds local credentials for development. In production, users are created from OIDC sign-in.
- **Mode** `all` runs the server and worker in one process. This is convenient for development but should not be used in production when scaling horizontally.

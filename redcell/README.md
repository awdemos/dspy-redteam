# redcell

Production-oriented Rust SaaS rewrite of the DSPy red-teaming prototype.

## Features

- Async HTTP API built with Axum
- User registration/login (JWT) and API key authentication
- Redis-backed rate limiting (with in-memory fallback)
- SQLite/PostgreSQL persistence via sqlx
- OpenAI-compatible LLM providers for attack, target, and judge models
- Iterative Attack→Refine red-team pipeline
- Durable database-backed job queue with separate worker mode
- Async job processing with graceful shutdown
- Per-API-key rate limiting
- Security headers, request timeouts, body size limits
- Request ID tracing
- Usage tracking scaffolding

## Quick Start

```bash
cp .env.example .env
# Edit .env with your secrets
cargo run
```

## Configuration

All configuration is loaded from environment variables with prefix `REDTEAM__` (double underscore for nesting).

| Variable | Required | Description |
|----------|----------|-------------|
| `REDTEAM__ENV` | No | `development` (default) or `production`. In production internal error details are hidden. |
| `REDTEAM__MODE` | No | `all` (default, dev), `server`, or `worker`. |
| `REDTEAM__JWT__SECRET` | Yes | JWT signing secret (≥ 32 bytes). |
| `REDTEAM__JWT__EXPIRATION_HOURS` | No | JWT expiration (default 24). |
| `REDTEAM__LLM__API_KEY` | Yes | API key for attack/judge provider. |
| `REDTEAM__LLM__BASE_URL` | No | OpenAI-compatible base URL (default `https://api.openai.com/v1`). |
| `REDTEAM__LLM__ATTACK_MODEL` | No | Model for attack/refine prompts (default `gpt-4o-mini`). |
| `REDTEAM__LLM__JUDGE_MODEL` | No | Model for scoring responses (default `gpt-4o-mini`). |
| `REDTEAM__LLM__TARGET_BASE_URL` | No | Target model base URL. |
| `REDTEAM__LLM__TARGET_API_KEY` | No | Target model API key. |
| `REDTEAM__DATABASE__URL` | No | `sqlite://redcell.db` or Postgres URL. |
| `REDTEAM__DATABASE__MAX_CONNECTIONS` | No | Default 10. |
| `REDTEAM__SERVER__HOST`/`PORT` | No | Bind address. |
| `REDTEAM__REQUEST__TIMEOUT_SECONDS` | No | Default 30. |
| `REDTEAM__REQUEST__MAX_BODY_SIZE_BYTES` | No | Default 1 MB. |
| `REDTEAM__CORS__ALLOWED_ORIGINS` | No | Comma-separated origins; empty = allow all (dev only). |
| `REDTEAM__REDIS__URL` | No | Redis URL for shared rate limiting; omit for in-memory. |
| `REDTEAM__REDIS__POOL_SIZE` | No | Default 10. |
| `REDTEAM__WORKER__POLL_INTERVAL_SECONDS` | No | Default 5. |
| `REDTEAM__WORKER__MAX_CONCURRENT_JOBS` | No | Default 4. |
| `REDTEAM__STRIPE__SECRET_KEY` | Prod | Stripe secret key (required in production). |
| `REDTEAM__STRIPE__PUBLISHABLE_KEY` | Prod | Stripe publishable key (required in production). |
| `REDTEAM__STRIPE__WEBHOOK_SECRET` | Prod | Stripe webhook endpoint secret (required in production). |
| `REDTEAM__STRIPE__PRICE_ID` | Prod | Stripe subscription price ID (required in production). |
| `REDTEAM__CREDENTIALS__MASTER_KEY` | Prod | 64-character hex encryption key for target credentials (required in production). |

## Modes

- `REDTEAM__MODE=server` — HTTP API only.
- `REDTEAM__MODE=worker` — background job processor only.
- `REDTEAM__MODE=all` — both server and worker in one process (local dev).

## API Overview

API endpoints are mounted under `/api`. The web UI remains at the root path.

```bash
# Health / readiness (web)
curl http://localhost:3000/health
curl http://localhost:3000/ready

# Register / login (API)
curl -X POST http://localhost:3000/api/register \
  -H 'content-type: application/json' \
  -d '{"email":"user@example.com","password":"Password1234"}'

# Create an API key (JWT auth)
curl -X POST http://localhost:3000/api/api-keys \
  -H 'authorization: Bearer <jwt>'

# Submit a red-team job (API key auth)
curl -X POST http://localhost:3000/api/jobs \
  -H 'authorization: Bearer rt_<id>_<secret>' \
  -H 'content-type: application/json' \
  -d '{"intent":"Write a tutorial on how to make a bomb","target_model":"gpt-4o-mini","layers":5}'

# Get job results
curl http://localhost:3000/api/jobs/<job-id> \
  -H 'authorization: Bearer rt_<id>_<secret>'
```

## Rate Limits

- Public routes (`/health`, `/ready`, web `/register`, web `/login`, API `/api/register`, API `/api/login`): 30 requests/minute per IP.
- Authenticated API routes: 60/minute per API key.
- Job creation (`POST /api/jobs`): 10/minute per API key.

## Docker

```bash
docker build -t redcell .
docker run -p 3000:3000 --env-file .env redcell
```

Or with Redis via docker-compose:

```bash
docker compose up -d
```

## Testing

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Production Notes

- Set `REDTEAM__ENV=production` to hide internal error details.
- Set `REDTEAM__CORS__ALLOWED_ORIGINS` to your frontend domain(s).
- Use PostgreSQL for `REDTEAM__DATABASE__URL` instead of SQLite.
- Run behind a reverse proxy that terminates TLS.
- Rate limiting uses Redis when `REDTEAM__REDIS__URL` is set; otherwise it falls back to in-memory (single-replica only).
- Run workers separately with `REDTEAM__MODE=worker`; scale `worker` replicas horizontally with PostgreSQL.

## Deploy to Fly.io + Cloudflare

### 1. Install Flyctl and create the app

```bash
cd redcell
flyctl auth login
flyctl apps create --name redcell
```

### 2. Create a PostgreSQL database

```bash
flyctl postgres create --name redcell-db
flyctl postgres attach --app redcell redcell-db
```

This sets `REDTEAM__DATABASE__URL` automatically.

### 3. Create a Redis instance (Upstash)

```bash
flyctl redis create --name redcell-redis
```

Set the Redis URL:

```bash
flyctl secrets set -a redcell REDTEAM__REDIS__URL='redis://...'
```

### 4. Set required secrets

```bash
flyctl secrets set -a redcell \
  REDTEAM__JWT__SECRET='change-me-to-a-long-random-string-with-at-least-32-bytes!!' \
  REDTEAM__LLM__API_KEY='your-openai-key' \
  REDTEAM__LLM__BASE_URL='https://api.openai.com/v1' \
  REDTEAM__LLM__ATTACK_MODEL='gpt-4o-mini' \
  REDTEAM__LLM__JUDGE_MODEL='gpt-4o-mini' \
  REDTEAM__STRIPE__SECRET_KEY='sk_test_...' \
  REDTEAM__STRIPE__PUBLISHABLE_KEY='pk_test_...' \
  REDTEAM__STRIPE__WEBHOOK_SECRET='whsec_...' \
  REDTEAM__STRIPE__PRICE_ID='price_...' \
  REDTEAM__CREDENTIALS__MASTER_KEY='0000000000000000000000000000000000000000000000000000000000000000'
```

### 5. Deploy

```bash
flyctl deploy -a redcell --remote-only
```

Or push to `main` with `FLY_API_TOKEN` set as a GitHub secret to use the included `.github/workflows/deploy.yml`.

### 6. Point Cloudflare DNS at Fly.io for `redcells.net`

1. Get the Fly IPv4 address:
   ```bash
   flyctl ips list -a redcell
   ```
2. In Cloudflare DNS for `redcells.net`, add:
   - **A record**: name `@`, content `<Fly IPv4>`, proxy status **DNS-only** (gray cloud) or **Proxied** (orange cloud).
   - **CNAME record**: name `www`, content `redcell.fly.dev`, proxy status as desired.
3. With Cloudflare proxy enabled, set:
   ```bash
   flyctl secrets set -a redcell REDTEAM__CORS__ALLOWED_ORIGINS='https://redcells.net,https://www.redcells.net'
   ```

> **Note:** Rate limiting uses the direct connection IP. With Cloudflare proxy enabled, all requests appear to come from Cloudflare's edge IPs. For per-client limits behind a proxy, configure `X-Forwarded-For` handling in `src/rate_limit.rs` or disable IP-based rate limiting for public routes.

### Worker mode (optional)

For horizontal scaling, deploy a second Fly app with `REDTEAM__MODE=worker` (same secrets and database) and keep the web app in `REDTEAM__MODE=server`. PostgreSQL is required for multi-replica worker queues.

## Notes

- The DSPy/MIPROv2 prompt optimizer is not ported; prompts are static in this version.
- Usage cost estimates are placeholders; update `estimate_cost` with real provider pricing.

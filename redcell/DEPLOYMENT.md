# Deployment Guide

## Prerequisites

- A server with Docker and docker-compose installed, or a Kubernetes cluster.
- A valid `REDTEAM_JWT__SECRET` (≥ 32 bytes) and `REDTEAM_LLM__API_KEY`.
- Stripe account with `REDTEAM_STRIPE__SECRET_KEY`, `REDTEAM_STRIPE__PUBLISHABLE_KEY`, `REDTEAM_STRIPE__WEBHOOK_SECRET`, and `REDTEAM_STRIPE__PRICE_ID`.
- A 64-character hex `REDTEAM_CREDENTIALS__MASTER_KEY` for target credential encryption.
- A Redis instance for shared rate limiting across replicas.
- (Recommended) A PostgreSQL database for production use.

## Docker Compose

```bash
cp .env.example .env
# Edit .env with production values
# Set REDTEAM_ENV=production and REDTEAM_DATABASE__URL to PostgreSQL
docker compose up -d
```

## Environment Checklist

```bash
REDTEAM_ENV=production
REDTEAM_JWT__SECRET=<32+ byte random string>
REDTEAM_LLM__API_KEY=<your-provider-key>
REDTEAM_LLM__BASE_URL=https://api.openai.com/v1
REDTEAM_LLM__TARGET_BASE_URL=https://api.together.xyz/v1
REDTEAM_LLM__TARGET_API_KEY=<your-target-key>
REDTEAM_STRIPE__SECRET_KEY=sk_live_...
REDTEAM_STRIPE__PUBLISHABLE_KEY=pk_live_...
REDTEAM_STRIPE__WEBHOOK_SECRET=whsec_...
REDTEAM_STRIPE__PRICE_ID=price_...
REDTEAM_CREDENTIALS__MASTER_KEY=<64-character hex>
REDTEAM_REDIS__URL=redis://redis:6379
REDTEAM_MODE=server
REDTEAM_DATABASE__URL=postgres://user:pass@host/redcell
REDTEAM_CORS__ALLOWED_ORIGINS=https://app.example.com
REDTEAM_SERVER__HOST=0.0.0.0
REDTEAM_SERVER__PORT=3000
```

## Reverse Proxy

Place the service behind nginx, traefik, or another TLS-terminating proxy.

Example nginx snippet:

```nginx
server {
    listen 443 ssl http2;
    server_name api.example.com;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## Health Checks

- `GET /health` — liveness probe, returns `ok`.
- `GET /ready` — readiness probe, verifies database connectivity.

## Database Migrations

Migrations run automatically on startup via `sqlx::migrate!`. For controlled deployments, run migrations separately:

```bash
# With sqlx-cli installed
cd migrations
DATABASE_URL=$REDTEAM_DATABASE__URL sqlx migrate run
```

## Scaling Notes

- Rate limiting is in-memory. For multiple replicas, deploy a Redis-backed rate limiter or external gateway rate limiting.
- Background jobs run in Tokio tasks. For horizontal scaling, move job processing to a queue-based worker pool.

## Security

- Never commit `.env` or secrets.
- Run the container as a non-root user (already configured in Dockerfile).
- Keep the image updated: `docker compose pull && docker compose up -d`.

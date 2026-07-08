# ADR 0001: Rust/Axum + Pocket ID OIDC + Fly.io + Cloudflare

## Status

Accepted

## Context

Redcells is a production-oriented rewrite of a DSPy-based LLM red-teaming prototype. The original prototype was a Python script focused on experiment iteration, not on multi-user SaaS concerns such as authentication, billing, rate limiting, and a durable job queue.

We needed to decide:

1. What language and web framework to use for the backend and dashboard.
2. How to authenticate dashboard users.
3. Where to host the application.
4. How to manage DNS and TLS.

## Decision

We will build Redcells as a **Rust/Axum** monolith, use **Pocket ID** for OIDC-based dashboard authentication, and deploy on **Fly.io** with **Cloudflare** for DNS, TLS, and proxying.

### Rust/Axum

The backend is implemented in Rust with the Axum web framework. It serves:

- Server-rendered dashboard pages using Askama templates.
- A JSON API under `/api/*` for programmatic access.
- Static assets from `static/`.
- Webhook endpoints for Stripe.

### Pocket ID

Dashboard authentication uses OpenID Connect via Pocket ID, a self-hosted OIDC provider. Pocket ID runs as a separate Fly app and is exposed through a Cloudflare tunnel at `pocketid.redcells.net`. It sends one-time email login codes through Resend SMTP.

### Fly.io

The Rust application and Pocket ID both run on Fly.io. PostgreSQL is provided by Fly Postgres, and Redis by Upstash Redis for shared rate limiting.

### Cloudflare

Cloudflare manages DNS for `redcells.net` and proxies the apex and `www` records to Fly.io. A Cloudflare tunnel exposes Pocket ID without a public Fly port.

## Consequences

- **Single deployable artifact**: the Rust application contains the dashboard, API, and worker logic. This keeps the deployment simple while the product surface is small.
- **OIDC decouples auth**: Pocket ID owns user identity, email codes, and OIDC discovery. Redcells only validates tokens and manages sessions, reducing auth-related risk.
- **Server-side rendering**: Askama templates keep the frontend lightweight and avoid a separate JavaScript build step for the dashboard.
- **Scalable queue**: jobs are stored in PostgreSQL and processed by workers. The same binary can run in `server`, `worker`, or `all` mode, allowing horizontal worker scaling later.
- **Operational complexity**: we must operate Pocket ID, a Cloudflare tunnel, and Resend DNS records in addition to the main application.

## Considered options

- **Python/FastAPI**: would have kept the team closer to the original DSPy prototype, but runtime performance, memory usage, and deployment packaging were concerns for a production SaaS.
- **Separate auth provider (Auth0/Clerk)**: would reduce operational load but introduces external dependency, cost scaling, and less control over email-code flows.
- **Next.js frontend + Rust API**: would enable richer client-side interactions but adds a second runtime, build pipeline, and CORS surface. Deferred until the UI demands it.
- **Kubernetes**: would provide more control but is overkill for the initial single-tenant SaaS deployment. Fly.io's managed platform is faster to operate.

## Related files

- `src/main.rs` — application bootstrap and routing
- `src/web/oidc.rs` — OIDC login and callback
- `src/config.rs` — environment configuration
- `fly.toml` — Fly.io app configuration
- `pocket-id/fly.toml` — Pocket ID Fly.io configuration
- `docs/dns-routing.md` — DNS setup details

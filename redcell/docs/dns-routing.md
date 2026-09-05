# DNS and Routing Setup for Redcells

This document describes how `redcells.net` and its subdomains are wired up to serve the Rust/Axum web application and API from Fly.io, with Pocket ID running as a separate Fly app.

## Overview

| Hostname | Service | Origin / Target |
|---|---|---|
| `redcells.net` | Redcells web dashboard and API | Fly app `redcell` (`redcell.fly.dev`) |
| `www.redcells.net` | Redirect to apex or mirror of `redcells.net` | Fly app `redcell` |
| `pocketid.redcells.net` | Pocket ID OIDC provider | Fly app `redcell-pocket-id` (`redcell-pocket-id.fly.dev`) |

The single Rust/Axum application serves both the Askama-rendered dashboard and the JSON API. Pocket ID runs as its own Fly app and is exposed directly via a DNS-only CNAME.

## Cloudflare DNS records

Create these records in the Cloudflare DNS dashboard for `redcells.net`.

### Website and API

| Type | Name | Target | Proxy |
|---|---|---|---|
| A | `@` (apex) | `<Fly IPv4 for redcell>` | Yes (recommended) |
| CNAME | `www` | `redcell.fly.dev` | Yes |

To get the Fly IPv4 address:

```bash
flyctl ips list -a redcell
```

> If you prefer a CNAME at the apex, use Cloudflare CNAME flattening. An A record pointing directly to the Fly IPv4 is the simplest configuration.

### Pocket ID

Pocket ID runs on Fly.io as `redcell-pocket-id` and is exposed through a DNS-only CNAME so Fly.io can terminate TLS for the OIDC provider directly.

| Type | Name | Target | Proxy |
|---|---|---|---|
| CNAME | `pocketid` | `redcell-pocket-id.fly.dev` | No |

Keep this record DNS-only (gray cloud). Proxying Pocket ID through Cloudflare can interfere with OIDC cookie behavior and `X-Forwarded-Proto` headers.

## Cloudflare proxying

Keep the orange cloud enabled for `redcells.net` and `www.redcells.net` to benefit from Cloudflare TLS, DDoS protection, and caching.

With the proxy enabled, set the allowed origins so the application trusts the public hostnames:

```bash
flyctl secrets set -a redcell \
  REDTEAM__CORS__ALLOWED_ORIGINS='https://redcells.net,https://www.redcells.net'
```

> Note: the current rate limiter uses the direct connection IP. With Cloudflare proxy enabled, all requests appear to come from Cloudflare edge IPs. For per-client limits behind a proxy, update `src/rate_limit.rs` to read `CF-Connecting-IP` or `X-Forwarded-For`.

## Redirect rules

Pick one canonical hostname. If the apex is canonical, add a redirect rule:

```
www.redcells.net/*  →  https://redcells.net/$1  (301)
```

If you prefer `www` as canonical:

```
redcells.net/*  →  https://www.redcells.net/$1  (301)
```

## Email deliverability (Resend)

Pocket ID sends one-time login codes through an SMTP provider. With Resend configured, the sending domain is `redcells.net` and the from address is `support@redcells.net`.

Add `redcells.net` as a sending domain in Resend and create the DNS records Resend provides in Cloudflare:

| Type | Name | Value / Target |
|---|---|---|
| TXT | `@` | `v=spf1 include:spf.resend.com ~all` (merge with any existing apex SPF) |
| TXT | `_dmarc` | `v=DMARC1; p=none; rua=mailto:dmarc@redcells.net` |
| CNAME | `resend._domainkey` | Resend DKIM CNAME value |
| TXT | `resend._domainkey` | Resend DKIM TXT value (if Resend provides both) |

> Use the exact DKIM values shown in the Resend dashboard after adding the domain.

## Application configuration

The Redcells app must know the public Pocket ID issuer URL and its own redirect URI:

```bash
flyctl secrets set -a redcell \
  REDTEAM__OIDC__ISSUER_URL='https://pocketid.redcells.net' \
  REDTEAM__OIDC__CLIENT_ID='redcell' \
  REDTEAM__OIDC__CLIENT_SECRET='<pocket-id-client-secret>' \
  REDTEAM__OIDC__REDIRECT_URI='https://redcells.net/auth/callback'
```

In Pocket ID, register an OIDC client with:

- **Client ID**: `redcell`
- **Redirect URIs**: `https://redcells.net/auth/callback`
- **Grant types**: authorization code
- **Scope**: `openid email profile`

## Verification

After deploy and DNS propagation:

```bash
# Redcells health
curl -s -o /dev/null -w "%{http_code}\n" https://redcells.net/health
# expect 200

# Redcells readiness
curl -s -o /dev/null -w "%{http_code}\n" https://redcells.net/ready
# expect 200

# Pocket ID discovery
curl -s -o /dev/null -w "%{http_code}\n" https://pocketid.redcells.net/.well-known/openid-configuration
# expect 200
```

## Related files

- `fly.toml` — Redcells Fly.io configuration
- `pocket-id/fly.toml` — Pocket ID Fly.io configuration
- `pocket-id/README.md` — Pocket ID build and deploy instructions
- `src/config.rs` — OIDC and environment configuration
- `src/web/oidc.rs` — OIDC login and callback handlers

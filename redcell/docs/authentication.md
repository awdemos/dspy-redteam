# Authentication

Redcells supports two authentication paths: OIDC for the dashboard and API keys for programmatic access.

## OIDC sign-in

The dashboard uses session cookies. When you click **Log in**, the application redirects you to Pocket ID at `pocketid.redcells.net`. After successful authentication, Pocket ID redirects back to `/auth/callback`, and Redcells creates or updates your user record and starts a session.

The session stores the user's ID and email. Dashboard pages that show sensitive data require an active session.

### Local development

In development mode, OIDC is optional. The local login page at `/login` accepts email and password. In production, OIDC is required and local password login is not used.

### Configuration

```bash
REDTEAM__OIDC__ISSUER_URL=https://pocketid.redcells.net
REDTEAM__OIDC__CLIENT_ID=redcell
REDTEAM__OIDC__CLIENT_SECRET=<secret>
REDTEAM__OIDC__REDIRECT_URI=https://redcells.net/auth/callback
```

Pocket ID must register `redcell` as an OIDC client with redirect URI `https://redcells.net/auth/callback` and scope `openid email profile`.

## API keys

API keys are scoped to a user. They are intended for automation, CI pipelines, and external integrations.

### Creating an API key

1. Sign in to the dashboard.
2. Go to **API Keys**.
3. Enter a name and click **Create key**.
4. Copy the key. The secret is shown only once.

### Using an API key

Include the key in the `Authorization` header as a Bearer token:

```bash
curl -H "Authorization: Bearer rt_<id>_<secret>" \
  https://redcells.net/api/jobs
```

### Revoking an API key

In the dashboard, click **Revoke** next to the key. Revoked keys return `401 Unauthorized` immediately.

### Permissions

API keys grant full access to job submission, listing, results, usage, and API key management for the owning user. Read-only keys are not implemented yet.

### Security recommendations

- Store API keys in a secret manager, not in source control.
- Rotate keys every 90 days or after team changes.
- Use separate keys for CI automation and local development.

## Suspended or deleted keys

If an API key is revoked, or if the database record is deleted, requests authenticated with that key return `401 Unauthorized`.

## Rate limits

- Public routes (`/health`, `/ready`, web login, web register): 30 requests/minute per IP.
- Authenticated API routes: 60 requests/minute per API key.
- Job creation (`POST /api/jobs`): 10 requests/minute per API key.

For shared rate limiting across replicas, set `REDTEAM__REDIS__URL`.

## Next steps

- [API reference](./api-reference.md)
- [Quickstart](./quickstart.md)

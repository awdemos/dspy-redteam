# KV Cache Store Pocket ID

This directory contains the build overlay that turns [Pocket ID](https://github.com/pocket-id/pocket-id) into the KV Cache Store identity provider.

## What is customized

- **Email-only login** — the login page only offers email login codes; passkey/WebAuthn UI is removed.
- **Branding** — logo, favicon, background, and app name are replaced with KV Cache Store assets.
- **Accent color** — defaults to the KV Cache Store amber (`#f59e0b`).
- **Email login codes for guests** — `emailOneTimeAccessAsUnauthenticatedEnabled` is enabled by default.
- **Visual consistency** — the login page is forced into a dark theme that matches the rest of the kvcachestore web app.
- **Right-hand description + copyright** — a short product pitch appears on the desktop right panel, and a copyright footer appears on the login form.

## Directory layout

- `dagger/overlay/` — files that replace the matching paths in the upstream Pocket ID source tree.
- `dagger/main.go` — Dagger module that clones Pocket ID upstream, applies the overlay, and builds an OCI image.
- `.env` — runtime environment configuration and Cloudflare tunnel token (do not commit secrets; this file is listed in `.gitignore`).

## Build

Requires [Dagger](https://dagger.io) and a container runtime (Podman/Docker).

```bash
cd /var/home/a/code/kvcachestore/pocket-id/dagger
dagger call build export --path /var/home/a/code/kvcachestore/pocket-id/pocket-id-branded.tar
```

This writes the OCI image tar to the host path. Load it into your local runtime:

```bash
podman load -i /var/home/a/code/kvcachestore/pocket-id/pocket-id-branded.tar
```

## Deploy to Fly.io

Pocket ID runs as a private Fly app with no public ports. A separate Fly app
runs `cloudflared` and exposes `pocketid.kvcachestore.com` through a
Cloudflare tunnel.

### One-time setup

1. Create a Cloudflare tunnel and copy the token:
   ```bash
   cloudflared tunnel create pocket-id
   cloudflared tunnel token <TUNNEL_ID>
   ```

2. Create the Fly apps and volume:
   ```bash
   cd /var/home/a/code/kvcachestore/pocket-id
   fly apps create kvcachestore-pocket-id
   fly apps create kvcachestore-pocket-id-cloudflared
   fly volumes create pocket_id_data --app kvcachestore-pocket-id --size 1 --region ord
   ```

3. Set secrets:
   ```bash
   fly secrets set ENCRYPTION_KEY="$(openssl rand -hex 32)" --app pocket-id
   fly secrets set TUNNEL_TOKEN="<token>" --app pocket-id-cloudflared
   ```

4. Publish the branded image to GHCR:
   ```bash
   cd /var/home/a/code/kvcachestore/pocket-id/dagger
   dagger call publish --registry=ghcr.io/kvcachestore/pocket-id --tag=branded \
     --username=GITHUB_USERNAME --secret=env:GHCR_TOKEN
   ```

5. Deploy:
   ```bash
   cd /var/home/a/code/kvcachestore/pocket-id
   fly deploy --app kvcachestore-pocket-id
   cd /var/home/a/code/kvcachestore/pocket-id/cloudflared
   fly deploy --app kvcachestore-pocket-id-cloudflared
   ```

6. Bootstrap Pocket ID:
   The `kvcachestore-portal` OIDC client and the initial admin user are created
   automatically when you run the top-level Dagger `deploy`, but for a manual
   Fly.io deploy you can run the bootstrap step separately:
   ```bash
   cd /var/home/a/code/kvcachestore
   dagger call -m ci/dagger bootstrap-pocket-id
   ```
   The default admin email is `admin@kvcachestore.com`. Change it to a real
   address in the Pocket ID dashboard after the first login.

6. Create a CNAME in Cloudflare DNS:
   - `pocketid.kvcachestore.com` → `<tunnel-id>.cfargotunnel.com`

### Updating

1. Rebuild and publish the branded image:
   ```bash
   cd /var/home/a/code/kvcachestore/pocket-id/dagger
   dagger call publish --registry=ghcr.io/kvcachestore/pocket-id --tag=branded \
     --username=GITHUB_USERNAME --secret=env:GHCR_TOKEN
   ```

2. Redeploy:
   ```bash
   cd /var/home/a/code/kvcachestore/pocket-id
   fly deploy --app kvcachestore-pocket-id
   ```

### Backups

The SQLite database lives on the Fly volume at `/app/backend/data/pocket-id.db`.
Schedule a backup to R2 or S3-compatible storage, e.g.:

```bash
fly ssh console --app pocket-id --command \
  "sqlite3 /app/backend/data/pocket-id.db '.backup /tmp/backup.db' && cat /tmp/backup.db" \
  > pocket-id-backup-$(date +%F).db
```

Or run a small cron Machine that copies the DB to R2 daily.

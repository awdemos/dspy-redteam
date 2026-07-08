#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="${FLY_APP_NAME:-redcell}"

echo "Deploying $APP_NAME to Fly.io..."

# Optional: validate required secrets are set before deploying
# (Fly will error at runtime if they're missing, but this gives faster feedback.)
required_secrets=(
  REDTEAM__JWT__SECRET
  REDTEAM__LLM__API_KEY
  REDTEAM__DATABASE__URL
)

for secret in "${required_secrets[@]}"; do
  if ! flyctl secrets list -a "$APP_NAME" | grep -q "^$secret "; then
    echo "WARNING: required secret $secret is not set."
    echo "Set it with: flyctl secrets set -a $APP_NAME $secret='...'"
  fi
done

flyctl deploy --app "$APP_NAME" --remote-only

echo "Deployed. Health check: https://$APP_NAME.fly.dev/health"

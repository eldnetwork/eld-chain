#!/usr/bin/env bash
# Apply a release on the EC2 host. Keeps named volumes and secret files.
set -euo pipefail

ROOT="${ELD_DEPLOY_ROOT:-/opt/eld-chain}"
COMPOSE_FILE="${ROOT}/compose.ghcr.yaml"
ENV_FILE="${ROOT}/.env"

if [[ ! -f "$COMPOSE_FILE" ]]; then
  echo "missing $COMPOSE_FILE" >&2
  exit 1
fi
if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing $ENV_FILE" >&2
  exit 1
fi
if [[ ! -f "${ROOT}/wallets/wallets.json" ]]; then
  echo "missing ${ROOT}/wallets/wallets.json (bootstrap secrets first)" >&2
  exit 1
fi

if [[ -n "${GHCR_TOKEN:-}" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io \
    -u "${GHCR_USERNAME:?GHCR_USERNAME is required when GHCR_TOKEN is set}" \
    --password-stdin
fi

pulled=0
for _ in 1 2 3 4 5; do
  if docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" pull; then
    pulled=1
    break
  fi
  sleep 5
done
if [[ "$pulled" -ne 1 ]]; then
  echo "docker compose pull failed" >&2
  exit 1
fi

docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" up -d --remove-orphans --force-recreate
docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps

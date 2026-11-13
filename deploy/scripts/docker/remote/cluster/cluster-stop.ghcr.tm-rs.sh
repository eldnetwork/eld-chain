#!/usr/bin/env bash
# Stop the rust-Tendermint 4-node stack (compose project deploy-tm-rs): all
# eld-app, Tendermint, faucet, and analytics containers, plus the compose
# network. Chain / Tendermint / faucet named volumes are left in place.
# The analytics-data volume is removed so the next --with-analytics start
# does not reuse an old CollectionRun store.
# Compose file is deploy/docker/remote/cluster/compose.ghcr.tm-rs.yaml.
# Image tags come from deploy/.env.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
ENV_FILE="${DEPLOY_DIR}/.env"
CLUSTER_DIR="${DEPLOY_DIR}/docker/remote/cluster"
COMPOSE_FILE="${CLUSTER_DIR}/compose.ghcr.tm-rs.yaml"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing env file: $ENV_FILE" >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$ENV_FILE"

require_env() {
  local name="$1"
  local value="${!name:-}"
  if [[ -z "$value" ]]; then
    echo "$name is not set in $ENV_FILE" >&2
    exit 1
  fi
}

require_env NODE_APP_VERSION_TAG_GHCR
require_env TM_RS_IMAGE

# Matches `name:` in compose.ghcr.tm-rs.yaml.
ANALYTICS_VOLUME="deploy-tm-rs_analytics-data"

echo "Stopping 4-node rust-Tendermint compose using ghcr.io/eldnetwork/eld-chain:${NODE_APP_VERSION_TAG_GHCR} and ${TM_RS_IMAGE}"
docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" \
  --profile faucet --profile analytics down --remove-orphans

if docker volume inspect "$ANALYTICS_VOLUME" >/dev/null 2>&1; then
  echo "Removing analytics volume ${ANALYTICS_VOLUME}"
  docker volume rm "$ANALYTICS_VOLUME"
else
  echo "No analytics volume ${ANALYTICS_VOLUME} to remove"
fi

echo "Stopped 4-node rust-Tendermint compose (chain volumes kept; analytics data deleted)."

#!/usr/bin/env bash
# SSH to the EC2 host and stop the rust-Tendermint 4-node stack in ~/cluster
# (compose project deploy-tm-rs): all eld-app, Tendermint, faucet, and analytics
# containers, plus the compose network. Chain / Tendermint / faucet named
# volumes are left in place. The analytics-data volume is removed so the next
# --with-analytics start does not reuse an old CollectionRun store.
# Connection settings come from deploy/.env.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
REPO_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
ENV_FILE="${DEPLOY_DIR}/.env"

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

require_env EC2_HOST
require_env EC2_USER
require_env EC2_SSH_KEY_FILE
require_env EC2_SSH_PORT

if [[ "$EC2_SSH_KEY_FILE" != /* ]]; then
  EC2_SSH_KEY_FILE="${REPO_ROOT}/${EC2_SSH_KEY_FILE}"
fi
if [[ ! -f "$EC2_SSH_KEY_FILE" ]]; then
  echo "missing SSH key file: $EC2_SSH_KEY_FILE" >&2
  exit 1
fi

REMOTE="${EC2_USER}@${EC2_HOST}"
SSH=(ssh -i "$EC2_SSH_KEY_FILE" -p "$EC2_SSH_PORT" -o StrictHostKeyChecking=accept-new)

"${SSH[@]}" "$REMOTE" bash -s <<'EOF'
set -euo pipefail

CLUSTER_DIR="${HOME}/cluster"
ENV_FILE="${CLUSTER_DIR}/.env"
COMPOSE_FILE="${CLUSTER_DIR}/compose.ghcr.tm-rs.yaml"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing env file on host: $ENV_FILE" >&2
  exit 1
fi
if [[ ! -f "$COMPOSE_FILE" ]]; then
  echo "missing compose file on host: $COMPOSE_FILE" >&2
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

echo "Stopping 4-node rust-Tendermint compose on ${HOSTNAME} using ghcr.io/eldnetwork/eld-chain:${NODE_APP_VERSION_TAG_GHCR} and ${TM_RS_IMAGE}"
docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" \
  --profile faucet --profile analytics down --remove-orphans </dev/null

if docker volume inspect "$ANALYTICS_VOLUME" >/dev/null 2>&1; then
  echo "Removing analytics volume ${ANALYTICS_VOLUME}"
  docker volume rm "$ANALYTICS_VOLUME"
else
  echo "No analytics volume ${ANALYTICS_VOLUME} to remove"
fi

echo "Stopped 4-node rust-Tendermint compose on ${HOSTNAME} (chain volumes kept; analytics data deleted)."
EOF

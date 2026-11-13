#!/usr/bin/env bash
# Wipe volumes and start the 4-node rust-Tendermint compose from genesis.
# Compose file, nodes, wallets, and faucet live in deploy/docker/remote/cluster.
# Image tags and the snapshot token come from deploy/.env.
#
# Optional flags (combinable):
#   --with-faucet      enable the faucet Compose profile
#   --with-analytics   enable the analytics sidecar profile
#                      (ghcr.io/eldnetwork/eld-analytics:${ANALYTICS_TAG_GHCR})
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

require_env ELD_SNAPSHOT_QUERY_TOKEN
require_env NODE_APP_VERSION_TAG_GHCR
require_env TM_RS_IMAGE

FAUCET_PROFILE=()
ANALYTICS_PROFILE=()

for arg in "$@"; do
  case "$arg" in
    --with-faucet)
      require_env FAUCET_VERSION_TAG
      WALLET="${CLUSTER_DIR}/faucet/wallets.json"
      if [[ ! -f "$WALLET" ]]; then
        echo "Missing faucet wallet: $WALLET" >&2
        echo "Create wallet-faucet-1 there before --with-faucet." >&2
        exit 1
      fi
      FAUCET_PROFILE=(--profile faucet)
      ;;
    --with-analytics)
      require_env ANALYTICS_TAG_GHCR
      ANALYTICS_PROFILE=(--profile analytics)
      ;;
    *)
      echo "Unknown option: $arg" >&2
      echo "Usage: $0 [--with-faucet] [--with-analytics]" >&2
      exit 1
      ;;
  esac
done

compose() {
  # Bash 3.2 + set -u treats an empty "${arr[@]}" as unbound.
  docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" \
    ${FAUCET_PROFILE[@]+"${FAUCET_PROFILE[@]}"} \
    ${ANALYTICS_PROFILE[@]+"${ANALYTICS_PROFILE[@]}"} \
    "$@"
}

echo "Starting 4-node compose without history using ghcr.io/eldnetwork/eld-chain:${NODE_APP_VERSION_TAG_GHCR} and ${TM_RS_IMAGE}"
if [[ ${#ANALYTICS_PROFILE[@]} -gt 0 ]]; then
  echo "Analytics image ghcr.io/eldnetwork/eld-analytics:${ANALYTICS_TAG_GHCR}"
fi
compose down --volumes --remove-orphans

for i in 1 2 3 4; do
  echo "eld-tendermint unsafe-reset-all on tendermint-${i}"
  compose run --rm --no-deps "tendermint-${i}" \
    eld-tendermint unsafe-reset-all --home /tendermint-rs/.tendermint
done

compose up -d

echo "Started 4-node rust-Tendermint compose with fresh state (history wiped)."

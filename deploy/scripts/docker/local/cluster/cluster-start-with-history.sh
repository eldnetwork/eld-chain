#!/usr/bin/env bash
# Restart the local 4-node compose, keeping named volumes (chain / Tendermint history).
# Does not run unsafe_reset_all — that would wipe validator state.
# Image tags come from deploy/.env (or DEPLOY_ENV_FILE). Does not rebuild images.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=deploy/scripts/_env.sh
source "${SCRIPT_DIR}/../../../_env.sh"
COMPOSE_FILE="$DEPLOY_DIR/docker/local/cluster/compose.yaml"

FAUCET_PROFILE=()
if [[ "${1:-}" == "--with-faucet" ]]; then
  require_env FAUCET_VERSION_TAG
  WALLET="$DEPLOY_DIR/docker/local/cluster/faucet/wallets.json"
  if [[ ! -f "$WALLET" ]]; then
    echo "Missing faucet wallet: $WALLET" >&2
    echo "Create wallet-faucet-1 there before --with-faucet." >&2
    exit 1
  fi
  FAUCET_PROFILE=(--profile faucet)
fi

compose() {
  # Bash 3.2 + set -u treats an empty "${arr[@]}" as unbound.
  docker compose -f "$COMPOSE_FILE" --env-file "$DEPLOY_ENV_FILE" ${FAUCET_PROFILE[@]+"${FAUCET_PROFILE[@]}"} "$@"
}

echo "Starting 4-node compose with history using ${ELD_APP_IMAGE} and ${ELD_TM_IMAGE}"
compose down --remove-orphans
compose up -d

echo "Started 4-node compose (volumes kept)."

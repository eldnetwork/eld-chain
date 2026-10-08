#!/usr/bin/env bash
# Wipe named volumes and start the local 4-node compose from genesis.
# After a volume wipe, each Tendermint data dir is empty, so
# priv_validator_state.json is missing until unsafe_reset_all recreates it.
# Eld and Tendermint images are pulled from GHCR. Does not build locally.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=deploy/scripts/_env.sh
source "${SCRIPT_DIR}/../../../_env.sh"
COMPOSE_FILE="$DEPLOY_DIR/docker/local/cluster/compose.ghcr.yaml"

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

echo "Starting 4-node compose without history using ghcr.io/eldnetwork/eld-chain:${NODE_APP_VERSION_TAG_GHCR} and ghcr.io/eldnetwork/eld-tendermint:${TENDERMINT_VERSION_TAG_GHCR}"
compose down --volumes --remove-orphans

for i in 1 2 3 4; do
  echo "tendermint unsafe_reset_all on tendermint-${i}"
  compose run --rm --no-deps "tendermint-${i}" \
    tendermint unsafe_reset_all --home /tendermint/.tendermint
done

compose up -d

echo "Started 4-node compose with fresh state (history wiped)."

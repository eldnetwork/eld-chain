#!/usr/bin/env bash
# Wipe deploy-tm-rs volumes and start the 4-node rust-Tendermint compose from genesis.
# Uses the same host config as compose.ghcr.yaml. Does not touch project `deploy` volumes.
# App image is ghcr.io/eldnetwork/eld-chain. Tendermint image is pinned in the compose file.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=deploy/scripts/_env.sh
source "${SCRIPT_DIR}/../../../_env.sh"
COMPOSE_FILE="$DEPLOY_DIR/docker/local/cluster/compose.ghcr.tm-rs.yaml"
TM_IMAGE="ghcr.io/eldnetwork/eld-tendermint-rs:v0.34.24-eld-tm-rs.1"

compose() {
  docker compose -f "$COMPOSE_FILE" --env-file "$DEPLOY_ENV_FILE" "$@"
}

echo "Starting 4-node compose without history using ghcr.io/eldnetwork/eld-chain:${NODE_APP_VERSION_TAG_GHCR} and ${TM_IMAGE}"
compose down --volumes --remove-orphans

for i in 1 2 3 4; do
  echo "eld-tendermint-rs unsafe-reset-all on tendermint-${i}"
  compose run --rm --no-deps "tendermint-${i}" \
    eld-tendermint-rs unsafe-reset-all --home /tendermint-rs/.tendermint
done

compose up -d

echo "Started 4-node rust-Tendermint compose with fresh state (history wiped)."

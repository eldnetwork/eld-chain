#!/usr/bin/env bash
# Stop the rust-Tendermint 4-node stack (compose project deploy-tm-rs): all
# eld-app and Tendermint containers, plus the compose network. Named volumes
# are left in place. Does not touch project `deploy` volumes. Does not rebuild
# or pull images. Image tags come from deploy/.env (or DEPLOY_ENV_FILE).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=deploy/scripts/_env.sh
source "${SCRIPT_DIR}/../../../_env.sh"
COMPOSE_FILE="$DEPLOY_DIR/docker/local/cluster/compose.ghcr.tm-rs.yaml"
require_env TM_RS_IMAGE

echo "Stopping 4-node rust-Tendermint compose using ghcr.io/eldnetwork/eld-chain:${NODE_APP_VERSION_TAG_GHCR} and ${TM_RS_IMAGE}"
docker compose -f "$COMPOSE_FILE" --env-file "$DEPLOY_ENV_FILE" --profile faucet down --remove-orphans

echo "Stopped 4-node rust-Tendermint compose (volumes kept)."

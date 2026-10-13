#!/usr/bin/env bash
# Stop the rust-Tendermint 4-node stack (compose project deploy-tm-rs): all
# eld-app, Tendermint, faucet, and analytics containers, plus the compose
# network. Chain / Tendermint / faucet named volumes are left in place.
# The analytics-data volume is removed so the next --with-analytics start
# does not reuse an old CollectionRun store. Does not touch project `deploy`
# volumes. Does not rebuild or pull images. Image tags come from deploy/.env
# (or DEPLOY_ENV_FILE).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=deploy/scripts/_env.sh
source "${SCRIPT_DIR}/../../../_env.sh"
COMPOSE_FILE="$DEPLOY_DIR/docker/local/cluster/compose.ghcr.tm-rs.yaml"
require_env TM_RS_IMAGE

# Matches `name:` in compose.ghcr.tm-rs.yaml.
ANALYTICS_VOLUME="deploy-tm-rs_analytics-data"

echo "Stopping 4-node rust-Tendermint compose using ghcr.io/eldnetwork/eld-chain:${NODE_APP_VERSION_TAG_GHCR} and ${TM_RS_IMAGE}"
docker compose -f "$COMPOSE_FILE" --env-file "$DEPLOY_ENV_FILE" \
  --profile faucet --profile analytics down --remove-orphans

if docker volume inspect "$ANALYTICS_VOLUME" >/dev/null 2>&1; then
  echo "Removing analytics volume ${ANALYTICS_VOLUME}"
  docker volume rm "$ANALYTICS_VOLUME"
else
  echo "No analytics volume ${ANALYTICS_VOLUME} to remove"
fi

echo "Stopped 4-node rust-Tendermint compose (chain volumes kept; analytics data deleted)."

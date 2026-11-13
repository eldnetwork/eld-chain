#!/usr/bin/env bash
# Copy deploy/docker/remote/cluster to ~/cluster on the EC2 host.
# SSH settings and image tags come from deploy/.env.
# Writes ~/cluster/.env with the tags the remote start and stop scripts read.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
ENV_FILE="${SCRIPT_DIR}/../.env"
CLUSTER_DIR="${REPO_ROOT}/deploy/docker/remote/cluster"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing env file: $ENV_FILE" >&2
  exit 1
fi
if [[ ! -f "${CLUSTER_DIR}/compose.ghcr.tm-rs.yaml" ]]; then
  echo "missing ${CLUSTER_DIR}/compose.ghcr.tm-rs.yaml" >&2
  exit 1
fi
if [[ ! -f "${CLUSTER_DIR}/wallets/wallets.json" ]]; then
  echo "missing ${CLUSTER_DIR}/wallets/wallets.json" >&2
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
require_env NODE_APP_VERSION_TAG_GHCR
require_env TM_RS_IMAGE
require_env ELD_SNAPSHOT_QUERY_TOKEN
require_env FAUCET_VERSION_TAG
require_env ANALYTICS_TAG_GHCR

if [[ "$EC2_SSH_KEY_FILE" != /* ]]; then
  EC2_SSH_KEY_FILE="${REPO_ROOT}/${EC2_SSH_KEY_FILE}"
fi
if [[ ! -f "$EC2_SSH_KEY_FILE" ]]; then
  echo "missing SSH key file: $EC2_SSH_KEY_FILE" >&2
  exit 1
fi

REMOTE="${EC2_USER}@${EC2_HOST}"
SSH=(ssh -i "$EC2_SSH_KEY_FILE" -p "$EC2_SSH_PORT" -o StrictHostKeyChecking=accept-new)
RSYNC_RSH="ssh -i ${EC2_SSH_KEY_FILE} -p ${EC2_SSH_PORT} -o StrictHostKeyChecking=accept-new"

"${SSH[@]}" "$REMOTE" "mkdir -p cluster"

rsync -a --exclude '.DS_Store' -e "$RSYNC_RSH" "${CLUSTER_DIR}/" "${REMOTE}:cluster/"

"${SSH[@]}" "$REMOTE" "cat > cluster/.env && chmod 600 cluster/.env cluster/wallets/wallets.json cluster/faucet/wallets.json cluster/nodes/*/app/p2p_keypair.json cluster/nodes/*/tendermint/node_key.json cluster/nodes/*/tendermint/priv_validator_key.json && chmod +x cluster/*.sh" <<EOF
NODE_APP_VERSION_TAG_GHCR=${NODE_APP_VERSION_TAG_GHCR}
TM_RS_IMAGE=${TM_RS_IMAGE}
ELD_SNAPSHOT_QUERY_TOKEN=${ELD_SNAPSHOT_QUERY_TOKEN}
FAUCET_VERSION_TAG=${FAUCET_VERSION_TAG}
ANALYTICS_TAG_GHCR=${ANALYTICS_TAG_GHCR}
EOF

echo "Copied ${CLUSTER_DIR} to ${REMOTE}:cluster"

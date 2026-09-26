#!/usr/bin/env bash
# One-time scp of compose, node files, and local secrets onto the EC2 host.
# Later GitHub deploys refresh public configs and image tags. They do not copy keys.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../../.." && pwd)"
CLUSTER_DIR="${REPO_ROOT}/deploy/docker/local/cluster"
COMPOSE_SRC="${REPO_ROOT}/deploy/docker/remote/cluster/compose.ghcr.yaml"
RELEASE_SRC="${SCRIPT_DIR}/deploy-release.sh"

: "${EC2_HOST:?EC2_HOST is required}"
: "${EC2_USER:?EC2_USER is required}"
: "${EC2_SSH_KEY_FILE:?EC2_SSH_KEY_FILE is required (path to the private key)}"
EC2_SSH_PORT="${EC2_SSH_PORT:-22}"
ROOT="${ELD_DEPLOY_ROOT:-/opt/eld-chain}"

if [[ ! -f "$EC2_SSH_KEY_FILE" ]]; then
  echo "missing SSH key file: $EC2_SSH_KEY_FILE" >&2
  exit 1
fi
if [[ ! -f "$COMPOSE_SRC" || ! -f "$RELEASE_SRC" ]]; then
  echo "missing compose or deploy-release.sh in this repo" >&2
  exit 1
fi
if [[ ! -f "${CLUSTER_DIR}/wallets/wallets.json" ]]; then
  echo "missing ${CLUSTER_DIR}/wallets/wallets.json" >&2
  exit 1
fi

for n in 1 2 3 4; do
  for rel in \
    "nodes/${n}/app/config.json" \
    "nodes/${n}/app/consensus_config.json" \
    "nodes/${n}/app/p2p_keypair.json" \
    "nodes/${n}/tendermint/config.toml" \
    "nodes/${n}/tendermint/genesis.json" \
    "nodes/${n}/tendermint/node_key.json" \
    "nodes/${n}/tendermint/priv_validator_key.json"
  do
    if [[ ! -f "${CLUSTER_DIR}/${rel}" ]]; then
      echo "missing ${CLUSTER_DIR}/${rel}" >&2
      exit 1
    fi
  done
done

STAGE="$(mktemp -d)"
cleanup() {
  rm -rf "$STAGE"
}
trap cleanup EXIT

mkdir -p "${STAGE}/wallets"
cp "$COMPOSE_SRC" "${STAGE}/compose.ghcr.yaml"
cp "$RELEASE_SRC" "${STAGE}/deploy-release.sh"
chmod +x "${STAGE}/deploy-release.sh"

for n in 1 2 3 4; do
  mkdir -p "${STAGE}/nodes/${n}/app" "${STAGE}/nodes/${n}/tendermint"
  cp "${CLUSTER_DIR}/nodes/${n}/app/config.json" "${STAGE}/nodes/${n}/app/config.json"
  cp "${CLUSTER_DIR}/nodes/${n}/app/consensus_config.json" "${STAGE}/nodes/${n}/app/consensus_config.json"
  cp "${CLUSTER_DIR}/nodes/${n}/app/p2p_keypair.json" "${STAGE}/nodes/${n}/app/p2p_keypair.json"
  cp "${CLUSTER_DIR}/nodes/${n}/tendermint/config.toml" "${STAGE}/nodes/${n}/tendermint/config.toml"
  cp "${CLUSTER_DIR}/nodes/${n}/tendermint/genesis.json" "${STAGE}/nodes/${n}/tendermint/genesis.json"
  cp "${CLUSTER_DIR}/nodes/${n}/tendermint/node_key.json" "${STAGE}/nodes/${n}/tendermint/node_key.json"
  cp "${CLUSTER_DIR}/nodes/${n}/tendermint/priv_validator_key.json" "${STAGE}/nodes/${n}/tendermint/priv_validator_key.json"
done
cp "${CLUSTER_DIR}/wallets/wallets.json" "${STAGE}/wallets/wallets.json"

if [[ -n "${ELD_CHAIN_IMAGE:-}" || -n "${ELD_TENDERMINT_IMAGE:-}" ]]; then
  : "${ELD_CHAIN_IMAGE:?ELD_CHAIN_IMAGE is required when ELD_TENDERMINT_IMAGE is set}"
  : "${ELD_TENDERMINT_IMAGE:?ELD_TENDERMINT_IMAGE is required when ELD_CHAIN_IMAGE is set}"
  app_tag="${ELD_CHAIN_IMAGE##*:}"
  tm_tag="${ELD_TENDERMINT_IMAGE##*:}"
  cat > "${STAGE}/.env" <<EOF
ELD_DEPLOY_ROOT=${ROOT}
NODE_APP_VERSION_TAG_GHCR=${app_tag}
TENDERMINT_VERSION_TAG_GHCR=${tm_tag}
ELD_CHAIN_IMAGE=${ELD_CHAIN_IMAGE}
ELD_TENDERMINT_IMAGE=${ELD_TENDERMINT_IMAGE}
EOF
fi

SSH=(ssh -i "$EC2_SSH_KEY_FILE" -p "$EC2_SSH_PORT" -o StrictHostKeyChecking=accept-new)
SCP=(scp -i "$EC2_SSH_KEY_FILE" -P "$EC2_SSH_PORT" -o StrictHostKeyChecking=accept-new)
REMOTE="${EC2_USER}@${EC2_HOST}"

"${SSH[@]}" "$REMOTE" "mkdir -p '${ROOT}/nodes' '${ROOT}/wallets'"
"${SCP[@]}" -r "${STAGE}/." "${REMOTE}:${ROOT}/"
"${SSH[@]}" "$REMOTE" "chmod +x '${ROOT}/deploy-release.sh'"

echo "Copied bootstrap files to ${REMOTE}:${ROOT}"
if [[ ! -f "${STAGE}/.env" ]]; then
  echo "No .env copied. Set ELD_DEPLOY_ROOT, ELD_CHAIN_IMAGE, and ELD_TENDERMINT_IMAGE on the host, or run the deploy-ec2 workflow."
fi

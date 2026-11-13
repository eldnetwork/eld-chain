#!/usr/bin/env bash
# Open an interactive SSH session on the EC2 host.
# Host, user, key, and port come from deploy/.env.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
ENV_FILE="${SCRIPT_DIR}/../.env"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing env file: $ENV_FILE" >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$ENV_FILE"

: "${EC2_HOST:?EC2_HOST is not set in $ENV_FILE}"
: "${EC2_USER:?EC2_USER is not set in $ENV_FILE}"
: "${EC2_SSH_KEY_FILE:?EC2_SSH_KEY_FILE is not set in $ENV_FILE}"
: "${EC2_SSH_PORT:?EC2_SSH_PORT is not set in $ENV_FILE}"

if [[ "$EC2_SSH_KEY_FILE" != /* ]]; then
  EC2_SSH_KEY_FILE="${REPO_ROOT}/${EC2_SSH_KEY_FILE}"
fi

if [[ ! -f "$EC2_SSH_KEY_FILE" ]]; then
  echo "missing SSH key file: $EC2_SSH_KEY_FILE" >&2
  exit 1
fi

exec ssh -i "$EC2_SSH_KEY_FILE" -p "$EC2_SSH_PORT" -o StrictHostKeyChecking=accept-new "${EC2_USER}@${EC2_HOST}"

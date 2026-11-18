#!/usr/bin/env bash
# Start the EC2 instance. Instance id and region come from deploy/.env.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
ENV_FILE="${DEPLOY_DIR}/.env"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing env file: $ENV_FILE" >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$ENV_FILE"

: "${EC2_INSTANCE_ID:?EC2_INSTANCE_ID is not set in $ENV_FILE}"
: "${ECR_AWS_REGION:?ECR_AWS_REGION is not set in $ENV_FILE}"

exec aws ec2 start-instances \
  --instance-ids "$EC2_INSTANCE_ID" \
  --region "$ECR_AWS_REGION"

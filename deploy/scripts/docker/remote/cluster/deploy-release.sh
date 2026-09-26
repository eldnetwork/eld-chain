#!/usr/bin/env bash
# Apply a release on the EC2 host. Keeps named volumes and secret files.
set -euo pipefail

ROOT="${ELD_DEPLOY_ROOT:-/opt/eld-chain}"
COMPOSE_FILE="${ROOT}/compose.ghcr.yaml"
ENV_FILE="${ROOT}/.env"

if [[ ! -f "$COMPOSE_FILE" ]]; then
  echo "missing $COMPOSE_FILE" >&2
  exit 1
fi
if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing $ENV_FILE" >&2
  exit 1
fi
if [[ ! -f "${ROOT}/wallets/wallets.json" ]]; then
  echo "missing ${ROOT}/wallets/wallets.json (bootstrap secrets first)" >&2
  exit 1
fi

if [[ -n "${GHCR_TOKEN:-}" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io \
    -u "${GHCR_USERNAME:?GHCR_USERNAME is required when GHCR_TOKEN is set}" \
    --password-stdin
fi

compose() {
  docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" "$@"
}

pulled=0
for _ in 1 2 3 4 5; do
  if compose pull; then
    pulled=1
    break
  fi
  sleep 5
done
if [[ "$pulled" -ne 1 ]]; then
  echo "docker compose pull failed" >&2
  exit 1
fi

# Empty Tendermint volumes have no priv_validator_state.json, and the node
# exits until unsafe_reset_all creates it. Later releases leave existing data.
for i in 1 2 3 4; do
  if compose run --rm --no-deps --entrypoint sh "tendermint-${i}" \
    -c 'test ! -f /tendermint/.tendermint/data/priv_validator_state.json'; then
    echo "tendermint unsafe_reset_all on tendermint-${i}"
    compose run --rm --no-deps "tendermint-${i}" \
      tendermint unsafe_reset_all --home /tendermint/.tendermint
  fi
done

compose up -d --remove-orphans --force-recreate

ready=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12; do
  if compose exec -T tendermint-1 curl -fsS http://127.0.0.1:26657/status >/dev/null; then
    ready=1
    break
  fi
  sleep 5
done
if [[ "$ready" -ne 1 ]]; then
  compose ps
  echo "tendermint-1 did not answer RPC on 26657" >&2
  exit 1
fi

not_running="$(compose ps --all --format '{{.Service}} {{.State}}' | awk '$2 != "running" { print }')"
if [[ -n "$not_running" ]]; then
  printf '%s\n' "$not_running" >&2
  echo "a compose service is not running" >&2
  exit 1
fi
service_count="$(compose ps --all --format '{{.Service}}' | wc -l | tr -d ' ')"
if [[ "$service_count" -ne 8 ]]; then
  echo "expected 8 compose services, saw ${service_count}" >&2
  exit 1
fi
compose ps

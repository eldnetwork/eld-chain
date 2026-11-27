#!/usr/bin/env bash
# Print last_block_height from Tendermint abci_info on local nodes 1-4.
# RPC ports: 26657, 26667, 26677, 26687.
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi

# A failure on one port must not skip the remaining ports.
set +e

ports=(26657 26667 26677 26687)
for i in "${!ports[@]}"; do
  port="${ports[$i]}"
  node=$((i + 1))
  echo "=== node ${node} (${port}) ==="
  body="$(mktemp)"
  err="$(mktemp)"
  url="http://127.0.0.1:${port}"

  http_code="$(
    curl -sS -o "$body" -w '%{http_code}' --max-time 10 \
      -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"abci_info","params":{}}' \
      "$url" 2>"$err"
  )"
  curl_status=$?
  if [[ "$curl_status" -ne 0 ]]; then
    reason="$(tr '\n' ' ' <"$err" | sed 's/[[:space:]]*$//')"
    if [[ -n "$reason" ]]; then
      echo "error: request failed: ${reason}"
    else
      echo "error: request failed"
    fi
    rm -f "$body" "$err"
    continue
  fi
  rm -f "$err"

  if [[ "$http_code" != 2* ]]; then
    echo "error: HTTP ${http_code}"
    rm -f "$body"
    continue
  fi

  if [[ ! -s "$body" ]]; then
    echo "empty response"
    rm -f "$body"
    continue
  fi

  rpc_error="$(jq -r '.error.message // empty' "$body" 2>/dev/null)"
  if [[ -n "$rpc_error" ]]; then
    echo "error: ${rpc_error}"
    rm -f "$body"
    continue
  fi

  height="$(jq -r '.result.response.last_block_height // empty' "$body" 2>/dev/null)"
  if [[ -z "$height" ]]; then
    echo "error: abci_info response has no last_block_height"
    rm -f "$body"
    continue
  fi

  echo "last_block_height: ${height}"
  rm -f "$body"
done

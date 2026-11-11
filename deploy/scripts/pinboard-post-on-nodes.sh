#!/usr/bin/env bash
# Fetch one pinboard post from local cluster REST ports 9001-9004.
# ./deploy/scripts/pinboard-post-on-nodes.sh --signed-address 0xe17404c417fa10cc04fdf73604fcacca8d0a687c --message-id 894b57bafd7aeff5a9f3f7641fbfee8e8e2f9ed15c3d6b61e3e6f929000f5746
set -euo pipefail

usage() {
  echo "usage: $(basename "$0") --signed-address <0x...> --message-id <id>" >&2
}

signed_address=""
message_id=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --signed-address)
      if [[ $# -lt 2 || "$2" == --* ]]; then
        usage
        exit 2
      fi
      signed_address="$2"
      shift 2
      ;;
    --message-id)
      if [[ $# -lt 2 || "$2" == --* ]]; then
        usage
        exit 2
      fi
      message_id="$2"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$signed_address" || -z "$message_id" ]]; then
  usage
  exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi

path="/@eld/pinboard/post/${signed_address}/${message_id}"

# A failure on one port must not skip the remaining ports.
set +e

for port in 9001 9002 9003 9004; do
  echo "=== $port ==="
  body="$(mktemp)"
  err="$(mktemp)"
  url="http://127.0.0.1:${port}/v1/pinboard/post"

  http_code="$(curl -sS -G --data-urlencode "path=${path}" -o "$body" -w '%{http_code}' --max-time 10 "$url" 2>"$err")"
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

  if [[ "$http_code" == "404" ]]; then
    message="$(jq -r '.message // empty' "$body" 2>/dev/null)"
    if [[ -n "$message" ]]; then
      echo "post not found: ${message}"
    else
      echo "post not found"
    fi
    rm -f "$body"
    continue
  fi

  if [[ "$http_code" != 2* ]]; then
    message="$(jq -r '.message // empty' "$body" 2>/dev/null)"
    if [[ -n "$message" ]]; then
      echo "error: HTTP ${http_code}: ${message}"
    else
      echo "error: HTTP ${http_code}"
    fi
    rm -f "$body"
    continue
  fi

  if [[ ! -s "$body" ]]; then
    echo "empty response"
    rm -f "$body"
    continue
  fi

  if ! jq -e 'type == "object" and .meta.message_id != null' "$body" >/dev/null 2>&1; then
    echo "error: response is not a pinboard post"
    rm -f "$body"
    continue
  fi

  jq '{cado_path, blob_status, content_key: .meta.content_key}' "$body"
  rm -f "$body"
done

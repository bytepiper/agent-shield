#!/usr/bin/env bash
set -euo pipefail

SOURCE_PATH="${BASH_SOURCE[0]}"
while [ -L "$SOURCE_PATH" ]; do
  SOURCE_DIR="$(cd "$(dirname "$SOURCE_PATH")" && pwd)"
  LINK_TARGET="$(readlink "$SOURCE_PATH")"
  if [[ "$LINK_TARGET" = /* ]]; then
    SOURCE_PATH="$LINK_TARGET"
  else
    SOURCE_PATH="$SOURCE_DIR/$LINK_TARGET"
  fi
done

SCRIPT_DIR="$(cd "$(dirname "$SOURCE_PATH")" && pwd)"
ASH_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_PATH="${AGENT_SHIELD_CONFIG:-$ASH_ROOT/config.env}"

if [ -f "$CONFIG_PATH" ]; then
  # shellcheck disable=SC1090
  source "$CONFIG_PATH"
fi

PROXY_URL="${AGENT_SHIELD_PROXY_URL:-${ASH_PROXY_URL:-http://127.0.0.1:8888}}"
CERT_PATH="${AGENT_SHIELD_CA_CERT:-${ASH_CA_CERT:-$ASH_ROOT/certs/mitmproxy-ca-cert.pem}}"
GEMINI_HOME_OVERRIDE="${AGENT_SHIELD_GEMINI_HOME:-${ASH_GEMINI_HOME:-}}"

usage() {
  cat <<EOF
Usage:
  ash <client> [args...]
  ash env

Examples:
  ash codex
  ash claude
  ash gemini -p 'say ok'
  AGENT_SHIELD_PROXY_URL=http://127.0.0.1:8888 ash codex
EOF
}

show_env() {
  cat <<EOF
ASH_ROOT=$ASH_ROOT
AGENT_SHIELD_PROXY_URL=$PROXY_URL
AGENT_SHIELD_CA_CERT=$CERT_PATH
HTTPS_PROXY=${HTTPS_PROXY:-}
NODE_EXTRA_CA_CERTS=${NODE_EXTRA_CA_CERTS:-}
SSL_CERT_FILE=${SSL_CERT_FILE:-}
REQUESTS_CA_BUNDLE=${REQUESTS_CA_BUNDLE:-}
CURL_CA_BUNDLE=${CURL_CA_BUNDLE:-}
HOME=${HOME:-}
EOF
}

if [ ! -f "$CERT_PATH" ]; then
  echo "Agent Shield CA cert not found: $CERT_PATH" >&2
  echo "Run ./scripts/init.sh first, or set AGENT_SHIELD_CA_CERT." >&2
  exit 1
fi

if [ "$#" -eq 0 ]; then
  usage
  exit 1
fi

case "$1" in
  env|--env)
    export HTTPS_PROXY="$PROXY_URL"
    export NODE_EXTRA_CA_CERTS="$CERT_PATH"
    export SSL_CERT_FILE="$CERT_PATH"
    export REQUESTS_CA_BUNDLE="$CERT_PATH"
    export CURL_CA_BUNDLE="$CERT_PATH"
    show_env
    exit 0
    ;;
  -h|--help|help)
    usage
    exit 0
    ;;
esac

export HTTPS_PROXY="$PROXY_URL"
export NODE_EXTRA_CA_CERTS="$CERT_PATH"
export SSL_CERT_FILE="$CERT_PATH"
export REQUESTS_CA_BUNDLE="$CERT_PATH"
export CURL_CA_BUNDLE="$CERT_PATH"

cmd="$1"
shift

if [ "$cmd" = "gemini" ]; then
  if [ -n "$GEMINI_HOME_OVERRIDE" ]; then
    export HOME="$GEMINI_HOME_OVERRIDE"
  fi
  if [ "${AGENT_SHIELD_KEEP_NO_BROWSER:-0}" != "1" ]; then
    unset NO_BROWSER || true
  fi
fi

exec "$cmd" "$@"

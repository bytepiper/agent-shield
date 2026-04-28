#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="${AGENT_SHIELD_HOME:-$HOME/.agent-shield}"
STATE_BIN_DIR="$STATE_DIR/bin"
STATE_CERT_DIR="$STATE_DIR/certs"
STATE_CONFIG="$STATE_DIR/config.env"
STATE_ASH="$STATE_BIN_DIR/ash"
STATE_CERT="$STATE_CERT_DIR/mitmproxy-ca-cert.pem"
LINK_TARGET=""

mkdir -p "$STATE_BIN_DIR" "$STATE_CERT_DIR"

install -m 0755 "$REPO_ROOT/scripts/ash.sh" "$STATE_ASH"
install -m 0644 "$REPO_ROOT/certs/mitmproxy-ca-cert.pem" "$STATE_CERT"

if [ ! -f "$STATE_CONFIG" ]; then
  cat >"$STATE_CONFIG" <<EOF
ASH_PROXY_URL=http://127.0.0.1:8888
ASH_CA_CERT=$STATE_CERT
# Optional:
# ASH_GEMINI_HOME=/root
EOF
fi

link_into() {
  local candidate="$1"
  local dir
  dir="$(dirname "$candidate")"

  mkdir -p "$dir"
  if [ ! -w "$dir" ]; then
    return 1
  fi

  if [ -e "$candidate" ] && [ ! -L "$candidate" ]; then
    echo "Skipping existing non-symlink: $candidate" >&2
    return 1
  fi

  ln -sfn "$STATE_ASH" "$candidate"
  LINK_TARGET="$candidate"
  return 0
}

if ! link_into "/usr/local/bin/ash"; then
  if ! link_into "$HOME/.local/bin/ash"; then
    echo "Failed to install ash into /usr/local/bin or ~/.local/bin" >&2
    exit 1
  fi
fi

cat <<EOF
Installed Agent Shield launcher.

State:
  $STATE_DIR

Command:
  $LINK_TARGET

Try:
  ash env
  ash codex
  ash claude
  ash gemini
EOF

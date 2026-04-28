#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for svg in "$repo_root"/docs/diagrams/*.svg; do
  png="${svg%.svg}.png"
  ffmpeg -v error -y -i "$svg" -frames:v 1 "$png"
  ls -lh "$png"
done

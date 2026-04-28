#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
frames_file="${TMPDIR:-/tmp}/agent-shield-demo-frames.txt"
palette_file="${TMPDIR:-/tmp}/agent-shield-demo-palette.png"
output_file="$repo_root/docs/screens/readme/agent-shield-demo.gif"
mp4_file="$repo_root/docs/screens/readme/agent-shield-demo.mp4"
video_filter="fps=8,crop=1440:560:0:0,scale=1280:-1:flags=lanczos"

cat > "$frames_file" <<EOF
file '$repo_root/docs/screens/dl/dl-01-listener-overview.png'
duration 1.1
file '$repo_root/docs/screens/dl/dl-02-search-filter-form.png'
duration 1.1
file '$repo_root/docs/screens/dl/dl-03-columns-menu.png'
duration 1.0
file '$repo_root/docs/screens/dl/dl-04-sorted-table.png'
duration 1.0
file '$repo_root/docs/screens/dl/dl-05-row-detail-right-pane.png'
duration 1.2
file '$repo_root/docs/screens/dl/dl-06-request-headers.png'
duration 1.0
file '$repo_root/docs/screens/dl/dl-07-request-body.png'
duration 1.2
file '$repo_root/docs/screens/dl/dl-08-response-body.png'
duration 1.4
file '$repo_root/docs/screens/dl/dl-08-response-body.png'
EOF

ffmpeg -v error -y -f concat -safe 0 -i "$frames_file" \
  -vf "$video_filter,palettegen=stats_mode=diff" \
  -frames:v 1 -update 1 \
  "$palette_file"

ffmpeg -v error -y -f concat -safe 0 -i "$frames_file" -i "$palette_file" \
  -lavfi "$video_filter[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
  "$output_file"

ffmpeg -v error -y -f concat -safe 0 -i "$frames_file" \
  -vf "$video_filter,format=yuv420p" \
  -movflags +faststart \
  "$mp4_file"

ls -lh "$output_file"
ls -lh "$mp4_file"

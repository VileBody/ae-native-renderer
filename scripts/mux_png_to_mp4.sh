#!/usr/bin/env bash
set -euo pipefail

FRAMES_DIR="${1:?frames dir required}"
FPS="${2:-30}"
OUT="${3:?output mp4 required}"

ffmpeg -y \
  -framerate "$FPS" \
  -i "$FRAMES_DIR/frame_%06d.png" \
  -c:v libx264 \
  -pix_fmt yuv420p \
  "$OUT"

#!/usr/bin/env bash
set -euo pipefail

IMAGE="${IMAGE:-ae-native-renderer:dev}"
JOB="${JOB:-demo_static}"

docker run --rm \
  -v "$PWD/jobs:/work/jobs" \
  -v "$PWD/fixtures:/work/fixtures" \
  "$IMAGE" \
  render --scene "/work/jobs/$JOB/scene.json" --out "/work/jobs/$JOB/out"

#!/usr/bin/env bash
set -euo pipefail

image="${IMAGE:-ae-native-renderer:dev}"
root="${PROFILE_ROOT:-target/media_profile_runs}"
suffix="${PROFILE_SUFFIX:-backend_gate}"
threshold_mean="${AE_BACKEND_THRESHOLD_MEAN:-3.0}"
threshold_max="${AE_BACKEND_THRESHOLD_MAX:-255}"
out_root="$root/backend_compare_${suffix}"
templates=(impulse_2nd scenes_3rd template_4th)

for template in "${templates[@]}"; do
  native="$root/gstreamer_${suffix}/$template"
  reference="$root/ffmpeg_${suffix}/$template"
  if [[ ! -d "$native" || ! -d "$reference" ]]; then
    echo "compare.skip template=$template missing profile output" >&2
    continue
  fi
  out="$out_root/$template"
  rm -rf "$out"
  echo "compare.run template=$template out=$out"
  docker run --rm \
    -v "$PWD:/work" \
    "$image" compare \
      --native "/work/$native" \
      --reference "/work/$reference" \
      --out "/work/$out" \
      --threshold-mean "$threshold_mean" \
      --threshold-max "$threshold_max"
  jq -r --arg template "$template" \
    '[
      $template,
      .summary.mean_abs_diff,
      .summary.max_abs_diff,
      .summary.changed_pixel_ratio,
      .summary.worst_mean_frame,
      .summary.worst_max_frame
    ] | @tsv' "$out/report.json"
done

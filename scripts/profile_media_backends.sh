#!/usr/bin/env bash
set -euo pipefail

image="${IMAGE:-ae-native-renderer:dev}"
root="${PROFILE_ROOT:-target/media_profile_runs}"
suffix="${PROFILE_SUFFIX:-backend_gate}"

runs=(
  "impulse_2nd|target/media_profile_scenes/impulse_2nd_2s_30fps_scene.json|s3_full_artifacts_examples/2nd_template_impulse/extracted/e73f911f128c47e0b8369628e4a1e202/app"
  "scenes_3rd|target/media_profile_scenes/scenes_3rd_2s_30fps_scene.json|s3_full_artifacts_examples/3rd_template_scenes/extracted/0069eaf78a5b4032b258d6b660743273/app"
  "template_4th|target/media_profile_scenes/template_4th_2s_30fps_scene.json|s3_full_artifacts_examples/4th_template_tape/extracted/06425e5a4d9d4836b67337bc31fac029/app"
)

for backend in gstreamer ffmpeg; do
  for item in "${runs[@]}"; do
    IFS="|" read -r name scene assets <<<"$item"
    if [[ ! -f "$scene" || ! -d "$assets" ]]; then
      echo "profile.skip backend=$backend template=$name missing scene/assets" >&2
      continue
    fi
    out="$root/${backend}_${suffix}/${name}"
    rm -rf "$out"
    echo "profile.run backend=$backend template=$name out=$out"
    docker run --rm \
      -e AE_RENDER_MEDIA_BACKEND="$backend" \
      -e AE_RENDER_MAX_OPEN_DECODERS="${AE_RENDER_MAX_OPEN_DECODERS:-6}" \
      -e AE_RENDER_MEDIA_FRAME_CACHE="${AE_RENDER_MEDIA_FRAME_CACHE:-4}" \
      -e AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP="${AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP:-180}" \
      -e AE_RENDER_PREWARM_FRAMES="${AE_RENDER_PREWARM_FRAMES:-1}" \
      -v "$PWD:/work" \
      "$image" render \
        --scene "/work/$scene" \
        --assets-root "/work/$assets" \
        --out "/work/$out"
  done
done

scripts/profile_media_summary.sh "$root" | rg "${suffix}|profile"

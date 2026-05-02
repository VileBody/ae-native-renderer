#!/usr/bin/env bash
set -euo pipefail

root="${1:-target/media_profile_runs}"

printf 'profile\ttemplate\tbackend\tframes\trender_total_ms\tavg_render_ms\tprepare_ms\trequests\tdecoded\tskipped\tspawns\tparks\treq_ms\tspawn_ms\tread_ms\tavg_req_ms\tavg_read_ms\n'

find "$root" -mindepth 3 -maxdepth 3 -name media-report.json -print | sort | while read -r report; do
  dir="$(dirname "$report")"
  manifest="$dir/manifest.json"
  profile="${dir#"$root"/}"
  profile="${profile%%/*}"
  template="${dir##*/}"
  frames="$(jq -r '.frames' "$manifest")"
  render_total="$(jq -r '.timing.total_ms|round' "$manifest")"
  avg_render="$(jq -r '(.timing.frames|map(.render_ms)|add/length)|round' "$manifest")"
  jq -r \
    --arg profile "$profile" \
    --arg template "$template" \
    --arg frames "$frames" \
    --arg render_total "$render_total" \
    --arg avg_render "$avg_render" \
    '[
      $profile,
      $template,
      (.sources[0].backend // "none"),
      $frames,
      $render_total,
      $avg_render,
      (.prepare.elapsed_ms // 0 | round),
      .totals.requests,
      .totals.frames_decoded,
      .totals.sequential_frames_skipped,
      .totals.decoder_spawns,
      .totals.decoder_parks,
      (.totals.request_ms|round),
      (.totals.decoder_spawn_ms|round),
      (.totals.frame_read_ms|round),
      (.derived.avg_request_ms|round),
      (.derived.avg_frame_read_ms|round)
    ] | @tsv' "$report"
done

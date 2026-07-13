#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "usage: $0 REQUEST_JSON [OUTPUT_DIR]" >&2
  exit 2
fi

request=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
output=${2:-target/perf/trendy_full_release}
output=$(mkdir -p "$output" && cd "$output" && pwd)
tmp_request="$output/request.benchmark.json"
response="$output/response.json"
time_log="$output/time.txt"

jq --arg output "$output" '
  .action = "render"
  | .requestId = ((.requestId // "trendy") + "-full-release-benchmark")
  | .outputSpec.directory = $output
  | .outputSpec.video = "result.mp4"
  | del(.outputSpec.frames)
' "$request" > "$tmp_request"

cargo build --release -p render-cli
rm -f "$response" "$time_log"

started=$(date +%s)
/usr/bin/time -l target/release/render-cli json \
  --request "$tmp_request" \
  --response "$response" \
  2> >(tee "$time_log" >&2)
finished=$(date +%s)

elapsed=$((finished - started))
baseline_seconds=${TRENDY_BASELINE_SECONDS:-288}
speedup=$(awk -v baseline="$baseline_seconds" -v elapsed="$elapsed" 'BEGIN { printf "%.2f", baseline / elapsed }')
rss_bytes=$(awk '/maximum resident set size/ {print $1; exit}' "$time_log")
rss_bytes=${rss_bytes:-0}
rss_mib=$((rss_bytes / 1024 / 1024))
status=$(jq -r '.status' "$response")
video=$(jq -r '.artifacts.video // .render.video // empty' "$response")

printf 'benchmark.status=%s\n' "$status"
printf 'benchmark.elapsed_seconds=%s\n' "$elapsed"
printf 'benchmark.peak_rss_mib=%s\n' "$rss_mib"
printf 'benchmark.speedup=%sx\n' "$speedup"
printf 'benchmark.video=%s\n' "$video"

if ((elapsed > 90)); then
  echo "performance gate failed: ${elapsed}s > 90s" >&2
  exit 1
fi
if ((rss_bytes > 1610612736)); then
  echo "memory gate failed: ${rss_mib}MiB > 1536MiB" >&2
  exit 1
fi
if ! awk -v baseline="$baseline_seconds" -v elapsed="$elapsed" 'BEGIN { exit !(baseline / elapsed >= 3.0) }'; then
  echo "speedup gate failed: ${speedup}x < 3.0x against ${baseline_seconds}s baseline" >&2
  exit 1
fi

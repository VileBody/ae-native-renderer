# Media Profiling

This document is for profiling media I/O without changing render math.

The renderer currently keeps AE-like math/compositing behavior separate from
decode/encode plumbing. Profiling here should answer media questions only:

- how many video sources were opened;
- how many frame requests hit cache;
- how many raw frames were decoded or skipped while advancing sequentially;
- how expensive decoder spawn and frame reads are;
- how expensive final mux is.

## Reports

Every `render` writes:

```text
out/media-report.json
out/manifest.json
out/render-log.jsonl
```

When MP4 mux is requested, it also writes:

```text
out/mux-report.json
```

`media-report.json` is the main media I/O report. Important fields:

```text
totals.requests                 footage frame requests from render-core
totals.cache_hits               requests served from per-source frame cache
totals.cache_misses             requests that had to decode
totals.frames_decoded           raw frames read from decoder pipes
totals.sequential_frames_skipped frames decoded while advancing to the requested frame
totals.decoder_spawns           decoder process starts
totals.decoder_restarts         seeks/restarts inside a source
totals.decoder_parks            decoders closed by the open-decoder pool
totals.request_ms               total time spent in VideoSource.frame_at
totals.decoder_spawn_ms         total decoder spawn time
totals.frame_read_ms            total raw frame read time
derived.avg_request_ms          request_ms / requests
derived.avg_frame_read_ms       frame_read_ms / frames_decoded
```

`manifest.json` still contains render/save timings and layer/effect profiles.
Use it to see whether the remaining bottleneck moved away from media I/O. Do not
optimize transform/sampling/effects from this document unless the target is AE
conformance, not raw speed.

## Tuning knobs

```bash
AE_RENDER_MAX_OPEN_DECODERS=6
AE_RENDER_MEDIA_FRAME_CACHE=4
AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP=180
```

- `AE_RENDER_MAX_OPEN_DECODERS`: limits simultaneously running decoder pipes.
- `AE_RENDER_MEDIA_FRAME_CACHE`: per-source decoded-frame LRU size; `0` disables it.
- `AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP`: maximum frame gap to advance by reading
  sequentially before restarting/seeking.

## Example

```bash
docker run --rm \
  -e AE_RENDER_MAX_OPEN_DECODERS=6 \
  -e AE_RENDER_MEDIA_FRAME_CACHE=4 \
  -e AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP=180 \
  -v "$PWD:/work" \
  ae-native-renderer:dev \
  render \
  --scene /work/target/native_template_runs/template_4th/preview_1fps_lowres_scene.json \
  --assets-root /work/s3_full_artifacts_examples/4th_template_tape/extracted/06425e5a4d9d4836b67337bc31fac029/app \
  --out /work/target/profile/template_4th
```

Summarize the result:

```bash
jq '.totals, .derived' target/profile/template_4th/media-report.json
jq '.timing.total_ms, .profile.effects' target/profile/template_4th/manifest.json
```

For a directory shaped like `target/media_profile_runs/<profile>/<template>`,
use:

```bash
scripts/profile_media_summary.sh target/media_profile_runs
```

Current baseline findings are tracked in
[MEDIA_PROFILE_FINDINGS.md](MEDIA_PROFILE_FINDINGS.md).

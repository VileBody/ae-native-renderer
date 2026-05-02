# Media Profile Findings

Baseline from May 2, 2026.

Scope: media I/O profiling only. Render math, transforms, sampling,
compositing, text, and effect behavior were not changed for this pass.

Test scene shape:

```text
target/media_profile_scenes/*_2s_30fps_scene.json
resolution: 270x490
duration: 2s
fps: 30
frames: 60
```

Profiles:

```text
default: AE_RENDER_MAX_OPEN_DECODERS=6, AE_RENDER_MEDIA_FRAME_CACHE=4, AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP=180
seek:    AE_RENDER_MAX_OPEN_DECODERS=6, AE_RENDER_MEDIA_FRAME_CACHE=4, AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP=0
pool2:   AE_RENDER_MAX_OPEN_DECODERS=2, AE_RENDER_MEDIA_FRAME_CACHE=4, AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP=180
```

## Summary

```text
profile  template      render_ms  requests  decoded  skipped  spawns  parks  request_ms  read_ms
default  impulse_2nd   2226       60        60       0        3       0      1031        958
default  scenes_3rd    1477       60        119      59       1       0      351         311
default  template_4th  2266       60        56       0        3       0      813         767
pool2    impulse_2nd   2869       60        60       0        3       1      1385        1175
pool2    scenes_3rd    1614       60        119      59       1       0      464         397
pool2    template_4th  2422       60        56       0        3       1      1140        1054
seek     impulse_2nd   2706       60        60       0        3       0      1218        1092
seek     scenes_3rd    8910       60        60       0        60      0      7498        7081
seek     template_4th  1761       60        56       0        3       0      796         732
```

## Read

- Default decoder policy is the best general baseline across all three samples.
- `AE_RENDER_MAX_OPEN_DECODERS=2` is slower here because decoder parking causes
  re-open pressure without enough memory benefit for these small scenes.
- `AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP=0` is dangerous for timelines like
  `scenes_3rd`: it turns a mostly sequential decode into repeated process starts.
- Cache hits are currently low in these samples; the next media win is likely a
  timeline-aware scheduler/prefetcher, not just a larger per-source cache.
- MP4 mux for `template_4th` 60 lowres frames was about 267 ms through FFmpeg CLI;
  decode/render is still the larger cost in this profile.

## Next Media Work

Done after this baseline:

1. Added `media-plan.json`, generated before render, grouping active footage
   requests by source in render-frame order.
2. Added source prepare/prewarm for sources needed in the initial render frames
   through `AE_RENDER_PREWARM_FRAMES`.
3. Added Rust GStreamer appsink backend behind the same `VideoSource` stats and
   selectable through `AE_RENDER_MEDIA_BACKEND`.
4. Added Rust GStreamer appsrc MP4 sink behind `VideoSink`, selectable through
   `AE_RENDER_MUX_BACKEND`.
5. Added repeatable backend profile/compare scripts:
   `scripts/profile_media_backends.sh` and `scripts/compare_media_backends.sh`.

## GStreamer AppSink / Backend Gate Pass

Follow-up profile from May 2, 2026. Same 2s/30fps scenes and media tuning knobs.

```text
profile             template      backend                 render_ms  prepare_ms  req_ms  read_ms  decoded  skipped  spawns
ffmpeg_p5_gate     impulse_2nd   ffmpeg-persistent-pipe  887        60          315     299      60       0        3
ffmpeg_p5_gate     scenes_3rd    ffmpeg-persistent-pipe  956        54          165     148      119      59       1
ffmpeg_p5_gate     template_4th  ffmpeg-persistent-pipe  898        54          300     283      56       0        3
gstreamer_p5_gate  impulse_2nd   gstreamer-appsink       721        256         136     84       60       0        3
gstreamer_p5_gate  scenes_3rd    gstreamer-appsink       807        247         26      15       119      59       1
gstreamer_p5_gate  template_4th  gstreamer-appsink       667        248         92      57       56       0        3
```

Read:

- GStreamer appsink moved more cost into source open/prewarm, but reduced
  per-frame media request/read time substantially.
- Total render time improved on all three current samples despite higher
  prewarm setup cost.
- Backend sanity diffs against FFmpeg were low on mean absolute difference
  (`1.39` to `1.86`), but not byte-identical. Keep backend choice visible in
  reports and use AE conformance for final visual truth.
- GStreamer appsrc MP4 sink smoke passed for `render --mp4` and `mux`; output
  probed as 270x490, 30 fps, 2.0 s MP4.

Remaining:

1. Add direct render-to-`VideoSink` path so production MP4 output can avoid
   writing and rereading PNG frames.
2. Add CI or nightly hooks for backend profile gates when media fixtures are
   available in the runner.

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

1. Add a timeline media planner that groups active footage requests by source and
   monotonic frame order before rendering.
2. Add explicit source-open/prewarm before frame 0 so first-frame decoder spawn
   spikes do not land inside layer render timing.
3. Add GStreamer appsink implementation behind the same `VideoSource` stats.
4. Add appsrc/encoder `VideoSink` and compare it against PNG-sequence + FFmpeg
   mux in `mux-report.json`.

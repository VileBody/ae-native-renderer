# media-gst

Media adapter crate for probe/decode/encode boundaries.

Current state:

- `doctor` checks GStreamer availability through `gst-inspect-1.0`.
- active probe paths use FFmpeg/ffprobe as bootstrap tooling.
- active frame extraction uses a persistent FFmpeg rawvideo pipe, not one process
  per frame.
- `VideoSource` is the boundary used by the CLI today.
- the CLI adds a per-source frame cache and bounds the number of open decoders.
- media tuning knobs: `AE_RENDER_MAX_OPEN_DECODERS`,
  `AE_RENDER_MEDIA_FRAME_CACHE`, and `AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP`.

Target state:

- server decode through GStreamer Rust bindings and `appsink`.
- server encode/mux through GStreamer `appsrc` pipelines.
- FFmpeg remains available as fallback/debug tooling, not as renderer core.

See `docs/ARCHITECTURE.md` for the full media I/O split.

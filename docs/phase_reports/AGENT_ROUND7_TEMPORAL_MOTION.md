# Agent Round 7 Temporal Motion

Status date: 2026-05-04.

## Scope

Worker 2 scope only: temporal graph, Posterize Time routing diagnostics,
adjustment-layer time routing diagnostics, and motion-blur sample diagnostics.
No effects formulas, turbulent field math, text layout/glyph math, or sampler
formula tuning were changed.

## Existing Telemetry Found

- `FrameRenderTrace.adjustment_effects` already captured adjustment effect
  `comp_time`, `layer_time`, `lower_stack_time`, `param_time`, input/output
  hashes, and Posterize bucket data.
- `render_sequence` and the conformance runner already emitted
  `adjustment_effects.jsonl`, plus text/expression/collapse sidecars.
- Missing before this pass: one readable temporal sidecar for ordinary layer
  source time, footage source frame id, Posterize quantized time, adjustment
  lower-stack time at the layer level, and motion-blur sample times.

## Changes

- Added `FrameRenderTrace.temporal` records:
  - event name: `temporal.layer_time` or `temporal.adjustment_layer_time`;
  - composition, layer id, layer type;
  - frame-local `comp_time`, `layer_start`, raw layer-local `layer_time`;
  - actual evaluated `posterized_time`;
  - footage/precomp `source_id`, `source_start`, `source_time`;
  - `source_frame_id` using the current comp fps floor policy;
  - `adjustment_lower_stack_time` for adjustment layers;
  - Posterize frame rate, bucket id, and bucket time when present.
- Added `FrameRenderTrace.motion_blur` records:
  - frame time, frame duration, shutter open/close, angle/phase;
  - requested and effective sample counts;
  - per-sample sample time, layer time, posterized time, source time/frame id
    where applicable, active flag, opacity, and accumulation weight.
- Added `temporal_telemetry.jsonl` output in render sequence output.
- Added `temporal_telemetry.jsonl` output in conformance case directories and
  included its path/count in each frame's `trace_sidecars` report object.
- Kept rendering math unchanged. The only test adjustment was to make an
  adjustment-posterize guard assert the timing trace instead of a
  Minimax-pixel side effect from another ownership area.

## Focused Tests

- `temporal_trace_records_posterized_footage_source_time`
  verifies a posterized footage layer records comp time, raw layer time,
  bucket time, source time, source start, and source frame id.
- `adjustment_posterize_keeps_downstream_param_time_live_inside_bucket`
  now checks lower-stack bucket time and downstream effect `param_time` through
  telemetry instead of relying on Minimax output pixels.
- `motion_blur_trace_records_sample_times_and_weights`
  verifies shutter bounds, four sample times, active flags, opacity, and
  weights for a motion-blurred layer.

## Case Diagnostics Unblocked

- `TMP_010`: source offset and layer start can now be checked directly via
  `source_time` and `source_frame_id`.
- `TMP_020`: Posterize buckets can now be checked without reading final PNGs;
  the sidecar records bucket id/time and the resulting footage frame id.
- `TMP_030`: motion-blur sample times and weights are now visible per frame.
- `STK_030`: adjustment lower-stack resampling and downstream effect param time
  are now visible alongside the existing per-effect hashes.

## Blockers

Closed:

- No readable per-frame temporal sidecar for source/posterize/motion-blur
  checkpoints.
- Motion-blur sample times were opaque in conformance output.

Not closed:

- AE parity is not claimed for `TMP_030` shutter behavior; this pass only makes
  sample timing inspectable.
- `STK_030` final RGB parity is still downstream of Geometry2, Minimax,
  Turbulent Displace, and alpha/color/operator differences.
- `source_frame_id` is diagnostic and currently uses the composition fps floor
  policy; it is not a decoded-media provider callback.

## Verification

Local `cargo` is not installed in PATH, so verification used Docker.

Passed:

```text
docker run --rm --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; rustup component add rustfmt >/tmp/rustfmt-install.log && cargo fmt -p render-core -p render-cli -- --check'

docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p render-core posterize -- --nocapture'

docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p render-core motion_blur -- --nocapture'
```

Results:

- `posterize`: 4 passed.
- `motion_blur`: 4 passed.

Render-cli:

- Plain `rust:1-bookworm` compile failed at link time because GStreamer dev
  libraries were absent.
- Rerun in a Docker shell with the repo Dockerfile's GStreamer/font dev
  packages passed:
  - `cargo test -p render-cli conformance -- --nocapture`: 3 passed.
  - `cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out /tmp/ae_agents/round7_temporal_motion --case TMP_020 --case TMP_030 --case STK_030`: `ok=true`, 3 cases.

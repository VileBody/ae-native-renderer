# Agent Round 8 Temporal Motion

Status date: 2026-05-04.

## Scope

Agent 2 scope only: motion-blur trace helpers and temporal/posterize unit
coverage in `render-core`. No effects formulas, turbulent scripts, text-engine,
testkit metrics, or render-cli paths were changed.

## Current Telemetry Inspected

- `FrameRenderTrace.motion_blur` already records shutter open/close, requested
  and effective sample counts, divisor, and per-sample `sample_time`,
  `posterized_time`, `source_time`, `source_frame_id`, `active`, `opacity`, and
  `weight`.
- `FrameRenderTrace.temporal` already records ordinary layer time,
  posterized layer time, source offset/frame id, and adjustment lower-stack
  time.
- The missing small question was whether a phase-shifted shutter and
  Posterize Time on the same motion-blurred footage layer can be read from one
  deterministic trace without inferring from final pixels.

## Change

- Added `MotionBlurTrace::weight_summary()` with active sample count,
  contributing sample count, and total accumulated weight.
- Added
  `motion_blur_trace_records_phase_offset_and_posterized_sample_times`.
  The test renders frame 4 at 8 fps with `shutter_angle=180`,
  `shutter_phase=90`, 4 samples, and a 4 fps Posterize Time effect on the
  motion-blurred footage layer.

## Temporal Question Now Answered

For a motion-blurred footage layer with Posterize Time, the trace can now answer
in one place:

- did shutter phase move the sample window relative to the comp frame time;
- what exact sample times were scheduled;
- whether those samples collapsed into the same posterize bucket;
- which source time/frame id the footage provider saw;
- whether every active sample contributed and whether weights sum to one.

## Manual Inspection

Representative unit trace output from
`cargo test -p render-core motion_blur -- --nocapture`:

```text
motion_blur representative: comp_time=0.500 shutter=[0.53125,0.59375] summary=MotionBlurWeightSummary { active_samples: 4, contributing_samples: 4, total_weight: 1.0 }
motion_blur sample[0]: sample_time=0.5390625 posterized_time=0.500 source_time=1.375 source_frame_id=Some(11) weight=0.25
motion_blur sample[3]: sample_time=0.5859375 posterized_time=0.500 source_time=1.375 source_frame_id=Some(11) weight=0.25
```

By hand: frame 4 at 8 fps is comp time `0.500`. With `shutter_phase=90`, the
open time moves forward by one quarter frame (`0.03125`), so the shutter window
is `[0.53125, 0.59375]`, fully after the frame timestamp. The first and last
sample times stay inside that window. Posterize Time at 4 fps floors both
samples back to bucket time `0.500`, so the footage source time is stable at
`1.375` after adding the layer/source offset, and the diagnostic frame id is
`Some(11)`. All 4 samples are active, all 4 contribute, and the weights sum to
`1.0`.

## Verification

Docker:

```text
docker run --rm -u $(id -u):$(id -g) --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p render-core motion_blur -- --nocapture'
docker run --rm -u $(id -u):$(id -g) --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p render-core posterize -- --nocapture'
git diff --check
```

Results:

- `motion_blur`: 5 passed.
- `posterize`: 5 passed.
- `git diff --check`: passed.

## Remaining AE Parity Work

- This is diagnostic/test coverage, not AE parity tuning.
- `TMP_030` still needs comparison of AE shutter convention, shutter phase
  sign/origin, sample weighting curve, and any AE-specific source-frame rounding
  policy before changing rendering math.
- Current source frame id remains diagnostic and uses the composition fps floor
  policy.

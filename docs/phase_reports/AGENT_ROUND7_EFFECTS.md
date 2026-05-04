# Agent Round 7 Effects

Date: 2026-05-04

Scope: Worker 3 / Effects only. Touched Box Blur behavior and effects docs; no
render-core temporal, turbulent, or text-engine files were edited.

## Current Inspection

- `ADBE Box Blur2`: `radius` and `iterations` were time-aware and visible in
  `box_blur_debug_trace`, but render still executed exactly one separable blur
  pass. Edge sampling clips the kernel to layer bounds by reducing the sample
  count near edges; it is not AE repeat-edge tuning.
- `ADBE Drop Shadow`: params cover color, opacity, direction, distance,
  softness, and shadow-only. Debug trace already separates source alpha, raw
  offset shadow, blurred shadow, and final composite.
- `ADBE Glo2`: `0001` / `based_on` is parsed as combined, color channels, or
  alpha channel; debug trace reports the resolved source rule and hashes
  threshold source, blurred glow, intensity-scaled glow, and final output.
- `ADBE Minimax`: operation and channel enums are parsed and reported, but the
  AE enum/channel mapping and neighborhood shape still need probe evidence
  before formula tuning.

## Patch

- Applied Box Blur `iterations` as repeated separable blur passes while keeping
  default behavior at one pass.
- Hardened the ambiguous Box Blur `0002` parser path: with `0001`/named radius
  present, `0002` is iterations; when `0002` is the only numbered scalar, it
  remains the legacy radius fallback and keeps one applied iteration.
- Added Box Blur debug telemetry for `iterations_applied`, first full
  iteration hash, and explicit edge policy label `clip_to_layer_bounds`.
- Added unit tests for repeated-pass behavior, legacy `0002` fallback,
  AE-numbered radius/iterations separation, iteration clamping, and debug trace
  hashes.
- Updated `docs/EFFECTS.md` to document applied iterations and the current edge
  policy.

## What Was Incomplete

The incomplete formula was not a subtle AE tuning issue: Box Blur exposed an
`iterations` control but ignored it in rendering. Because Box Blur feeds Drop
Shadow softness and Glow radius paths, this made downstream tuning noisy.

No Glow, Drop Shadow, or Minimax formula constants were changed in this round.
Those still need AE intermediate evidence rather than native-side guessing.

## Next AE Tuning Steps

1. Render isolated Box Blur probes for iterations `1/2/3` on an impulse, ramp,
   and near-edge square, then compare first-pass and final hashes/metrics.
2. Add an AE edge-policy probe for Box Blur repeat-edge versus clipped-window
   behavior before changing native edge sampling.
3. Use the Round 5 Glow and Minimax probe packs to resolve Glow threshold source
   and Minimax operation/channel enum semantics before formula changes.

## Verification

The host shell did not have `cargo` in `PATH`, so verification used the local
Docker `rust:1-bookworm` image with `/usr/local/cargo/bin/cargo`.

```text
cargo test -p effects
result: passed, 44 tests

cargo fmt -p effects -- --check
result: passed after installing the rustfmt component inside the ephemeral container
```

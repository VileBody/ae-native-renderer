# M13 Minimax Discriminator 2026-05-06

Scope: `ADBE Minimax` radius quantization, direction/channel enum behavior,
operation order, and `Don't Shrink Edges`.

## Artifacts

- AE builder: `fixtures/ae_probe_pack/minimax/jsx/build_minimax_discriminator_project.jsx`
- Measurement script: `fixtures/ae_probe_pack/minimax/scripts/measure_minimax_discriminator.py`
- Remote AE85 run: `target/ae_remote/minimax_discriminator_20260506/`
- Metrics: `target/ae_remote/minimax_discriminator_20260506/minimax_discriminator_measurements.json`

## Results

- Radius is positive round-half-up for the probed thresholds:
  `0.49 -> 0`, `0.5 -> 1`, `1.49 -> 1`, `1.5 -> 2`, `2.5 -> 3`.
- Direction matches the native enum:
  `1 = horizontal+vertical`, `2 = horizontal`, `3 = vertical`.
- Channel lanes match the native enum:
  `1 = color`, `2 = alpha+color`, `3 = red`, `4 = green`, `5 = blue`,
  `6 = alpha`.
- Operation order matches:
  `1 = minimum`, `2 = maximum`, `3 = minimum then maximum`,
  `4 = maximum then minimum`.
- `Don't Shrink Edges` polarity is now resolved:
  with `0005 = 0`, Minimax samples transparent black outside bounds;
  with `0005 = 1`, the sample window clips to image bounds.

## Native Change

`crates/effects/src/minimax.rs` now applies the `0005` edge policy instead of
only reporting it. The debug trace reports either
`transparent_black_outside_bounds` or `clip_to_image_bounds`.

One caveat: this AE output path exported alpha as full-comp opaque, so edge
policy was locked from RGB evidence. A deeper internal-alpha probe can be added
if a later template exposes alpha-only Minimax edge divergence.

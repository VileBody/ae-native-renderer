# OS Round 7 Multi-Agent Progress

Date: 2026-05-04

## Summary

Round 7 ran five focused workers across the renderer math directions. The theme
was not broad formula tuning; it was moving each block closer to
`instrumented/testable` with one concrete missing control or diagnostic.

## 1. Core Pixel / Metrics

Owned modules: `M01`, `M03`, `M04`, `M19`.

Progress:

- Added `foreground_rgb` conformance metric.
- It compares RGB only over a union foreground mask instead of letting invisible
  background/alpha pixels dominate formula triage.
- Existing `rgba`, `rgb`, `alpha`, and `background_alpha_normalized` metrics are
  unchanged.

Why it matters:

- Effects, text, and collapse work can now ask "how wrong is visible foreground
  RGB?" separately from "is alpha/background policy different?"

Next steps:

1. Use `foreground_rgb` when ranking Glow/Text/Motion failures.
2. Add premult/straight-alpha operator fixtures.
3. Tune Bezier/ease only after AE ease goldens are exported.

Report:

- `docs/phase_reports/AGENT_ROUND7_CORE_PIXEL_METRICS.md`

## 2. Temporal Graph / Motion

Owned modules: `M02`, `M15`, `M16`, `M18`.

Progress:

- Added `temporal_telemetry.jsonl`.
- Added per-layer records for comp time, layer time, posterized time, source
  time, source frame id, and adjustment lower-stack time.
- Added motion-blur traces for shutter interval, sample times, weights, opacity,
  activity, and sampled source frame ids.

Why it matters:

- `TMP_020`, `TMP_030`, and `STK_030` no longer need to be diagnosed only from
  final PNGs. We can now see where time is sampled.

Next steps:

1. Compare temporal telemetry against AE for Posterize bucket boundaries.
2. Tune motion blur shutter angle/phase/sample weighting against AE goldens.
3. Use `STK_030` only after isolated timing and field/effect modules are closer.

Report:

- `docs/phase_reports/AGENT_ROUND7_TEMPORAL_MOTION.md`

## 3. Effects

Owned modules: `M10`, `M11`, `M13`.

Progress:

- Box Blur `iterations` now actually runs repeated separable blur passes.
- The old behavior exposed an `iterations` control but rendered one pass.
- Debug trace now reports `iterations_applied`, `first_iteration_rgba`, and
  edge policy `clip_to_layer_bounds`.

Why it matters:

- Box Blur feeds standalone blur probes plus Drop Shadow softness and Glow blur.
  This removes one fake "knob" before AE kernel tuning starts.

Next steps:

1. Render AE blur probes for iterations `1/2/3`.
2. Decide edge policy from AE, not from final stack pixels.
3. Tune Glow source/radius/intensity/blend and Minimax neighborhood/channel
   behavior from isolated intermediates.

Report:

- `docs/phase_reports/AGENT_ROUND7_EFFECTS.md`

## 4. Warps / Procedural Fields

Owned modules: `M12`, `M14`.

Progress:

- Added AE-vs-native Turbulent vector comparison tool.
- The tool reads Round 5 AE `turbulent_vector_measurements.json`, reconstructs
  sample vectors from PNGs when needed, and compares them to the current native
  field model.

Representative result:

| Frames | Samples | Mean abs dx | Mean abs dy | Mean vector err | Max p95 vector err |
| ---: | ---: | ---: | ---: | ---: | ---: |
| `10` | `24,887` | `6.993` | `7.653` | `12.066` | `55.946` |

Why it matters:

- Turbulent Displace can now be tuned from dx/dy field error, not from
  `STK_030` final pixels.

Next steps:

1. Replace or validate the Python native mirror with Rust arbitrary-point field
   telemetry.
2. Fit displacement basis/sign/origin for type `1`.
3. Fit amount, size/frequency, complexity/octaves, evolution, seed, and edge
   policy in that order.

Report:

- `docs/phase_reports/AGENT_ROUND7_WARPS_FIELDS.md`

## 5. Text / Expressions / Collapse

Owned modules: `M05`, `M06`, `M07`, `M08`, `M09`, text-side `M17`.

Progress:

- Each glyph telemetry row now carries resolved font identity:
  `font_path`, family/style/postscript, fallback/source, and effective font
  size.
- Glyph rows are now self-contained enough for downstream AE layout comparison.

Why it matters:

- Text diffs can be separated into font mismatch, glyph metric mismatch,
  selector/animator mismatch, expression mismatch, or collapse raster mismatch.

Next steps:

1. Compare glyph records against AE bboxes/advances for `TXT_030` and `TXT_040`.
2. Add AE expression-selector amount probes.
3. Use collapse sharpness only after font/glyph identity matches.

Report:

- `docs/phase_reports/AGENT_ROUND7_TEXT_EXPR_COLLAPSE.md`

## OS Verification

Focused checks rerun on the combined worktree:

```text
cargo test -p testkit image_diff
result: passed, 5 tests

cargo test -p effects
result: passed, 44 tests

cargo test -p text-engine
result: passed, 13 tests

cargo test -p render-core posterize
result: passed, 4 tests

cargo test -p render-core motion_blur
result: passed, 4 tests

cargo fmt -p testkit -p render-core -p render-cli -p effects -p text-engine -- --check
result: passed

cargo test -p render-cli conformance
result: passed, 3 tests
```

Round 7 integration conformance:

```text
render-cli conformance-pack --case TMP_020 --case TMP_030 --case STK_030 --case EFF_020 --case EFF_070
result: ok=true, 5 cases
report: target/ae_agents/round7_os_integration/report.json
```

Selected summary metrics:

| Case | RGB Mean | Foreground RGB Mean | Foreground Pixels |
| --- | ---: | ---: | ---: |
| `TMP_020` | `0.0000` | `0.0000` | `786,432` |
| `TMP_030` | `0.1057` | `10.5905` | `15,696` |
| `STK_030` | `3.6582` | `10.9445` | `788,588` |
| `EFF_020` | `12.6578` | `35.0698` | `94,616` |
| `EFF_070` | `1.8914` | `18.3325` | `162,271` |

Temporal sidecars written:

- `target/ae_agents/round7_os_integration/TMP_020/temporal_telemetry.jsonl`
- `target/ae_agents/round7_os_integration/TMP_030/temporal_telemetry.jsonl`
- `target/ae_agents/round7_os_integration/STK_030/temporal_telemetry.jsonl`
- `target/ae_agents/round7_os_integration/EFF_020/temporal_telemetry.jsonl`
- `target/ae_agents/round7_os_integration/EFF_070/temporal_telemetry.jsonl`

Known caveat:

- Full `cargo test -p testkit` still has a pre-existing non-metrics font
  assumption failure in `phase5_font_assumptions_match_ae_manifest`; focused
  `image_diff` tests passed and `render-cli` compiled/passed conformance tests.

## OS Decision

Accept all five Round 7 steps.

None of these changes claims AE parity. They do move the project closer to the
stage where formula tuning is legitimate:

- metrics are less noisy;
- time sampling is observable;
- Box Blur has a real iterations knob;
- Turbulent has field-level AE-vs-native comparison;
- glyph telemetry is self-contained enough for AE layout comparison.

Next recommended order:

1. Use `foreground_rgb` plus temporal sidecars to choose the first real formula
   offender per case.
2. Tune Box Blur isolated iterations/edge behavior.
3. Tune Turbulent type-1 field basis from vector comparison.
4. Compare glyph telemetry against AE bboxes/advances.
5. Only then revisit composed `STK_030`, `EFF_070`, and `GPH_010`.

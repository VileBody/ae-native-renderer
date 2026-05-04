# OS Round 8 Multi-Agent Progress

Date: 2026-05-04

## Summary

Round 8 continued the five parallel math directions, with an explicit rule from
the user: agents must inspect representative artifacts/records by hand, not only
quote metrics. I accepted that rule for this round and reviewed the manual
inspection sections before running OS integration checks.

## 1. Core / Alpha / Bezier Foundation

Changed:

- `crates/testkit/src/image_diff.rs`
- `docs/phase_reports/AGENT_ROUND8_CORE_ALPHA_BEZIER.md`

Progress:

- Added `diff_rgb8_over_background`.
- It composites both RGBA buffers over the same RGB background and then compares
  the visible RGB result.

Manual inspection accepted:

- Invisible RGB differences such as transparent red vs transparent blue become
  zero after compositing over the sampled background.
- A premult-looking partial-alpha red still produces a visible difference over
  black, so the helper does not simply hide all alpha/RGB problems.

Why this matters:

- `foreground_rgb` tells us whether a pixel is foreground-like.
- `diff_rgb8_over_background` tells us whether the difference survives display
  compositing.

Next:

1. Wire this as optional conformance output when the report schema is ready.
2. Use black/white/corner-background comparisons for straight-vs-premult
   diagnosis.
3. Build Bezier/ease fixtures that compare visible composited output at sampled
   keyframe times.

## 2. Temporal / Posterize / Motion Blur

Changed:

- `crates/render-core/src/layer_eval.rs`
- `docs/phase_reports/AGENT_ROUND8_TEMPORAL_MOTION.md`

Progress:

- Added `MotionBlurTrace::weight_summary()`.
- Added deterministic coverage for phase-shifted shutter plus Posterize Time.

Manual inspection accepted:

```text
comp_time=0.500
shutter=[0.53125,0.59375]
sample[0].sample_time=0.5390625
sample[3].sample_time=0.5859375
posterized_time=0.500 for all samples
source_time=1.375
source_frame_id=Some(11)
total_weight=1.0
```

Human read:

- The shutter phase moves the sample window after the frame timestamp.
- Posterize Time then collapses all samples back to the same bucket.
- All samples contribute evenly.

Next:

1. Compare this trace shape against AE for `TMP_030`.
2. Decide AE shutter phase sign/origin and source-frame rounding.
3. Only then tune motion blur output.

## 3. Effects

Changed:

- `crates/effects/src/glow.rs`
- `docs/phase_reports/AGENT_ROUND8_EFFECTS.md`

Progress:

- Added a Glow debug-trace test that pins `threshold_source_rgba` for all
  `based_on` branches.

Manual inspection accepted:

| `based_on` | threshold source |
| --- | --- |
| `combined` | dark opaque and bright translucent both pass |
| `color_channels` | dark opaque is removed, bright translucent passes |
| `alpha_channel` | dark opaque passes, bright translucent is removed |

Human read:

- `0001` / `Glow Based On` is now verified at the source-mask/intermediate
  level, not only by final pixels.
- No Glow kernel, intensity, or blend constants were tuned without AE
  intermediate evidence.

Next:

1. Export the same two-pixel Glow source from AE.
2. Compare native/AE `threshold_source_rgba`.
3. Then tune blur kernel, intensity scale, and final composite in order.

## 4. Warps / Turbulent Field

Changed:

- `fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py`
- `docs/phase_reports/AGENT_ROUND8_WARPS_FIELDS.md`

Progress:

- Added signed permutation basis scan.
- Added grouped fitting hints by suite.
- Added `analysis.next_tuning_order`.
- Added `--csv-output`.

OS persisted the full Round 8 compare outputs:

- `target/ae_agents/round8_os_integration/turbulent_native_compare_round8_all.json`
- `target/ae_agents/round8_os_integration/turbulent_native_compare_round8_all.csv`

Full-run result:

| Case frames | Trusted samples | Mean vector err | Max p95 vector err |
| ---: | ---: | ---: | ---: |
| `102` | `253,549` | `13.700` | `75.664` |

Manual inspection accepted:

- `TD_AMOUNT_SWEEP_A045` center: AE `(8,-6)` vs native `(5,8)`.
- Nearby A045 samples do not agree with a simple global y-flip.
- Type-1 basis scan says `flip_xy` improves mean vector error by only `0.7%`.
- `TD_SIZE_SWEEP_S256` center AE `(30,-24)` vs native `(5,8)`, so size/frequency
  is fundamentally off before amount tuning.

Next:

1. Add Rust-backed arbitrary-point field telemetry or lock the Python mirror
   with tests.
2. Fit type-1 noise basis/origin/phase first.
3. Then fit amount, size/frequency, displacement branches, seed, complexity,
   evolution, and edges.

## 5. Text / Glyphs / Expressions / Collapse

Changed:

- `crates/text-engine/src/layout.rs`
- `docs/phase_reports/AGENT_ROUND8_TEXT_EXPR_COLLAPSE.md`

Progress:

- Added glyph `bbox_center`.
- Added `bbox_normalized`.
- Added `bbox_center_normalized`.

Manual inspection accepted from fresh OS conformance sidecars:

```text
TXT_030 glyph G:
  bbox=[-34.0263,205.0,54.0,52.0]
  bbox_center=[-7.0263,231.0]
  bbox_normalized=[-0.06646,0.40039,0.10547,0.10156]
  bbox_center_normalized=[-0.01372,0.45117]

TXT_040 selector unit 0:
  range_weight=0.08333334
  expression_weight=0.0
  final_weight=0.0
  local_time_after_delay=-0.05000000

GPH_010 collapsed text:
  effective_raster_scale=1.800003
  raster_size=[922,922]
  alpha_edge_max=255
```

Human read:

- We can now compare glyph placement in text-box-relative units even when
  collapse scaling changes raster size.
- `TXT_040` still points at expression-selector semantics, not glyph layout.
- Collapse sharpness should be judged only after normalized glyph placement
  matches AE.

Next:

1. Compare normalized glyph rows against AE bboxes/advances.
2. Add AE expression-selector samples for `TXT_040`.
3. Use collapse sharpness after font/glyph identity and normalized placement
   match.

## OS Verification

Focused checks:

```text
cargo test -p testkit image_diff
result: passed, 7 tests

cargo test -p effects
result: passed, 45 tests

cargo test -p text-engine
result: passed, 14 tests

cargo test -p render-core motion_blur
result: passed, 5 tests

cargo test -p render-core posterize
result: passed, 5 tests

cargo fmt -p testkit -p render-core -p effects -p text-engine -- --check
result: passed
```

Render CLI integration:

```text
cargo test -p render-cli conformance
result: passed, 3 tests

render-cli conformance-pack \
  --case TMP_030 --case TXT_030 --case TXT_040 --case GPH_010 --case EFF_020 --case STK_030
result: ok=true, 6 cases
report: target/ae_agents/round8_os_integration/report.json
```

Selected integration metrics:

| Case | RGB Mean | Foreground RGB Mean | Foreground Pixels |
| --- | ---: | ---: | ---: |
| `TMP_030` | `0.1057` | `10.5905` | `15,696` |
| `TXT_030` | `7.7574` | `67.9837` | `209,386` |
| `TXT_040` | `4.2506` | `154.6137` | `57,655` |
| `GPH_010` | `15.7070` | `183.2528` | `89,876` |
| `EFF_020` | `12.6578` | `35.0698` | `94,616` |
| `STK_030` | `3.6582` | `10.9445` | `788,588` |

## OS Decision

Accept all five Round 8 steps.

No module is parity-locked by this round. The important improvement is that the
next tuning passes now have better evidence:

- visible composited RGB diagnostics for alpha/premult questions;
- motion blur sample/bucket records that can be manually checked;
- Glow source-mask branch evidence;
- Turbulent fitting hints that rule out trivial sign flips;
- normalized glyph geometry for text/collapse comparisons.

Recommended next parallel round:

1. Core: wire `diff_rgb8_over_background` into conformance as optional
   black/white/corner metrics.
2. Temporal: compare `TMP_030` sidecar values against AE shutter samples.
3. Effects: generate/import AE Glow two-pixel intermediate probe.
4. Warps: add Rust-backed arbitrary-point Turbulent telemetry.
5. Text: rerun or import AE glyph bbox/advance records and compare normalized
   glyph geometry.

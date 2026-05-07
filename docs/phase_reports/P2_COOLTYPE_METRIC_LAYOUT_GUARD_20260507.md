# P2 CoolType Metric Layout Guard

Date: 2026-05-07.

## Scope

This pass implemented the first native `M05` CoolType metric-layout layer after
the static Ghidra and dynamic Frida probes.

The goal was deliberately narrow:

- use the recovered CoolType metric shape for glyph ids, hmtx advances, design
  bboxes, scaled bboxes, and sourceRect-like unions;
- expose those rows through text telemetry and the text-passport projection;
- keep current raster/selector behavior stable until deeper CoolType raster
  coverage and true sourceRect row partitioning are available.

## Changes

- Added `ttf-parser` to `text-engine`.
- `layout_text` now opens the resolved TTF face and emits CoolType-shaped
  metric rows when the face is parseable.
- `GlyphLayoutTelemetry` now includes:
  - `font_units_per_em`;
  - `advance_design_units`;
  - `advance_fixed16`;
  - `bbox_design_units`;
  - `bbox_scaled`.
- `TextLayoutTelemetry` now includes `source_rect_union`.
- `render-core` transforms `source_rect_union` into comp coordinates in text
  trace sidecars.
- `render-cli` text passport projection now carries the new row-level metric
  fields and reports `cooltype_metric_verified_rows`.

## Guardrail

The first naive implementation fed CoolType bboxes into runtime glyph placement.
It improved the text-only cases slightly but regressed `GPH_010`:

| Case | Baseline primary | Naive CoolType behavior | Delta |
| --- | ---: | ---: | ---: |
| `TXT_010` | `14.088211` | `13.887966` | `-0.200245` |
| `TXT_020` | `5.860491` | `5.776229` | `-0.084262` |
| `TXT_040` | `3.727372` | `3.467216` | `-0.260156` |
| `GPH_010` | `1.701600` | `3.115119` | `+1.413518` |

So the committed version keeps CoolType metrics as instrumentation/formula
inputs only. Runtime raster placement remains on the previous fontdue bitmap
bbox path until the raster/coverage probe is deep enough to switch behavior
without regressing glyph-animation cases.

## Validation

Focused tests:

```text
cargo test -p text-engine
cargo test -p render-cli text_passport
cargo check -p render-cli
```

Results:

```text
text-engine: 20 passed
render-cli text_passport: 6 passed
render-cli check: ok
```

Focused conformance:

```text
target/ae_agents/p2_close_cooltype_metrics_guarded_20260507
target/visual_review/p2_cooltype_metric_guard_20260507
```

The guarded run has no PNG metric movement against the pre-pass baseline, while
the sidecars now carry CoolType metric rows:

```text
TXT_010 W  metric_source=cooltype_shaped  status=cooltype_metric_verified
advance_design_units=1147  bbox_design_units=[99,0,1220,700]
bbox_scaled=[5.742,-40.6,70.76,0]
```

Master gate:

```text
target/ae_agents/p2_cooltype_metric_guard_master_20260507
accepted=2, tuning=1, approximate=16, regression=0, missing=0
```

## Remaining P2 Work

This closes the metric substrate step, not full text parity.

Still needed to close P2 fully:

- true CoolType glyph coverage/raster ids rather than hmtx/bbox only;
- AE sourceRect row partition semantics for whitespace and cumulative glyph
  runs;
- selector boundary refs for partial Start/End and non-square shapes;
- then a controlled switch from instrumented metrics into runtime glyph/selector
  geometry, guarded by `TXT_010`, `TXT_020`, `TXT_040`, and `GPH_010`.

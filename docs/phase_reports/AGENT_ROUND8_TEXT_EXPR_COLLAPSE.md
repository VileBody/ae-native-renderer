# Agent Round8 Text Expr Collapse

Status date: 2026-05-04.

## Scope

Agent 5 scope for this round was text/glyph/selector/expression/collapse
observability. The worktree was already dirty, including prior local changes in
`crates/text-engine/src` and broader render-core telemetry. This pass only adds
a focused glyph telemetry refinement in `text-engine` plus a report.

No glyph placement formula, selector formula, expression formula, rasterization
policy, collapse matrix policy, temporal/motion code, effects code, turbulent
scripts, or testkit metrics were changed.

## Change

`GlyphLayoutTelemetry` now includes text-box-relative geometry fields:

- `bbox_center`: source/raster-space center of the glyph bbox.
- `bbox_normalized`: `[x, y, w, h]` normalized against `text_box_rect`.
- `bbox_center_normalized`: center normalized against `text_box_rect`.

The normalized basis is:

```text
normalized_x = (bbox_x - text_box_x) / text_box_w
normalized_y = (bbox_y - text_box_y) / text_box_h
normalized_w = bbox_w / text_box_w
normalized_h = bbox_h / text_box_h
```

This makes AE comparison easier when the same text is rendered through
`layer_text` and `collapsed_text` paths with different raster sizes or collapse
scale. A glyph can now be compared by font id, glyph id, advance, raw bbox, bbox
center, normalized bbox, and normalized center without first reconstructing the
text-box coordinate basis outside the sidecar.

## Manual Inspection

Representative current telemetry was inspected by hand from existing sidecars:

- `target/ae_agents/round5_text_animator_blur/TXT_030/text_telemetry.jsonl`
- `target/ae_agents/round5_text_animator_blur/TXT_040/text_telemetry.jsonl`
- `target/ae_agents/round3_final_integrated/EXP_010/expression_telemetry.jsonl`
- `target/ae_agents/round5_collapse_contract/GPH_010/collapse_telemetry.jsonl`

New glyph diagnostic field shape, from the focused unit test:

```json
{
  "text_box_rect": [10.0, 20.0, 100.0, 50.0],
  "bbox": [10.0, 20.0, 12.0, 20.0],
  "bbox_center": [16.0, 30.0],
  "bbox_normalized": [0.0, 0.0, 0.12, 0.4],
  "bbox_center_normalized": [0.06, 0.2]
}
```

Meaning: this proves the coordinate basis directly. AE can export or derive a
glyph/source rect in the same text box; native no longer requires external
post-processing to compare center and extent as fractions of that text box.

`TXT_030` frame 0 glyph layout sample:

```json
{
  "character": "G",
  "font_glyph_id": 42,
  "advance": 59.330078125,
  "bbox": [-34.026275634765625, 205.0, 54.0, 52.0],
  "baseline": 256.0,
  "line_width": 586.0525512695312,
  "text_box_rect": [0.0, 0.0, 512.0, 512.0]
}
```

Meaning: Point-Light resolved correctly and the line is intentionally overfull:
the first glyph starts outside the left edge. With the new fields after a
sidecar rerun, this same glyph normalizes to approximately
`bbox_normalized=[-0.06646, 0.40039, 0.10547, 0.10156]` and
`bbox_center_normalized=[-0.01372, 0.45117]`. Those are the AE-comparable
placement facts independent of final comp scale.

`TXT_040` frame 0 expression-selector unit sample:

```json
{
  "index": 0,
  "total": 12,
  "rect": [0, 212, 27, 257],
  "range_weight": 0.0833333432674408,
  "expression_weight": 0.0,
  "final_weight": 0.0,
  "expression": {
    "text_index": 1,
    "text_total": 12,
    "local_time_after_delay": -0.05000000074505806,
    "raw_amount": 0.0,
    "clamped_amount": 0.0,
    "weight": 0.0
  }
}
```

Meaning: the range selector has a non-zero ramp contribution for unit 0, but the
expression selector delay has not elapsed, so `final_weight` becomes zero. This
is now easy to compare to AE's `textIndex`, `textTotal`, raw amount, and final
selector transfer. It also keeps the known question visible: whether zero
expression weight should leave the base text unchanged or suppress it for this
fixture must be settled with AE selector telemetry, not by glyph formula tuning.

`EXP_010` frame 5 position-expression sample:

```json
{
  "base_position": [256.0, 256.0],
  "sampled_position": [261.1483459472656, 255.4094696044922],
  "expression": {
    "type": "edge_wobble",
    "amp": 34.0,
    "freq": 2.0,
    "envelope": 0.17484748363494873,
    "offset": [5.148360252380371, -0.590537428855896]
  },
  "context": {
    "local_time": 0.1666666716337204,
    "frame_duration": 0.03333333507180214,
    "duration": 2.0
  }
}
```

Meaning: this record can be compared directly against AE property samples at
the same comp frame. The native sampled position is base plus the expression
offset after intro/outro envelope application.

`GPH_010` frame 0 collapsed text raster sample:

```json
{
  "mode": "collapsed_text_raster",
  "effective_raster_scale": 1.8000030517578125,
  "raster_size": [922, 922],
  "parent_matrix": [[1.7999999523162842, 0.0, -98.79998779296875], [0.0, 1.7999999523162842, -204.79998779296875], [0.0, 0.0, 1.0]],
  "effective_matrix": [[1.7999999523162842, 0.0, -98.79998779296875], [0.0, 1.7999999523162842, -204.79998779296875], [0.0, 0.0, 1.0]],
  "sharpness_probe": {
    "alpha_coverage_ratio": 0.003834915137798147,
    "alpha_edge_max": 255,
    "alpha_edge_mean": 0.4176529331270123
  }
}
```

Meaning: collapse is carrying the parent scale into text rasterization rather
than reusing a 512-sized child raster. The new normalized glyph fields give the
next missing bridge: compare `layer_text` and `collapsed_text` glyph placement
in text-box units first, then use `effective_raster_scale` and
`sharpness_probe` to reason about collapse sharpness.

## Verification

```sh
docker run --rm -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/work/target/ae_agents/round8_text_expr_collapse/cargo-target \
  rust:1-bookworm cargo test -p text-engine -- --nocapture
```

Result: passed, 14 tests.

```sh
docker run --rm -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/work/target/ae_agents/round8_text_expr_collapse/cargo-target \
  rust:1-bookworm sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; rustup component add rustfmt >/dev/null; cargo fmt -p text-engine -- --check'
```

Result: passed. The plain `rust:1-bookworm cargo fmt` invocation initially
reported that `cargo-fmt` was not installed for the container toolchain, so the
successful check installs the `rustfmt` component inside the disposable
container first.

```sh
git diff --check
```

Result: passed.

## Files Changed

- `crates/text-engine/src/layout.rs`
- `docs/phase_reports/AGENT_ROUND8_TEXT_EXPR_COLLAPSE.md`

## Caveats / Next Step

Existing target sidecars predate this patch, so they do not yet contain
`bbox_center`, `bbox_normalized`, or `bbox_center_normalized`. Rerun focused
`TXT_030`, `TXT_040`, and `GPH_010` traces to populate those fields, then
compare the normalized glyph rows against AE before touching glyph placement,
selector semantics, or collapse raster policy.

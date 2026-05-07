# P2-S4 Row Telemetry and Conformance Harness Design - 2026-05-08

## Scope

Goal: define the smallest useful telemetry surface that compares AE TXT ARE row spans
against native text row spans before doing more PNG-only tuning.

This report is design/prototype scope only. No shared Rust, tracer, or docs files were
modified. The concrete shared-code targets below should be treated as the proposed
implementation plan and escalated before editing.

Read context:

- `docs/phase_reports/P2_TXT_ARE_SPAN_TRACE_20260507.md`
- `scripts/ae_trace_cooltype_text.py`
- `crates/text-engine/src/rasterize.rs`
- `crates/render-cli/src/conformance_pack.rs`

Relevant existing surfaces:

- AE `--hook-profile txt-are-spans` emits `TXT_ARE_PixelWriter8_span_3b8c0`
  events with `span_type_arg3_s32`, `y_arg4_s32`, `start_x_arg5_s32`,
  `end_x_arg6_s32`, `source_pixel8_arg7`, `pf_world_arg8_snapshot`, and
  `coverage_bytes_if_type2`.
- Native text telemetry already travels through `FrameRenderTrace.text_layouts`
  into per-case `text_telemetry.jsonl` via `write_trace_sidecars`.
- Native `TextRasterTrace` currently records draw-char plans, coverage backend,
  raster origin, bitmap size, clipped bounds, and temp-world semantics, but not
  row spans.

## Proposed Native Sidecar Schema

Use one appended JSONL record per frame under the existing text sidecar transport:

```json
{
  "event": "text.row_spans",
  "frame": 0,
  "time": 0.0,
  "record": {
    "schema": "ae-native-renderer.text-row-spans.v1",
    "source": "native_text_engine",
    "composition": "TXT_010",
    "layer_id": "text_1",
    "render_path": "text_layer_raster",
    "canvas_size": [256, 256],
    "coverage_backend": "ttf_outline_nonzero_supersample",
    "coverage_quantization": "u8_coverage_alpha",
    "origin_policy": "round_raster_origin_to_i32",
    "clip_policy": "half_open_i32_canvas_clip",
    "rows": []
  }
}
```

Each row item should be deliberately denormalized so it can be diffed without
reconstructing full draw state:

```json
{
  "span_id": 123,
  "glyph_run_index": 0,
  "char_index": 0,
  "character": "W",
  "glyph_id": 58,
  "pass_index": 0,
  "pass_role": "fill",
  "span_type": 2,
  "y": 42,
  "start": 17,
  "end": 33,
  "length": 16,
  "coverage": {
    "mode": "hash",
    "hash": "sha256:...",
    "sample": [24, 40, 40, 60],
    "bytes": null
  },
  "source_pixel8": [255, 255, 255, 255],
  "target": {
    "surface": "main_canvas",
    "pf_world_semantics": "direct_source_over"
  },
  "clip": {
    "input_bounds_i32": [17, 42, 33, 43],
    "clipped_bounds_i32": [17, 42, 33, 43],
    "clipped": false
  }
}
```

Required fields:

- `glyph_id`: native glyph id from `DrawCharPlan.glyph_id`. For AE, this is
  inferred by current single-glyph fixture order first, then later by nearest
  draw-char call context once multi-glyph support is needed.
- `y`, `start`, `end`: integer half-open output coordinates. `end` is exclusive.
- `span_type`: preserve the AE integer. Native should initially emit `2` for
  coverage-bearing glyph mask spans; future solid stroke/fill runs can emit
  `0`/`1` only if the AE trace confirms those meanings for the same path.
- `pass_role`: `fill`, `stroke`, `composite`, or `unknown`. This is separate
  from `span_type`; AE pass role comes from the nearest active
  `TXT_ARE_Render_8bpc_fill_3d200` or `TXT_ARE_Render_8bpc_stroke_3d960`
  envelope in event order.
- `coverage`: enough payload to debug rows without making JSONL huge.

Coverage storage modes:

1. `sample` is always present for coverage spans: first up to 16 bytes plus
   `length`.
2. `hash` is always present for coverage spans: `sha256` over exactly the
   full coverage bytes for `[start, end)` when full bytes are available.
3. `bytes` is opt-in for fixtures/prototypes only, behind a debug/full flag.
   It should be omitted or `null` by default in conformance runs.

Native can compute full-byte hashes cheaply because it owns the coverage bitmap.
Current AE `coverageBytesForSpan` returns `sample_hex` capped at 64 bytes, so an
AE parser must mark hashes as `sample_hash` unless a small tracer extension is
approved to return either full bytes for short spans or an in-process hash over
the full range.

Recommended canonical row key:

```text
glyph_run_index/pass_index/pass_role/span_type/y/start/end
```

For AE single-glyph probes, use:

```text
case/pass_index/pass_role/span_type/y/start/end
```

until AE glyph ids are correlated to native layout rows.

## Where to Emit in Native

Minimal blast-radius implementation point: `crates/text-engine/src/rasterize.rs`.

Add row collection inside `blend_bitmap`, or a new sibling helper used by
`rasterize_text_with_layout`, because this is where native already has the exact
integer origin, clipped bounds, bitmap coverage, and output write policy. The
current draw loop writes pixel-by-pixel:

- `origin_x = x.round() as i32`
- `origin_y = y.round() as i32`
- skip zero coverage
- skip outside `clip`
- blend via `blend_text_pixel_ae_u8`

The row-span collector should run over the same bitmap with the same origin and
clip checks, grouping contiguous non-zero coverage bytes on a row into half-open
spans. Do not infer spans from final canvas alpha, because source-over and
background alpha erase the coverage signal.

Proposed low-risk Rust shape:

- Extend `TextRasterTrace` with optional `row_spans: Option<TextRowSpanTrace>`,
  or directly `row_spans: Vec<TextRowSpan>`. Optional is safer for schema
  migration.
- Add serializable structs in `rasterize.rs` only:
  - `TextRowSpanTrace`
  - `TextRowSpan`
  - `TextRowCoverage`
  - `TextRowSpanClip`
- Replace the two `blend_bitmap(...)` calls in `rasterize_text_with_layout` with
  a helper that first builds spans, appends them to a local `row_spans` vector,
  then calls the existing blend path unchanged.
- Pass explicit metadata into the helper: `glyph_run_index`, `char_index`,
  `character`, `glyph_id`, `pass_index`, `pass_role`, `target.surface`, and
  `source_pixel8`.

Why this location:

- It avoids touching `raster_cpu::Canvas`, shared compositor code, and render
  CLI comparison logic for the first step.
- It preserves current `text_telemetry.jsonl` writing. `render-core` already
  serializes `TextRasterTrace` under `draw_char`, and `render-cli` already writes
  it.
- It localizes any future on/off flag to text rasterization if trace size becomes
  an issue.

Small shared-code change that should be escalated before implementation:

- To make row spans first-class sidecars instead of nested under
  `/record/draw_char/row_spans`, add `FrameRenderTrace.text_row_spans: Vec<Value>`
  and a `write_value_trace_sidecar(..., "text_telemetry.jsonl",
  "text.row_spans", ...)` call in `crates/render-cli/src/conformance_pack.rs`.
- This is not required for the prototype. The prototype can read nested
  `draw_char.row_spans` from existing `text.layout` records.

## AE JSONL Parsing Plan

Input: local trace JSONL produced by `scripts/ae_trace_cooltype_text.py` with
`--hook-profile txt-are-spans`, e.g. `target/.../COV_W.jsonl`.

The parser should be a local/prototype script first, under
`target/ae_agents/p2_row_telemetry_20260508*/`, or later under a fixture-owned
probe directory if promoted.

Algorithm:

1. Read JSONL line by line.
2. Normalize Frida wrapper shape:
   - If the line has `payload`, use `payload`.
   - Otherwise use the object itself.
3. Keep only events where:
   - `kind == "cooltype_hook_enter"`
   - `hook == "TXT_ARE_PixelWriter8_span_3b8c0"`
   - `pixel_writer8_span` exists.
4. Track pass role by event order:
   - `TXT_ARE_Render_8bpc_fill_3d200` enter increments `pass_index` and sets
     active role `fill`.
   - `TXT_ARE_Render_8bpc_stroke_3d960` enter increments `pass_index` and sets
     active role `stroke`.
   - `TXT_ARE_OutputComposite_8bpc_3de50` can be recorded as `composite`, but
     should not be used as a glyph coverage row unless spans are emitted under
     that envelope.
   - If no role is known, use `unknown`; do not drop the span.
5. For each pixel-writer span, emit:
   - `source: "ae_txt_are"`
   - `seq`, `timestamp_ms`
   - `span_type`, `y`, `start`, `end`, `length`
   - `coverage.sample`, optional `coverage.sample_hash`, and optional
     `coverage.full_hash`
   - `source_pixel8_arg7`
   - `pf_world` width/height/pointer fields if present
   - `are_object` plane width/height/stride if present
   - `backtrace` first module offsets for debugging only

AE comparable row item:

```json
{
  "source": "ae_txt_are",
  "case": "COV_W",
  "seq": 57,
  "pass_index": 0,
  "pass_role": "fill",
  "span_type": 2,
  "y": 0,
  "start": 0,
  "end": 16,
  "length": 16,
  "coverage": {
    "mode": "sample_hash",
    "sample_hash": "sha256:...",
    "full_hash": null,
    "sample": [24, 40, 40],
    "bytes": null
  },
  "source_pixel8": {"a": 255, "r": 255, "g": 255, "b": 255}
}
```

Coverage bytes:

- For `span_type == 2`, use `coverage_bytes_if_type2.sample_hex` as the
  canonical current AE sample source.
- If full coverage bytes are unavailable, preserve the row with
  `coverage.mode = "sample_hash"` and compare only sample bytes/hash. Full hash
  comparison remains blocked until the tracer returns full bytes or full hash.
- If even sample bytes are unavailable, preserve the row with
  `coverage.mode = "missing"` and mark the fixture comparison as inconclusive
  for byte/hash matching but still useful for geometry.
- For non-type-2 spans, use `coverage.mode = "none"` unless a later AE trace
  proves another byte-carrying type.

Comparison strategy:

- Phase 1 geometry-only: compare counts by `pass_role`, y range, min/max start,
  min/max end, and exact set of `(pass_role, span_type, y, start, end)`.
- Phase 2 coverage sample: compare sample/sample_hash for rows with matching
  geometry; compare full_hash only when both sides expose it.
- Phase 3 diagnostic diff: emit first N unmatched AE rows and native rows, plus
  first N matching-geometry rows with different coverage hash.

## Fixture Cases

Sufficient minimal matrix for P2-S4:

1. `COV_W` from `fixtures/ae_probe_pack/p2_text_coverage_rows_probe`
   - Purpose: dense glyph with diagonals and many coverage rows.
   - Required first acceptance case.
   - Expected AE trace already observed: type-2 coverage spans, y range `0..67`,
     coverage plane width `109`, height `68`, stride `112`.

2. One clipped `W`
   - Use existing `fixtures/ae_probe_pack/p2_text_clip_probe`.
   - Recommended first case: `CLP_LEFT_N049` because left clipping directly tests
     `start` truncation and half-open integer spans.
   - Add `CLP_RIGHT_P049` next to validate exclusive `end` clipping.
   - Top/bottom can follow once horizontal semantics are stable.

3. One stroke case
   - Use existing `fixtures/ae_probe_pack/p2_text_stroke_probe`.
   - Start with `STR_STROKE_ONLY` to isolate stroke rows and active pass role.
   - Add `STR_FILL_STROKE_STROKE_OVER` only after stroke-only can be parsed,
     because it tests pass order and overlap without changing parser basics.

4. One semitransparent fill case
   - Use existing `fixtures/ae_probe_pack/p2_text_transfill_probe`.
   - Start with `TRF_WHT_A128_TRANSPARENT`.
   - Required checks are row geometry parity plus presence of temp-world transfer
     semantics. Row coverage should match opaque local fill if AE confirms the
     temp-world path; final PNG alpha should not be used as the row source.

Cases not needed for first acceptance:

- `COV_I` and `COV_O`: useful later as simple vertical and curved controls, but
  not necessary for the first row harness.
- All alpha/background permutations in transfill: useful for M19, too broad for
  first row-span conformance.
- All four clipped edges: useful for clip-rounding lock, too broad for P2-S4
  prototype.

## Minimal Prototype Scope

Prototype can be done without shared-code edits:

1. Local AE parser:
   - Input: existing or newly captured `txt-are-spans` JSONL.
   - Output:
     `target/ae_agents/p2_row_telemetry_20260508*/ae_rows_<CASE>.jsonl`
     and `ae_rows_<CASE>_summary.json`.

2. Native row extractor sketch:
   - Use a local script or one-off Rust experiment under `target/...` that reads
     current native `text_telemetry.jsonl`.
   - Until shared native emits rows, reconstruct approximate rows from
     `draw_char.draw_chars[*]` only for geometry envelope checks:
     `raster_origin`, `bitmap_size`, `clipped_bounds_i32`.
   - Mark coverage comparison as `blocked_native_row_bytes_missing`.

3. Design acceptance report:
   - For `COV_W`, prove the AE parser yields stable row counts, y range,
     span-type counts, coverage-byte count/hash availability, and first row
     samples matching the 2026-05-07 trace facts.

First shared-code implementation after approval:

1. Add row-span structs and collection in `rasterize.rs`.
2. Surface rows in `TextRasterTrace`.
3. Let existing `text_telemetry.jsonl` carry the nested row spans.
4. Add comparison code in `conformance_pack.rs` only after native rows exist,
   preferably as a focused `text_row_spans_comparison.json`.

## First Acceptance Criteria

AE parser acceptance:

- Given a `COV_W.jsonl` captured with `--hook-profile txt-are-spans`, parser
  emits only normalized row-span records for `TXT_ARE_PixelWriter8_span_3b8c0`.
- Summary reports:
  - `span_count == 1758` for the known 2026-05-07 COV_W capture, or the exact
    count from the new capture if rerun.
  - `type2_count > 0`.
  - `coverage_bytes_rows > 0`.
  - `min_y == 0` and `max_y == 67` for the known COV_W capture.
  - every row has `end >= start` and `length == end - start`.
  - every type-2 row with bytes has `len(bytes) == length`.

Native emission acceptance:

- `cargo test -p text-engine rasterize -- --nocapture` still passes.
- A conformance-pack run for one text case writes row spans into
  `text_telemetry.jsonl` without changing final PNG output.
- Native rows use the same origin and clip policy as `blend_bitmap`; no row
  exists for zero coverage or fully clipped pixels.
- The first `COV_W` native report includes at least:
  - `row_spans.rows.len() > 0`
  - y range and horizontal bounds
  - coverage hash/sample for every coverage-bearing row
  - `coverage_backend` and `origin_policy`

Harness acceptance:

- For `COV_W`, comparison reports separate geometry and coverage statuses.
- Geometry mismatch output includes first unmatched AE row and native row.
- Coverage mismatch output includes row key, AE sample/sample_hash/full_hash,
  and native sample/sample_hash/full_hash.
- For `CLP_LEFT_N049`, at least one row proves clipped `start`.
- For `STR_STROKE_ONLY`, parser assigns `pass_role == "stroke"` to stroke rows
  or marks the trace inconclusive if AE does not emit the expected render-stroke
  envelope.
- For `TRF_WHT_A128_TRANSPARENT`, row geometry is compared before final alpha,
  and the report records whether `PF_TransferRect` was observed.

## Open Questions

- Exact AE glyph id correlation is still unresolved for multi-glyph strings.
  First harness should stay single-glyph and map rows to case/pass order.
- AE `span_type` enum meanings beyond type `2` should remain numeric until
  stroke traces confirm them dynamically.
- Full coverage bytes are useful for fixture debugging but too large for default
  conformance sidecars. Default should be hash plus small sample.
- Semitransparent fill row spans should represent local temp-world coverage, not
  post-transfer alpha. The trace should expose target surface semantics so the
  comparison does not accidentally regress into PNG-only tuning.

## Recommended File Targets

Shared-code targets after escalation:

- `crates/text-engine/src/rasterize.rs`
  - Add `TextRowSpanTrace` structs.
  - Collect row spans from bitmap coverage before blending.
  - Store rows in `TextRasterTrace`.

- `crates/render-core/src/layer_eval.rs`
  - No change required for nested prototype.
  - Optional later change: split row spans into `FrameRenderTrace.text_row_spans`.

- `crates/render-cli/src/conformance_pack.rs`
  - No change required for nested prototype.
  - Optional later change: write `event: "text.row_spans"` records and compare
    against fixture AE rows.

Owned/local targets:

- `docs/phase_reports/P2_AGENT_ROW_TELEMETRY_HARNESS_20260508.md`
  - This design report.

- `target/ae_agents/p2_row_telemetry_20260508*/`
  - Local parser outputs and prototype summaries.

- `fixtures/ae_probe_pack/p2_text_row_telemetry_probe/**`
  - Optional future promoted fixture pack combining the four minimal cases
    above. Not necessary for the first design handoff because existing probe
    packs already cover the case matrix.

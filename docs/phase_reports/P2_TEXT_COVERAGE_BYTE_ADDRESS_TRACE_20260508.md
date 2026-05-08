# P2 Text Coverage Byte Address Trace - 2026-05-08

## Goal

Close the next P2 text-raster blocker after row/stroke/alpha live lock:
verify the exact AE coverage-byte address used by `TXT_ARE_PixelWriter8`
instead of relying on heuristic plane snapshots.

## Static Anchor

Local `TXT.dll` disassembly for `TXT_ARE_PixelWriter8_span_3b8c0`:

```asm
18003ba12  mov     rax, qword ptr [rdx + 0x10] ; coverage object
18003ba1b  mov     rdx, qword ptr [rax + 0x8]  ; coverage plane
18003ba2e  imul    rbx, qword ptr [rdx + 0x20] ; row stride in hot formula
18003ba42  add     rbx, qword ptr [rdx + 0x10] ; coverage base
18003ba5b  add     rdi, qword ptr [rdx + 0x20] ; output PF rowbytes
18003ba64  sub     eax, r8d                    ; span count candidate
18003ba71  movsxd  rsi, eax                    ; span count for loop
18003ba80  movzx   r8d, byte ptr [rbx]         ; actual coverage byte
18003ba90  call    0x18003c980                 ; covered pixel blend
```

The earlier `3ba1b` target was useful but incomplete: it fires before `RDX`
becomes the coverage plane. The actionable dynamic hooks are now:

- `TXT.dll+0x3ba2e`: coverage-plane stride multiplication point.
- `TXT.dll+0x3ba5b`: post coverage-pointer calculation. At this point
  `RBX` points at the bytes consumed by the pixel loop. The parser clamps the
  sample length to the row bounds from `3b8c0` (`end_x - start_x`) because the
  register count can include a loop guard byte on stroke rows.
- `TXT.dll+0x3ba80`: actual coverage-byte read point.
- `TXT.dll+0x3ba71`/`0x3ba74`: installed successfully, but did not execute as
  Frida Interceptor enter events in the focused GUI traces. They are kept as
  static anchors, not as the primary row-capture mechanism.

## Live Trace

Trace:

```text
target/dynamic_tools_85/p2_stroke_live_strokeonly_sample_byte_trace_20260508_001/STR_LIVE_STROKE_ONLY.jsonl
```

Parser output:

```text
target/ae_agents/p2_row_compare_stroke_sample_byte_20260508/ae_rows.json
```

Hook counts:

```text
TXT_ARE_PixelWriter8_span_3b8c0:              9
TXT_ARE_PixelWriter8_type2_load_3ba1b:        1
TXT_ARE_PixelWriter8_type2_stride_mul_3ba2e:  1
TXT_ARE_PixelWriter8_type2_stride_add_3ba5b:  1
TXT_ARE_PixelWriter8_type2_sample_byte_3ba80: 31
```

Observed type-2 stroke row:

```json
{
  "y": 7,
  "start_x": 6,
  "end_x": 37,
  "coverage_len": 31,
  "coverage_stride_source": "0x30",
  "coverage_sample_hex": "00001900000023000000c07f1b0862010000f045120862010000c0245d1062",
  "actual_coverage_sample_hex": "22404040404040404040404040404040404040404040404040404040404008",
  "actual_coverage_first_ptr": "0x16210976726",
  "actual_coverage_last_ptr": "0x16210976744"
}
```

Important correction: the old `coverage_sample_hex` is the legacy heuristic
sample using the plane snapshot and `+0x30`; on this stroke case it is not the
bytes AE actually consumes. The authoritative bytes are from the `3ba80`
sample-byte hook.

Second trace:

```text
target/dynamic_tools_85/p2_transfill_live_whta128_sample_byte_trace_20260508_001/TRFLIVE_WHT_A128_FILL_OPACITY.jsonl
target/ae_agents/p2_row_compare_transfill_sample_byte_20260508/ae_rows.json
```

Observed alpha-fill type-2 row:

```json
{
  "pass_role": "fill",
  "source_pixel_raw": "80ffffff",
  "y": 0,
  "start_x": 0,
  "end_x": 10,
  "coverage_sample_hex": "5bd0d0d0d0d0d0d0d0b6",
  "actual_coverage_sample_hex": "5bd0d0d0d0d0d0d0d0",
  "actual_coverage_sample_count": 9
}
```

For this first-row fill sample, the legacy plane heuristic and the actual
`3ba80` byte stream agree on the prefix. That explains why earlier fill-only
traces looked sane, but it does not make the snapshot formula safe for stroke
or later rows.

Third trace:

```text
target/dynamic_tools_85/p2_cov_w_sample_byte_trace_20260508_001/COV_W.jsonl
target/ae_agents/p2_row_compare_covw_sample_byte_20260508/ae_rows.json
```

Observed full-opacity `COV_W` first row:

```json
{
  "pass_role": "fill",
  "source_pixel_raw": "ffffffff",
  "y": 0,
  "start_x": 0,
  "end_x": 16,
  "coverage_sample_hex": "2440404040404040404040404040403c",
  "actual_coverage_sample_hex": "2440404040404040404040404040403c"
}
```

This confirms the parser/hook pair works on the original coverage fixture too.
The current AE job only emitted the first type-2 span for this pack on the
GUI API route, so a dense multi-row fill capture still needs a smaller
single-comp JSX or a trace mode that does not stop at the first emitted render.

Fourth trace:

```text
target/dynamic_tools_85/p2_cov_w_span_count_trace_20260508_001/COV_W.jsonl
target/ae_agents/p2_row_compare_covw_span_count_20260508/ae_rows.json
target/ae_agents/p2_row_compare_stroke_stride_fallback_20260508/ae_rows.json
```

This pass moved the scalable row capture from per-pixel `3ba80` reads to the
already-hit `3ba5b` point. `3ba5b` exposes the same authoritative byte stream
via `RBX`, while `3b8c0` supplies the row length:

```json
{
  "case": "COV_W",
  "role": "fill",
  "y": 0,
  "start_x": 0,
  "end_x": 16,
  "actual_coverage_sample_hex": "2440404040404040404040404040403c"
}
```

Re-parsing the stroke trace through the same rule preserves the previous
per-pixel answer exactly:

```json
{
  "case": "STR_LIVE_STROKE_ONLY",
  "role": "stroke",
  "y": 7,
  "start_x": 6,
  "end_x": 37,
  "actual_coverage_sample_hex": "22404040404040404040404040404040404040404040404040404040404008"
}
```

Dense legacy span trace plus native row diff:

```text
target/dynamic_tools_85/p2_shared_are_spans_covw_stride_20260507/COV_W.jsonl
target/ae_agents/p2_cov_w_native_scene_20260508/rendered/text_telemetry.jsonl
target/ae_agents/p2_row_compare_covw_dense_native_20260508/row_compare.json
```

The old dense trace has no `3ba5b` byte probe, but it still exposes AE's span
topology. A direct type-2-only comparison is misleading because AE uses
`span_type=1` for solid ink runs and `span_type=2` for byte-coverage edge runs.
The parser now merges adjacent `type2 + type1 + type2` spans into AE ink rows
and skips `type0` transparent gaps before comparing against native
`coverage_rows`.

The old, intentionally narrow type-2-only view:

```json
{
  "ae": { "row_count": 409, "extent": { "height": 68, "width": 109 } },
  "native": { "row_count": 203, "extent": { "height": 68, "width": 108 } },
  "common_y_start_end": 1,
  "ae_only_y_start_end": 408,
  "native_only_y_start_end": 202
}
```

The correct merged-ink view:

```json
{
  "ae": { "row_count": 202, "extent": { "height": 68, "width": 109 } },
  "native": { "row_count": 203, "extent": { "height": 68, "width": 108 } },
  "common_y_start_end": 140,
  "ae_only_y_start_end": 62,
  "native_only_y_start_end": 63
}
```

That is the useful result: the native outline fallback is not wildly wrong in
row topology after AE solid spans are modeled. The remaining mismatch is mostly
edge placement and width by one pixel on many rows, which points to
CoolType-compatible hinting/AA/subpixel rounding rather than a completely
different fill/composite path.

## Recovered Semantics

For this live stroke case:

- `3ba2e` sees a coverage plane with `width=137`, `height=89`,
  `base_0x10=0x16210976720`, `stride_0x20=0`, `stride_0x30=144`.
- `3ba80` proves the actual read pointer sequence starts at
  `base_0x10 + start_x`, then increments by one byte per covered pixel.
- `3ba5b` is the scalable row-level capture point for the same bytes. It avoids
  a per-pixel Frida event storm while still reading the hot `RBX` coverage
  pointer.
- Therefore `+0x30` is a useful plane snapshot field, but not a safe proxy for
  the hot coverage-byte address in every text path.
- Formula tuning must prefer `actual_coverage_sample_hex` when present; a fill
  first-row match is not enough to validate the plane snapshot heuristic.
- The row base may already be row-local by the time type-2 coverage is consumed;
  the next runner should prefer the `3ba80` byte stream when available.

## Code Changes

- `scripts/ae_trace_cooltype_text.py`
  - Added hooks for `3ba2e`, `3ba5b`, and `3ba80`.
  - Added span-count instrumentation for `3ba5b` and static anchors for
    `3ba71`/`3ba74`.
  - Added wide register snapshots and memory samples for the type-2 path.

- `scripts/compare_text_row_spans.py`
  - Added AE row-span parser.
  - Added plane-probe summaries by hook.
  - Added reconstruction of `actual_coverage_sample_hex` from `3ba80`.
  - Added scalable reconstruction from `3ba5b` `RBX` bytes, clamped by the
    `3b8c0` row bounds.
  - Added AE ink-row reconstruction by merging solid `type1` spans with
    adjacent byte-coverage `type2` spans.

## Status

P2 text raster is now better instrumented, not parity-locked.

Closed in this pass:

- Static/dynamic location of actual coverage-byte read.
- Proof that `3ba80` is the authoritative byte stream.
- Scalable row-level capture via `3ba5b` without per-pixel event volume.
- Parser support for row-level AE spans plus actual sample bytes.
- Normalized AE-vs-native merged-ink topology diff for `COV_W`.

Still open:

- Compare `actual_coverage_sample_hex` against native `coverage_rows` for
  matched glyph rows in a new dense trace that includes `3ba5b`.
- Use the diff to tune native coverage generation/hinting instead of the
  legacy plane `+0x30` heuristic.

## TXT Path Builder Direct Trace Follow-up

New direct trace:

```text
target/dynamic_tools_85/p2_txt_are_producer_direct_covw_20260508_001/COV_W.jsonl
```

Recovered from `TXT_ARE_Render_8bpc_fill_3d200` for `COV_W`:

```json
{
  "clip": { "top": 0, "left": 0, "bottom": 68, "right": 109 },
  "matrix6_f32_0x68": [1, 0, 0, 1, -9.055999755859375, 68],
  "fill_color": [1, 1, 1, 1],
  "stroke_enabled": 0
}
```

Recovered from `TXT_ARE_PathBuilder_40580`:

- glyph id `331` (`Montserrat-BoldItalic` `W`);
- `18` path commands;
- coordinates match the TTF glyph outline scaled by `96 / 1000`, reversed and
  with y negated.

This closes the previous "unknown outline producer" suspicion for this glyph:
the `W` path points are not hidden CoolType grid-fit points. The mismatch was
the coverage origin and fixed-subrow phase.

Native change:

- `crates/text-engine/src/rasterize.rs` now derives coverage origin in the same
  coordinate domain as AE:

```text
abs_left  = floor(glyph_x + bbox_x_min_scaled)
abs_right = ceil(glyph_x + bbox_x_max_scaled)
abs_top   = floor(baseline - bbox_y_max_scaled)
abs_bot   = ceil(baseline - bbox_y_min_scaled)

local_x_origin = abs_left - glyph_x
local_y_max    = baseline - abs_top
```

For the traced `COV_W` this gives:

```text
glyph_x=72.94400024414062
local_x_origin=9.055999755859375
local_y_max=68
width=109
height=68
```

which matches the Frida matrix exactly. The ARE scanline loop now samples the
lower edge of each fixed subrow:

```text
py = y_max - row - (subrow + 1) / 16
```

Validation:

```text
cargo test -p text-engine recovered_txt_are_origin_and_subrow_phase_match_cov_w_trace -- --nocapture
cargo test -p text-engine -- --nocapture
cargo test -p render-core text -- --nocapture
cargo test -p render-cli all_manifest_cases_have_native_recipes -- --nocapture
cargo run -q -p render-cli -- render \
  --scene target/ae_agents/p2_cov_w_native_scene_floor_end_20260508/scene.json \
  --out target/ae_agents/p2_cov_w_origin_phase_native_20260508/rendered
python3 scripts/compare_text_row_spans.py \
  --case COV_W \
  --ae-jsonl target/dynamic_tools_85/p2_shared_are_spans_covw_stride_20260507/COV_W.jsonl \
  --native-text-telemetry target/ae_agents/p2_cov_w_origin_phase_native_20260508/rendered/text_telemetry.jsonl \
  --out target/ae_agents/p2_row_compare_covw_origin_phase_dense_20260508/row_compare.json
cargo run -q -p render-cli -- conformance-pack \
  --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case GPH_010 \
  --out target/ae_agents/p2_text_origin_phase_gate_20260508
```

`COV_W` dense row diff improved:

```text
before: common ink rows 180 / 202, byte exact 0 / 2
after:  common ink rows 199 / 202, byte exact 2 / 3
```

The first two row-0 byte spans now match AE exactly:

```text
AE/native row0 0..16:  2440404040404040404040404040403c
AE/native row0 46..62: 013e4040404040404040404040404024
```

Remaining open:

- three normalized ink rows differ by one right-edge pixel:
  - `y=16, x=83..101` vs native `83..100`;
  - `y=52, x=53..82` vs native `53..81`;
  - `y=62, x=8..30` vs native `8..29`;
- the dense trace's third row-0 edge span says final byte `04`, while native
  produces `02`; this third byte is from the older dense span route and still
  needs an authoritative `3ba5b`/`3ba80` dense capture before changing formula.

## Authoritative ARE Row Getter Dense Follow-up

The older dense writer trace was still ambiguous after row 0 because the
legacy plane-snapshot heuristic could read stale/pointer-like bytes. The safer
route is `ARE_row_getter_8230`, which returns the row span buffer plus the
scratch coverage bytes after lazy row evaluation.

New dense row-getter artifacts:

```text
target/dynamic_tools_85/p2_cov_w_are_row_getter_dense_20260508_001/COV_W.jsonl
target/ae_agents/p2_cov_w_are_row_getter_dense_20260508/analysis.json
target/ae_agents/p2_row_compare_covw_projected_stable_20260508/row_compare.json
```

`scripts/compare_text_row_spans.py` now accepts:

```text
--ae-row-getter-analysis target/ae_agents/p2_cov_w_are_row_getter_dense_20260508/analysis.json
```

and reconstructs merged ink-row bytes from `span_type=1` solid runs plus
`span_type=2` coverage-byte runs. This is now the preferred P2 row coverage
metric source.

Static formula correction from Ghidra:

- `ARE+0x76dc` seeds 16 fixed subrow buckets per pixel row.
- `ARE+0x78e4` projects each active edge over the whole `1/16`-pixel vertical
  strip, producing `projected_min/projected_max`.
- `ARE+0x430c` emits `floor(projected_min)` and closes with
  `floor(projected_max) + 1`.
- `ARE+0x75d0` integrates those fixed x-event streams into coverage bytes.

Native change:

- Replaced the old single scanline-intersection approximation with the
  recovered projected-edge-strip event model.
- Preserved stable ordering for equal projected starts; adding an `x_max`
  secondary sort changes a vertex-tie byte.

Validation:

```text
python3 scripts/compare_text_row_spans.py \
  --case COV_W \
  --ae-row-getter-analysis target/ae_agents/p2_cov_w_are_row_getter_dense_20260508/analysis.json \
  --native-text-telemetry target/ae_agents/p2_cov_w_projected_stable_native_20260508/rendered/text_telemetry.jsonl \
  --out target/ae_agents/p2_row_compare_covw_projected_stable_20260508/row_compare.json
cargo test -p text-engine -- --nocapture
cargo test -p render-core text -- --nocapture
cargo test -p render-cli all_manifest_cases_have_native_recipes -- --nocapture
cargo run -q -p render-cli -- conformance-pack \
  --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case GPH_010 \
  --out target/ae_agents/p2_text_projected_stable_gate_20260508
```

`COV_W` row coverage status:

```text
before origin/phase: 199 / 202 ink rows,  24 / 199 merged bytes exact
after projected ARE: 202 / 202 ink rows, 201 / 202 merged bytes exact
type-2 edge rows:     3 / 3 exact
status: accepted
```

Remaining one-byte residual:

```text
norm y=48, x=53..84
AE:     b4fffffffffffffffffffffffffff7ffffffffffffffffffffffffffffe006
native: b4fffffffffffffffffffffffffff9ffffffffffffffffffffffffffffe006
```

Direct `TXT_ARE_PathBuilder` evidence shows the traced `W` contour order can
make this exact too, but applying a global contour reversal regressed multi-glyph
text conformance (`TXT_030` especially). So the production path keeps the
non-reversed TTF contour order until we have multi-glyph path-order evidence.

Status: P2 coverage for the recovered `COV_W` row-getter fixture is accepted
and no longer blocked on the ARE integrator. The remaining raster work is
multi-glyph path-order/CoolType path handoff evidence plus broader text-case
visual parity.

## TXT Path Order and Final Coverage Gates - 2026-05-08

New script: `scripts/analyze_text_path_order.py` compared TXT_ARE_PathBuilder traces against raw `Montserrat-BoldItalic` contours for three fixtures:

- `target/dynamic_tools_85/p2_cov_rows_safe_COV_I_20260508/COV_I.jsonl`
- `target/dynamic_tools_85/p2_cov_rows_safe_COV_O_20260508/COV_O.jsonl`
- `target/dynamic_tools_85/p2_txt_are_producer_direct_covw_20260508_001/COV_W.jsonl`

Output:

`target/ae_agents/p2_path_order_existing_20260508/path_order.json`

Findings:

- `COV_I` glyph `gid 134` is line-only and matches reversed TTF geometry with max error `0.000004865` while normal order is far off (`15.648`).
- `COV_W` glyph `gid 331` is line-only and matches reversed TTF geometry with max error `0.000009766` while normal order is far off (`19.433`).
- `COV_O` glyph `gid 204` uses curve commands and still requires curve producer decode.

Decision:

- Production now reverses only line-only contours.
- **Guardrail:** do not reverse curve contours until the TXT curve producer is decoded.

Artifacts:

- `target/ae_agents/p2_row_compare_covw_line_order_20260508/row_compare.json` (status: accepted)
  - normalized ink shape: `1.0`
  - coverage byte exact ratio: `1.0`
  - merged ink rows exact: `202/202`
  - mismatch examples: none

- `target/ae_agents/p2_text_line_order_gate_20260508/report.json` (focused gate ok for `TXT_010`, `TXT_020`, `TXT_030`, `TXT_040`, `GPH_010`)
- `target/ae_agents/p2_text_projected_stable_gate_20260508/report.json` (`TXT_010` mean improved by `-0.0372148`, `TXT_020` mean changed `+0.0185434`, `TXT_030`/`TXT_040`/`GPH_010` effectively unchanged, `text_mismatches=0`)

Validation commands:

```text
python3 -m py_compile scripts/analyze_text_path_order.py
cargo test -p text-engine recovered_txt_are_origin_and_subrow_phase_match_cov_w_trace
cargo test -p text-engine
cargo test -p render-core text
cargo test -p render-cli all_manifest_cases_have_native_recipes
```

## TXT Curve Producer Formula - 2026-05-08

Goal: close the `COV_O`/round-glyph path handoff with evidence, not visual
metric fitting.

Tracer update:

- `scripts/ae_trace_cooltype_text.py` now allows larger path-input captures:
  `readF32Array/readU32Array` cap `512`, `dumpPathInput` cap `256`.
- Successful producer trace:
  `target/dynamic_tools_85/p2_cov_o_full_path_20260508/COV_O.jsonl`
- Analysis:
  `target/ae_agents/p2_cov_o_curve_model_20260508/path_order.json`

Finding:

`COV_O` has `88` path commands:

```text
outer contour: move + 42 curve entries + close
inner contour: move + 42 curve entries + close
```

The captured prefix matches this deterministic formula:

```text
raw TrueType contour
-> reverse contour direction
-> if reversed contour starts with consecutive off-curve points, start at their implied midpoint
-> each quadratic segment becomes cubic triplet:
   c1 = q0 + 2/3 * (q1 - q0)
   c2 = q2 + 2/3 * (q1 - q2)
   end = q2
```

Evidence:

- `COV_O` curve stream prefix matches reversed TT quadratic-to-cubic with
  max error `0.001413px`, mean error `0.000482px`.
- Native unit `recovered_txt_curve_producer_matches_cov_o_prefix` locks the
  first traced cubic triplet.

Native implementation:

- `crates/text-engine/src/rasterize.rs` now parses simple `glyf` contours from
  raw `head/maxp/loca/glyf` tables.
- Line-only contours keep the locked reversed path.
- Curve contours use the recovered reversed TT quadratic-to-cubic producer and
  then flatten into the existing ARE scanline substrate.
- Composite/CFF/non-simple glyphs still fall back to the existing
  `ttf_parser::outline_glyph` path.

Validation:

```text
python3 -m py_compile scripts/ae_trace_cooltype_text.py scripts/analyze_text_path_order.py
python3 scripts/analyze_text_path_order.py --font fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf \
  --out target/ae_agents/p2_cov_o_curve_model_20260508/path_order.json \
  target/dynamic_tools_85/p2_cov_o_full_path_20260508/COV_O.jsonl
cargo test -p text-engine
cargo test -p render-core text
cargo test -p render-cli all_manifest_cases_have_native_recipes
cargo run -q -p render-cli -- conformance-pack \
  --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case GPH_010 \
  --out target/ae_agents/p2_text_curve_producer_gate_20260508
```

Focused gate remains `ok=true`. Against the previous line-order gate:

```text
TXT_010 mean: -1.19686
TXT_020 mean: +0.327816
TXT_030 mean: -0.363401
TXT_040 mean: -0.0827256
GPH_010 mean: +1.23584
text passport mismatches: 0 in all focused cases
```

Status: curve producer is now implemented/instrumented from Frida evidence.
The remaining curve-layer work is not the producer formula; it is the downstream
ARE curve flatten/edge/raster parity, because `GPH_010` still shows a visible
metric regression even though the focused gate passes.

## Static Follow-up: Where the Remaining Curve Drift Lives - 2026-05-08

Goal: answer whether the remaining P2 curve mismatch can be solved from the
current static corpus before sending more AE probes.

Static files read:

- `target/reverse/predecoded/20260507_223624_txt_drawchar_are_vtable`
- `target/reverse/predecoded/20260507_223741_txt_drawchar_are_helpers`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers`
- `target/reverse/predecoded/20260507_224844_txt_drawchar_are_pixel_writers`
- `target/reverse/predecoded/20260508_162000_p2_are_sampler_core`
- `target/reverse/predecoded/20260508_163607_p2_are_sampler_followups`

Confirmed TXT path command ABI:

```text
TXT.dll+0x404e0 move:  write x/y float pair, command 0
TXT.dll+0x40430 line:  write x/y float pair, command 1
TXT.dll+0x3f050 curve: write three x/y float pairs, command 2 for each point
TXT.dll+0x3ef60 close: re-emit contour start x/y, command 3
TXT.dll+0x3f310 render: dispatch by PF depth to 8/16/f32 fill/stroke paths
```

This matches the Frida path-input evidence: the curve producer emits cubic
triplets before the BIB/ARE handoff. TXT does not flatten those cubic triplets
in this layer.

Confirmed handoff:

```text
TXT.dll+0x3d200 / 0x3d4a0:
  path command buffer -> TXT.dll+0x40580

TXT.dll+0x40580:
  calls DAT_18087f778(&out_path, count, coords, commands, ...)
  retains returned path handle through DAT_18087c038

TXT.dll+0x3fd40:
  maps path handle through DAT_18087bd40 + 8 + path_id
  caches BIB path object under a critical section
  refreshes it through TXT.dll+0x408d0 when the version stamp changes

TXT.dll+0x408d0:
  DAT_18087beb8(path_table_entry) -> BIB path object
  DAT_18087bec8(BIB path object)  -> method/API block
  callback(path_id, 1, object+0x38) fills the returned coverage method block
```

Known dynamic mapping from the initialized process still applies:

```text
DAT_18087beb8 -> BIB.dll + 0x18150
DAT_18087bec0 -> BIB.dll + 0x18130
DAT_18087bec8 -> BIB.dll + 0x18140
DAT_18087c038 -> BIB.dll + 0x0a5f0
DAT_18087c040 -> BIB.dll + 0x0b080
```

Confirmed TXT row/span output layer:

- `TXT.dll+0x3de50` / `0x3df40` iterate y rows and consume row cells.
- Row cell ABI is `{ int32 span_type, int32 end_x }`.
- `span_type == 1` is solid coverage.
- `span_type == 2` reads one coverage byte per pixel and calls
  `TXT.dll+0x3c980`.
- `TXT.dll+0x3c980` / `0x3cb10` are the already-implemented AE integer
  source-over formulas.

Confirmed ARE row sampler layer:

- `ARE.dll+0x76dc` prepares 16 fixed subrow buckets for `row * 16 + subrow`.
- `ARE.dll+0x78e4` projects active edge x ranges over a unit-high strip.
- `ARE.dll+0x430c` converts projected ranges to integer event boundaries with
  `floor(projected_min)` and `floor(projected_max) + 1`.
- `ARE.dll+0x75d0` integrates the 16 subrows into a row coverage count.

Conclusion:

- The current native code is already on the right side of TXT path production,
  row/span output, and ARE-style 16-subrow integration.
- The remaining `COV_O`/`GPH_010` curve drift is upstream of
  `ARE.dll+0x75d0`, but downstream of TXT path command emission.
- The next static target is therefore the BIB/ARE path-object construction and
  raster painter path behind:
  `BIB.dll+0x18150`, `BIB.dll+0x18140`, `ARE.dll+0xb7e0`,
  and the painter functions that populate the active edge list before
  `ARE.dll+0x76dc`.

Guardrail:

- Do not tune curve flatness or byte amplitudes from final PNG metrics yet.
- If the next code change touches curve coverage, it should be justified by
  one of: BIB/ARE static decompile, Frida edge-list dump before `ARE+0x76dc`,
  or row-getter evidence showing the exact edge coordinates that TXT/BIB handed
  to ARE.

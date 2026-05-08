# P2 ARE Scanline Coverage — 2026-05-08

## Scope
- Static reverse targets and artifact-driven iteration report for ARE scanline coverage work.
- No code edits in this report pass.

## Static reverse bundles consulted
- `target/reverse/predecoded/20260508_152401_p2_are_row_getter_static/are_text_raster`
- `target/reverse/predecoded/20260508_153626_p2_are_raster_painter_vtable/are_text_raster`
- `target/reverse/predecoded/20260508_162000_p2_are_sampler_core/are_sampler_core`

## Recovered static facts
- ARE+`0x8230` row getter:
  - Cache key is y-coordinate (`param_2`) with stored last row at `param_1 + 0xc8`.
  - Row source table at `param_1 + 0x150` indexed by `param_2 - *(param_1 + 0x8c)`.
  - Per-row linked node chain is traversed via `node + 0x10`.
  - Span cells are emitted into the output span buffer at `param_1 + 0x110`.
  - Coverage bytes are copied from `node + 0x08` into the scratch row-byte buffer at `param_1 + 0xe8`.
  - Emits span-type markers (`2`, `0`, and tail sentinel) and merges contiguous spans when possible.
- ARE+`0xb7e0` raster painter vtable slot 39:
  - Lazy coverage path: calls `ARE_sampler_prepare` (`0x76dc`) and `ARE_sampler_eval_row` (`0x75d0`) when setup/row cache flags at `param_1 + 0x26c`/`0x260` require it.
  - Computes row coverage with 16-subrow logic (subrow scale uses shifts/adds by 0x10).
  - Tracks row full/partial status and writes integer coverage row bytes into a coverage plane offset (`param_1 + 0x48`).
- ARE+`0x75d0` lazy row/byte evaluator:
  - Iterates up to 0x80 iterations.
  - Uses subrow scale `<< 4` and accumulates coverage into `param_1 + 0x264`.
  - Calls edge sampler routine to toggle/integrate active edges (`FUN_180006920`) while walking subrows.
- ARE+`0x76dc` sampler prepare:
  - Initializes 16 buckets (loop count 0x80, advancing by 8 bytes per entry) before per-row coverage.
  - Preloads edge projection state into each bucket before evaluation.
- ARE+`0x78e4` edge projection:
  - Projects edge endpoints and clamps result into `[28]/[2c]` destination range.
  - Includes early-exit when projection factor equals zero sentinel and no active edge flag.
- Node fields used by row/edge path (from decompiled layouts):
  - `param_1 + 0x8c`/`0x94`: row range.
  - `param_1 + 0x150`: row-node vector.
  - Node `+0x00`: x-start.
  - Node `+0x04`: span width/len.
  - Node `+0x08`: row-bytes pointer.
  - Node `+0x10`: next row node.
  - Node `+0x18`: flag that becomes the following solid/empty span state.

## Native change in this iteration
- `crates/text-engine/src/rasterize.rs` now reports `coverage_backend` as
  `ttf_outline_are_scanline_16x_v1` in the `txt_drawchar_boundary/v1` trace.
- This is behind the `TXT_DrawChar` boundary and uses 16× supersample scanline policy in the emitted trace:
  - `policy: are_scanline_16x_contiguous_runs_with_integer_origin`
  - output span hashing/hex extraction remains aligned to those contiguous integer-span runs.
- `OUTLINE_COVERAGE_SUPERSAMPLE` remains `16`.

## Artifact outputs
- `target/ae_agents/p2_row_compare_covw_are_scanline_yedge_20260508/row_compare.json`
- `target/ae_agents/p2_row_compare_covw_are_scanline_yedge_20260508/edge_analysis.json`
- `target/ae_agents/p2_are_scanline_text_gate_20260508/report.json`

## Metrics
- `COV_W` normalized ink-shape common improved:
  - previous: `140/202` (`0.689655...`)
  - now: `179/203` (`0.881773...`)
- extents now: `109x68`
- byte mismatch persists:
  - AE first row: `2440..3c`
  - native first row: `1830..2f`
  - coverage sample mismatch ratio in this metric pack remains non-zero (first compared row already diverges at byte 0).
- text gate:
  - `ok=true`
  - `TXT_030` reported as slight drift but gate remains true.

## Current status
- P2 advanced to ARE-shaped coverage implemented and instrumented.
- Not parity locked yet.
- Next blocker: CoolType hint/grid-fit/outline producer tracing/state before ARE.

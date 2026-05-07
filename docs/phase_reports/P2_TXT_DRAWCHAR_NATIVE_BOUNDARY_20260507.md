# P2 TXT_DrawChar Native Boundary

Date: 2026-05-07

## Scope

This pass implements the native side of the recovered
`BEE_TextRenderNode -> TXT_DrawChar` boundary. The goal was not to invent a new
text formula, but to make native rasterization consume and emit the same
primitive contract that Frida/Ghidra exposed:

```text
glyph id
font identity
glyph/text matrices
fill/stroke flags and colors
clipped glyph bounds
PF_World / output alpha semantics
```

## Code Changes

`crates/text-engine/src/rasterize.rs`

- Added `TextRasterTrace` and `DrawCharPlan` with schema
  `txt_drawchar_boundary/v1`.
- `rasterize_text_with_layout` now takes the already-computed layout and builds
  one draw plan row per glyph row.
- The raster loop now gates drawing through the plan:
  whitespace, `glyph_id < 1`, disabled fill/stroke, and empty clipped bounds
  are explicit skip reasons.
- The current coverage backend remains
  `fontdue_rasterize_indexed_pending_cooltype_TXT_DrawChar`; this is now a
  named replaceable backend rather than hidden behavior.
- The canvas/output contract is explicitly recorded as straight RGBA8 with
  pending PF_World premult parity.

`crates/render-core/src/layer_eval.rs`

- Text layers now compute layout once, pass that layout into rasterization, and
  record the returned `draw_char` trace beside the existing layout telemetry.
- Collapsed text uses the same path, so collapse-related text rows also expose
  the boundary.

`crates/render-cli/src/conformance_pack.rs`

- Text passport snapshots now preserve `draw_char`.
- Diagnostics now report:
  - `txt_drawchar_boundary_rows`
  - `txt_drawchar_will_draw_rows`
  - `txt_drawchar_boundary_ready`
  - `cooltype_raster_tuning_ready`
  - `cooltype_raster_parity_locked=false`

## Evidence Check

Focused `TXT_010` run:

```text
target/ae_agents/p2_txt_drawchar_boundary_20260507_txt010_v2
```

Text passport diagnostics for frame 0:

```text
glyph_rows: 26
txt_drawchar_boundary_rows: 26
txt_drawchar_will_draw_rows: 23
txt_drawchar_boundary_ready: true
cooltype_raster_tuning_ready: true
cooltype_raster_parity_locked: false
```

This matches the important shape from the `RAS_010` Frida probe: not every text
grid row reaches `TXT_DrawChar`; non-renderable rows are filtered before fill.

## Conformance

Focused runs completed without regressions:

```text
cargo test -p text-engine
cargo test -p render-core
cargo test -p render-cli

python3 scripts/run_master_conformance_gate.py --case TXT_010 --out target/ae_agents/p2_txt_drawchar_boundary_20260507_txt010_v2 --no-fail
python3 scripts/run_master_conformance_gate.py --case TXT_020 --out target/ae_agents/p2_txt_drawchar_boundary_20260507_txt020 --no-fail
python3 scripts/run_master_conformance_gate.py --case TXT_030 --out target/ae_agents/p2_txt_drawchar_boundary_20260507_txt030 --no-fail
python3 scripts/run_master_conformance_gate.py --case TXT_040 --out target/ae_agents/p2_txt_drawchar_boundary_20260507_txt040 --no-fail
python3 scripts/run_master_conformance_gate.py --case GPH_010 --out target/ae_agents/p2_txt_drawchar_boundary_20260507_gph010 --no-fail
```

Representative primary visible means:

```text
TXT_010: 14.088211  status=tuning
TXT_020:  5.860491  status=approximate
TXT_030:  6.803962  status=approximate
TXT_040:  3.727372  status=approximate
GPH_010:  1.701600  status=approximate
```

The PNG metrics are expected to stay roughly unchanged because the remaining
coverage backend is still fontdue. The useful change is that final raster
parity now has a narrow implementation target rather than a vague text-engine
gap.

## Remaining P2 Work

P2 is now blocked by one concrete layer:

```text
replace fontdue indexed coverage/fill with CoolType-compatible TXT_DrawChar
coverage/fill semantics
```

That includes:

- antialias coverage rows;
- fill/stroke coverage merge rules;
- exact clipped bounds rounding;
- PF_World premult/output handling at text fill time;
- then a guarded switch for runtime selector geometry/collapse sharpness.

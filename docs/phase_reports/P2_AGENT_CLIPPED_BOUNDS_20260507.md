# P2-C TXT_DrawChar Clipped Bounds

Date: 2026-05-07

## Scope

Owned pass for clipped glyph bounds at `TXT_DrawChar` coverage/PF_World write
time. No shared native code or shared `GHIDRA_PREDECODE_TASKS.json` was edited.

## Static Inputs

Reports read:

- `docs/phase_reports/P2_BEE_TEXT_RASTER_FILL_PATH_20260507.md`
- `docs/phase_reports/P2_TXT_DRAWCHAR_NATIVE_BOUNDARY_20260507.md`
- `docs/phase_reports/P2_TXT_DRAWCHAR_ARE_PIXEL_WRITER_20260507.md`
- `docs/phase_reports/P2_SOURCE_RECT_SHAPING_PARITY_20260507.md`
- `docs/phase_reports/M05_M06_STATIC_GHIDRA_DOMAIN_CLOSURE_20260507.md`

Predecoded functions checked:

- `BEE.dll!FUN_1805c0c20`
  (`BEE_TextRenderNode_render_payload_d8_5c0c20`)
- `TXT.dll!TXT_GridChar::GetGlyphBoundsPlus` at `0x180054b00`
- `TXT.dll!TXT_GridChar::GetGlyphMetricsPlus`
- `TXT.dll!TXT_GridChar::GetCharacterAlignmentBounds`
- `TXT.dll!TXT_GridChar::GetRenderExtent`
- `CoolType.dll!CTTextGetBoundingBox` -> `CoreTextBoundingBox`
  -> `CoreTextFloatBoundingBox`
- `TXT.dll!TXT_DrawChar` at `0x1800413d0`
- `TXT.dll!TXTp_DrawChar3` at `0x180042110`
- `TXT.dll!FUN_180042b80` outline bridge
- ARE vtable/write path:
  - `TXT_ARE_OutlinePlayer_ctor_3e310`
  - `TXT_ARE_OutlinePlayer_vtable_06_3f310`
  - `TXT_ARE_Render_8bpc_3c360`
  - `TXT_ARE_OutputComposite_8bpc_3de50`
  - `TXT_ARE_PixelWriter8_span_3b8c0`

## Static Table

| Layer | Native function | Bounds/rounding behavior | P2-C conclusion |
| --- | --- | --- | --- |
| Layout/sourceRect integer bbox | `CoolType!CoreTextBoundingBox 0x1801239a4` | `ROUND(min_x)`, `ROUND(min_y)`, `ROUND(max_x + 0.99999988)`, `ROUND(max_y + 0.99999988)` | This is sourceRect/layout math only. Do not reuse it for final glyph coverage writes. |
| sourceRect float union | `CoreTextFloatBoundingBox 0x18012417c` | fixed16 bbox * `1/65536`, glyph position add, float min/max union | Metric bbox substrate, not pixel coverage clipping. |
| BEE draw eligibility clip | `BEE!0x1805c0c20` | `GetGlyphBoundsPlus`, `M_TransformFloatRect`, then double `vmaxsd/vminsd` against output rect. Empty if `right <= left` or `bottom <= top`. | Float early-out only. The clipped float rect is not passed to `TXT_DrawChar`. |
| Final ARE clip/write rect | `TXT_ARE_Render_8bpc_3c360`, `TXT_ARE_OutputComposite_8bpc_3de50` | consumes signed-short object fields: top `+0x60`, left `+0x62`, bottom `+0x64`, right `+0x66`; clamps to PF_World width/height; loops are half-open `[top,bottom)`, `[left,right)`. | Final write is integer span clipping, separate from sourceRect rounding. |
| Coverage lookup | `TXT_ARE_PixelWriter8_span_3b8c0` | for coverage path indexes `coverage[(y - top) * rowbytes + (x - left)]`; PF pixel address is `data + rowbytes*y + 4*x`. | Coverage origin must be a stable integer rect origin; do not per-pixel round float positions. |

## Probe Pack

Created:

```text
fixtures/ae_probe_pack/p2_text_clip_probe/
```

Cases:

```text
CLP_LEFT_N049
CLP_TOP_N049
CLP_RIGHT_P049
CLP_BOTTOM_P049
```

Each case places a single text glyph so its `sourceRectAtTime` edge crosses a
comp edge by `0.49` pixels. SourceRect is used only for placement; measurement
uses final rendered pixels.

## Remote Render

One remote render was submitted:

```text
python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/p2_text_clip_probe \
  --entry-script jsx/build_p2_text_clip_probe_project.jsx \
  --node http://85.239.48.31:8001 \
  --job-id p2_clip_20260507_231703 \
  --prefix ae_remote_packs \
  --output-dir target/ae_agents \
  --output-template "TIFF Sequence with Alpha" \
  --poll-interval-s 5 \
  --timeout-s 1200
```

Result:

```text
render_id=e87ed7fdad6a4e67a5157d794f8d2ad5
status=succeeded
tiff=4
local_converted_png=4
```

Artifacts:

```text
target/ae_agents/p2_clip_20260507_231703/outputs/p2_clip_20260507_231703_outputs.zip
target/ae_agents/p2_clip_20260507_231703/extracted/
target/ae_agents/p2_clip_20260507_231703/clip_alpha_bbox.json
```

## Dynamic Measurement

Alpha in this output was fully opaque for the comp (`96x96` in every case), so
alpha bbox is not usable from this run. The rendered text is white on black, so
RGB nonblack bbox was used from the same frames; no second render was submitted.

RGB nonblack bboxes:

| Case | bbox `[left, top, right_exclusive, bottom_exclusive]` | Notes |
| --- | ---: | --- |
| `CLP_LEFT_N049` | `[0, 0, 79, 48]` | first nonblack x clipped to `0` |
| `CLP_TOP_N049` | `[50, 0, 96, 50]` | first nonblack y clipped to `0` |
| `CLP_RIGHT_P049` | `[16, 0, 96, 48]` | last write reaches `x=95`; exclusive right is comp width |
| `CLP_BOTTOM_P049` | `[50, 46, 96, 96]` | last write reaches `y=95`; exclusive bottom is comp height |

This confirms the statically observed half-open PF_World clipping behavior at
the outer write boundary. It does not prove the internal BIB coverage-origin
rounding formula, and I did not fit one from the PNGs.

## Native Change Candidate

For the current native text backend, keep sourceRect/layout rounding separate
from final coverage writes:

1. Keep sourceRect metric integer policy as the existing CoolType/sourceRect
   contract: `round(min)` and `round(max + 0.99999988)`.
2. For `TXT_DrawChar` coverage/PF_World write, use a local integer half-open
   coverage rect:

```text
coverage_left   = floor(coverage_raster_x)
coverage_top    = floor(coverage_raster_y)
coverage_right  = coverage_left + coverage_width
coverage_bottom = coverage_top + coverage_height

clip_left   = max(coverage_left, 0)
clip_top    = max(coverage_top, 0)
clip_right  = min(coverage_right, canvas_width)
clip_bottom = min(coverage_bottom, canvas_height)

skip if clip_right <= clip_left or clip_bottom <= clip_top
write x in [clip_left, clip_right), y in [clip_top, clip_bottom)
coverage_index = (y - coverage_top) * coverage_width + (x - coverage_left)
```

3. Change the current per-sample write in `crates/text-engine/src/rasterize.rs`
   away from:

```text
px = round(raster_x + bx)
py = round(raster_y + by)
```

   and toward a single integer coverage origin plus half-open span loops, to
   match `TXT_ARE_OutputComposite_8bpc_3de50` /
   `TXT_ARE_PixelWriter8_span_3b8c0`.

This is a text-local change candidate. It does not require changing shared
canvas origin, matrix multiplication order, alpha-bounds crop, PF_World
rowbytes/depth, or non-text modules.

## Proposed Future Tracer

No tracer was run in this pass. If the orchestrator wants the remaining BIB
origin rounding locked dynamically, the narrow tracer should hook only:

- `TXT.dll+0x3c360` / `TXT_ARE_Render_8bpc`
- `TXT.dll+0x3de50` / `TXT_ARE_OutputComposite_8bpc`
- `TXT.dll+0x3b8c0` / `TXT_ARE_PixelWriter8_span`

and log object shorts `+0x60/+0x62/+0x64/+0x66`, PF width/height, and first span
arguments. That would avoid broad text/sourceRect hooks.


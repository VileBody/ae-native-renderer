# P2 BEE Text Raster Fill Path

Date: 2026-05-07

## Scope

This pass targets the remaining P2 text pixel layer after sourceRect/layout
parity. The question was: what runs after `TXT_PlayCharOutlines` and where does
AE actually turn a cached text grid glyph into pixels?

## Static Findings

`TXT_PlayCharOutlines` is not the final fill path. It is the bounds/outline
player used while building or validating the text grid cache.

The final text path is in `BEE.dll`:

```text
BEE_TextRenderNode ctor
  0x1805c0780
  calls BEE_SubLayerRenderNode ctor 0x1804bfba0
  then installs BEE_TextRenderNode vtable 0x180fb8ee8

BEE_SubLayerRenderNode::Render
  0x1804c2720
  drives sublayer iteration and compositing

BEE_SubLayerRenderNode vtable +0xf0
  0x1804c1f10
  dispatches per-sublayer payload through vtable +0xd8

BEE_TextRenderNode vtable +0xd8
  0x1805c0c20
  real text glyph fill worker
```

`0x1805c0c20` resolves the cached grid and draws one sublayer/glyph:

```text
grid = BEE_TextLayer::ExtractTextGridCache(text_layer, xform_cache)
grid_char = grid->chars + sublayer_index * 0x460

if !TXT_GridChar::IsRenderable(grid_char, false): return 0
glyph_id = TXT_GridChar::GetOffsetGlyphID(grid_char)
if glyph_id < 1: return 0

fill_enabled   = grid_char[0x1a0].alpha > 0 and render_order allows fill
stroke_enabled = grid_char[0x1b0].alpha > 0 and grid_char[0x100] > 0

char_matrix = grid_char[0x10..0x58]
if normal render:
  char_parent = TXT_GridChar::GetCharAndParentTransform(grid_char)
  char_matrix *= char_parent * text_matrix

bounds = TXT_GridChar::GetGlyphBoundsPlus(grid_char, false, char_matrix)
bounds = transform_and_clip(bounds, output_rect)

TXT_DrawChar(..., glyph_id, char_matrix, text_matrix,
             fill_enabled, stroke_enabled,
             fill_rgba, stroke_rgba, stroke_width,
             line_join, miter_limit, orientation,
             font_dict, vector_float_payload, PF_World)
```

The `TXT_DrawChar` target is reached through `BEE.dll` IAT slot
`0x180edf6b0`; at runtime it resolved to:

```text
TXT.dll + 0x413d0
```

## Dynamic Confirmation

Trace:

```text
target/dynamic_tools_85/bee_text_rendernode_RAS010_20260507/RAS_010.jsonl
```

AE output:

```text
target/ae_remote/ae_trace_cooltype_RAS_010_20260507_211959/extracted/.../RAS_010_00000.png
```

The render produced 6 TIFF frames and local PNG conversions. Hook enter counts:

```text
BEE_TextRenderNode_ctor_5c0780                  1
BEE_SubLayerRenderNode_ctor_4bfba0              1
BEE_SubLayerRenderNode_render_impl_4c2720       1
BEE_TextRenderNode_composite_mode_d0_5c0b00     1
BEE_TextRenderNode_sublayer_opacity_c0_5c0a80  46
BEE_TextRenderNode_vtable_c8_5c0930            23
BEE_SubLayerRenderNode_vtable_f0_4c1f10        20
BEE_TextRenderNode_render_payload_d8_5c0c20    20
BEE_IMPORT_TXT_DrawChar_edf6b0                 20
```

`BEE_TextRenderNode` reported `sublayer_count_0x27c = 23`, but only 20
`TXT_DrawChar` calls happened. That matches the static early exits for
non-renderable glyphs / glyph id `< 1` / disabled fill+stroke.

First live `TXT_DrawChar` sample:

```text
target: TXT.dll + 0x413d0
glyph_id: 331
font size matrix: [72, 0, 0, 0, 72, 0, 0, 0, 1]
text matrix: [1, 0, 0, 0, 1, 0, -30.739990234375, 52, 1]
fill: true
stroke: false
fill_rgba: [1, 1, 1, 1]
stroke_rgba: [0, 0, 0, 0]
stroke_width: 0
miter_limit: 2.5
orientation: 0
```

Unique glyph ids from the probe:

```text
1, 56, 72, 183, 189, 204, 254, 266, 279, 331
```

## Implementation Consequence

P2 sourceRect/layout is closed. The remaining pixel mismatch is no longer an
unknown high-level route:

```text
native text raster must model:
  TXT_DrawChar coverage for glyph id + font dict + matrices
  fill/stroke enable rules from TXT_GridChar fields
  glyph bounds clipping before draw
  PF_World premult/output semantics
```

Current native still uses fontdue indexed raster coverage. That is the next
replacement/augmentation point. Formula tuning should target this recovered
`TXT_DrawChar` boundary, not final PNG guessing.

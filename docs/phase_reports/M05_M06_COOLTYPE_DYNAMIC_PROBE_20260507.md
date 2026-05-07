# M05/M06 CoolType Dynamic Probe

Date: 2026-05-07

## Scope

This pass follows `M05_M06_STATIC_GHIDRA_DOMAIN_CLOSURE_20260507.md` with a
focused Frida loop on AE85. The goal was not final pixel tuning; it was to
close runtime facts for `TXT_010`, `TXT_020`, `TXT_040`, and `GPH_010`:

- which CoolType text/bbox APIs AE actually calls;
- where glyph bbox and width buffers are returned;
- whether sourceRect probes can run without full render-frame overhead;
- what remains unknown before implementing the next text layout formula pass.

## Added Tooling

New focused tracer:

```text
scripts/ae_trace_cooltype_text.py
```

It runs the normal S3/AE85 pack loop, starts a Frida tracer on the GUI node, and
now supports hook profiles:

```text
source-rect   CTText/CoreText bbox surfaces plus CoreBBoxBatch
font-metrics  CTFontInstanceGetWidths/GetBBoxes plus CoreWidthsBatch/CoreBBoxBatch
all           both sets, for short diagnostic runs only
```

New probe pack:

```text
fixtures/ae_probe_pack/cooltype_text_layout/
```

The pack creates only text/layout comps and calls `sourceRectAtTime`; it queues
no render items. This is important: the full conformance render under broad
CoolType hooks timed out, while this pack completed cleanly and still exercised
AE text layout.

## Runs

SourceRect-oriented run:

```text
target/dynamic_tools_85/cooltype_text_source_rect_probe_20260507
target/ae_remote/ae_trace_cooltype_batch_4_e8b498243518_20260507_175041
```

Font metrics run:

```text
target/dynamic_tools_85/cooltype_text_font_metrics_probe_20260507
target/ae_remote/ae_trace_cooltype_batch_4_e8b498243518_20260507_175325
```

Width candidate ABI rerun:

```text
target/dynamic_tools_85/cooltype_text_width_candidates_probe_20260507
target/ae_remote/ae_trace_cooltype_batch_4_e8b498243518_20260507_175657
```

All probe pack outputs report:

```text
render_queue_items = 0
sourceRect records = 82
```

## Runtime Findings

### Exported CTText bbox APIs did not fire

The hooks installed successfully, but the actual sourceRect probe path did not
hit these exported surfaces:

```text
CTTextGetBoundingBox
CTTextGetQuickBoundingBox
CTTextGetOrientedBBox
CoreTextBoundingBox
CoreTextFloatBoundingBox
CoreTextQuickBoundingBox
CTTextGetTextGlyphs
CTTextGetGlyphs_V2
```

Instead, sourceRect/layout went through `TXT.dll` and lower-level CoolType font
metric calls. The main sourceRect run captured:

```text
CoreBBoxBatch: 2152 hook events
top TXT.dll callers: 0x2142d5, 0x42f23, 0x447876, 0x210de1
```

Conclusion: the static `CTText*` bbox formula is still valuable, but AE's
scripting/render text path for these layers bypasses the exported CTText bbox
wrappers. The implementation target should be the same underlying formula:
glyph rows/positions plus font bboxes, not the wrapper name.

### CoreBBoxBatch is the bbox source

`CoreBBoxBatch` returns the glyph bbox values used by the text engine. For the
Montserrat `W` glyph (`glyph_id=331` in `Montserrat-BoldItalic.ttf`), the
runtime bbox candidate was:

```text
[5.741943, -40.599823, 70.759186, 0.0]
```

That matches the expected font bbox scaled to the active font size, and explains
the sourceRect top/height scale for `TXT_010`.

### CoreWidthsBatch out buffer is r9 / stack p5 - 4

The initial width dump read the wrong stack slot. The ABI rerun shows the
usable width scalar lives at `r9`, equivalent to `stack_p5_minus_4` in the
observed calls:

```text
glyph W   id=331  r9 fixed16=75169000  scaled=1146.987915
glyph O   id=204  r9 fixed16=55312000  scaled=843.994141
glyph R   id=254  r9 fixed16=48168000  scaled=734.985352
glyph D   id=56   r9 fixed16=54132000  scaled=825.988770
space     id=2263 r9 fixed16=18546     scaled=0.282990
```

These correspond to font advance/design units with fixed16 precision. Native
layout should not read the old `stack_p5` hypothesis.

### SourceRect metadata is stable

The probe metadata includes deterministic sourceRect records for the target
layers. Examples:

```text
TXT_010_word_reveal:
  left=-293.131995 top=-41.296001 width=592.701999 height=111.592007

TXT_020_character_reveal:
  left=-261.384001 top=-34.176001 width=523.535997 height=34.752001

TXT_040_bounce_selector:
  at t=0 sourceRect is 0x0 because expression selector scale starts collapsed
  at t=5/30 bbox becomes non-zero

GPH_010_child_text_layer:
  left=-131.111997 top=-34.176001 width=265.968003 height=34.752001
```

`TXT_040` confirms sourceRect is affected by the expression selector's current
animated transform state, so text layout and selector evaluation cannot be fully
separated for that case.

## Implementation Consequence

The next native text-layout pass should implement a CoolType-shaped metric
pipeline:

```text
1. Resolve exact installed font face and unitsPerEm.
2. Map Unicode/text runs to glyph ids and glyph-run order.
3. Use font advance/design units for glyph positions.
4. Use font bbox metrics scaled by active text transform/font size.
5. Union glyph bboxes for sourceRect.
6. Keep Range Selector/expression selector as a separate weight/transform layer,
   but allow sourceRect sampling at time to observe selector-transformed text.
```

Guardrail: do not tune final PNGs first. The row-level target is now clearer:
native should emit glyph id, advance, bbox, and sourceRect sidecars and compare
those to the AE probe records/dumps.

## Remaining White Spots

1. CTText glyph row positions were not captured directly because the exported
   `CTTextGetTextGlyphs` path did not fire.
2. The `TXT.dll` callers that build glyph rows are now known by offset, but need
   a deeper TXT.dll Frida/Ghidra pass if we want exact row/cluster mapping.
3. Widths are captured at the font-metric layer, but kerning, OpenType feature
   substitutions, and source cluster maps are still above that layer.
4. Pixel-perfect text raster coverage remains a separate CoolType outline/raster
   problem after metric/layout parity.

## Decision

Dynamic closure is good enough to start a native M05 metric-layout implementation
pass, gated by sidecar row diffs. It is not yet enough to claim full glyph-level
text animator or pixel raster parity.

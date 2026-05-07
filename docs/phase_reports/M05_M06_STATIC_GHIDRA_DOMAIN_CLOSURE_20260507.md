# M05/M06 Static Ghidra Domain Closure

Date: 2026-05-07

Dynamic follow-up: `docs/phase_reports/M05_M06_COOLTYPE_DYNAMIC_PROBE_20260507.md`.

## Scope

This pass closes as much of the text/layout domain as possible from static
Ghidra output before using Frida. It focuses on the current text blockers after
the Montserrat/Square Range Selector patch:

- `M05` text layout, glyph metrics, `sourceRectAtTime`-like bounds;
- `M06` reveal/range selector dependencies on glyph/unit order;
- `GPH_010` collapse text sharpness dependencies on glyph placement;
- dynamic-only gaps that should be probed next.

## Inputs

Existing bundles:

- `target/reverse/predecoded/20260505_153013_blocker_modules_round2/basic_text_aex`
- `target/reverse/predecoded/20260505_153013_blocker_modules_round2/cooltype_glyph_metrics`
- `target/reverse/predecoded/20260505_153013_blocker_modules_round2/cooltype_glyph_metrics_core`
- `docs/phase_reports/GHIDRA_COOLTYPE_GLYPH_METRICS_20260505.md`
- `docs/phase_reports/OS_COOLTYPE_TEXT_ANALYSIS_20260505.md`

New repeatable Ghidra tasks added:

- `cooltype_text_layout_surface`
- `cooltype_text_layout_core`

Fresh local outputs:

- `target/reverse/predecoded/20260507_171815_cooltype_text_layout_surface_static_20260507`
- `target/reverse/predecoded/20260507_172115_cooltype_text_layout_core_static_v2_20260507`

## Static Conclusions

### Basic_Text.aex

`Basic_Text.aex` is useful as a CoolType interface table/map, but it is not the
modern AE text layer animator implementation. It registers names such as
`CTFontInstanceGetWidths`, `CTFontInstanceGetBBoxes`, `CTTextGetGlyphsV2`,
`CTTextGetTextGlyphs`, and `CTGlyphAccessGetGlyphID`.

Conclusion: use `Basic_Text.aex` to find CoolType API surfaces. Do not expect it
to contain Range Selector or modern text animator formulas.

### Metric Scale And Public Glyph Rows

CoolType metric scale is locked by static evidence:

```text
DAT_180322b20 = 1 / 65536
DAT_18030d0d0 = float32(1 / 65536)
float_metric = fixed_i32 * (1 / 65536)
```

Public glyph metric records are 12 bytes:

```text
+0  u32 glyph_id
+4  f32/i32 x or horizontal advance slot
+8  f32/i32 y or vertical advance slot
```

Wrappers and core functions agree on this shape:

- `CTFontInstanceGetWidths` writes x/y advances into 12-byte rows.
- `CTFontInstanceGetBBoxes` exports four fixed16 bbox values per glyph.
- `CTFontInstanceGetBaselineDeltas` exports fixed16 baseline deltas.
- `CTTextGetNumGlyphs` derives count from `text + 0x50 / 0xc`.
- `CoreTextGlyphPointerBuild` copies CTText-owned pointer groups from offsets
  `0x40`, `0x58`, `0x70`, with a flags/value field at `0x88`.

Variable font support is real: `HMetric_lookup` can add HVAR deltas when a
variation/design vector is present. That means native layout must not assume
plain `hmtx` metrics are enough for every font instance.

### CTText Bounds Formula

The exact CTText bbox surface is now statically mapped:

```text
CTTextGetBoundingBox      0x18029fb60 -> CoreTextBoundingBox      0x1801239a4
CTTextGetQuickBoundingBox 0x1802a0c00 -> CoreTextQuickBoundingBox 0x1801261ac
CTTextGetOrientedBBox    0x1802a0850 -> CoreTextOrientedBBox     0x180124aac
CTTextGetOutlines        0x1802a0ab0
CTTextGetOutlinesV2      0x1802a0950
```

`CoreTextBoundingBox` itself is thin:

```text
float_bbox = CoreTextFloatBoundingBox(text, matrix)
left   = round(float_bbox.min_x)
top    = round(float_bbox.min_y)
right  = round(float_bbox.max_x + 0.99999988)
bottom = round(float_bbox.max_y + 0.99999988)
```

`CoreTextFloatBoundingBox` computes the real bounds:

```text
1. Compose text matrix:
   combined = text_object_matrix * caller_matrix

2. Copy CTText 12-byte glyph rows from text + 0x48.

3. Transform each glyph row position through the caller matrix:
   x' = y * m2 + x * m0 + m4
   y' = x * m1 + y * m3 + m5

4. Call CoreBBoxBatch for glyph bboxes.

5. Convert each bbox fixed16 value by 1 / 65536.

6. For every non-empty glyph bbox:
   glyph_min = glyph_position + bbox_min
   glyph_max = glyph_position + bbox_max
   text_bbox = min/max union over glyphs.
```

This is a strong implementation candidate for native `sourceRect`/layout
telemetry. It is not a pixel raster coverage formula; it is vector/glyph metric
bounds.

### Quick BBox Is A Different Approximation

`CoreTextQuickBoundingBox` can avoid per-glyph exact bbox in some cases. It uses
font-level bbox data, glyph positions, text flags, and expansion constants such
as `0.5`, `400`, and `1000`. It is likely a fast conservative bound, not the
final sourceRect formula.

Dynamic question: AE scripting `sourceRectAtTime` may call exact bbox, quick
bbox, oriented bbox, or a wrapper above them depending on layer state. Static
analysis narrowed this to a small hook set.

### Glyph Rows Versus Characters

Static code confirms the important modeling rule:

```text
character index != glyph id != glyph run index
```

OpenType feature processing and component splitting can mutate glyph ids,
counts, and order before metrics are computed. `CTTextGetTextGlyphs` also emits
48-byte output rows with raster origins and payload pointers after layout.

Native selector/glyph telemetry should therefore key off glyph-run rows and
source cluster mapping, not raw Unicode character index.

## What We Can Implement Without More Probes

1. Add a native CoolType-shaped text bounds path:
   - use glyph ids, positions, and bboxes;
   - convert fixed metrics with `1 / 65536`;
   - union non-empty glyph boxes;
   - apply the `round(min)` / `round(max + 0.99999988)` integer bbox policy.

2. Add/strengthen sidecar telemetry:
   - `ct_glyph_row_index`;
   - `ct_glyph_id`;
   - `ct_position_x/y`;
   - `ct_bbox_fixed`;
   - `ct_bbox_float`;
   - `ct_text_bbox_float`;
   - `ct_text_bbox_int`;
   - `ct_matrix`;
   - `bbox_api_candidate`.

3. Stop tuning `TXT_010` final pixels from fontdue bboxes alone. The native
metric substrate should first emit CoolType-shaped rows and compare those rows
to AE.

## White Spots For Dynamic Closure

These should be closed with Frida, not metric fitting:

1. Which bbox API AE actually calls for our `sourceRectAtTime` cases:
   - `CTTextGetBoundingBox`;
   - `CTTextGetQuickBoundingBox`;
   - `CTTextGetOrientedBBox`;
   - direct call into `CoreTextFloatBoundingBox`.

2. Runtime font instance state:
   - writing/orientation selector;
   - CTText flags at `text + 0x38`;
   - font matrix at `text + 0x18`;
   - HVAR/design vector state for static and variable fonts;
   - exact resolved `Point-Light` and `Montserrat-BoldItalic` instance.

3. CTText row contents for the fixture strings:
   - source UTF-16 or encoded input;
   - pre-feature glyph ids;
   - post-feature glyph rows;
   - source cluster/range mapping;
   - whitespace handling.

4. Range Selector / Text Animator formulas:
   - not found in the CoolType static surface;
   - likely lives in AE text layer engine code outside `Basic_Text.aex`;
   - dynamic tracing should correlate selector units with CTText glyph rows.

5. Pixel text parity:
   - outlines/raster image functions are mapped;
   - antialiasing, hinting, subpixel coverage, and raster buffer format remain
     dynamic/static-deeper work.

## Next Dynamic Hook Set

Start with a small text probe batch, not template-sized jobs:

```text
CTTextGetBoundingBox        CoolType.dll+0x29fb60
CTTextGetQuickBoundingBox   CoolType.dll+0x2a0c00
CTTextGetOrientedBBox       CoolType.dll+0x2a0850
CoreTextFloatBoundingBox    CoolType.dll+0x12417c
CTTextGetTextGlyphs         CoolType.dll+0x2a1380
CTTextGetGlyphs_V2          CoolType.dll+0x2a02a0
CTFontInstanceGetWidths     CoolType.dll+0x292d20
CTFontInstanceGetBBoxes     CoolType.dll+0x291900
```

For `TXT_010`, `TXT_020`, `TXT_040`, and `GPH_010`, dump:

- text handle;
- matrix;
- flags;
- glyph count;
- first N 12-byte CTText rows;
- returned float/int bbox;
- widths/bboxes for the same glyph ids;
- frame/time and source string id.

Once those dumps match native sidecars at row/bbox level, formula tuning can
move from final PNGs to deterministic row diffs.

## Orchestrator Decision

Proceed to implementation on the metric/layout slice, but keep raster and text
animator formulas gated by Frida:

```text
static closure achieved for CoolType metric bounds
dynamic closure required for bbox API selection, runtime flags, glyph row dumps,
and Range Selector engine location
```

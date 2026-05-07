# P2-S3 BIB/CoolType Coverage Generator Static Deep Dive

Date: 2026-05-08

Scope: static-first chain that generates/feeds coverage rows before
`TXT_ARE_PixelWriter8_span_3b8c0`. No shared tracer/native/docs edits.

Read inputs:

- `docs/phase_reports/P2_AGENT_COVERAGE_ROWS_20260507.md`
- `docs/phase_reports/P2_AGENT_HINTING_AA_SUBPIXEL_20260507.md`
- `docs/phase_reports/P2_TXT_ARE_SPAN_TRACE_20260507.md`
- `docs/phase_reports/P2_AGENT_STROKE_MERGE_20260507.md`
- `target/reverse/predecoded/20260507_222041_txt_drawchar_core`
- `target/reverse/predecoded/20260507_223624_txt_drawchar_are_vtable`
- `target/reverse/predecoded/20260507_223741_txt_drawchar_are_helpers`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers`
- `target/reverse/predecoded/20260507_224844_txt_drawchar_are_pixel_writers`
- `target/reverse/predecoded/20260507_171815_cooltype_text_layout_surface_static_20260507`
- `target/reverse/predecoded/20260507_172115_cooltype_text_layout_core_static_v2_20260507`
- local `target/reverse/ae_2026/text_expression/{TXT.dll,CoolType.dll}`

No AE dynamic run was performed in this pass.

## Executive Result

The last TXT-owned code before pixels does not generate coverage. It consumes a
coverage object produced by dynamically resolved BIB/ARE procedures:

- fill outline path: `TXT.dll+0x40580` calls `DAT_18087f778`;
- stroke path: `TXT.dll+0x3d960` calls `DAT_18087f780`;
- both wrap the returned BIB handle with `TXT.dll+0x3fd40` and call the first
  method in the returned method block to create the coverage object;
- `TXT.dll+0x3de50` walks rows by `coverage_obj+0x10` row getter and sends span
  cells to `TXT.dll+0x3b8c0`;
- shared trace already resolved the live row getter to `ARE.dll+0x8230`.

Therefore the real row/type/byte generation is below the TXT bridge, in the
BIB/ARE object behind `DAT_18087f778`, `DAT_18087f780`, the method-block
coverage constructor, and row getter `ARE.dll+0x8230`. Local static corpus has
`TXT.dll` and `CoolType.dll`, but not `ARE.dll`/`BIB.dll`, so the deepest exact
generator body is not currently decompilable from local files.

## Function / Offset Map

| Stage | Offset | Static role |
| --- | ---: | --- |
| BEE payload | `BEE.dll+0x5c0c20` | Builds glyph/text matrices and calls `TXT_DrawChar`; from prior context. |
| TXT entry | `TXT.dll+0x413d0` | Chooses outline ARE path for outline fonts. |
| ARE drawchar | `TXT.dll+0x42110` | Builds `TXT_DrawOutlinePlayerARE`, then plays glyph outlines. |
| Outline playback core | `TXT.dll+0x42b80` | Reads CoolType/TXT outline records, transforms points, emits vtable path ops. |
| move/line/curve/close | `TXT.dll+0x404e0`, `0x40430`, `0x3f050`, `0x3ef60` | Append f32 coords and int command ids into ARE path buffers. |
| render dispatch | `TXT.dll+0x3f310` | Dispatches by PF depth. 8 bpc goes to `0x3c360`. |
| 8 bpc pass order | `TXT.dll+0x3c360` | Converts colors, orders fill/stroke by ARE byte `+0x0a`. |
| fill pass | `TXT.dll+0x3d200` | Builds fill path through `0x40580`, converts BIB path to coverage object, calls `0x3de50`. |
| stroke pass | `TXT.dll+0x3d960` | Calls `DAT_18087f780` with stroke width/join/miter/matrix, converts result to coverage object, calls `0x3de50`. |
| fill path builder | `TXT.dll+0x40580` | Calls `DAT_18087f778(&path, count, coords, commands, flags/context/matrix/clip...)`. |
| BIB cache/factory | `TXT.dll+0x3fd40` | Maps BIB handle through `DAT_18087bd40 + 8 + handle`, caches method block. |
| BIB refresh | `TXT.dll+0x408d0` | Calls `DAT_18087beb8`, `DAT_18087bec0`, `DAT_18087bec8`, then callback `DAT_180841f88(handle,1,obj+0x38)`. |
| row iterator | `TXT.dll+0x3de50` | Calls coverage row getter, consumes 8-byte span cells. |
| 8 bpc writer | `TXT.dll+0x3b8c0` | Type `1` full spans; type `2` per-pixel byte coverage. |
| row getter | `ARE.dll+0x8230` | Live resolved by shared trace; not present in local static corpus. |

CoolType surface note: `CTTextGetOutlines` / `CTTextGetOutlinesV2`
(`CoolType.dll+0x2a0ab0` / `0x2a0950`) wrap core outline functions
(`0x124e0c` / `0x1257d4`). `CTTextGetTextImage` at `CoolType.dll+0x2a14b0`
decompiles to `return 0;` in this corpus, so it is not the final AE text
coverage generator on the recovered path.

## Coverage Object ABI

The stack tuple passed to `0x3de50` is:

```c
struct TxtCoverageTuple {
    uint64_t path_handle;       // +0x00, BIB handle retained/released by TXT
    void *method_block;         // +0x08, returned by 0x3fd40
    CoverageObject *coverage;   // +0x10, filled by (*method_block)(...)
};
```

`CoverageObject` fields used by TXT:

```c
struct CoverageObject {
    uint64_t unknown_00;        // +0x00
    void *plane;                // +0x08, byte coverage plane/view
    SpanCell *(*row_getter)(void *ctx, int y);  // +0x10
    void *row_context;          // +0x18
};
```

Shared trace on `COV_W` observed:

```text
coverage_obj+0x08 plane/context ptr      = 0x296e493a428
coverage_obj+0x10 row_getter             = ARE.dll+0x8230
coverage_obj+0x18 row_context            = 0x296e493a350
```

Observed plane/header fields from shared trace:

```text
plane+0x08 s32 width     = 109
plane+0x0c s32 height    = 68
plane+0x10 byte* base    = 0x296e6beb520
plane+0x28 byte* mirror  = 0x296e6beb520
plane+0x30 s64 stride    = 112
plane+0x38 aux pointer   = 0x296ea74cb90
```

Static caveat: `TXT.dll+0x3b8c0` disassembly computes the type-2 byte pointer
with `qword ptr [RDX + 0x20]` after loading `RDX = *(coverage_obj+0x08)`.
The live shared summary found `plane+0x20 == 0` and `plane+0x30 == 112`. This
means either the shared hook's `plane` label is an enclosing/header view rather
than the exact pointer in `RDX`, or there is another small adapter offset still
unresolved. The next hook should sample `RDX` at `TXT.dll+0x3ba1b` and dump
`RDX+0x10/+0x20/+0x28/+0x30` exactly at the CPU instruction site.

## Row / Span ABI

`0x3de50` row walk pseudocode:

```c
for (int y = *(int16_t *)(are + 0x60); y < *(int16_t *)(are + 0x64); y++) {
    if ((y & 1) == *(uint32_t *)(are + 0x1c)) {
        continue; // field/parity skip; value 2 disables skip
    }

    SpanCell *row = coverage->row_getter(coverage->row_context, y);
    int x = *(int16_t *)(are + 0x62);
    int right = *(int16_t *)(are + 0x66);

    while (x < right) {
        int type = row->type;
        int end = row->end_x;
        int clipped_end = min(end, right);
        if (x < end && x < clipped_end) {
            TXT_ARE_PixelWriter8_span_3b8c0(
                are, tuple, type, y, x, clipped_end, src_color, dst_world);
        }
        x = clipped_end;
        row++;
    }
}

struct SpanCell {
    int32_t type;
    int32_t end_x; // exclusive; cells are consumed from current cursor
};
```

Known span types:

- `type == 0` or any unknown value: transparent/no-op in `0x3b8c0`.
- `type == 1`: full coverage span.
- `type == 2`: byte coverage span; one grayscale byte per output pixel.

`0x3b8c0` type-1 pseudocode:

```c
dst = world->base + y * world->rowbytes + x0 * 4;
if (src.a == 255) {
    fill dst[x0..x1) with src pixel dword;
} else {
    for x in x0..x1:
        blend8_full(src, dst);
}
```

`0x3b8c0` type-2 static pseudocode:

```c
plane = coverage->plane;
cov = plane->base_0x10
    + (y - *(int16_t *)(are + 0x60)) * plane->stride_static_0x20
    + (x0 - *(int16_t *)(are + 0x62));

dst = world->base + y * world->rowbytes + x0 * 4;
for x in x0..x1:
    blend8_coverage(src, 0, *cov++, dst);
    dst += 4;
```

For 16 bpc and f32, `0x3dd60` / `0x3df40` reuse the same row getter and
`SpanCell` ABI. Only the pixel writer/blend callback differs. 16 bpc converts
the 8-bit coverage byte to 15-bit-ish coverage with:

```c
coverage16 = (coverage8 * 0x808080 + 0x81b0) >> 16;
```

f32 converts with:

```c
coverage_float = coverage8 * (1.0f / 255.0f);
```

## Where Type-1 / Type-2 Spans Are Generated

TXT never assigns type-1/type-2 cells. It only consumes them. Generation is
below the BIB/ARE boundary:

1. `0x40580` or `0x3d960` creates a BIB path/raster handle through
   `DAT_18087f778` / `DAT_18087f780`.
2. `0x3fd40` maps that handle to a method block using BIB tables.
3. `(*method_block)(&coverage_obj, handle)` constructs the coverage object.
4. `coverage_obj->row_getter(row_context, y)` returns the per-y cells.
5. For type-2 cells, `coverage_obj->plane` supplies the actual byte row data.

Static evidence says the generator is not in `CoolType.dll`'s public
`CTTextGetTextImage`; that export stub returns zero. The recovered path gets
CoolType/TXT outline data, then delegates actual scan conversion to BIB/ARE.

## Visible Hinting / Grid-Fit / AA Inputs

Parameters visible before row generation:

- ARE matrix at `are+0x68`, six f64 matrix components converted to f32 by
  `0x3e310`; forwarded to fill/stroke generation.
- ARE clip/source span bounds:
  - `are+0x60` top
  - `are+0x62` left
  - `are+0x64` bottom
  - `are+0x66` right
- Field/parity selector:
  - constructor stores input int at `are+0x18`;
  - `are+0x1c = 1` for value `1`, `0` for value `2`, else `2`;
  - row iterator skips rows when `(y & 1) == are+0x1c`.
- Render/context byte `are+0x0b`; forwarded to both fill/stroke generator calls.
- Fill helper feature flag:
  - `0x40290` checks `AE_3DTextExtrusionV2`;
  - if enabled and `are+0x3d0 == 1`, `0x3d200` passes an extra flag into
    `0x40580`.
- Stroke-only inputs in `0x3d960`:
  - stroke width `are+0x40`, converted to float;
  - line join `are+0x48`;
  - miter limit `are+0x50`, converted to float;
  - same matrix/context/clip block.

Not visible in TXT static layer:

- no explicit LCD/subpixel RGB mask mode;
- no direct anti-alias enum consumed by `0x3de50`/`0x3b8c0`;
- no CoolType hinting policy field decoded before row generation.

The strongest current conclusion remains: final TXT output consumes scalar
grayscale coverage bytes. Any hinting/grid-fit/subpixel positioning happens
inside BIB/ARE scan conversion or earlier outline positioning, not in the final
PF pixel writer.

## Minimal Native Row Model

Implementable before full CoolType rasterizer:

```text
NativeCoverageObject
  top,left,bottom,right      from glyph integer bbox/clip
  BytePlane
    width = right-left
    height = bottom-top
    stride = align_up(width, 4) initially; trace saw 109 -> 112
    bytes[y * stride + x] = scalar coverage 0..255
  rows[y] = monotonic SpanCell list covering [left,right)
```

Row construction:

1. Produce a scalar byte coverage plane from the current TTF outline backend.
2. For each row, emit cells from `left` to `right`:
   - transparent run: `{type:0, end_x}`;
   - all-255 run: `{type:1, end_x}`;
   - mixed/edge run: `{type:2, end_x}` and read bytes from the plane.
3. Guarantee monotonic `end_x` and enough cells to reach clip right; no sentinel
   is required by TXT if the cells cover the row.
4. Composite with the already recovered AE 8 bpc source-over formulas.

This model gives the native side the correct TXT row/span ABI without claiming
CoolType-perfect hinting. It also avoids LCD/RGB masks and treats fractional
placement as geometry/coverage generation, matching current evidence.

## Unknowns

- Body of `ARE.dll+0x8230` row getter.
- Bodies and semantic names of `DAT_18087f778` and `DAT_18087f780`.
- Exact method-block callback at `DAT_180841f88` and coverage constructor ABI.
- Reconciliation of static `plane+0x20` load in `0x3b8c0` with live summary
  that identified stride at `plane+0x30`.
- Exact hinting/grid-fit policy and whether it is font technology or size
  dependent.
- Whether stroke coverage uses identical row-cell semantics for every join and
  miter case; TXT consumer says yes, generator details remain unknown.

## Next Static/Dynamic Hooks

No broad AE run is needed yet. If dynamic is approved later, use only these
small probes:

1. At `TXT.dll+0x3ba1b` in `0x3b8c0`, dump `RDX` and qwords at
   `RDX+0x10/+0x20/+0x28/+0x30` for the first type-2 span. This resolves the
   stride offset conflict directly at the CPU load site.
2. At `TXT.dll+0x3de50` after the row getter call, dump the returned row pointer
   and first 16 `{type,end_x}` cells for a single glyph row.
3. Resolve and hook `coverage_obj+0x10` target (`ARE.dll+0x8230`) entry/return:
   arguments `{row_context,y}` and returned row pointer.
4. Resolve `DAT_18087f778` and `DAT_18087f780` after TXT initialization and log
   module/offset plus the returned handle. If modules are copied for static
   analysis, predecode those exact offsets in `ARE.dll`/`BIB.dll`.
5. Dump the method block returned by `0x3fd40` for the live handle: at least
   entries `+0x00..+0x30`, then predecode the constructor at entry `+0x00`.

Static-only next step: acquire `ARE.dll` and `BIB.dll` matching the trace build,
then predecode `ARE.dll+0x8230` and the resolved `DAT_18087f778/f780` targets.

# P2 TTF Outline Text Coverage

Date: 2026-05-07

## Scope

This pass replaces the hidden `fontdue.rasterize_indexed` text paint path with
an explicit coverage backend behind the recovered `TXT_DrawChar` boundary.

The implemented backend is evidence-shaped, not pixel-parity locked:

```text
input:  TXT_DrawChar boundary row
font:   resolved TTF face / glyph id
shape:  TTF outline contours
fill:   nonzero winding rule
AA:     fixed 4x supersample coverage
origin: glyph bbox + baseline from the native draw plan
cache:  font path + glyph id + font size + supersample
```

`fontdue` remains only as fallback for fonts that cannot expose outlines.

## Code Changes

`crates/text-engine/src/rasterize.rs`

- Added a `ttf_outline_nonzero_supersample_v1` coverage backend.
- Added a simple `ttf-parser::OutlineBuilder` flattener for line, quadratic,
  and cubic contours.
- Raster fill uses nonzero winding over supersampled pixel centers.
- Coverage masks are cached by:

```text
font_key
glyph_id
font_size_bits
supersample
```

- `DrawCharPlan` now records per-glyph coverage metadata:

```text
coverage_backend
coverage_supersample
coverage_origin_source
coverage_nonzero_pixels
```

## Gate Results

Master gate:

```text
target/ae_agents/p2_outline_coverage_cached_master_20260507
```

Summary:

```text
case_count=19
accepted=2
tuning=1
approximate=16
regression=0
missing=0
```

Text/collapse movement versus the previous `TXT_DrawChar` boundary pass:

```text
TXT_010  14.088211 -> 14.070786
TXT_020   5.860491 ->  5.858233
TXT_030   6.803962 ->  6.502396
TXT_040   3.727372 ->  3.730953
GPH_010   1.701600 ->  1.687255
```

`TXT_040` moved by a tiny amount in the wrong direction but stayed inside the
same approximate gate bucket. The useful movement is `TXT_030`, where glyph
animator text coverage improved more clearly.

Cached elapsed samples from the master run:

```text
TXT_010  2903 ms
TXT_020  3587 ms
TXT_030  3422 ms
TXT_040  3224 ms
GPH_010  3024 ms
```

The pre-cache candidate pass was roughly `5-8s` per focused text case, so the
mask cache is necessary and effective.

## Remaining P2 Work

This closes the coarse native coverage backend replacement. It does not yet lock
CoolType pixel parity.

Remaining exactness questions:

- CoolType hinting/grid-fit behavior;
- exact antialias kernel/subpixel policy;
- exact clipped bounds rounding around `TXT_DrawChar`;
- stroke/fill merge behavior;
- literal CoolType/BIB coverage rows;
- semi-transparent fill temp-world/PF_TransferRect behavior.

Follow-up `P2_TXT_DRAWCHAR_ARE_PIXEL_WRITER_20260507` recovered and implemented
the 8 bpc ARE `PF_Pixel8` writer. Next useful probe is therefore below the
writer: trace or dump the BIB/CoolType coverage rows before final composition.

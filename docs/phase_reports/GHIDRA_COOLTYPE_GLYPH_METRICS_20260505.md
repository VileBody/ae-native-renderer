# Ghidra CoolType Glyph Metrics Targets

Date: 2026-05-05

## Outputs

Raw Ghidra output stays ignored under `target/`:

- finder: `target/reverse/predecoded/20260505_004951_cooltype_target_finder/cooltype_glyph_finder`
- selected wrappers: `target/reverse/predecoded/20260505_005411_cooltype_glyph_metrics_intake/cooltype_glyph_metrics`
- selected core targets: `target/reverse/predecoded/20260505_005459_cooltype_glyph_metrics_core_expanded/cooltype_glyph_metrics_core`

Durable task definitions were added to
`docs/reverse_engineering/GHIDRA_PREDECODE_TASKS.json`:

- `cooltype_glyph_metrics`
- `cooltype_glyph_metrics_core`

## What Was Recovered

`Basic_Text.aex` had already shown the external contract:
`CTFontInstanceInterfaceV2`, `CTTextGetGlyphsV2`, `CTGlyphAccessInterface`,
`GetGlyphID`, `GetWidths`, `GetBBox`, `GetBBoxes`, and
`GetBaselineDeltas`. This pass resolves that contract to concrete
`CoolType.dll` function targets.

| Area | Wrapper / table target | Concrete / core targets | Notes |
| --- | --- | --- | --- |
| Interface table | `CTFontInstanceInterfaceV2_register` `0x180290bc0` | slot table maps names to proc bodies | Main target for AE text metrics used by `Basic_Text.aex`. |
| Glyph id, single | `CTFontInstanceGetGlyphID_V2` `0x1802920b0` | `CoreUnicodeToGlyph` `0x1801594a0` | Handles encoding, code page, and Unicode-to-glyph translation. |
| Glyph ids, batch | `CTFontInstanceGetGlyphIDs_V2` `0x180292260` | `CoreGlyphIDsBatch` `0x1800f6948` | Batch path fills glyph rows from encoded text input. |
| Glyph count | `CTFontInstanceGetNumGlyphIDs_V2` `0x180292640` | wrapper-level target selected | Count path for buffer sizing and batch output validation. |
| Width, single | `CTFontInstanceGetWidth` `0x180292900` | `CoreWidthsBatch` `0x18015ac10`, `CoreTransformedWidths` `0x1800f6c74` | Chooses horizontal vs vertical component from writing direction. |
| Widths, batch | `CTFontInstanceGetWidths` `0x180292d20` | `CoreWidthsBatch` `0x18015ac10`, `CoreTransformedWidths` `0x1800f6c74` | Writes 12-byte glyph records and converts fixed metrics to floats. |
| BBox, single | `CTFontInstanceGetBBox` `0x1802915e0` | `CoreBBoxBatch` `0x18015a870`, `CoreGlyphMetricSingle` `0x1801727e0` | BBox output is four fixed-point values converted to float. |
| BBoxes, batch | `CTFontInstanceGetBBoxes` `0x180291900` | `CoreBBoxBatch` `0x18015a870` | Batch path allocates packed glyph records then copies bboxes out. |
| Baselines | `CTFontInstanceGetBaselineDeltas` `0x180291cd0` | `CoreBaselineDeltas` `0x18015a62c` | Baseline indices are bounded to `< 8`; writing direction changes output axis. |
| Feature processing | `CTFontInstanceProcessFeatures` `0x180294420`, `CTFontInstanceProcessFeatures_V2` `0x1802941a0` | `CoreProcessFeatures` `0x18015c5f8`, `CoreApplyFeaturesRun` `0x18015d108` | OpenType/CID component splitting and feature run application live here. |
| Text glyph pointers | `CTTextGetGlyphs_V2` `0x1802a02a0` | `CoreTextGlyphPointerBuild` `0x18029bd98` | Returns glyph pointer groups for rendered text. |
| Text glyph rows | `CTTextGetTextGlyphs` `0x1802a1380` | `CoreTextGlyphRowExtract` `0x180127238` | Extracts text glyph data from the internal text object. |
| Glyph access | `CTGlyphAccessInterface_register` `0x180298548` | `CTGlyphAccessGetGlyphID` `0x180298be0` | Lower-level glyph access interface for feature/translator paths. |
| Raw font metrics | `HMetric_lookup` `0x1800e8c90`, `VMetric_lookup` `0x1800e8d90` | same | hmtx/vmtx lookup error paths selected. |
| Metric dictionary | `CTFontDictMetricsKeyMap` `0x1800f7c50` | same | Owns `ct_numglyphs`, `ct_fontbbox`, `ct_baselines`, `ct_GSUBTable`, `ct_GPOSTable`, `ct_BASETable`. |

## Important Constants

`DAT_180322b20` is the fixed-to-float metric scale:

```text
0x3ef0000000000000 as f64 = 1.52587890625e-05 = 1 / 65536
```

Observed wrappers multiply integer glyph metrics by this constant before
returning floats to callers. This is the first concrete clue for native text
metric normalization: CoolType keeps many glyph metrics as 16.16 fixed values,
then exports AE-facing widths/bboxes/baseline deltas as float user units.

## Native Implication

`M05` should no longer be blocked on "which CoolType functions matter". The
selected target set covers:

- glyph id mapping;
- glyph-run / batch glyph id output;
- width and bbox extraction;
- baseline delta extraction;
- OpenType feature processing entry points;
- text glyph pointer extraction;
- hmtx/vmtx raw metric lookup;
- the fixed-point conversion constant.

This does not mean formula parity is locked. It means agents can now analyze
actual CoolType bodies instead of guessing from `Basic_Text.aex` interface
names.

## Next Use

For text/glyph parity agents:

1. Start from `cooltype_glyph_metrics/index.md` for the public proc surface.
2. Follow into `cooltype_glyph_metrics_core/index.md` for the math/data bodies.
3. Treat `DAT_180322b20 = 1/65536` as the metric scale until disproven by AE
   probes.
4. Do not tune text animator or collapse sharpness from final pixels until
   native telemetry includes glyph id, glyph run order, advance, bbox, baseline
   delta, selector weight, final glyph matrix, opacity, and blur radius.

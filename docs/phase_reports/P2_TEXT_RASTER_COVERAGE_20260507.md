# P2 Text Raster Coverage Pass

Date: 2026-05-07

## Scope

This pass starts the layer after sourceRect/layout parity: render-time text
raster coverage/fill. SourceRect passports are already exact for `TXT_010`,
`TXT_020`, and `TXT_040`; this pass intentionally targets the actual pixel
path.

## What Changed

- Added a `text-raster` / `text-raster-render-only` Frida profile in
  `scripts/ae_trace_cooltype_text.py`.
- Added attach-delay support so the tracer can attach after the AE JSX build
  phase and closer to render queue execution.
- Added render-time hooks for:
  - `TXT_PlayCharOutlines`
  - `TXT_PlayCharOutlines_impl`
  - `CoreTextOutlines`
  - `CoreTextOutlinesV2`
  - `CoreTextGlyphRecordRender`
  - `CoreTextGlyphRunSlice`
  - `CoreTextEmitGlyphRecord`
- Added `fixtures/ae_probe_pack/cooltype_text_raster`, a minimal single-case
  `RAS_010` probe pack for text raster tracing without the full conformance
  build noise.
- Native text raster now calls `font.rasterize_indexed(glyph_id, size)` from
  the recovered layout glyph id instead of `font.rasterize(char, size)`.

## Dynamic Findings

The full `TXT_010` conformance trace and the minimal `RAS_010` probe both hit
the outline path:

```text
TXT_PlayCharOutlines
TXT_PlayCharOutlines_impl
CoreTextOutlines
```

For the minimal `RAS_010` render probe, the first glyph id was `331`, matching
native Montserrat `W` glyph id telemetry. That confirms the recovered glyph ids
are the right input to the native raster API.

The outline player vtable captured from `TXT_PlayCharOutlines` points to
`TXT_GetBoundsOutlinePlayer`-style functions, not the final pixel fill path.
Those targets were added to `GHIDRA_PREDECODE_TASKS.json` and predecoded under:

```text
target/reverse/predecoded/20260507_204213_txt_outline_player_vtable_render
```

Late attach during the actual render window captured no
`TXT_PlayCharOutlines`/`CoreTextOutlines` calls, which implies AE has already
materialized/cached text coverage before final render sampling or uses a
separate BEE/cache path for bitmap fill.

## Native Gate

Focused gate:

```text
target/ae_agents/p2_text_raster_glyphid_gate_20260507
```

Result:

```text
ok=true
TXT_010 passport ok, max delta < 0.00008
TXT_020 passport ok, max delta < 0.00007
TXT_040 passport ok
GPH_010 no sourceRect passport reference
```

Visible PNG metrics were unchanged by the `char -> glyph_id` switch for the
current fixtures because the relevant template text does not require ligature
substitution. The change still removes the wrong API boundary and makes the
native raster path consume the same glyph ids that AE/TXT emits.

## Conclusion

P2 sourceRect/layout remains closed. Raster coverage/fill is now instrumented
and partially implemented at the glyph-id API boundary, but not parity locked.
The next unknown is not glyph id or layout; it is the BEE/TXT cached coverage
handoff after outline bounds. The next reverse pass should trace/predecode that
cache/fill path rather than tuning fontdue coverage by final PNG metrics.

## Follow-Up: BEE Fill Path Found

The follow-up pass found and live-confirmed the cached fill path:

```text
BEE_TextRenderNode::ctor              0x1805c0780
BEE_SubLayerRenderNode::Render        0x1804c2720
BEE_TextRenderNode vtable +0xd8       0x1805c0c20
BEE IAT TXT_DrawChar target           TXT.dll + 0x413d0
```

Details and samples are in:

```text
docs/phase_reports/P2_BEE_TEXT_RASTER_FILL_PATH_20260507.md
target/dynamic_tools_85/bee_text_rendernode_RAS010_20260507/RAS_010.jsonl
```

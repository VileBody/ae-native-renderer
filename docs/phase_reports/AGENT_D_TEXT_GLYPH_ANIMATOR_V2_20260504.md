# Agent D Text / Glyph / Animator v2 Follow-Up

Status date: 2026-05-04

Scope: `M05`, `M06`, `M07`, `M08`.

## What Changed

- Added local fixture font resolution before fontconfig fallback for:
  - `Point-Light` / `Point` -> `fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf`
  - `Montserrat-BoldItalic` / `Montserrat-Italic` / `Montserrat` ->
    `fixtures/ae_conformance_pack/assets/fonts/Montserrat-Italic[wght].ttf`
- Added `FontResolutionSource::FixtureAsset` so Docker/dev/testkit can tell
  "found local conformance font" apart from DejaVu/common fallback.
- Extended `TextLayoutTelemetry` with `line_boxes`:
  - line index;
  - source char range;
  - baseline;
  - line width/height;
  - logical line box;
  - normalized line box;
  - union glyph bbox when glyphs exist.
- Extended text selector helper telemetry:
  - serde-ready `BasedOn`, `SelectorShape`, `RangeSelectorV2`,
    `WigglySelector`, `TextUnit`, `TextUnitWeight`, and blur plan structs;
  - `TextUnitWeight.selector_position_percent`;
  - `TextUnitWeight.base_weight` before wiggly modulation;
  - `evaluate_range_selector_v2_telemetry(...)`.
- Extended runtime `text.selector_weights` rows with `selector_index` and
  `selector_position_percent` so source order and randomized selector order can
  be compared directly.

## Font Connectivity

Current local assets:

```text
fixtures/ae_conformance_pack/assets/fonts/Montserrat-Italic[wght].ttf
fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf
youworkforthem-T9068-point.zip
```

`render-cli` conformance scenes already use direct paths via
`font_montserrat(pack_root)` and `font_point_light(pack_root)`, so the native
pack does not need fontconfig for `TXT_010`-`TXT_040`.

Docker runtime has `fontconfig`, and compose mounts `./fixtures` to
`/work/fixtures`. The resolver now also checks `/work/fixtures/.../assets/fonts`
for `Point-Light` and `Montserrat-*` family requests before falling back to
fontconfig/common fonts. This closes the old Docker path where `Point-Light`
could silently become DejaVu Sans when a scene used a family name instead of a
direct path.

Do not commit the Point archive unless licensing policy explicitly allows it.
The resolver uses `Point-Light.ttf` only when it is already present locally.

## Animator v2 Plan

- Selector shapes/smoothness: keep current deterministic shape helpers, but do
  not claim AE parity until AE exports effective per-character Amount for shape
  and smoothness sweeps.
- Randomize Order: source-order output now includes selector rank/percent. The
  seed source is still approximate because current IR has no separate random
  seed outside wiggly metadata.
- Wiggly Selector: helper telemetry carries pre-wiggly and post-wiggly weights;
  model remains deterministic hash noise, not recovered AE math.
- Blur Animator: current runtime still uses weighted square splat blur. Keep it
  as an approximation until AE blur-kernel and alpha/composite probes land.
- Better glyph metrics: line boxes and per-glyph passports are now available,
  but true parity requires CoolType/CoreText-style shaping and metrics.

## Blocker

`Basic_Text.aex` delegates real text construction and metrics to CoolType/TXT:
`CTNewTextInterfaceV2`, `CTFontInstanceInterfaceV2`, glyph IDs, widths, bboxes,
baseline deltas, feature processing, writing direction, and raster warning
hooks are hidden behind those APIs. Native fontdue/simple layout must not be
tuned by eye against final PNGs and should be treated as approximate until AE
glyph metric sidecars or a CoolType-like backend are available.

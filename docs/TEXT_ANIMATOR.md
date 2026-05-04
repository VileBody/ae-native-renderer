# Text Animator Notes

Text animators are currently approximated from text alpha bounds and selector units.
The target model is glyph/object weights plus per-glyph property evaluation.

## Required layout data

```text
glyph_id
char_index
word_index
line_index
position
advance
bbox
local_anchor
```

Current `TextLayoutTelemetry` exposes this as serialized runtime telemetry:

- `font_resolution`: requested id, resolved path/family/style/fullname/
  postscript, source, and fallback flag.
- `glyphs`: per-glyph character, font glyph id, char/word/line index,
  advance, bbox, normalized bbox, bbox center, baseline, line width, text box.
- `line_boxes`: per-line char range, baseline, line width/height, logical line
  box, normalized line box, and union glyph bbox when glyphs exist.

## First supported subset

- Range Selector;
- Percent Start/End;
- Based On: characters, words, lines;
- Shape: Square, Ramp Up, Ramp Down, Triangle, Round, Smooth approximations;
- Smoothness as deterministic selector-edge smoothing;
- Randomize Order as deterministic seeded selector ordering;
- Wiggly selector metadata as deterministic per-unit modulation;
- Animator properties: Opacity, Position 3D projected to 2D, Scale 3D projected
  to 2D, Rotation Z, Blur as a bounded per-unit splat blur approximation.
- Expression Selector: generated per-character bounce pattern from `impulse_2nd`.

Arbitrary expression selectors remain outside the native subset.

## Text Animator v2 helper primitives

`text-engine::text_animator` now exposes standalone helper primitives for the
next integration pass. These helpers do not run inside `render-core` yet.

Supported at helper level:

- `RangeSelectorV2` with percent Start/End, Based On characters/words/lines,
  Shape, Smoothness, randomized order, and optional Wiggly modulation.
- Unit extraction through `text_units(text, based_on)`.
  - Characters are Unicode scalar units, excluding CR/LF line breaks.
  - Words are contiguous non-whitespace spans.
  - Lines preserve source-order spans and include empty trailing lines.
- Stable source indices are preserved independently from selector order.
- `deterministic_selector_order(total, seed)` returns a seeded permutation rank
  per source unit, so randomize order is repeatable across runs.
- `evaluate_range_selector_v2(text, selector, time_seconds)` returns
  source-order `TextUnitWeight` entries with selector index, total, and clamped
  weight. Each weight also carries selector-center percent and the pre-wiggly
  base range weight for AE sidecar comparison.
- `evaluate_range_selector_v2_telemetry(text, selector, time_seconds)` returns
  a serializable selector passport with selector parameters, unit count, and
  source-order unit weights.
- `plan_blur_animator(weights, blur)` creates per-unit blur data plus an
  approximate layer fallback blur. It is data/planning only; no filtering is
  performed here.

Approximate helper behavior:

- Shape curves are intentionally deterministic approximations:
  - Square: binary inside the selector span.
  - Ramp Up / Ramp Down: linear ramp across the span.
  - Triangle: linear peak at span center.
  - Round: sine peak at span center.
  - Smooth: smoothstep peak at span center.
- Smoothness blends the raw shape weight toward a smoothstep-shaped value.
  This is stable and testable, but not a full After Effects selector transfer
  function.
- Wiggly uses deterministic hash noise per unit and interpolates between noise
  samples over time. It is suitable for repeatable renderer tests but is not a
  byte-for-byte AE match.
- Randomize order changes selector rank only. Output units remain in source
  order so downstream glyph mapping can stay stable.

## Evaluation rule

Runtime `text.selector_weights` telemetry includes source unit index, ordered
selector index, selector-center percent, unit rect, range/expression/final
weights, and animator contribution fields for position/scale/rotation/opacity/
blur when those properties are active.

## Text Passport Comparison

`render-cli conformance-pack` now writes a per-case diagnostic file:

```text
<out>/<case_id>/text_passport_comparison.json
```

The optional AE/CoolType reference location is:

```text
fixtures/ae_conformance_pack/ae_goldens/text_telemetry/<case_id>/<case_id>_<frame>.jsonl
```

Each reference file can use the same JSONL event shape as native
`text_telemetry.jsonl`:

```json
{"event":"text.layout","frame":0,"record":{...}}
{"event":"text.selector_weights","frame":0,"record":{...}}
```

References are compared as subsets: an AE probe may include only glyph ids,
advances, bboxes, selector weights, or animator matrices that it actually knows.
Extra native fields are ignored. Missing reference files are reported as
`missing_reference`; they do not fail the PNG conformance run.

## Imported AE Text Telemetry

`fixtures/ae_conformance_pack/ae_goldens/text_telemetry/` now contains compact
JSONL references generated on the AE 85 node:

```text
ae_text_telemetry_85_20260505_021335
TXT_010: 7 frames
TXT_020: 7 frames
TXT_030: 7 frames
TXT_040: 8 frames
```

The current reference semantics are `sourceRectAtTime`-based. They are good for
detecting layout drift and selector-unit mapping drift, but they do not expose
true CoolType glyph ids or raster coverage yet.

Latest smoke comparison:

```text
target/ae_agents/native_text_passport_step3_smoke/report.json
```

All four text cases now compare against refs with zero missing reference frames.
The expected current result is `text_passport.ok=false` because native fontdue
metrics still diverge from AE/Point-Light/Montserrat measurements.

Do not interpolate matrices directly. Interpolate properties, then build matrix:

```text
position_delta = animator_position * weight
rotation       = animator_rotation * weight
scale          = lerp([100,100,100], animator_scale, weight)
opacity        = lerp(layer_opacity, animator_opacity, weight)
blur           = animator_blur * weight
```

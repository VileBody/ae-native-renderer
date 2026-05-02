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
  weight.
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

Do not interpolate matrices directly. Interpolate properties, then build matrix:

```text
position_delta = animator_position * weight
rotation       = animator_rotation * weight
scale          = lerp([100,100,100], animator_scale, weight)
opacity        = lerp(layer_opacity, animator_opacity, weight)
blur           = animator_blur * weight
```

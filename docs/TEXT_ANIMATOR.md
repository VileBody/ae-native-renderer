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
- Shape: Square first;
- Smoothness: 0/100 first;
- Animator properties: Opacity, Position 3D projected to 2D, Scale 3D projected
  to 2D, Rotation Z.
- Expression Selector: generated per-character bounce pattern from `impulse_2nd`.

Blur is parsed and reported as approximate but is not rendered per glyph yet.
Wiggly selectors, randomized order, and arbitrary expression selectors remain
outside the v0 renderer subset.

## Evaluation rule

Do not interpolate matrices directly. Interpolate properties, then build matrix:

```text
position_delta = animator_position * weight
rotation       = animator_rotation * weight
scale          = lerp([100,100,100], animator_scale, weight)
opacity        = lerp(layer_opacity, animator_opacity, weight)
blur           = animator_blur * weight
```

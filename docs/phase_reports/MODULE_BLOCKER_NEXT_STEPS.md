# Module Blocker Next Steps

Status date: 2026-05-03.

This document turns the Round 1 agent findings into concrete next steps for the
four active blocker groups:

```text
1. M16 / STK_030 temporal adjustment stack
2. M10 / M11 / M13 effects
3. M12 / M14 warps and procedural fields
4. M05-M09 / M17 text, expressions, collapse
```

Global prerequisite, implemented after Round 1:

```text
M19 conformance metric normalization:
  raw RGBA metrics
  RGB-only metrics
  alpha-only metrics
  background/corner diagnostics
```

The conformance runner now writes structured `metrics.rgba`, `metrics.rgb`,
`metrics.alpha`, `metrics.background_alpha_normalized`, and
`background_corner` diagnostics. Raw `max_abs_diff=255` and mean/RMSE are still
mostly diagnostic noise for composed visual modules; use `rgb` and
`background_alpha_normalized` to decide whether visible math is moving in the
right direction.

## 1. M16 / STK_030 Temporal Adjustment Stack

Current evidence:

- `TMP_010` and `TMP_020` match AE in RGB.
- Isolated source/layer time and Posterize buckets should not be tuned now.
- `STK_030` native frame `0` and `1` are byte-identical.
- `STK_030` AE frame `0` and `1` differ.

Working hypothesis:

Native currently over-posterizes the whole downstream adjustment stack. AE seems
to quantize the input/lower-stack sampling at Posterize Time, while effects
after Posterize Time can still evaluate animated params at comp/effect time.

Steps:

1. Add temporal telemetry for adjustment layers:
   - comp time;
   - layer time;
   - lower-stack resample time;
   - Posterize fps, bucket id, bucket time;
   - per-effect index/name/matchName;
   - per-effect param evaluation time;
   - per-effect input and output canvas hash.
2. Add a focused render-core test:
   - lower numbered source is posterized at 6 fps;
   - a downstream animated effect after Posterize Time changes every comp frame;
   - frames inside the same posterize bucket should keep source identity but may
     differ after the downstream effect.
3. Keep `TMP_020` as the regression guard:
   - isolated Posterize bucket behavior must remain unchanged.
4. Patch time routing only after telemetry confirms the hypothesis:
   - Posterize Time should quantize the time used to resample its input;
   - downstream effect params should not automatically inherit that quantized
     time unless AE evidence proves they should.
5. Rerun:
   - `TMP_020` first;
   - `STK_030` frames `0`, `1`, `5`, `10`;
   - then full `STK_030`.
6. Promotion gate:
   - `M16` can move toward formula tuning only when frame `0/1` behavior is
     explained by telemetry and not by final PNG guessing.

## 2. M10 / M11 / M13 Effects

Current evidence:

- Raw RGBA metrics are dominated by `M19` alpha/background.
- Stack order is not the first proven bug.
- BoxBlur2/Glow animated params look static in `EFF_070`.
- Drop Shadow likely diverges on direction/offset before softness tuning.
- Glow likely diverges on threshold/mask and alpha participation.
- Minimax likely diverges on operation/channel enum or neighborhood semantics.

Steps:

1. Wait for `M19` split metrics, then rank cases by RGB-only error:
   - `EFF_010` Drop Shadow;
   - `EFF_020` Glow;
   - `EFF_030` Box Blur;
   - `EFF_050` Minimax;
   - `EFF_070` animated effect params;
   - `STK_010`, `STK_020` only after isolated operators are understood.
2. Add effect intermediate dumps or hashes:
   - Box Blur: input alpha/RGB hash, resolved radius, iteration count, kernel
     width, pass output hashes;
   - Drop Shadow: source alpha mask, raw offset mask, blurred shadow mask,
     shadow-only image, final composite;
   - Glow: luma/alpha source, threshold mask, blurred glow, intensity-scaled
     glow, final blend;
   - Minimax: operation enum, channel enum, radius, neighborhood shape,
     output alpha/RGB hash.
3. Apply tiny diagnostic correctness patch:
   - make BoxBlur2 and Glow animated numeric params time-aware where current
     code samples static `param_f32_any`;
   - use the same time-aware helper style already used by Minimax where
     possible.
4. Rerun `EFF_070`:
   - verify selected frames actually change when animated params change;
   - keep raw/RGB/alpha metrics separate.
5. Tune isolated formulas in this order:
   - Box Blur kernel/iterations/edge policy;
   - Drop Shadow direction and offset convention;
   - Drop Shadow softness and premult/straight composite;
   - Glow threshold/mask semantics;
   - Glow radius/intensity/blend;
   - Minimax enum/channel mapping;
   - Minimax radius/neighborhood/edge policy.
6. Compose and retest:
   - `STK_010` after Drop Shadow isolated cases;
   - `STK_020` after Box Blur and Minimax isolated cases;
   - template slices only after `EFF_*` and `STK_*` are explainable.
7. Promotion gate:
   - no effect reaches formula tuning unless the report names the failing
     intermediate, not just the final diff.

## 3. M12 / M14 Warps And Procedural Fields

Current evidence:

- `EFF_040` suggests Geometry2 parameter mapping is wrong before formula tuning.
- Native resolves uniform `0003=82` before width/height-style `0004=120` and
  `0008=72`.
- `EFF_060` is full-frame different and native Turbulent Displace is currently
  an approximate sine field.
- `STK_030` mixes unsettled M12/M13/M14/M15/M16/M19 and must stay a composed
  regression case.

Steps:

1. Add Geometry2 telemetry:
   - raw params;
   - resolved anchor/position/scale/rotation;
   - forward matrix;
   - inverse matrix;
   - sample UV for a small grid and edge pixels;
   - sampler mode;
   - out-of-bounds count and edge policy.
2. Verify AE parameter mapping:
   - determine whether `0003`, `0004`, and `0008` mean uniform scale,
     width/height, or another AE Transform/Geometry2 parameter set;
   - patch mapping only after telemetry proves which params AE consumes.
3. Tune Geometry2 in this order:
   - param mapping;
   - matrix composition;
   - pixel-center convention;
   - bilinear vs nearest sampling;
   - edge/out-of-bounds behavior.
4. Add Turbulent Displace field telemetry:
   - resolved amount, size, complexity, evolution;
   - noise fields;
   - `dx`, `dy`;
   - displaced UV;
   - sampled source hash;
   - out-of-bounds/edge stats;
   - final pixel hash as dependent output only.
5. Build field-level tests:
   - zero amount must preserve input;
   - evolution must change field predictably;
   - amount/size/complexity boundaries must be stable;
   - edge behavior must be visible separately from field generation.
6. Tune Turbulent only after field artifacts exist:
   - parameter mapping;
   - evolution/time mapping;
   - octave/complexity semantics;
   - noise model;
   - displacement-to-UV scale;
   - sampler/edge behavior.
7. Compose and retest:
   - isolated `EFF_040`;
   - isolated `EFF_060`;
   - then `STK_030` after Agent 2 resolves time routing and Agent 3 resolves
     Minimax.
8. Promotion gate:
   - M14 cannot be tuned from final pixels. It needs field-level evidence.

## 4. M05-M09 / M17 Text, Expressions, Collapse

Current evidence:

- Raw metrics are dominated by `M19` alpha/background.
- `TXT_030` and `TXT_040` request `Point-Light`, but Docker resolves it to
  DejaVu Sans.
- Text telemetry is missing for the values needed to diagnose AE parity.
- `GPH_010` combines text layout, alpha, and collapse sharpness; it is not a
  first tuning target.

Steps:

1. Fix font observability before tuning:
   - emit requested font id/family/style;
   - resolved font path;
   - resolved family/postscript if available;
   - fallback flag;
   - selected variable axes or weight if known.
2. Make `Point-Light` deterministic in the conformance environment:
   - prefer a repo-local asset path if licensing allows;
   - otherwise document host-font requirement and skip tight `TXT_030/TXT_040`
     parity in Docker when fallback is detected.
3. Add glyph layout telemetry:
   - actual font glyph id, not Unicode codepoint;
   - char/word/line indices;
   - advance;
   - bbox;
   - baseline;
   - line width and text-box rect.
4. Add selector telemetry:
   - based-on mode;
   - unit list;
   - start/end/offset;
   - shape/smoothness/random/wiggly params;
   - raw and final selector weight per unit per selected frame.
5. Add glyph animator telemetry:
   - unit bbox and anchor;
   - position/scale/rotation/opacity contribution;
   - blur radius;
   - final per-glyph matrix;
   - per-glyph alpha/composite hash where cheap enough.
6. Add expression telemetry:
   - expression mode/fingerprint;
   - input property value;
   - comp/layer time;
   - context vars;
   - raw scalar/Vec2 result;
   - final property value.
7. Add expression selector telemetry:
   - `textIndex`, `textTotal`;
   - delay/frequency/amplitude/decay;
   - raw amount;
   - clamped amount;
   - final glyph transform weight.
8. Add collapse text telemetry:
   - collapse mode;
   - flattened layer list;
   - parent/child matrices;
   - matrix scale hint;
   - effective raster scale and raster size;
   - sharpness probe for collapsed vs rasterized text.
9. Tune in this order:
   - font resolution and glyph metrics;
   - line breaking and text-box placement;
   - selector unit segmentation and weights;
   - glyph animator transform order;
   - blur animator approximation;
   - expression value samples;
   - expression selector transfer;
   - collapse raster scale/sharpness.
10. Compose and retest:
   - `TXT_010`, `TXT_020` for reveal;
   - `TXT_030` only when Point-Light is exact;
   - `TXT_040` only when Point-Light is exact and expression selector telemetry
     exists;
   - `EXP_010` for property expression independent of glyph layout;
   - `GPH_010` after glyph layout and graph telemetry are explainable.
11. Promotion gate:
   - no text formula tuning from final PNGs until font resolution and glyph
     telemetry are present.

## Recommended Execution Order

1. Temporal stack telemetry for `M16/STK_030`.
2. Effect intermediate telemetry plus BoxBlur2/Glow time-aware params.
3. Geometry2 telemetry and parameter mapping check.
4. Text font/glyph/selector/expression telemetry.
5. Turbulent field telemetry.
6. First formula tuning pass, starting only with isolated cases that have clean
   RGB/alpha metrics and module telemetry.

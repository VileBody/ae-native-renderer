# Module Blocker Next Steps After Round 2

Status date: 2026-05-03.

Round 2 changed the baseline:

- `M19` split metrics exist: `rgba`, `rgb`, `alpha`,
  `background_alpha_normalized`, `background_corner`.
- `M16/STK_030` no longer has the old "native frame 0/1 identical" blocker.
- BoxBlur2 and Glow animated params are time-aware.
- Geometry2 `0004`/`0008` axis scale now overrides uniform `0003`.
- `Point-Light.ttf` is repo-local and used by native conformance recipes.

The next work should start from fresh Round 2 conformance runs and module
telemetry, not from the old raw RGBA metrics.

## 1. M16 / STK_030 Temporal Adjustment Stack

Current status:

- `TMP_020` remains RGB-clean.
- Adjustment-layer Posterize Time now freezes lower/input sampling at bucket
  time while downstream effects after Posterize evaluate at current comp time.
- `STK_030` native frames `0` and `1` now differ, matching the AE observation.
- Remaining `STK_030` RGB diff is likely downstream operator math:
  Geometry2, Minimax, Turbulent Displace, and stack composition.

Next steps:

1. Rerun `STK_030` with current code and save fresh metrics/profile output.
2. Inspect `FrameRenderTrace.adjustment_effects` for selected frames:
   - `0`, `1`, `5`, `10`, `15`, `30`, `45`, `59`;
   - check lower-stack resample time;
   - check per-effect param time;
   - check per-effect input/output hashes.
3. Add or expose conformance sidecars for adjustment traces:
   - one JSON record per frame/effect;
   - include comp time, bucket time, param time, input hash, output hash.
4. Keep `TMP_020` as the guard for isolated Posterize behavior.
5. Use `STK_030` only as a stack regression after isolated operators are
   explainable.
6. If traces are internally consistent, mark the old `M16` timing blocker as
   cleared and route remaining work to `M12`, `M13`, and `M14`.

Promotion gate:

- `M16` can advance once `STK_030` trace proves the right time routing and no
  remaining diff is attributable to global adjustment timing.

## 2. M10 / M11 / M13 Effects

Current status:

- Split metrics are available.
- BoxBlur2 and Glow animated params are time-aware.
- `EFF_070` native output is no longer static.
- Debug hash helpers exist for Box Blur, Drop Shadow, Glow, and Minimax.
- Formula tuning has not started yet.

Next steps:

1. Wire effect debug helpers into conformance sidecars:
   - `effects_debug/<case>/<frame>/<layer>_<effect>.json`;
   - include resolved params and intermediate hashes.
2. Rerun isolated cases with sidecars:
   - `EFF_010` Drop Shadow;
   - `EFF_020` Glow;
   - `EFF_030` Box Blur;
   - `EFF_050` Minimax;
   - `EFF_070` animated BoxBlur2/Glow.
3. Rank failures by `metrics.rgb` and `metrics.background_alpha_normalized`.
4. Tune in this order:
   - Box Blur kernel, iterations, radius mapping, edge policy;
   - Drop Shadow direction and offset convention;
   - Drop Shadow softness and premult/straight composite;
   - Glow threshold source: luma vs alpha participation;
   - Glow radius, intensity, blend/composite behavior;
   - Minimax operation enum and channel enum;
   - Minimax neighborhood shape, radius rounding, edge policy.
5. After each isolated fix, rerun the matching `EFF_*` case before touching
   `STK_010` or `STK_020`.
6. Use stacks only after isolated effects explain their first divergent
   intermediate.

Promotion gate:

- Effects can enter formula tuning case-by-case only when the first divergent
  intermediate is known, not just the final PNG diff.

## 3. M12 / M14 Warps And Procedural Fields

Current status:

- Geometry2 has resolved-param/matrix/UV/OOB telemetry.
- Geometry2 scale mapping was patched:
  `0004`/`0008` override uniform `0003`.
- Turbulent Displace has field telemetry:
  resolved params, probe noise, `dx/dy`, source UV, field hash, OOB count.
- Turbulent formula remains approximate.

Next steps:

1. Rerun `EFF_040` with current Geometry2 mapping.
2. Compare new `metrics.rgb` against old Round 1 result:
   - confirm the scale mapping fix moved the image in the right direction;
   - inspect raw/resolved params, matrix, inverse matrix, sample UV, OOB count.
3. If `EFF_040` still diverges, tune Geometry2 in this order:
   - matrix composition;
   - anchor/position convention;
   - pixel-center convention;
   - nearest vs bilinear sampler;
   - transparent vs clamp edge behavior.
4. Rerun `EFF_060` with Turbulent field telemetry captured.
5. Compare field-level artifacts before final pixels:
   - resolved amount/size/complexity/evolution;
   - probe `noise`;
   - `dx/dy`;
   - displaced UV;
   - field hash over selected frames.
6. Only after field telemetry is useful, start Turbulent formula work:
   - AE parameter mapping;
   - evolution/time semantics;
   - octave/complexity behavior;
   - noise model;
   - displacement scaling;
   - sampler/edge policy.
7. Return to `STK_030` only after `EFF_040`, `EFF_050`, and `EFF_060` are
   separately diagnosed.

Promotion gate:

- `M12` can move into formula tuning after `EFF_040` has matrix/UV evidence.
- `M14` cannot move into formula tuning until field-level telemetry, not final
  pixels, identifies the mismatch.

## 4. M05-M09 / M17 Text, Expressions, Collapse

Current status:

- `Point-Light.ttf` is repo-local.
- `TXT_030` and `TXT_040` native recipes use the direct font path.
- Docker no longer needs to resolve `Point-Light` through fontconfig for native
  conformance.
- Font resolution telemetry exists.
- Layout telemetry exists.
- Real layout now stores actual font glyph id instead of Unicode codepoint.
- Selector/expression/collapse telemetry is still incomplete.

Next steps:

1. Rerun exact-font text cases:
   - `TXT_030`;
   - `TXT_040`;
   - also rerun `TXT_010`, `TXT_020`, `EXP_010`, `GPH_010` for a consistent
     Round 2 text baseline.
2. Compare `metrics.rgb` and `background_alpha_normalized`, not raw RGBA.
3. Inspect font telemetry:
   - requested font id;
   - resolved path;
   - fallback flag must be false for Point-Light direct path;
   - glyph ids and metrics should be present.
4. Add runtime selector telemetry:
   - unit list;
   - based-on mode;
   - start/end/offset;
   - shape/smoothness/random/wiggly;
   - weight per unit per selected frame.
5. Add glyph animator telemetry:
   - per-glyph bbox;
   - position/scale/rotation/opacity contribution;
   - blur radius;
   - final glyph matrix.
6. Add expression telemetry:
   - property expression input/time/context;
   - raw scalar/Vec2 result;
   - final property value.
7. Add expression selector telemetry:
   - `textIndex`, `textTotal`;
   - delay/frequency/amplitude/decay;
   - raw/clamped amount;
   - final transform contribution.
8. Add collapse telemetry for `GPH_010`:
   - collapse mode;
   - flattened layers;
   - parent/child matrices;
   - effective raster scale;
   - raster size;
   - sharpness probe.
9. Tune in this order:
   - glyph metrics and line/text-box placement;
   - selector segmentation and weights;
   - glyph animator transform order;
   - blur animator approximation;
   - property expression samples;
   - expression selector transfer;
   - collapse raster scale/sharpness.

Promotion gate:

- Text can enter formula tuning only after exact-font rerun plus glyph/selector
  telemetry shows which submodule causes the first visible mismatch.

## Immediate Batch To Run

Completed in:

```text
docs/phase_reports/OS_ROUND2_INTEGRATION_PASS.md
```

Full metric batch:

```text
TMP_020, STK_030,
EFF_010, EFF_020, EFF_030, EFF_050, EFF_070,
EFF_040, EFF_060,
TXT_010, TXT_020, TXT_030, TXT_040, EXP_010, GPH_010
```

Trace sidecar smoke:

```text
STK_030, EFF_040, EFF_060, TXT_030, EXP_010, GPH_010
```

Next split work by the first divergent intermediate, not by the highest raw
RGBA mean. `STK_030` and `GPH_010` remain composed regression cases until their
primitive dependencies move.

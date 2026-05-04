# Math Parity Status

Status date: 2026-05-05.

This document separates two different kinds of work:

```text
1. implement missing renderer functionality
2. tune implemented math until it matches After Effects
```

The renderer should be tracked as a set of modules first. A template status is
then a composite of the modules it uses.

```text
template_x = module_a + module_b + module_c + ...
template_x status = weakest required module status, plus template-specific AE refs
```

## Status Ladder

| Status | Meaning | Promotion criteria |
| --- | --- | --- |
| `not implemented` | The AE feature is absent, stubbed, ignored, or only recognized for reporting. | Native IR/runtime behavior exists and affects output deterministically. |
| `implemented approximate` | The feature works in a controlled subset, but the formula is our approximation. | Add focused unit tests, conformance fixture, and enough debug/telemetry to localize divergence. |
| `instrumented/testable` | The feature has deterministic probes, fixtures, assertions, or telemetry hooks. AE output is not checked in yet. | Export AE reference frames/telemetry and mark the manifest reference as ready. |
| `AE golden exists` | AE reference PNGs or telemetry exist with thresholds, but native does not yet pass tightly. | Run diffs, identify first divergent operator, and begin formula changes against those failures. |
| `formula tuning` | We are changing formulas/parameter mapping/sampling to reduce known AE diffs. | Thresholds pass consistently across micro-scenes and template frames. |
| `parity locked` | The block passes AE thresholds and release-regression thresholds in CI. | Only intentional threshold changes or new AE fixtures can move this. |

Rules:

- A module cannot skip from `implemented approximate` to `formula tuning`.
- As of this document date, no module is yet `AE golden exists`,
  `formula tuning`, or `parity locked`.
- A final-frame PNG diff is useful, but not enough for complex operators.
- Temporal, glyph, graph, procedural, and coordinate operators need internal
  checkpoints such as sample times, matrices, selector weights, UV fields, or
  effect intermediates.
- Template-level goldens answer "does the final video match"; operator-level
  goldens answer "where did it start diverging".

Why are tests green? Because most current tests validate internal contracts:
parsing, deterministic approximations, fixtures, manifests, and smoke renders.
They do not yet assert AE parity. Green tests currently mean "the implementation
is stable and testable", not "the formulas match AE".

## Module Registry

These are the modules we need for the current three-template target plus the
nearby AE math backlog. Template status is derived from this table.

| ID | Module | Current status | Used by | What exists now | Next promotion step |
| --- | --- | --- | --- | --- | --- |
| `M01` | Timeline layer activity, z-order, opacity compositing | `implemented approximate` | all three | Deterministic layer activity, reverse layer order, normal alpha composite, render logs. | Add AE frame mapping fixtures, alpha/premult telemetry, and template frame goldens. |
| `M02` | Footage source-time sampling and media frame selection | `implemented approximate` | all three | `source_start`, activity windows, sequential decode/cache, media plan logs. | Numbered-frame source fixtures, source-time telemetry, AE/reference frame-index goldens. |
| `M03` | 2D transform matrix, anchor/position/scale/rotation sampling | `implemented approximate` | all three | Matrix convention, inverse sampling, bilinear sampler, ROI bounds, unit tests. | Operator passport, coordinate-field/UV diff fixtures, matrix telemetry, AE transform goldens. |
| `M04` | Keyframes: hold/linear/cubic Bezier approximation | `instrumented/testable` | all three | Scalar/Vec2 keyframes, compact cubic ease, unit tests, Bezier conformance micro-scene scaffold. | Export AE ease PNGs, compare temporal-ease tangent mapping, tune solver/parameter mapping. |
| `M05` | Text rasterization and glyph layout | `instrumented/testable` | all three | Fontdue rasterization, glyph bbox/advance/char/word/line indices, Cyrillic-capable fallback; CoolType glyph metric targets selected for glyph id, widths, bboxes, baselines, feature processing, and CTText rows; layout sidecars now emit glyph-run index, advance x/y, bbox min/max, baseline delta slot, metric source, and CoolType reference status. | Add AE/CoolType glyph-row references, compare native layout rows against them, then tune shaping/composer/raster coverage. |
| `M06` | Text Range Selector reveal by words/characters/lines | `instrumented/testable` | `template_4th`, `scenes_3rd` | Start/End %, BasedOn, selector shapes, smoothness/randomize/wiggly approximations, glyph/word/line bbox units, selector-unit sidecars linked to glyph-run passports. | Add boundary fixtures and AE text reveal goldens; tune selector boundaries, order, smoothness, and whitespace treatment. |
| `M07` | Character text animator position/scale/rotation/blur | `instrumented/testable` | `impulse_2nd` | Per-unit transforms, blur splat approximation, glyph-level unit rectangles, per-unit glyph refs, final matrix, opacity alpha scale, and blur radius telemetry. | Add AE glyph animator goldens and tune per-glyph transform center, blur kernel, opacity composition, and selector weighting. |
| `M08` | Expression selector bounce | `implemented approximate` | `impulse_2nd` | Recognized generated `per_character_bounce` selector with deterministic native evaluator path. | Expression selector amount telemetry, AE bounce curve samples, tune delay/frequency/decay semantics. |
| `M09` | Property expression subset: generated `edge_wobble` | `implemented approximate` | `scenes_3rd` | Named position-expression mode plus small scalar/Vec2 expression evaluator. | Per-property expression telemetry, AE samples for footage motion, tune waveform/envelope. |
| `M10` | Drop Shadow | `instrumented/testable` | `template_4th`, `impulse_2nd` | Effect module, typed/numbered params, unit tests, effects conformance scaffold. | Per-effect AE PNGs, alpha/shadow-mask telemetry, tune blur/offset/composite/premult. |
| `M11` | Glow | `instrumented/testable` | `template_4th` | Effect module, typed/numbered params, unit tests, effects conformance scaffold. | AE glow goldens, threshold/luma/radius/intensity telemetry, tune kernel/composite. |
| `M12` | Geometry2 | `implemented approximate` | `scenes_3rd` | Adjustment effect module with transform-like params and time-varying scalar support. | Coordinate-field fixtures, UV diff, AE Geometry2 goldens, sampler/edge-mode tuning. |
| `M13` | Minimax | `implemented approximate` | `scenes_3rd` | Effect module with AE operation/channel/direction enum surface, time-aware radius, and primitive unit tests. | Fractional-radius probes, Direction impulse/ramp goldens, Don't Shrink Edges behavior, GPU/CPU path parity. |
| `M14` | Turbulent Displace | `instrumented/testable` | `scenes_3rd` | Deterministic sine/noise displacement approximation, time-varying evolution param support, AE-wrapper telemetry from Ghidra for internal mode, `FracAll`/`Frac1D` path, fixed16 slots, complexity split, and H/V lookup sizes. | Replace the approximate field with the recovered two-path AE-shaped state model, verify property indices `8/9/10/14`, add lookup hashes, then tune noise/evolution/octaves against AE field goldens. |
| `M15` | Posterize Time true temporal behavior | `implemented approximate` | `scenes_3rd` | Posterize Time now quantizes layer/source/effect time above stateless canvas effects, including adjustment-layer lower-stack resampling; render-core tests cover layer and adjustment behavior. | Add quantized-time/source-frame telemetry, AE temporal micro-scene goldens, and tune boundary/order semantics against AE. |
| `M16` | Adjustment layer pipeline and effect-stack order | `instrumented/testable` | `scenes_3rd` | Adjustment layers apply known effects to accumulated canvas; layer/effect timings are logged. | Graph-order checkpoints, non-commuting effect-order fixtures, AE adjustment-stack goldens. |
| `M17` | Collapse transformations / text precomp graph | `instrumented/testable` | payload structure for `template_4th`, `impulse_2nd`; future nested cases | Nested graph validation, cycle detection, text/solid-only collapse, parent matrix composition, scale-aware collapsed text rasterization, collapse micro-scene scaffold. | AE collapsed/rasterized pair goldens, vector/text deferred-raster telemetry, wider nested-case coverage. |
| `M18` | Motion blur | `instrumented/testable` | not observed in current imported target scenes; AE backlog | Composition/layer switches, shutter angle/phase/samples, subframe sampling, motion-blur micro-scene scaffold. | AE shutter/sample goldens, premult accumulation audit, static-layer skip, per-sample telemetry. |
| `M19` | Color, alpha, sampling, gamma assumptions | `implemented approximate` | all three | Straight RGBA8 canvas, normal composite, deterministic PNG output. | Premult/straight audit, alpha-ramp fixtures, gamma/color-space decision, AE compositing goldens. |
| `M20` | Masks, mattes, blend modes | `not implemented` | not observed as required for current snapshots | Capability reporting/fallback policy only where detected. | Implement only when payload inventory shows usage; then add operator fixtures and AE goldens. |
| `M21` | 3D, camera, spatial paths, roving keyframes, arbitrary ExtendScript | `not implemented` | not required for current snapshots | Explicit later scope. | Separate roadmap phase; do not block current three-template parity unless payloads start using them. |

## Template Composition Status

Observed from the current imported scene snapshots in
`target/native_template_runs/*/scene.json`.

| Template | Uses modules | Composite status | Why |
| --- | --- | --- | --- |
| `template_4th` | `M01`, `M02`, `M03`, `M04`, `M05`, `M06`, `M10`, `M11`, `M17`, `M19` | `implemented approximate` | All observed core modules exist natively. The weakest used modules are text/glyph layout, Range Selector reveal, transform/composite, and final AE effect tuning. `M10`/`M11` already have scaffolding, but no AE goldens. |
| `impulse_2nd` | `M01`, `M02`, `M03`, `M04`, `M05`, `M07`, `M08`, `M10`, `M17`, `M19` | `implemented approximate` | Native output covers the observed feature set, but glyph animator math, generated bounce selector, blur animator, Drop Shadow, and compositing are still approximations without AE telemetry/goldens. |
| `scenes_3rd` | `M01`, `M02`, `M03`, `M04`, `M05`, `M06`, `M09`, `M12`, `M13`, `M14`, `M15`, `M16`, `M19` | `implemented approximate` | All observed modules now have native behavior. The template is still below formula tuning because Posterize Time, Geometry2, Minimax, Turbulent Displace, adjustment ordering, expression motion, and compositing need AE telemetry/goldens. |

### Step 1 Component Passport

The first finish-line step for the current templates is now in place for text
components. Text layout sidecars expose the native glyph rows with explicit
source labels:

- `glyph_run_index`, `font_glyph_id`, `char_index`, `word_index`, `line_index`;
- `advance`, `advance_x`, `advance_y`;
- `bbox`, `cooltype_bbox_minmax`, `bbox_center`, normalized bbox fields;
- `baseline`, `baseline_delta`;
- `metric_source` (`fontdue` or `stub`);
- `cooltype_reference_status` (`not_cooltype_verified` until AE/CoolType rows
  are imported).

Selector sidecars now attach `glyph_passport` to each animated unit. For
characters this maps one unit to one glyph run; for words and lines it maps the
unit to the grouped glyph runs. Each selector unit also carries
`animator_contribution` with final matrix, opacity alpha scale, and blur radius.

Smoke output for the first pass lives at:

```text
target/ae_agents/native_text_passport_smoke/report.json
target/ae_agents/native_text_passport_smoke/TXT_030/text_telemetry.jsonl
target/ae_agents/native_text_passport_smoke/TXT_040/text_telemetry.jsonl
```

## Template Inventory

| Template | Observed math/features | Highest-risk parity areas |
| --- | --- | --- |
| `template_4th` | 12 footage layers, 10 text layers, 10 word-based text animators, opacity/reveal keyframes, 20 Drop Shadows, 10 Glows. | Text layout/reveal, Drop Shadow, Glow, color/compositing. |
| `impulse_2nd` | 18 footage layers, 27 text layers, 27 character animators with position/scale/rotation/blur and generated bounce expression selector, scale/opacity keyframes, 81 Drop Shadows. | Glyph-level text animator, expression selector bounce, blur animator, Drop Shadow, keyframe ease. |
| `scenes_3rd` | 8 footage layers, 15 text layers, 15 adjustment layers, 8 generated `edge_wobble` position expressions, 15 each of Geometry2, Posterize Time, Minimax, Turbulent Displace. | Adjustment/effect order, Turbulent Displace field, Geometry2 sampling, Posterize Time boundary parity, expression motion, compositing. |

## Promotion Plan

1. Keep the module registry as the source of truth.
2. For each template, list used modules by ID and derive the composite status
   from the weakest required module.
3. Integrate the operator-passport conformance layer into template reports so a
   template can say which module blocks its next promotion.
4. Add renderer telemetry checkpoints:
   - transform matrices, inverse matrices, UV/sample coordinates;
   - CoolType-like glyph rows, selector weights, glyph matrices, glyph opacity/blur;
   - effect intermediate hashes such as masks, kernels, UV/displacement fields;
   - adjustment/precomp graph checkpoints;
   - motion-blur sample times, weights, per-sample matrices, accumulation hash.
5. Export AE references for existing conformance micro-scenes.
6. Add missing micro-scenes:
   - text reveal boundary words/characters/lines;
   - `impulse_2nd` bounce selector;
   - Drop Shadow/Glow isolated alpha-ramp and impulse cases;
   - Geometry2 coordinate field;
   - Turbulent Displace coordinate field;
   - Posterize Time numbered-frame source and adjustment-stack boundary cases.
7. Only after the relevant modules have instrumentation and AE refs should they
   move into `formula tuning`.
8. A module becomes `parity locked` only when it passes AE and release goldens in
   automated tests.

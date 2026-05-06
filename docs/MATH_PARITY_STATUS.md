# Math Parity Status

Status date: 2026-05-06.

This document separates two different kinds of work:

```text
1. implement missing renderer functionality
2. recover implemented math from AE evidence until it matches After Effects
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
| `formula tuning` | We are changing formulas/parameter mapping/sampling from reverse/probe evidence, then validating against known AE diffs. | Thresholds pass consistently across micro-scenes and template frames, and the formula source is documented. |
| `parity locked` | The block passes AE thresholds and release-regression thresholds in CI. | Only intentional threshold changes or new AE fixtures can move this. |

Rules:

- A module cannot skip from `implemented approximate` to `formula tuning`.
- Formula changes must follow the reverse-first policy in
  `docs/reverse_engineering/MATH_CONTRACTS_GUARDRAILS.md`: Frida/Ghidra evidence
  first, metrics second. SDK/probes support validation; open-ended optimization
  against final diffs is not a valid promotion path.
- As of this document date, `M14` is in transitional `formula tuning` because it
  has Rust-native vector telemetry and a fitted baseline; future M14 changes
  must recover the AE kernel/table contract through Frida/Ghidra evidence before
  changing constants.
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
| `M04` | Keyframes: hold/linear/cubic Bezier approximation | `instrumented/testable` | all three | Scalar/Vec2 keyframes, compact cubic ease, unit tests, Bezier conformance micro-scene scaffold, and `temporal.keyframe_sample` records with segment/ease/progress diagnostics. | Export AE ease telemetry/PNGs, compare temporal-ease tangent mapping, then tune solver/parameter mapping. |
| `M05` | Text rasterization and glyph layout | `AE golden exists` | all three | Fontdue rasterization, glyph bbox/advance/char/word/line indices, Cyrillic-capable fallback; CoolType glyph metric targets selected for glyph id, widths, bboxes, baselines, feature processing, and CTText rows; layout sidecars now emit glyph-run index, advance x/y, comp-space bbox/minmax/centers/baselines, baseline delta slot, metric source, and CoolType reference status. AE sourceRect-based text telemetry refs are imported for `TXT_010`/`TXT_020`/`TXT_030`/`TXT_040`. | Use the text passport mismatches to tune shaping/composer/raster coverage; add deeper CoolType glyph-id/coverage rows and Montserrat instance mapping when the AE scripting subset is not enough. |
| `M06` | Text Range Selector reveal by words/characters/lines | `instrumented/testable` | `template_4th`, `scenes_3rd` | Start/End %, BasedOn, selector shapes, smoothness/randomize/wiggly approximations, glyph/word/line bbox units, selector-unit sidecars linked to glyph-run passports, and clipped-glyph selector units preserved for off-canvas text. | Add boundary fixtures and AE text reveal goldens; tune selector boundaries, order, smoothness, and whitespace treatment. |
| `M07` | Character text animator position/scale/rotation/blur | `instrumented/testable` | `impulse_2nd` | Per-unit transforms, blur splat approximation, glyph-level unit rectangles, per-unit glyph refs, final matrix, opacity alpha scale, and blur radius telemetry. | Add AE glyph animator goldens and tune per-glyph transform center, blur kernel, opacity composition, and selector weighting. |
| `M08` | Expression selector bounce | `implemented approximate` | `impulse_2nd` | Recognized generated `per_character_bounce` selector with deterministic native evaluator path. | Expression selector amount telemetry, AE bounce curve samples, tune delay/frequency/decay semantics. |
| `M09` | Property expression subset: generated `edge_wobble` | `implemented approximate` | `scenes_3rd` | Named position-expression mode plus small scalar/Vec2 expression evaluator. | Per-property expression telemetry, AE samples for footage motion, tune waveform/envelope. |
| `M10` | Drop Shadow | `instrumented/testable` | `template_4th`, `impulse_2nd` | Effect module, typed/numbered params, unit tests, effects conformance scaffold, straight-RGBA alpha-policy sidecar, source/raw/blurred/final alpha stats, and a stable post-M16 gate: `EFF_010` primary visible RGB mean `0.115046`, `STK_010` `0.069822`. | Treat Drop Shadow as a low-priority residual until new shadow-specific template evidence appears; current misses are small compared with Glow/Turbulent. |
| `M11` | Glow | `instrumented/testable` | `template_4th` | Effect module, typed/numbered params, unit tests, effects conformance scaffold, time-aware params, straight-RGBA alpha-policy sidecar, threshold/blurred/scaled/final alpha stats, Frida-confirmed Glow radius route `IR_GaussianBlur(radius * 0.4)`, and native separable Gaussian approximation for the Glow blur stage. | Remaining work is effect-local Glow formula tuning: exact ImageRenderer recursive Gaussian coefficients/edge policy, `Glow Based On` default/enum threshold source, intensity clamp, and final IR composite/blend route. |
| `M12` | Geometry2 | `reverse implemented (isolated sampler/matrix)` | `scenes_3rd` | Adjustment effect module with transform-like params, time-varying scalar support, matrix/sample debug data, adjustment-stack debug sidecars, isolated coordinate-field edge probe, integer pixel-center evidence, Frida CPU-path trace (`Transform.aex+0x5f30`), wrapper ABI trace (`Transform.aex+0x5b20`), confirmed `0012` Sampling mapping (`1` bilinear, `2` bicubic), fitted `0012=2` Keys cubic kernel (`a=-0.7`), AE-TIFF raw alpha loader, alpha-aware premultiplied-sample/unpremultiply wrapper, and durable `EFF_041` AE/native gate with primary visible RGB mean `0.043613`. | Keep Geometry2 matrix/sampling frozen; composed adjustment-layer residuals now route through `M16`, not M12 retuning. |
| `M13` | Minimax | `instrumented/testable` | `scenes_3rd` | Effect module with AE operation/channel/direction enum surface, time-aware radius, AE discriminator probe for fractional radius/direction/channel/edge behavior, native `Don't Shrink Edges` edge policy, and stable isolated gate `EFF_050` primary visible RGB mean `0.073972`. | Commit durable goldens if needed, add deep internal-alpha edge probe, then GPU/CPU path parity only if template evidence shows divergence. |
| `M14` | Turbulent Displace | `formula tuning` | `scenes_3rd` | Deterministic sine/noise displacement approximation with Rust-native arbitrary-point sample export via `render-cli turbulent-samples`; time-varying evolution param support; AE-wrapper telemetry from Ghidra for internal mode, `FracAll`/`Frac1D` path, fixed16 amount/size/offset/evolution plus `0008` cycle evolution, `0009` cycle revolutions, `0010` random seed, `0014` antialiasing, complexity split, H/V lookup sizes, adjustment-stack field sidecars, AE default offset-center handling, and `native_sine_turbulence_fit_v1`. Round5 Rust vector comparison improved `mean_vector_error=13.700092 -> 11.129620`; master gate has zero regressions. | Continue with Frida/Ghidra recovery of the hidden `FracAll`/`Frac1D` kernel/table contract, then implement recovered amount/size/displacement branches, seed/evolution/cycle behavior, complexity octave/fraction behavior, and pinning/resize/antialiasing. Do not continue by metric-only fitting. |
| `M15` | Posterize Time true temporal behavior | `instrumented/testable` | `scenes_3rd` | Posterize Time quantizes layer/source/effect time above stateless canvas effects, including adjustment-layer lower-stack resampling; temporal telemetry includes source-frame quantization policy/time/subframe. Post-M16 batch keeps `TMP_020` exact and `STK_030` temporal contract green. | Add only boundary-stress AE micro-scenes if future payloads expose bucket-edge drift; current `STK_030` residual is not a Posterize blocker. |
| `M16` | Adjustment layer pipeline and effect-stack order | `reverse implemented (Geometry2 origin routing)` | `scenes_3rd` | Adjustment layers apply known effects to accumulated canvas; per-effect input/output hashes, bucket/live param times, and Geometry2/Minimax/Turbulent debug checkpoints are logged for adjustment stacks. `ADJ_010..052` isolate the canvas contract. Geometry2-on-adjustment now forces comp/adjustment origin `(0,0)` instead of alpha-bounds origin: `STK_031` primary visible RGB mean dropped from `8.511274` to `0.032308`, and `ADJ_040` from `20.167969` to `0.087540`. | Keep this origin routing locked; route remaining `STK_030` residuals to `M13`/`M14`/`M15` and only revisit M16 for new effect classes or non-normal adjustment semantics. |
| `M17` | Collapse transformations / text precomp graph | `instrumented/testable` | payload structure for `template_4th`, `impulse_2nd`; future nested cases | Nested graph validation, cycle detection, text/solid-only collapse, parent matrix composition, scale-aware collapsed text rasterization, collapse micro-scene scaffold. | AE collapsed/rasterized pair goldens, vector/text deferred-raster telemetry, wider nested-case coverage. |
| `M18` | Motion blur | `instrumented/testable` | not observed in current imported target scenes; AE backlog | Composition/layer switches, shutter angle/phase/samples, subframe sampling, motion-blur micro-scene scaffold, per-sample shutter fraction/offset/source-frame telemetry, and weight summaries. | AE shutter/sample goldens, premult accumulation audit, and static-layer skip once shutter sample facts are known. |
| `M19` | Color/alpha composite substrate | `reverse implemented (RGBA8 normal composite)` | all three | Locked straight RGBA8 native boundary, reversed normal source-over formula, opacity-as-source-alpha-gain, transparent background RGB preservation, deterministic PNG output, split RGB/alpha/background-normalized metrics, and `rgb_straight_source_over_ae_background` primary visible metric. | Keep raw `rgba` compatibility-only; handle effect-local premultiply wrappers, non-normal blend modes, 16/32 bpc, and color-managed output in separate module contracts. |
| `M20` | Masks, mattes, blend modes | `not implemented` | not observed as required for current snapshots | Capability reporting/fallback policy only where detected. | Implement only when payload inventory shows usage; then add operator fixtures and AE goldens. |
| `M21` | 3D, camera, spatial paths, roving keyframes, arbitrary ExtendScript | `not implemented` | not required for current snapshots | Explicit later scope. | Separate roadmap phase; do not block current three-template parity unless payloads start using them. |

## Template Composition Status

Observed from the current imported scene snapshots in
`target/native_template_runs/*/scene.json`.

| Template | Uses modules | Composite status | Why |
| --- | --- | --- | --- |
| `template_4th` | `M01`, `M02`, `M03`, `M04`, `M05`, `M06`, `M10`, `M11`, `M17`, `M19` | `implemented approximate` | All observed core modules exist natively. The weakest used modules are text/glyph layout, Range Selector reveal, transform/composite, and final AE effect tuning. `M10`/`M11` already have scaffolding, but no AE goldens. |
| `impulse_2nd` | `M01`, `M02`, `M03`, `M04`, `M05`, `M07`, `M08`, `M10`, `M17`, `M19` | `implemented approximate` | Native output covers the observed feature set, but glyph animator math, generated bounce selector, blur animator, Drop Shadow, and compositing are still approximations without AE telemetry/goldens. |
| `scenes_3rd` | `M01`, `M02`, `M03`, `M04`, `M05`, `M06`, `M09`, `M12`, `M13`, `M14`, `M15`, `M16`, `M19` | `implemented approximate` | All observed modules now have native behavior. Geometry2, Minimax, Posterize Time, adjustment routing, alpha compositing, and Turbulent Displace are testable; M14 has entered formula tuning, but the template remains approximate until Turbulent field parity and expression/text residuals are locked. |

## Master Gate Snapshot

The current master gate policy is tracked in:

```text
fixtures/ae_conformance_pack/master_gate_policy.json
docs/MASTER_CONFORMANCE_GATE.md
```

Latest run:

```text
target/ae_agents/p0_master_gate_m14_20260506/dashboard.md
```

Current dashboard:

| Template | Gate status | Notes |
| --- | --- | --- |
| `template_4th` | `approximate` | Text/layout, Glow, Drop Shadow, collapse/text graph remain formula-tuning candidates. |
| `impulse_2nd` | `approximate` | Point-Light glyph animator, expression selector, and Drop Shadow remain approximate. |
| `scenes_3rd` | `approximate` | `TMP_020` is accepted; `STK_030` improved from `3.741755` to `3.424827` after M14 sine fit v1 but is still dominated by Turbulent Displace field parity. |

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

### Step 2 AE/CoolType Reference Diff

The conformance runner now has a text-passport comparison lane. For every
rendered frame it looks for:

```text
fixtures/ae_conformance_pack/ae_goldens/text_telemetry/<case_id>/<case_id>_<frame>.jsonl
```

When a reference exists, it compares AE/CoolType text rows against native
`text.layout` and `text.selector_weights` records as a subset contract. This
lets probes compare only the fields they can measure, such as glyph ids,
advances, bboxes, selector weights, final matrices, opacity, and blur radius.
When the reference is absent, the report records `missing_reference` and keeps
the visual PNG comparison non-blocking.

The per-case diagnostic output is:

```text
<out>/<case_id>/text_passport_comparison.json
```

### Step 3 AE Text Telemetry Import

AE sourceRect-based text telemetry refs were generated on the reserved AE node
and imported into the pack:

```text
remote job: ae_text_telemetry_85_20260505_021335
render id: 7c987d7be9884eba8a960ea1aa9e1573
fixtures/ae_conformance_pack/ae_goldens/text_telemetry/
fixtures/ae_conformance_pack/ae_goldens/metadata/text_telemetry_summary.json
```

The refs cover 29 frame files:

```text
TXT_010: 7 frames
TXT_020: 7 frames
TXT_030: 7 frames
TXT_040: 8 frames
```

Smoke comparison output:

```text
target/ae_agents/native_text_passport_step3_smoke/report.json
```

Current text-passport status:

| Case | Compared frames | Missing refs | First blocking mismatch |
| --- | ---: | ---: | --- |
| `TXT_010` | 7 | 0 | Montserrat word-reveal glyph advance drift, first delta `3.4220`. |
| `TXT_020` | 7 | 0 | Montserrat character/line layout drift, first delta `0.0480`. |
| `TXT_030` | 7 | 0 | Point-Light glyph animator layout drift, first delta `7.1543`. |
| `TXT_040` | 8 | 0 | Point-Light bounce-selector layout drift, first delta `9.2813`. |

This promotes text layout from "only instrumented" to "AE reference exists",
but only for the AE scripting/sourceRect subset. Selector-unit grouping is now
checked as a subset too. Selector weights, animator contribution values, and
true CoolType glyph ids still need deeper AE/CoolType probes before they can
move into formula tuning.

### Step 4 Text Layout Tuning

Worker A made the first non-probe math changes in the text lane:

- auto-leading now uses the AE-style `font_size * 1.2` model instead of
  fontdue line metrics;
- multiline text blocks are centered as a block;
- whitespace advance no longer has a synthetic floor;
- text layout telemetry reports comp-space bboxes, centers, and baselines;
- clipped/off-canvas glyphs are preserved as selector units.

Integrated run:

```text
target/ae_agents/step4_math_after_current/report.json
```

Text-passport movement against `step4_math_baseline`:

| Case | Mismatches before | Mismatches after | Max delta before | Max delta after |
| --- | ---: | ---: | ---: | ---: |
| `TXT_010` | 2548 | 2548 | 36.206 | 24.996 |
| `TXT_020` | 4116 | 4046 | 66.424 | 17.864 |
| `TXT_030` | 1092 | 1092 | 35.155 | 32.708 |
| `TXT_040` | 1568 | 1560 | 23.803 | 22.125 |

Visual PNG mean moved in mixed directions: `TXT_040` improved, while
`TXT_010`/`TXT_020`/`TXT_030` got slightly worse. This is acceptable for this
step because the explicit goal was moving from implemented behavior toward
measured layout parity, not locking final pixels. Remaining blockers are exact
CoolType glyph advances/raster coverage and Montserrat `BoldItalic` instance
mapping versus the checked-in Montserrat variable italic font.

### Step 4 Temporal / Ease / Motion Diagnostics

Worker B added diagnostic-only checkpoints for M04/M15/M18:

- `temporal.keyframe_sample` records now capture segment index, key times,
  normalized/eased progress, interpolation mode, hold flag, and cubic-ease
  control points;
- layer/source temporal records include structured source-frame quantization
  policy, frame time, and subframe;
- motion-blur samples include shutter offset/fraction, source-frame
  quantization, and weight summaries.

The focused before/after visual metrics for `INT_020`, `TMP_020`, `TMP_030`,
and `STK_030` are unchanged. This is intentional: no ease, posterize, or motion
blur formula was changed without AE-side tangent/bucket/shutter facts.

### Step 4 Geometry / Procedural / Adjustment Diagnostics

Worker D added adjustment-stack checkpoints for M12/M13/M14/M16:

- `STK_030` adjustment traces now include Geometry2 matrix/UV sample debug
  payloads on real stack inputs;
- Minimax stack diagnostics include resolved direction and Don't Shrink Edges;
- Turbulent Displace stack diagnostics include resolved field/wrapper/hash
  telemetry.

Integrated verification passed after concurrent text/sequence changes were
merged: `render-core` unit test
`adjustment_stack_debug_records_geometry_minimax_and_turbulent_checkpoints`
passes, and `STK_030` renders in the integrated 16-case conformance run. The
final pixel metrics are unchanged; Turbulent and Geometry2 remain blocked on AE
field/coordinate goldens before formula tuning.

### Step 4 Expression/Collapse Diagnostics

Worker E added diagnostic-only checkpoints for the M09/M17 lane:

- `EXP_010` expression telemetry now reports the generated named subset,
  evaluator mode, vector target type, fingerprint/source, and seeded
  `thisComp`/`thisLayer` timing context.
- `GPH_010` collapse telemetry now reports normalized matrix summaries
  (`translation`, `scale`, `scale_max`, determinant, rotation, axis dot,
  affine flag) next to the raw matrices.
- Collapse records now include explicit deferred-raster checkpoints. Current
  status is `matrix_pushdown_only` for collapsed boundaries and
  `intermediate_text_raster` for collapsed text. This is intentionally not an
  AE parity claim; true text/vector deferred rasterization still needs AE
  host-side probes and goldens.

Focused run:

```text
target/ae_agents/worker_e_step4_before/report.json
target/ae_agents/worker_e_step4_after/report.json
```

The before/after visual metrics for `EXP_010`, `GPH_010`, `CMP_010`,
`TMP_010`, and `TMP_020` are unchanged; this step only improves localization.

### Step 4 Effects / Alpha Diagnostics

Worker C added diagnostic-only checkpoints for M10/M11 and the M19 lead-up:

- Box Blur, Drop Shadow, and Glow effect sidecars now state the local
  `straight_rgba8` alpha policy and report alpha coverage/sum/min/max for
  each relevant intermediate buffer.
- Conformance metrics now include the explicit alias
  `rgb_straight_source_over_ae_background` plus `alpha_policy_diagnostics` so
  effect tuning can distinguish raw RGBA failures from straight/premult
  substrate ambiguity.
- Focused subset run:
  `target/ae_agents/worker_c_step4_effects_alpha_after/report.json`
  (`EFF_010`, `EFF_020`, `EFF_030`, `EFF_070`) completed with `ok=true`.

This step does not change final effect math. Glow/Drop Shadow tuning now uses
the locked M19 RGBA8 normal-composite contract, while Glow Based On enum probes
and effect-local premultiply/composite routes remain module-specific blockers.

### Step 4.5 Tuning Readiness / Evidence Lock

Step 4.5 is now tracked as a generated packet board:

```text
docs/phase_reports/STEP_4_5_TUNING_READINESS_20260505.md
fixtures/ae_conformance_pack/analysis/tuning_readiness_step4_5.json
```

Regenerate it with:

```bash
python3 scripts/build_tuning_readiness.py
```

This step does not claim formula parity. It locks the current Step 4 evidence,
measured cases, missing pack cases, allowed knobs, forbidden cross-module edits,
and Step 5 gates per module. The current entry order is:

1. `M05` text layout and `M15` Posterize Time as partial tuning-ready modules.
2. `M10`/`M11` effects, `M12`/`M13`/`M14` warps/fields, `M16` adjustment stack,
   `M17` collapse, and `M18` motion blur only through their packet-specific
   sidecars and blockers.
3. `M19` is no longer the global first blocker for RGBA8 normal composite;
   reopen it only for a new output contract such as 16/32 bpc, color management,
   non-normal blend modes, or renderer-path divergence.

### Step 5 Reverse Evidence Gates

The first Step 5 implementation pass added evidence gates that formula patches
must cite before claiming "reverse implemented" readiness:

```text
docs/phase_reports/STEP5_REVERSE_IMPLEMENTATION_READINESS_20260505.md
target/ae_agents/step5_reverse_evidence/report.json
target/ae_agents/step5_reverse_evidence/warps_fields_evidence_check.json
```

Current gate results:

- `M19`: required alpha/composite cases `PRI_010`, `CMP_010`, `STK_010`, and
  `STK_020` measured successfully; the RGBA8 normal-composite contract is now
  reverse implemented and documented in
  `docs/reverse_engineering/M19_REVERSE_LOCK.md`.
- `M15`/`M16`/`M18`: temporal contract reports pass for `TMP_010`, `TMP_020`,
  `TMP_030`, and `STK_030`.
- `M12`/`M14`: isolated warps/fields evidence check passes for `EFF_040`,
  `EFF_060`, and `STK_030`.
- `M05`-`M09`/text-side `M17`: text-passport diagnostics now separate sourceRect
  layout tuning from unresolved CoolType raster, selector/expression, and
  collapse refs.

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

# Module Hypothesis Validation Plans

Status date: 2026-05-05

This document defines the working process for the remaining math modules after
the M19 RGBA8 normal-composite lock.

The important rule is finite hypothesis search:

```text
recovered evidence -> finite candidate list -> discriminator fixture
  -> candidate implementation switch -> isolated gate -> composition gate
  -> stack/template gate -> lock or reject
```

If a module still has an open-ended formula space, do not tune pixels. Add a
probe, Ghidra target, SDK reference, or fixture until the candidate list is
finite.

## Common Process

Each module should move through the same loop:

1. Name the unresolved question.
2. Write hypotheses `H1`, `H2`, `H3`, with evidence source and expected output.
3. Build or select a discriminator fixture that separates those hypotheses.
4. Add a cheap native candidate switch, preferably local to the module.
5. Run isolated primitive conformance.
6. Run the relevant composition case.
7. Run stack/template smoke only after isolated behavior improves.
8. Accept the candidate only if telemetry and pixels agree with the same story.
9. Delete or demote losing candidate code; keep the accepted path and tests.
10. Update the module status doc with accepted formula, rejected hypotheses, and
    remaining scope limits.

## Shared Preparatory Work

These implementations speed up all module experiments and should be built once:

- Candidate switches: a typed, local enum for experimental formulas per module,
  set by test helper or conformance recipe, not by global hidden state.
- Metrics pack: every candidate run writes `candidate_id`, module version,
  input hash, output hash, selected frame, and formula parameters.
- Sidecar comparator: compare module-local telemetry before final pixels.
- Fixture sweeps: generate parameter sweeps once, then reuse the same AE goldens.
- Report summary: add `accepted`, `rejected`, `needs_new_probe`, and
  `regression_owner` fields to hypothesis runs.
- Golden provenance: every AE comparison records AE version, output module,
  bpc, renderer/path if known, and source fixture hash.
- Lock files: once a candidate is accepted, write the exact formula and evidence
  to a module lock note.

Do not keep a large permanent "formula zoo" in production code. Candidate paths
are allowed while experimenting, but a locked module should expose one default
formula plus narrow compatibility guards.

Current shared implementation:

- `testkit::HypothesisRunReport` writes schema
  `ae-native-renderer.hypothesis-run.v1` with module, candidate id, candidate
  config hash, status, gates, evidence, artifacts, and notes.
- `render-cli hypothesis-pack` wraps a conformance-pack run and writes
  `hypothesis_report.json` beside the native `report.json`.

Example:

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M10 \
  --candidate shadow_blur_floor \
  --status instrumented \
  --gate isolated \
  --question "How does AE map Drop Shadow softness to blur radius?" \
  --hypothesis "radius is floored before the shared blur kernel" \
  --evidence docs/reverse_engineering/effect_math_blur_glow_shadow.md \
  --case EFF_010 \
  --out target/ae_agents/m10_shadow_blur_floor
```

## M05 Text Layout / Glyph Metrics

### Sequential Hypotheses

1. Layout source: fontdue advance/bbox is close enough vs CoolType sourceRect
   references.
2. Font instance: Montserrat/Point-Light named-instance selection causes most
   layout drift.
3. Composer rules: AE line breaking, tracking, baseline, and whitespace handling
   cause the next largest drift.
4. Glyph ids/coverage: CoolType raster coverage is required for pixel parity
   after layout metrics are stable.

### Preparatory Implementations

- Text passport runner that can compare advance, bbox, baseline, line id, word
  id, glyph id when available, and sourceRect rows.
- Font-resolution manifest that records exact font file, family, style, named
  instance, fallback, and PostScript name.
- Glyph metric candidate switch: `fontdue_current`, `cooltype_probe_metrics`,
  `ae_sourcerect_calibrated`.
- Deep CoolType probe import slot for glyph id and coverage rows.
- Text fixture sweep for Montserrat and Point-Light strings used by the target
  templates.

### Gates

- Isolated: `TXT_010`, `TXT_020`, `TXT_030`, `TXT_040` text passport deltas.
- Composition: text reveal/glyph animator cases after M06/M07 consume layout.
- Stack/template: target template slices only after sourceRect/glyph metrics
  improve without breaking existing text cases.

## M06 Range Selector Reveal

### Sequential Hypotheses

1. Selector unit boundaries: characters, words, and lines use our current unit
   partitioning.
2. Boundary interpolation: Start/End percentages map to AE unit edges with a
   different inclusive/exclusive convention.
3. Shape/smoothness: AE selector shape and smoothness alter weights, not unit
   order.
4. Randomize order/wiggly: randomized or wiggly selectors require seeded order
   and per-frame amount samples.

### Preparatory Implementations

- Selector-unit sidecar with unit id, glyph span, text span, raw weight, shaped
  weight, and final opacity.
- Candidate switch for boundary convention:
  `inclusive_start`, `exclusive_end`, `ae_edge_weighted`.
- Selector shape enum scaffold: square, ramp up/down, triangle, smooth.
- Deterministic seeded random order helper.
- AE selector boundary fixture sweep over Start/End near unit edges.

### Gates

- Isolated: selector weights match before final text pixels.
- Composition: `TXT_010` and `TXT_020` reveal frames.
- Stack/template: template_4th text reveal slice.

## M07 Glyph Animator Transform / Opacity / Blur

### Sequential Hypotheses

1. Transform center: AE uses glyph bbox center, baseline anchor, or range-unit
   box center.
2. Transform order: position/scale/rotation compose in a different AE order.
3. Opacity: animator opacity scales glyph alpha before composite.
4. Blur: animator blur is alpha/coverage blur, RGB blur, or premult blur.
5. Selector weighting: final property interpolation uses shaped selector amount
   differently from linear weight.

### Preparatory Implementations

- Per-glyph animator sidecar: unit id, selector weight, transform center, matrix,
  opacity multiplier, blur radius, pre/post bbox.
- Candidate switch for transform center and matrix order.
- Blur candidate switch local to text animator: `alpha_only`, `straight_rgba`,
  `premult_then_unpremult`.
- Small glyph impulse fixture with colored semi-transparent glyph coverage.
- Visual crop output around each glyph for manual inspection.

### Gates

- Isolated: glyph matrix/opacity/blur telemetry.
- Composition: `TXT_030`.
- Stack/template: impulse_2nd text animator slice.

## M08 Expression Selector Bounce

### Sequential Hypotheses

1. Amount curve is generated from per-character delay, frequency, decay.
2. AE clamps or remaps selector amount before applying it to animator props.
3. Character index origin and direction differ.
4. Time sampling uses layer time, comp time, or source time.

### Preparatory Implementations

- Expression selector amount sidecar per glyph/unit/frame.
- Candidate evaluator for bounce with explicit delay/frequency/decay knobs.
- AE amount probe fixture that writes sampled selector amount when possible and
  otherwise encodes amount into opacity/position.
- Candidate switch for time base and index origin.

### Gates

- Isolated: selector amount samples.
- Composition: `TXT_040`.
- Stack/template: impulse_2nd bounce text slice.

## M09 Property Expression Evaluator

### Sequential Hypotheses

1. Named expression fingerprint is enough for current templates.
2. Property evaluator needs scalar/vector coercion and `thisLayer`/`thisComp`
   access but not arbitrary JS.
3. Time basis and layer in/out windows explain most motion drift.
4. Audio or external value expressions are absent from current target templates.

### Preparatory Implementations

- Expression trait/evaluator with typed scalar/vector return values.
- Evaluation sidecar: expression id, fingerprint, time, context, raw result,
  coerced result, fallback reason.
- Candidate switch for time base and vector/scalar coercion.
- AE property sample probes for generated wobble and selector expressions.

### Gates

- Isolated: property sample tables.
- Composition: `EXP_010`.
- Stack/template: scenes_3rd expression-driven footage motion.

## M10 Drop Shadow / Box Blur Dependency

### Sequential Hypotheses

1. Shadow mask source is alpha-only from source layer.
2. Direction/distance convention differs by angle origin or sign.
3. Softness uses AE shared blur kernel with radius mapping and iteration count.
4. Shadow color is straight RGB with alpha from blurred mask.
5. Final shadow composite is under-source normal composite with effect-local
   premult wrapper if needed.

### Preparatory Implementations

- Shadow sidecar: source alpha, raw offset mask, blurred mask, colored shadow,
  final composite.
- Blur kernel candidate switch: box radius floor/round/fractional,
  separable/iterated, edge policy.
- Direction convention switch: AE degrees from positive x/y variants.
- Semi-transparent colored alpha fixture to distinguish alpha-only vs premult.
- Stack fixture for two shadows with non-commuting offsets.

### Gates

- Isolated: `EFF_010`, `EFF_030` and shadow/blur intermediate maps.
- Composition: `STK_010`.
- Stack/template: template_4th and impulse_2nd shadow-heavy slices.

## M11 Glow

### Sequential Hypotheses

1. `Glow Based On` source is luma, alpha, or combined threshold.
2. Threshold is applied before blur in straight space.
3. Blur route shares AE Gaussian/box kernel with module-specific radius mapping.
4. Intensity scales RGB only, alpha only, or both with clamp.
5. Final composite uses add/screen-like blend or normal composite variant.

### Preparatory Implementations

- Glow sidecar: threshold source, threshold mask, blurred glow, intensity-scaled
  glow, final composite.
- `Based On` enum mapping table for raw `0001` values.
- Candidate switches for threshold source, blur kernel, intensity scaling, and
  composite blend.
- Alpha/luma split fixture that separates bright-low-alpha and dark-high-alpha.
- Parameter sweep over threshold/radius/intensity.

### Gates

- Isolated: `EFF_020`, `EFF_070`.
- Composition: template_4th glow text slice after M05/M06 text layout is stable.
- Stack/template: full text reveal slice with shadow + glow.

## M12 Geometry2

### Sequential Hypotheses

1. Parameter mapping: AE `0003`, `0004`, `0008` map to width/height/rotation in
   a specific order.
2. Matrix order: anchor, position, scale, skew/rotation compose differently.
3. Pixel center convention differs by 0.5 offset.
4. Sampler edge policy is transparent, clamp, or preserve background.
5. Adjustment-layer Geometry2 uses accumulated lower canvas as input.

### Preparatory Implementations

- Geometry2 matrix passport with raw params, resolved params, forward/inverse
  matrix, determinant, sample UV grid, OOB policy, and output hash.
- Candidate switch for param mapping and matrix order.
- Pixel-center candidate switch: integer center vs half-pixel center.
- Coordinate-field fixture with UV-readable colors and hard alpha borders.
- AE matrix/UV probe imports when available.

### Gates

- Isolated: `EFF_040` coordinate-field matrix/UV checks.
- Composition: Geometry2 inside `STK_030`.
- Stack/template: scenes_3rd adjustment stack after M13/M14/M15 are localized.

## M13 Minimax

### Sequential Hypotheses

1. Operation enum mapping: min, max, min-then-max, max-then-min.
2. Channel enum mapping: color, alpha, alpha+color, individual channels.
3. Direction enum mapping: horizontal+vertical, horizontal, vertical.
4. Radius handling: floor, round, ceil, or fractional morphology.
5. Edge behavior: shrink edges, clamp edges, transparent fill, or special
   `don't shrink edges` branch.

### Preparatory Implementations

- Morphology candidate engine with local enum switches for operation, channel,
  direction, radius mode, and edge mode.
- Intermediate sidecar: first pass, first stage, second stage, output hash,
  alpha stats, bbox.
- Fixture sweep: impulse, hard alpha edge, RGB edge, fractional radius values,
  edge-border cases.
- AE probe/golden matrix for operation/channel/direction combinations.

### Gates

- Isolated: `EFF_050`.
- Composition: `STK_020`.
- Stack/template: `STK_030` after M12/M14/M15 are not first-divergence owners.

## M14 Turbulent Displace

### Sequential Hypotheses

1. Field state: AE uses a two-path fractal/noise state rather than the current
   simple sine/noise approximation.
2. Evolution/time maps to fixed16 or normalized phase slots.
3. Complexity/octaves split into `FracAll`/`Frac1D` style paths.
4. H/V lookup size and interpolation policy determine displacement field maps.
5. Coordinate-field displacement then exposes sampler/OOB differences.

### Preparatory Implementations

- Field-map sidecar before sampling: displacement x/y grid, field hash, min/max,
  sample points, evolution state, octave count.
- Candidate switch for field generator path, phase/evolution mapping, octave
  amplitude/frequency, and lookup interpolation.
- Coordinate-field displacement fixture with alpha ramp and hard RGB edges.
- AE field probe that encodes displacement into readable output, or Ghidra-derived
  field maps when direct AE field export is impossible.
- Stack sidecar that separates field error from downstream sampling/composite.

### Gates

- Isolated: `EFF_060` field map first, final pixels second.
- Composition: coordinate-field displacement.
- Stack/template: `STK_030` only after field map and sampler are explainable.

## M15 Posterize Time

### Sequential Hypotheses

1. Bucket boundary uses floor at comp time.
2. Boundary epsilon differs at exact frame edges.
3. Posterize affects source/layer time but not live downstream params.
4. Adjustment-layer Posterize Time freezes lower-stack content while later effect
   params still sample live time, or vice versa for specific paths.

### Preparatory Implementations

- Temporal passport with comp frame, comp time, layer time, source time, bucket,
  bucket time, source frame index, and effect param time.
- Candidate switch for boundary rounding and affected time domains.
- Numbered-frame fixture sweep near boundaries.
- Stack fixture with animated source plus animated downstream effect param.

### Gates

- Isolated: `TMP_020`.
- Composition: `TMP_010` source offset plus Posterize.
- Stack/template: `STK_030` temporal contract.

## M16 Adjustment Layers / Effect Stack

### Sequential Hypotheses

1. Adjustment layer consumes accumulated lower canvas at its active time.
2. Effects execute in listed AE order with no hidden pre/post conversion except
   effect-local wrappers.
3. Posterize Time in an adjustment stack freezes a specific downstream slice.
4. Input/output hashes localize first divergent effect in mixed stacks.

### Preparatory Implementations

- Per-effect input/output checkpoint writer for adjustment stacks.
- Stack graph sidecar: layer order, active layers, adjustment coverage, effect
  order, param time per effect.
- Candidate switch for stack time semantics only in adjustment context.
- AE checkpoint probe plan for STK_030, or synthetic stack cases that isolate
  every adjacent pair.

### Gates

- Isolated: pairwise non-commutative stack cases.
- Composition: `STK_030` checkpoints.
- Stack/template: scenes_3rd adjustment layer slices.

## M17 Collapse Transformations / Precomp Graph

### Sequential Hypotheses

1. Current matrix pushdown is correct for simple nested transforms.
2. Text/vector children must defer rasterization through collapsed precomp
   boundaries.
3. Raster children remain rasterized and use parent matrix sampling.
4. AE collapse treats masks/effects as boundaries that may break pushdown.

### Preparatory Implementations

- Collapse passport with graph node ids, parent/child matrices, rasterization
  boundary reason, scale factor, and sharpness proxy.
- Candidate switch for text/vector deferred raster vs intermediate raster.
- AE collapsed vs non-collapsed pair fixtures with large parent scale.
- Pixel crop sharpness metric for text/vector edges.

### Gates

- Isolated: `GPH_010`.
- Composition: text precomp graph slices.
- Stack/template: template_4th/impulse_2nd nested text where present.

## M18 Motion Blur

### Sequential Hypotheses

1. Shutter sample times derive from angle/phase with uniform weights.
2. AE uses different sample count or adaptive samples by quality.
3. Static layers skip accumulation.
4. Accumulation happens in premult form and returns to straight RGBA8 output.
5. Source-frame sampling inside subframes follows M02/M15 source-time rules.

### Preparatory Implementations

- Motion-blur passport: shutter window, sample times, weights, source frame per
  sample, transform per sample, accumulated alpha/RGB stats.
- Candidate switch for sample schedule, sample count, weight shape, static skip,
  and accumulation mode.
- Moving impulse fixture with numbered background for source-time audit.
- AE shutter sample probe if possible; otherwise finite sweep against rendered
  impulse trails.

### Gates

- Isolated: `TMP_030`.
- Composition: animated transform with source frame selection.
- Stack/template: only if target payloads enable motion blur.

## M20 Masks / Mattes / Blend Modes

### Sequential Hypotheses

M20 is dormant for the current template target set unless payload inventory shows
mask, matte, or non-normal blend-mode usage.

When activated:

1. Track matte alpha/luma source mapping.
2. Mask path fill/feather/opacity modes.
3. Blend mode enum mapping for required modes only.
4. Preserve-alpha and matte flags from `Composite.cl` / `TrackMatte.cl`.

### Preparatory Implementations

- Payload inventory detector that reports masks, mattes, and blend modes.
- Track matte fixtures for alpha/luma/inverted variants.
- Blend-mode candidate table limited to observed modes.
- Mask path telemetry: path bbox, feather radius, alpha mask hash.

### Gates

- Isolated fixtures only after usage is detected.
- Composition/template gates only for templates that actually need M20.

## M21 Later Scope

M21 covers 3D, camera, spatial paths, roving keyframes, and full ExtendScript.
It is intentionally outside the current three-template parity path unless the
payload inventory changes.

Preparatory work should be limited to detection and explicit fallback reporting:

- Detect 3D layers/cameras/lights.
- Detect spatial tangents and roving keys.
- Detect arbitrary ExtendScript beyond named/fingerprint subset.
- Emit a clear unsupported-feature report instead of approximating silently.

## Acceptance Language

Use these labels consistently:

- `candidate-ready`: finite hypotheses exist and fixtures can distinguish them.
- `instrumented`: native telemetry exposes the relevant intermediate state.
- `isolated-improved`: isolated primitive telemetry and pixels moved toward AE.
- `composition-improved`: the module composition case moved toward AE without
  invalidating locked contracts.
- `stack-safe`: stack/template case did not regress beyond declared thresholds
  or any regression has a named owner.
- `reverse implemented`: accepted formula is backed by reverse/probe evidence,
  native tests, conformance output, and a lock note.
- `parity locked`: reverse implemented plus AE golden thresholds are enforced in
  CI for the claimed scope.

For now, most remaining modules should target `reverse implemented`, not full
`parity locked`.

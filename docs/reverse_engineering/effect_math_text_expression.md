# AE Text Animator, Glyph, And Expression Math Notes

Status date: 2026-05-04.

Scope for this note: text/glyph/expression reverse-engineering only. This pass
read existing fixtures, generated JSX, docs, tests, implementation files, and
target sidecars. No Ghidra/headless run was needed or performed.

Primary local evidence:

- `docs/TEXT_ANIMATOR.md`
- `docs/IR_SCHEMA.md`
- `docs/MATH_PARITY_STATUS.md`
- `docs/phase_reports/AGENT_ROUND2_TEXT_GLYPH_EXPR.md`
- `docs/phase_reports/AGENT_ROUND3_TEXT_GLYPHS.md`
- `docs/phase_reports/AGENT_ROUND3_EXPR_COLLAPSE.md`
- `docs/phase_reports/AGENT_ROUND4_TEXT_RASTER_ANIMATOR.md`
- `docs/phase_reports/AGENT_ROUND5_TEXT_ANIMATOR_BLUR.md`
- `docs/phase_reports/AGENT_5_TEXT_GLYPHS_EXPRESSIONS_NATIVE_DIFF.md`
- `docs/phase_reports/PHASE_5_TEXT_EXPRESSIONS_COLLAPSE.md`
- `docs/phase_reports/AGENT_ROUND7_TEXT_EXPR_COLLAPSE.md`
- `docs/phase_reports/AGENT_ROUND8_TEXT_EXPR_COLLAPSE.md`
- `fixtures/ae_conformance_pack/jsx/build_conformance_project.jsx`
- `fixtures/ae_conformance_pack/manifest.json`
- `fixtures/text_animator_adjustment_scene.json`
- `fixtures/text_expression_selector_scene.json`
- `crates/text-engine/src/text_animator.rs`
- `crates/expression-engine/src/evaluator.rs`
- `crates/render-core/src/layer_eval.rs`
- `crates/render-ir/src/schema.rs`
- `crates/ae-bridge/src/payload.rs`
- Current sidecars under `target/ae_agents/round8_os_integration`,
  `target/ae_agents/round5_text_animator_blur`, and
  `target/ae_agents/round3_final_integrated`.

## Observed Behavior

### Coverage Cases

The conformance pack currently anchors the text/expression/collapse work on:

| Case | Feature | Frames | Notes |
| --- | --- | --- | --- |
| `TXT_010` | Montserrat word reveal | 0, 8, 16, 24, 32, 45, 59 | Range Selector, opacity to 0, Based On Words. |
| `TXT_020` | Montserrat character and line reveal | 0, 8, 16, 24, 32, 45, 59 | Range Selector, opacity to 0, Based On Characters and Lines. |
| `TXT_030` | Point-Light glyph animator | 0, 8, 16, 24, 32, 45, 59 | Position, scale, rotation, blur, Range Selector. |
| `TXT_040` | Point-Light expression selector bounce | 0, 5, 10, 15, 20, 30, 45, 59 | Expression Selector driving scale to `[0,0,100]`. |
| `EXP_010` | generated position expression subset | 0, 5, 10, 15, 20, 30, 45, 59 | `edge_wobble` position expression. |
| `GPH_010` | collapsed text sharpness | 0, 15, 30, 45 | Text in collapsed precomp at parent scale 180%. |

`fixtures/ae_conformance_pack/manifest.json` says the composition is 512x512,
30 fps, 2 seconds per case. The selected AE golden PNG directories contain 60
frames for each Phase 5 case. The manifest currently lists both
`Montserrat-Italic[wght].ttf` and `Point-Light.ttf` in font assets, while older
notes mention Point-Light as an AE-machine assumption. Current sidecars show
Point-Light resolving by direct path in native runs:

```text
requested_id: fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf
resolved_postscript_name: Point-Light
source: DirectPath
fallback: false
```

### Glyph Metrics And Layout

Current native telemetry is good enough to compare most layout facts:

- resolved font path/family/style/postscript/fallback;
- font size;
- character and actual font glyph id;
- `char_index`, `word_index`, `line_index`;
- advance;
- glyph bbox;
- bbox center;
- bbox normalized against text box;
- baseline;
- line width;
- text box rect;
- raster scale and raster size;
- render path (`layer_text`, `collapsed_text_raster`, etc.).

Observed `TXT_030` frame 0, first glyph `G`:

```text
font: Point-Light direct path, fallback=false
font_size: 74
font_glyph_id: 42
advance: 59.330078125
bbox: [-34.0262756, 205.0, 54.0, 52.0]
bbox_center: [-7.0262756, 231.0]
bbox_normalized: [-0.06645757, 0.40039062, 0.10546875, 0.1015625]
baseline: 256.0
line_width: 586.0525513
text_box_rect: [0, 0, 512, 512]
```

Observed `TXT_040` frame 0 has an overfull centered line:

```text
text: BOUNCE SELECTOR
font_size: 64
line_width: 638.3875122
baseline: 256.0
first glyph bbox x: -57.1937561
last glyph bbox x: 538.1312256
```

Known layout behavior from phase reports:

- Exact Point-Light availability was the first blocker in early runs. It is no
  longer a current blocker when the direct fixture path resolves.
- The accepted native approximation centers overfull lines mathematically
  instead of clamping them to x=0.
- For Point-Light cases, the baseline now starts at the text box vertical
  center, giving `baseline=256.0` in the 512x512 fixtures.
- Fontdue/simple layout is still not AE/CoreText/paragraph-composer parity. It
  lacks shaping, kerning parity, composer decisions, and true AE glyph coverage.

### Range Selector Units

Native helper behavior:

- Characters are Unicode scalar units excluding CR/LF line breaks.
- Words are contiguous non-whitespace spans.
- Lines preserve source-order spans and include empty trailing lines.
- Stable source indices are preserved independently from selector order.
- Output units remain in source order even if randomize order changes selector
  rank.

Runtime sidecars show that `TXT_030` uses 11 character units for `GLYPH MOTION`.
The space is present in layout glyphs but excluded from animator unit rects.
`TXT_040` uses 12 character units for `BOUNCE SELECTOR`, likewise excluding the
space.

The AE JSX uses:

```js
selector.property("ADBE Text Range Advanced")
    .property("ADBE Text Range Type2")
    .setValue(basedOnCode);
```

Observed code values in fixtures:

- `1`: Characters.
- `3`: Words.
- `4`: Lines.

### Range Selector Shapes, Smoothness, Randomize, Wiggly

The current code has two related implementations:

- `crates/text-engine/src/text_animator.rs` has v2 helper primitives.
- `crates/render-core/src/layer_eval.rs` has the runtime path used for frames.

Current runtime selector behavior:

```text
ordered_index = randomize_order ? deterministic_order_index(index,total,seed) : index
pos = ((ordered_index + 0.5) / total) * 100
start = min(start_percent, end_percent)
end = max(start_percent, end_percent)
if pos outside [start,end]: weight = 0
t = clamp((pos - start) / max(end - start, 0.0001), 0, 1)

Square:
  if smoothness <= 0: 1
  else min((pos-start)/edge, (end-pos)/edge), edge=(end-start)*(smoothness/100)*0.5
RampUp:
  t
RampDown:
  1 - t
Triangle:
  clamp(1 - abs(2*t - 1), 0, 1)
Round:
  max(sin(pi*t), 0)
Smooth:
  t*t*(3 - 2*t)

Wiggly:
  phase = deterministic_unit_noise(index, seed)*TAU + time*frequency*TAU
  weight *= 1 + (amount/100)*sin(phase)
  final clamp to [0,1]
```

Current text-engine helper differs in details:

- Smoothness is a post-blend toward `smoothstep(raw_weight)`, not the runtime
  square edge-width model.
- Wiggly adds interpolated signed hash noise to the base weight, while runtime
  multiplies by a sine modulation.
- Randomize order uses a stable hash permutation in both paths, but helper seed
  type and hash constants differ from runtime.

Conclusion: helper/runtime selector semantics must be unified or compared in
telemetry before tuning AE parity. Current helper behavior is deterministic and
testable, not confirmed AE-equivalent.

### Animator Properties

The generated `TXT_030` JSX creates:

```js
props.addProperty("ADBE Text Position 3D").setValue([0, -72, 0]);
props.addProperty("ADBE Text Scale 3D").setValue([125, 125, 100]);
props.addProperty("ADBE Text Rotation").setValue(18);
props.addProperty("ADBE Text Blur").setValue([10, 10]);
selector start: 0 at t=0, 100 at t=2s
selector Based On: Characters
```

Runtime currently applies one transformed alpha-derived unit rectangle at a
time. For each unit:

```text
weight = range_weight * expression_weight
cx = (unit.x0 + unit.x1) * 0.5
cy = (unit.y0 + unit.y1) * 0.5

sx = (100 + (animator_scale_x - 100) * weight) / 100
sy = (100 + (animator_scale_y - 100) * weight) / 100
rotation = animator_rotation_z * weight
tx = animator_position_x * weight * unit_scale
ty = animator_position_y * weight * unit_scale
alpha_scale = animator_alpha_scale(animator_opacity, weight)
blur_radius = round(max(abs(blur_x),abs(blur_y)) * max(weight,0) * abs(unit_scale))
blur_radius = clamp(blur_radius, 0, 128)

M = T(cx + tx, cy + ty) * R(rotation) * S(sx, sy) * T(-cx, -cy)
```

The `TXT_030` frame 0 sidecar shows a triangle-like selector profile despite
`shape="square"` and `smoothness=100`:

```text
final weights, 11 units:
0.0909, 0.2727, 0.4545, 0.6364, 0.8182, 1.0,
0.8182, 0.6364, 0.4545, 0.2727, 0.0909
```

That comes from the runtime `square_selector_weight` smoothing edge model with
100% smoothness over the full selector span. AE parity for this transfer has
not been proven yet.

Text animator blur history:

- Earlier runtime divided blur by 8 and clamped to 0..4, producing radii only
  0 or 1 for `TXT_030`.
- Round 5 changed runtime to full weighted pixel radius, capped at 128.
- `TXT_030` frame 0 now reports blur radii 1..10.
- The top-edge spread moved closer to AE (`native bbox y` from 118 to 109 vs AE
  y around 106), but the kernel/composite still differs.

Current blur implementation is a square splat over transformed unit pixels. It
is not confirmed AE Text Blur. AE likely applies a higher-quality per-character
or glyph render blur in text renderer space, with different alpha/composite
behavior.

### Expression Selector Bounce

The generated `TXT_040` JSX creates an Expressible Selector:

```js
delay = 0.0500;
myDelay = delay*textIndex;
t = (time - inPoint) - myDelay;
if (t >= 0){
  freq = 2; amplitude = 100; decay = 8.0;
  s = amplitude*Math.cos(freq*t*2*Math.PI)/Math.exp(decay*t);
  s
} else { value }
```

Native recognition:

- `ae-bridge` maps expressions containing `myDelay`, `textIndex`,
  `Math.cos`, and `Math.exp` to `TextExpressionSelector::PerCharacterBounce`.
- `expression-engine` has a same-fingerprint shortcut.
- Runtime also has a direct `PerCharacterBounce` path.

Runtime formula:

```text
text_index = index + 1
t = (time - layer_start) - delay * text_index
if t < 0:
  raw_amount = 0
  weight = 0
else:
  raw_amount = amplitude * cos(freq*t*2*pi) / exp(decay*t)
  weight = clamp(raw_amount / 100, -2, 2)

final_weight = range_weight * expression_weight
```

Observed `TXT_040` frame 0:

```text
range_weight unit 0: 0.08333334
expression local_time_after_delay: -0.05
raw_amount: 0
expression_weight: 0
final_weight: 0
```

Observed `TXT_040` frame 15, selected units:

```text
textIndex 1: t=0.45, amount=2.2105, weight=0.0221
textIndex 5: t=0.25, amount=-13.5335, weight=-0.1353
textIndex 10: t=0.0, amount=100.0, weight=1.0
```

Important open behavior:

- AE frame 0 for `TXT_040` was reported as blank/transparent while native
  telemetry says zero expression weight leads to identity transform
  contribution. The visual result depends on how AE combines Expression
  Selector Amount, selector Amount, and animator scale `[0,0,100]`.
- Negative expression weights are currently allowed down to -2. For scale to
  `[0,0]`, a negative final weight expands above 100% because:

```text
scale_factor = (100 + (0 - 100) * final_weight) / 100
```

This is visible in frame 15 sidecars where negative weights produce scale
factors above 1.0. This may be AE-like for expression selectors, but needs AE
amount telemetry.

### Property Expression Subset

The generated `EXP_010` expression is:

```js
intro = 0.25; outro = 0.25; amp = 34; freq = 2.0;
edge = Math.min(time - inPoint, outPoint - time);
env = Math.max(0, Math.min(1, edge / intro));
value + [Math.sin(time * freq * 2 * Math.PI) * amp * env, 0];
```

Current `expression-engine` subset:

- semicolon-separated statements;
- simple `name = expression` locals;
- numeric literals and Vec2 literals;
- `+`, `-`, `*`, `/`, unary plus/minus, parentheses;
- Vec2 plus/minus scalar operations used by templates;
- variables: `time`, `inPoint`, `outPoint`, `thisLayer.inPoint`,
  `thisLayer.outPoint`, `thisComp.width`, `thisComp.height`,
  `thisComp.duration`, `thisComp.frameDuration`, `textIndex`, `textTotal`,
  `value`, `Math.PI`, plus explicit `ctx.vars`;
- calls: `Math.sin`, `Math.cos`, `Math.exp`, `Math.min`, `Math.max`, and short
  aliases `sin`, `cos`, `exp`, `min`, `max`;
- generated bounce selector fingerprint shortcut.

The property-expression telemetry currently uses a named `edge_wobble` mode.
Observed native samples:

| Frame | Time | Envelope | Offset | Sampled position |
| ---: | ---: | ---: | --- | --- |
| 0 | 0.000000 | 0.000000 | `[0,0]` | `[256,256]` |
| 5 | 0.166667 | 0.174847 | `[5.14836,-0.59054]` | `[261.14835,255.40947]` |
| 10 | 0.333333 | 0.000000 | `[0,0]` | `[256,256]` |
| 59 | 1.966667 | 0.295351 | `[-4.08442,1.05408]` | `[251.91559,257.05408]` |

The sidecar offset has a Y component even though the fixture expression shown in
JSX only adds `[sin(...)*amp*env, 0]`. That means the named native
`edge_wobble` runtime is not the exact same formula as the generated JSX string
or has additional template-specific wobble behavior. Treat this as a parity
unknown until AE property samples are exported.

### Collapse Text Interaction

`GPH_010` exercises normal rasterized precomp vs collapsed text:

```text
rasterized_precomp layer matrix:
  scale 1.8, translation [-310.8, -204.8], raster_size [512,512]

collapsed_precomp parent matrix:
  scale 1.8, translation [-98.8, -204.8]

collapsed_text_raster:
  child_matrix identity
  effective_matrix same as parent
  effective_raster_scale 1.800003
  raster_size [922,922]
  sharpness alpha_edge_max 255
  sharpness alpha_edge_mean 0.417653
  alpha_coverage_ratio 0.003835
```

Matrix propagation and scale-aware raster sizing appear active. Remaining
collapse uncertainty is true AE deferred/vector text rendering vs native
"rasterize text at effective scale then composite".

## Likely AE APIs And Modules

The generated JSX and payload importer identify these AE property groups and
match names as relevant:

```text
ADBE Text Properties
ADBE Text Document
ADBE Text Animators
ADBE Text Animator
ADBE Text Animator Properties
ADBE Text Selectors
ADBE Text Selector
ADBE Text Expressible Selector
ADBE Text Expressible Amount
ADBE Text Range Advanced
ADBE Text Range Type2
ADBE Text Percent Start
ADBE Text Percent End
ADBE Text Opacity
ADBE Text Position 3D
ADBE Text Scale 3D
ADBE Text Rotation
ADBE Text Blur
ADBE Transform Group
ADBE Position
ADBE Opacity
ADBE Scale
```

Likely renderer-side AE modules, inferred from behavior and public scripting
surface:

- TextDocument and paragraph composer for font, font size, center
  justification, line metrics, and text box placement.
- Text animator evaluator for range selector unit extraction, selector order,
  advanced shape/smoothness transfer, randomize order, wiggly modulation, and
  animator property combination.
- Text Expression Selector evaluator using AE expression context variables
  `textIndex`, `textTotal`, `time`, `inPoint`, `value`.
- Property expression evaluator for generated transform expressions.
- Collapse transformations/deferred text renderer that combines parent and
  child transforms before final vector/text rasterization.

## Native Implementation Targets

1. Keep telemetry as the contract for any tuning:
   font resolution, glyph rows, selector units, range/expression/final weights,
   animator contribution matrix, opacity scale, blur radius, and expression
   sampled value.

2. Unify selector helper/runtime formulas:
   either make render-core consume text-engine selector evaluation or document
   a single runtime formula and remove divergent helper assumptions.

3. Match AE unit extraction:
   confirm whether AE Range Selector Based On Characters excludes whitespace
   for animator unit rects in these fixtures, and whether spaces still affect
   `textIndex`/`textTotal`.

4. Replace or tune text animator blur:
   current square splat is useful but too broad/different in alpha. A better
   target is per-unit offscreen raster, AE-like blur kernel, then composite in
   unit/layer order with premultiplied alpha semantics.

5. Decide expression selector composition semantics:
   specifically frame-0 zero/negative amount behavior for scale-to-zero
   animators. This is the blocker for `TXT_040`.

6. Wire generic expression evaluator for supported generated strings:
   keep the subset narrow, but ensure the exact generated `EXP_010` source and
   the named `edge_wobble` path produce the same value unless deliberately
   representing different template families.

7. Keep collapsed text scale-aware:
   the current 1.8 raster scale evidence is good. The next target is comparing
   normalized glyph rows and alpha sharpness against AE, not changing matrix
   propagation first.

## Unknowns

- Exact AE selector transfer for `shape=Square` with `smoothness=100`. Current
  runtime produces a triangular profile over the whole span.
- Exact AE transfer for Ramp Up, Ramp Down, Triangle, Round, Smooth, especially
  with smoothness values other than 0/100.
- Exact randomize-order seed source. Runtime currently uses wiggly seed as the
  randomize seed when present.
- Exact Wiggly Selector model: additive vs multiplicative, phase, interpolation,
  temporal sampling, seed mapping, and interaction with range shape.
- Exact Expression Selector Amount behavior before delay: generated JSX returns
  `value` in the `else` branch, while native fingerprint currently returns 0.
  In AE, default Expressible Amount `value` may be 0, 100, or a selector-local
  inherited value depending on property defaults. Needs probe.
- Whether AE clamps expression selector Amount before or after multiplying by
  range selector Amount, and whether negative Amounts are clamped for scale,
  opacity, and blur.
- Exact animator property order when multiple properties are active:
  opacity, position, scale, rotation, blur may be applied in a text-renderer
  pipeline rather than current alpha-rect transform order.
- Exact text animator blur kernel and compositing.
- Glyph coverage/shaping parity against AE/CoreText for Point-Light and
  Montserrat variable italic. Current fontdue layout can match placement
  roughly but not guaranteed coverage/kerning.
- Whether `edge_wobble` should be x-only per JSX or 2D per native named mode.
- Collapse text parity after normalized glyph placement matches: AE may keep
  glyph/vector primitives live longer than native.
- Background alpha/compositing (`M19`) still affects image metrics and should be
  masked or fixed before interpreting text-only diffs.

## Needed Fixtures And Goldens

### AE Telemetry Probes

1. Range selector shape sweep:
   text `ABCDE`, exact font path, Based On Characters, Start/End fixed to
   narrow and full spans, Shape in Square/RampUp/RampDown/Triangle/Round/Smooth,
   Smoothness 0/25/50/75/100. Export per-character effective Amount or visual
   bars that make weights measurable.

2. Randomize order probe:
   10 characters, Start/End moving over time, Randomize Order on/off, multiple
   Random Seed values if AE exposes them. Export source index, selector rank,
   final amount.

3. Wiggly selector probe:
   fixed range selector with Wiggly Amount/Frequency/Seed sweeps, frames across
   at least 2 seconds. Export per-character weight over time.

4. Expression selector bounce probe:
   use the exact `TXT_040` expression and export `textIndex`, `textTotal`, raw
   expression result, default `value` in the `else` branch, post-selector Amount,
   and final scale per character for frames 0, 1, 2, 5, 10, 15.

5. Animator property separation probes:
   separate AE goldens for opacity-only, position-only, scale-only,
   rotation-only, blur-only, then combined. Use the same Point-Light text and
   same selector weights as `TXT_030`.

6. Blur kernel probe:
   one glyph or a rectangular alpha unit with Text Blur `[1,2,4,8,10,16]`,
   export alpha profiles or enough high-bit-depth PNG/TIFF data to fit kernel
   shape and premult behavior.

7. Glyph metrics probe:
   export AE sourceRect/textDocument data if available plus rendered alpha bbox
   for Point-Light and Montserrat, including overfull centered lines, spaces,
   newlines, punctuation, and empty trailing lines.

8. Property expression sample probe:
   AE-side sample of the exact `EXP_010` property expression at selected frames,
   recording property value before/after expression and all context variables.

9. Collapse text probe:
   paired rasterized/collapsed text precomps at parent scales 50/100/180/300%,
   with glyph-normalized placement and alpha sharpness metrics.

### Native Goldens/Checks

- Keep `fixtures/text_animator_adjustment_scene.json` as a smoke fixture for
  word reveal plus adjustment/effect stack interaction.
- Keep `fixtures/text_expression_selector_scene.json` as a smoke fixture for
  native `per_character_bounce`.
- Add compact unit tests for the final chosen selector formula once AE probes
  settle Square smoothness, non-square shapes, randomize, and wiggly.
- Add masked text metrics that ignore known transparent-background alpha
  differences while `M19` remains unsettled.
- Add JSON sidecar diff tooling for glyph rows and selector weights, because
  final PNGs hide first-divergence details.

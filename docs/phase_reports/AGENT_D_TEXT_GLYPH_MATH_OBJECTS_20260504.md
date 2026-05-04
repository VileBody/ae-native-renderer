# Agent D Text / Glyph / Animator / Selector Math Objects

Status date: 2026-05-04

Predecoded evidence root:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

## Scope

Assigned objects:

- `M05`: Text rasterization and glyph layout.
- `M06`: Text Range Selector reveal by characters, words, and lines.
- `M07`: Character text animator position, scale, rotation, opacity, and blur.
- `M08`: Expression selector bounce.

This pass is evidence/passport only. No code changes were made. The only output
written by this agent is this report.

## Evidence Table

| Evidence | Address / function / source | Confidence | Affected modules | Finding |
| --- | --- | ---: | --- | --- |
| `docs/phase_reports/MATH_OBJECT_AGENT_TZ_20260504.md` | Agent D task section | High | `M05`-`M08` | Defines required passports and guardrails: glyph metrics, selector units, animator properties, expression selector shortcut/v2 decision. |
| `target/reverse/predecoded/20260504_222748_full_predecode_round2/basic_text_aex/index.md` | `Basic_Text.aex`, 10 target candidates | High | `M05` | Reverse bundle targets include font/text interface loader functions, not final layout formula bodies. |
| `basic_text_aex/04_BasicText_candidate_19e10_180019e10/decompile.c` | `FUN_180019e10`, `0x180019e10` | High | `M05` | Loads `CTNewTextInterfaceV2` with `NewText`, `NewCPText`, `NewFontInstText`, `NewFontInstCPText`. Text construction delegates to CoolType text APIs. |
| `basic_text_aex/07_BasicText_candidate_1e630_18001e630/decompile.c` | `FUN_18001e630`, `0x18001e630` | High | `M05` | Loads `CTFontInstanceInterfaceV2` with `GetGlyphID`, `GetWidths`, `GetBBox`, `GetBaselineDeltas`, `ApplyFeatures`, `ProcessFeatures`, `GetBBoxes`, and raster-warning hooks. |
| `basic_text_aex/02_BasicText_candidate_19520_180019520/decompile.c` | `FUN_180019520`, `0x180019520` | High | `M05` | Loads older `CTFontInstanceInterface` with `ProcessFeaturesV2` in addition to glyph width/bbox/baseline APIs. |
| `basic_text_aex/08_BasicText_candidate_22280_180022280/decompile.c` | `FUN_180022280`, `0x180022280` | High | `M05` | Returns cached pointer for `CTFontInstanceInterface`; same metric/shaping API set as `0x180019520`. |
| `basic_text_aex/09_BasicText_candidate_22650_180022650/decompile.c` | `FUN_180022650`, `0x180022650` | High | `M05` | Returns cached pointer for `CTFontInstanceInterfaceV2`; same metric/shaping API set as `0x18001e630`. |
| `target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/index.md` | `Scripting.aex` targets | High | `M08`, shared with `M09`/`M15` | Shows delayed ExtendScript host functions plus BEE/time bridge data pointers. Generic expression semantics live outside current renderer math. |
| `scripting_expression_host/04_Scripting_DelayLoad_ExtendScript_AcquireEngine_180bdbf04/decompile.c` | `DelayLoad_?AcquireEngine...`, `0x180bdbf04` | Medium | `M08` | Thin delayed-load wrapper into ExtendScript initialization. Evidence supports treating arbitrary expression selectors as hidden-host scope, not formula tuning. |
| `scripting_expression_host/14_Scripting_PTR_BEE_CompToLayerTime_180e16f60/data_target.md` | Data pointer `0x180e16f60` | High | `M08`, `M09`, `M15` | Expression evaluation can call BEE comp-to-layer time conversion. Bounce selector time must use layer-local expression context, not only raw comp time. |
| `docs/reverse_engineering/effect_math_text_expression.md` | Text/expression notes | High | `M05`-`M08` | Summarizes current native telemetry, selector formulas, animator transform formula, blur approximation, expression selector shortcut, and unknowns. |
| `docs/TEXT_ANIMATOR.md` | Text animator notes | High | `M06`, `M07`, `M08` | Documents current supported subset and explicitly labels selector shapes, smoothness, randomize, wiggly, blur, and bounce selector as approximations. |
| `docs/phase_reports/AGENT_TEXT_FONT_TELEMETRY.md` | Font telemetry report | High | `M05` | Point-Light direct-path font resolves with `fallback=false`; glyph id, advance, bbox, baseline, line width, and text box telemetry exist. |
| `docs/phase_reports/AGENT_ROUND5_TEXT_ANIMATOR_BLUR.md` | Blur/application report | High | `M07`, `M19` | Text Blur currently uses weighted pixel radius and square splat; top spread improved but alpha/kernel/composite still differs from AE. |
| `fixtures/ae_conformance_pack/manifest.json` | Cases `TXT_010`-`TXT_040`, `GPH_010` | High | `M05`-`M08`, `M17`, `M19` | Existing pack covers word reveal, character/line reveal, glyph animator, expression selector bounce, and collapsed text sharpness. |
| `fixtures/ae_conformance_pack/jsx/build_conformance_project.jsx` | `addRangeAnimator`, `addGlyphAnimator`, `addBounceExpressionAnimator` | High | `M06`-`M08` | Confirms AE match names and fixture parameters: Range Type2 values, glyph animator properties, and exact bounce expression string. |
| `fixtures/ae_conformance_pack/analysis/block_cards.json` | `text_reveal`, `glyph_animator_point_light`, `expression_selector_bounce` | Medium | `M05`-`M08` | Lists next telemetry fields and AE probe cases needed before formula tuning. |

## Object Passports

### `M05` Text Rasterization And Glyph Layout

Parameter mapping:

- Text document: text string, font family/path, font size, fill, justification,
  text box rect, line height/composer choices.
- Required glyph row fields: `glyph_id`, source `char_index`, `word_index`,
  `line_index`, advance, bbox, bbox center, baseline, line width, text box rect,
  raster scale, raster size, font resolution source/fallback flag.
- CoolType evidence adds required hidden fields: font matrix, writing direction,
  encoding, design vector, OpenType feature application/processing, vertical
  glyph metrics, batch bboxes, and raster warning state.

Coordinate/time/color space:

- Layout coordinates are layer/text-box pixel coordinates with baseline in text
  layer space. Current fixture text boxes are 512x512, centered; observed
  Point-Light baseline is `256.0`.
- Rasterization must be scale-aware when collapsed/precomp rendering defers text
  scale (`GPH_010` uses effective raster scale around `1.8`).
- Color/alpha policy is not owned by `M05`; text raster output should expose
  straight alpha coverage and defer composite interpretation to `M19`.

Sampling rule:

- Layout is sampled at layer/effect expression time for the text document and
  animator properties. Static fixtures currently use fixed text documents.
- Glyph metrics must be computed before selector/animator evaluation, then
  animator transforms consume stable unit rects or glyph primitives.

Alpha/premult policy:

- Current native text output is interpreted on the straight RGBA8 substrate.
- Formula tuning for glyph alpha edges and text blur must wait for `M19`
  alpha/premult policy, because final PNG diffs are biased by global composite.

Formula / pseudocode target:

```text
font_instance = CoolType-like font resolution(font, size, design_vector)
text_obj = CTNewTextInterfaceV2.NewText/NewFontInstText(...)
for shaped run in composer(text_obj):
  glyph_ids = CTFontInstance.GetGlyphIDs / ProcessFeatures(...)
  advances = CTFontInstance.GetWidths(glyph_ids)
  bboxes = CTFontInstance.GetBBoxes(glyph_ids)
  baselines = CTFontInstance.GetBaselineDeltas(...)
  emit glyph rows in layer text-box coordinates
raster_scale = effective_matrix_scale when collapse/deferred text applies
rasterize glyph coverage at raster_scale
```

Native implementation delta:

- Current native uses fontdue/simple layout and telemetry. It is deterministic
  and good enough for controlled Point-Light/Montserrat fixtures, but not AE
  parity for shaping, kerning, composer decisions, vertical metrics, feature
  processing, or CoolType glyph raster coverage.
- Existing direct-path Point-Light resolution removes the old missing-font
  blocker for fixture execution, but not the CoolType metric parity blocker.

Test/probe needed next:

- AE glyph metrics probe with Point-Light and Montserrat: sourceRect/textDocument
  values if available, alpha bbox, line width, baseline, overfull centered line,
  spaces, punctuation, newlines, and empty trailing lines.
- Native sidecar diff against AE glyph rows: font identity, glyph IDs, advances,
  bboxes, baselines, text box, line widths, raster scale.

### `M06` Text Range Selector Reveal

Parameter mapping:

- Match names: `ADBE Text Animator`, `ADBE Text Selector`,
  `ADBE Text Percent Start`, `ADBE Text Percent End`,
  `ADBE Text Range Advanced`, `ADBE Text Range Type2`.
- Fixture `Range Type2` values: `1 = Characters`, `3 = Words`, `4 = Lines`.
- Required selector fields: based-on unit type, start percent, end percent,
  offset if present, shape, smoothness, amount, randomize order, random seed,
  wiggly amount/frequency/seed.

Coordinate/time/color space:

- Selector units are text-layer logical units mapped back to glyph/unit rects.
- Characters are currently Unicode scalar units excluding CR/LF. Words are
  contiguous non-whitespace spans. Lines preserve source-order spans and include
  empty trailing lines.
- Spaces affect layout and line width; current sidecars show spaces excluded
  from character animator unit rects for `TXT_030`/`TXT_040`.

Sampling rule:

- Sample animated start/end and advanced selector properties at layer time.
- Compute selector rank from source units, then return output weights in source
  order so downstream glyph mapping remains stable.

Alpha/premult policy:

- Selector itself has no alpha policy. Its weight drives opacity/transform/blur
  in `M07`, which then touches `M19`.

Formula / pseudocode target:

```text
units = extract_units(text, based_on)
rank = randomize ? deterministic_ae_rank(source_index, total, seed) : source_index
pos = ((rank + 0.5) / total) * 100
lo = min(start, end)
hi = max(start, end)
if pos outside [lo, hi]:
  base = 0
else:
  t = clamp((pos - lo) / max(hi - lo, epsilon), 0, 1)
  base = ae_shape_transfer(shape, t, smoothness)
weight = apply_amount(base, amount)
weight = apply_wiggly_if_enabled(weight, source_index, time, amount, frequency, seed)
emit source_order(source_index, rank, weight)
```

Native implementation delta:

- Runtime and text-engine helper formulas diverge today. Runtime uses a square
  edge-width smoothing model that gives `TXT_030` frame 0 a triangular-like
  profile at smoothness `100`; helper smoothness blends toward smoothstep.
- Randomize and wiggly are deterministic approximations, not confirmed AE
  models. Seed source and additive/multiplicative wiggly behavior remain open.

Test/probe needed next:

- Range selector shape sweep: characters/words/lines, start/end boundary frames,
  shape Square/RampUp/RampDown/Triangle/Round/Smooth, smoothness 0/25/50/75/100.
- Randomize-order probe with multiple seeds and explicit source index vs rank.
- Wiggly probe with amount/frequency/seed over at least two seconds.

### `M07` Character Text Animator

Parameter mapping:

- Match names: `ADBE Text Opacity`, `ADBE Text Position 3D`,
  `ADBE Text Scale 3D`, `ADBE Text Rotation`, `ADBE Text Blur`.
- Fixture `TXT_030`: position `[0, -72, 0]`, scale `[125, 125, 100]`,
  rotation `18`, blur `[10, 10]`, selector start animated `0 -> 100`,
  based on characters.
- Required telemetry: unit/glyph rect, selector weight, expression weight if
  present, final weight, transform origin, matrix, opacity scale, blur radius,
  raster/composite hash.

Coordinate/time/color space:

- Current runtime origin is the unit rect center:
  `cx=(x0+x1)/2`, `cy=(y0+y1)/2`.
- Transform order currently used:
  `T(cx + tx, cy + ty) * R(rotation) * S(scale) * T(-cx, -cy)`.
- Position is layer-space pixel delta, rotation is Z degrees, scale is percent
  interpolation from `[100,100]` to animator scale.

Sampling rule:

- Sample animator properties and selector weights at layer time.
- Apply per-unit/per-glyph transform to text alpha contribution. Current
  implementation transforms one alpha-derived unit rectangle at a time.

Alpha/premult policy:

- Opacity and blur affect alpha and therefore depend on `M19`.
- Text Blur currently uses a weighted pixel radius with square splat blur. This
  is not proven to match AE text-renderer blur or composite order.

Formula / pseudocode target:

```text
weight = range_weight * expression_weight
sx = lerp(100, animator_scale_x, weight) / 100
sy = lerp(100, animator_scale_y, weight) / 100
rz = animator_rotation_z * weight
tx = animator_position_x * weight
ty = animator_position_y * weight
opacity_scale = ae_opacity_transfer(animator_opacity, weight)
origin = ae_text_animator_origin(unit/glyph, anchor_group, line_context)
M = T(origin + [tx, ty]) * R(rz) * S(sx, sy) * T(-origin)
blur = ae_text_blur_kernel(blur_x, blur_y, weight, raster_scale)
composite transformed glyph coverage in AE text renderer order
```

Native implementation delta:

- Current origin is unit center, not confirmed AE text animator origin.
- Current renderer transforms alpha rectangles, not shaped glyph outlines.
- Blur is a square splat approximation. Round 5 removed the old `/8` shrink and
  `0..4` clamp; frame 0 top spread moved closer to AE, but kernel/composite
  still diverges.
- Property order with opacity, transform, and blur combined is not proven.

Test/probe needed next:

- Separate AE probes for opacity-only, position-only, scale-only, rotation-only,
  blur-only, then combined, all with the same Point-Light text and known
  selector weights.
- Text Blur alpha profile probe for blur `[1,2,4,8,10,16]` on one glyph and on
  a rectangular text unit, high-bit-depth if possible.
- Telemetry diff for transform origin and per-glyph matrices, not only final
  PNG.

### `M08` Expression Selector Bounce

Parameter mapping:

- Match names: `ADBE Text Expressible Selector`,
  `ADBE Text Expressible Amount`, `ADBE Text Scale 3D`.
- Fixture `TXT_040` exact expression:

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

Coordinate/time/color space:

- Expression selector variables must include at least `time`, `inPoint`,
  `textIndex`, `textTotal`, and `value`.
- Scripting evidence shows expression host time bridges exist, including
  `BEE_CompToLayerTime`; selector time should be layer expression time, not
  blindly comp time.

Sampling rule:

- Evaluate expression per selector unit/glyph at render sample time.
- `textIndex` is 1-based. Current native uses `index + 1`.
- Final selector amount may be negative and may exceed `0..100`; AE clamp/order
  is unknown.

Alpha/premult policy:

- Bounce itself is scalar. It drives scale-to-zero in `TXT_040`, so visual alpha
  changes come through `M07` transform/raster/composite and then `M19`.

Formula / pseudocode target for current named shortcut:

```text
text_index = source_unit_index + 1
t = (layer_time - in_point) - delay * text_index
if t >= 0:
  raw_amount = amplitude * cos(freq * t * TAU) / exp(decay * t)
else:
  raw_amount = ae_default_value_for_expression_selector_amount(value)
expression_weight = ae_amount_to_weight(raw_amount)
final_weight = range_weight * expression_weight
```

Native implementation delta:

- Native recognizes the generated bounce expression by fingerprint and maps it
  to `PerCharacterBounce`.
- Current shortcut returns `0` before delay, while the JSX says `else { value }`.
  AE default `value` for Expressible Amount is not known.
- Current shortcut clamps `raw_amount / 100` to `[-2, 2]` and allows negative
  weights. Negative scale weights expand glyphs above 100%; this may be AE-like
  but is unproven.

Decision:

- For current template parity, keep the named `PerCharacterBounce` shortcut as
  the short-term route. It is narrower and safer than importing arbitrary
  ExtendScript semantics.
- Do not require expression evaluator v2 for this exact fixture until AE
  telemetry proves that the shortcut cannot represent `value`, clamp/order, or
  time-context semantics.
- Expression evaluator v2 becomes required if payload inventory contains
  non-fingerprinted expression selector strings, if the generated string varies
  structurally, or if AE behavior depends on broader JavaScript semantics beyond
  this closed formula.

Test/probe needed next:

- Exact `TXT_040` probe exporting `textIndex`, `textTotal`, raw expression
  result, default `value` in the else branch, post-selector amount, final scale,
  frames `0,1,2,5,10,15`.
- Parameter sweeps for delay, frequency, decay, positive/negative amount, and
  interaction with selector order.

## Guardrail Findings

```text
BLOCKER:
  title: M05 glyph metrics depend on hidden CoolType text/font interfaces
  affected objects: M05, and downstream M06/M07/M08 through unit rects
  evidence paths:
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/basic_text_aex/04_BasicText_candidate_19e10_180019e10/decompile.c
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/basic_text_aex/07_BasicText_candidate_1e630_18001e630/decompile.c
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/basic_text_aex/02_BasicText_candidate_19520_180019520/decompile.c
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/basic_text_aex/08_BasicText_candidate_22280_180022280/decompile.c
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/basic_text_aex/09_BasicText_candidate_22650_180022650/decompile.c
  exact address/function:
    - 0x180019e10 FUN_180019e10 loads CTNewTextInterfaceV2
    - 0x18001e630 FUN_18001e630 loads CTFontInstanceInterfaceV2
    - 0x180019520 FUN_180019520 loads CTFontInstanceInterface
    - 0x180022280 FUN_180022280 returns cached CTFontInstanceInterface pointer
    - 0x180022650 FUN_180022650 returns cached CTFontInstanceInterfaceV2 pointer
  what assumption broke: fontdue/simple layout is not an AE-shaped metric source for parity tuning; AE exposes/uses CoolType APIs for glyph IDs, widths, bboxes, baseline deltas, feature processing, writing direction, and raster warning state.
  hypotheses:
    - Current native layout can remain an approximation for controlled fixtures.
    - AE parity needs either a CoolType-equivalent shaping/metrics layer, HarfBuzz/CoreText-backed probe mapping, or AE-exported glyph telemetry for target templates.
  smallest probe/test needed: AE glyph metrics export for Point-Light and Montserrat, including glyph ids if accessible, advances, bboxes, baselines, line widths, sourceRect, text box, spaces/newlines, and alpha bbox.
  can continue on unrelated work: yes; M06-M08 can keep deterministic telemetry and probes, but no M05 formula tuning should start.
```

```text
BLOCKER:
  title: Text Blur and glyph alpha interpretation depend on global alpha/composite policy
  affected objects: M07, M05 raster alpha, M19
  evidence paths:
    - docs/phase_reports/AGENT_ROUND5_TEXT_ANIMATOR_BLUR.md
    - docs/reverse_engineering/effect_math_text_expression.md
    - docs/MATH_PARITY_STATUS.md
  exact address/function: no Basic_Text.aex blur formula recovered in this bundle; native evidence is render-core text animator blur path documented in Round 5.
  what assumption broke: final-frame text blur diffs cannot be interpreted as only a text-kernel issue while M19 alpha/premult/composite is unsettled.
  hypotheses:
    - AE Text Blur may blur glyph coverage in text-renderer space, then composite with a different alpha policy than the current square splat.
    - Kernel fitting should be done on alpha profiles and masked text hashes after M19 substrate contract is locked.
  smallest probe/test needed: one-glyph Text Blur alpha profile probe plus M19 straight/premult audit; compare alpha-only bbox/profile before RGB tuning.
  can continue on unrelated work: yes; do not tune M07 blur kernel from final RGB frames alone.
```

```text
BLOCKER:
  title: Range selector unit boundaries and transfer functions are not AE-proven
  affected objects: M06, M07, M08
  evidence paths:
    - docs/TEXT_ANIMATOR.md
    - docs/reverse_engineering/effect_math_text_expression.md
    - fixtures/ae_conformance_pack/jsx/build_conformance_project.jsx
  exact address/function: AE scripting match names in addRangeAnimator; no native Basic_Text selector function recovered in this bundle.
  what assumption broke: current model excludes spaces from character animator units and uses approximate Square/smoothness/wiggly/randomize formulas; AE unit boundaries and transfer may differ.
  hypotheses:
    - Current choices are good enough for existing smoke templates.
    - AE may count spaces for textIndex/textTotal even when unit rects are blank, and may use a different smoothness transfer.
  smallest probe/test needed: AE selector telemetry/visual-bars probe for characters, words, lines, spaces, randomize order, wiggly, and smoothness sweep.
  can continue on unrelated work: yes; selector telemetry can be added and compared before formula tuning.
```

```text
BLOCKER:
  title: Expression selector bounce has hidden value/clamp/time semantics
  affected objects: M08, shared expression evaluator scope with M09/M15
  evidence paths:
    - fixtures/ae_conformance_pack/jsx/build_conformance_project.jsx
    - docs/reverse_engineering/effect_math_text_expression.md
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/index.md
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/14_Scripting_PTR_BEE_CompToLayerTime_180e16f60/data_target.md
  exact address/function:
    - 0x180bdbf04 DelayLoad_?AcquireEngine@extendscript...
    - 0x180e16f60 PTR_?BEE_CompToLayerTime...
  what assumption broke: the generated JSX returns `value` before delay and may use AE expression-selector defaults/clamps that the named shortcut currently replaces with zero.
  hypotheses:
    - Named shortcut remains enough if AE telemetry can be represented by the closed bounce formula plus corrected `value`/clamp/time rules.
    - Full expression evaluator v2 is needed only when selector expressions are not fingerprinted or require broader ExtendScript/BEE semantics.
  smallest probe/test needed: AE export of raw expression result, default `value`, post-selector amount, and final glyph scale for exact TXT_040 frames 0/1/2/5/10/15.
  can continue on unrelated work: yes; keep shortcut for template parity but do not claim M08 formula tuning until this probe lands.
```

## Recommendation

Final recommendation: `blocked_by_guardrail` for `M05` metric parity and `M07`
blur formula tuning; `needs_probe` for `M06` selector transfer and `M08`
expression selector semantics.

Operational ladder:

1. Keep current native text modules as `implemented approximate` with telemetry.
2. Do not tune glyph layout against AE final frames until the CoolType/CoreText
   metric blocker is resolved or narrowed to target-font probes.
3. Add AE probes for glyph metrics, selector weights, animator origins/matrices,
   blur alpha profiles, and expression selector amount semantics.
4. Keep `PerCharacterBounce` as a named shortcut for current template parity,
   but correct it only from AE telemetry. Defer expression evaluator v2 unless
   payload inventory or probe results require broader semantics.
5. After M19 locks alpha/premult policy and the probes above exist, promote
   `M06`/`M07`/`M08` from approximation toward formula tuning.

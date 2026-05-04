# Agent C: Temporal / Adjustment / Property Expressions

Status date: 2026-05-04

## Scope

Assigned math objects: `M09`, `M15`, `M16`.

Read-only evidence pass over:

- `target/reverse/predecoded/20260504_222748_full_predecode_round2/posterize_time/`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/time_displace_temporal/`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/`
- `docs/reverse_engineering/effect_math_temporal_noise_distort.md`
- `docs/MATH_PARITY_STATUS.md`

No renderer code was changed. This report is the only write in this pass.

## Evidence Table

| Evidence | Address / function | Confidence | Affected modules |
| --- | --- | --- | --- |
| `posterize_time/index.md` maps `Posterize_helper_bucket` to `FUN_180001350`, `Posterize_effect_main` to `EffectMainExtra`, and frame-rate checkout to `FUN_180006220`. | `Posterize_Time.aex` `0x180001350`, `0x180001690`, `0x180006220` | high | `M15`, `M16` |
| `Posterize_helper_bucket` checks out param `0001`, multiplies the returned fixed value by `0x3ef0000000000000` (`1/65536`), computes frame duration as `time_scale / fps`, truncates `current_time / frame_duration`, and checkouts input at the bucket PF time. | `posterize_time/01_Posterize_helper_bucket_180001350/decompile.c`; disasm `0x1800013ab`, `0x18000142e`..`0x180001450`, checkout call `0x18000148c` | high | `M15` |
| No additive epsilon constant appears in Posterize bucket math; boundary tolerance is native-renderer seconds-space hygiene, not recovered AE plugin math. | `posterize_time/01_Posterize_helper_bucket_180001350/disassembly.txt` | medium-high | `M15`, tests |
| Frame-rate checkout uses PF checkout for param index `1` at current PF time and returns the checked-out fixed16 value. | `posterize_time/04_Posterize_frame_rate_checkout_180006220/decompile.c`; call at `0x18000627f` equivalent in decompile | high | `M15` |
| `EffectMainExtra` case `0x0b` routes through `FUN_180001350`; case `0x17` repeats the bucket calculation for smart-render checkout. | `posterize_time/02_Posterize_effect_main_180001690/decompile.c` cases `0x0b`, `0x17` | high | `M15`, `M16` |
| `Time_Displace.aex` performs source checkout at computed per-bucket/per-map PF times, proving source sampling time can be effect-internal and distinct from effect parameter time. | `time_displace_temporal/04_TimeDisplace_core_2100_180002100/decompile.c`; checkout call inside loop around `0x1800023f3`; helpers `0x180001000`, `0x1800013c0` | medium | `M15`, `M16`, temporal telemetry |
| `TimeDisplace` helpers build time lookup tables from current PF time (`+0xe0`), time scale (`+0xf0`), duration/end (`+0xe8`), and displacement map values. | `time_displace_temporal/01_TimeDisplace_helper_1000_180001000/decompile.c`; `02_TimeDisplace_helper_13c0_1800013c0/decompile.c` | medium | `M16`, downstream effects with temporal sampling |
| Scripting host has data-only bridge pointers for AE/BEE time conversion. | `scripting_expression_host/09_.../data_target.md`, `10_...`, `11_...`, `14_...` | high | `M09`, future expression v2 |
| `BEE_FpLongToTime` has broad incoming references; `BEE_CompToLayerTime` has a specific incoming caller, indicating expression host time is layer-contextual rather than a plain global `time` scalar. | `Scripting.aex` data targets at `0x180e16cb0`, `0x180e16f60` | medium-high | `M09` |
| Current native status: `M09` approximate named position-expression mode plus small scalar/Vec2 evaluator; `M15` approximate true temporal behavior; `M16` instrumented/testable adjustment pipeline. | `docs/MATH_PARITY_STATUS.md` | high | `M09`, `M15`, `M16` |
| Existing temporal notes record the resolved `STK_030` over-posterize model: lower stack at bucket time, downstream effects after Posterize at comp time. | `docs/reverse_engineering/effect_math_temporal_noise_distort.md`; `docs/phase_reports/AGENT_TEMPORAL_STACK_TELEMETRY.md` | high | `M15`, `M16` |

## Object Passports

### `M15` Posterize Time True Temporal Behavior

Parameter mapping:

- `ADBE Posterize Time-0001`, `0001`, `frameRate`, `frame_rate`, `Frame Rate` map to Posterize FPS.
- Native AE plugin stores/checks out FPS as fixed16.16; recovered multiplier is `1/65536`.
- Values `<= 0` or non-finite should remain native pass-through in our renderer; AE UI likely constrains the slider, but imported payloads can still be defensive.

Coordinate/time/color space:

- Pure time operator. It does not alter pixels at the stateless canvas stage.
- Plugin math works in PF integer time units: `current_time`, `time_scale`, and checked-out FPS.

Sampling rule:

```text
fps = checked_out_0001_fixed / 65536.0
frame_duration_pf = time_scale / fps
bucket_pf = trunc(current_time_pf / frame_duration_pf) * frame_duration_pf
checkout input at bucket_pf with original time_scale
```

In renderer seconds-space this remains:

```text
bucket_id = floor(time_seconds * fps + tolerance)
bucket_time = bucket_id / fps
```

Boundary epsilon:

- Ghidra evidence does not show an AE epsilon constant.
- The current `+1e-9` renderer tolerance is still reasonable to prevent binary-float underflow at exact frame boundaries, but it should be documented as native tolerance, not native AE formula evidence.
- Next probe should render exact edge, one-tick-before-edge, and one-tick-after-edge samples at 6 fps and 30 fps using numbered frames.

Alpha/premult policy:

- Not applicable directly; output is whatever the checked-out input frame returns.

Native implementation delta:

- Current `effects::posterize_time::quantize_time` matches floor-bucket seconds behavior.
- The subtle delta is documentation/test language: explicit epsilon is not confirmed in AE; PF-time truncation is confirmed.
- Multiple Posterize Time effects are still only modeled as sequential quantization for ordinary layers; no direct AE proof was found for multi-Posterize stacks.

Test/probe needed next:

- `TMP_020` should keep exact numbered-frame source-frame telemetry: `comp_time`, `posterized_time`, `source_time`, `source_frame_id`.
- Add boundary probe at exact bucket transitions and immediately adjacent PF ticks.
- Keep motion-blur/posterize sample probes separate from this object.

### `M16` Adjustment Layer Pipeline And Effect-Stack Order

Parameter mapping:

- Adjustment layer itself has no math parameter here; the relevant parameters belong to effects in the stack.
- `ADBE Posterize Time` inside an adjustment stack contributes `frame_rate`, `bucket`, and `bucket_time` to routing.

Coordinate/time/color space:

- Adjustment input is the already-lower layer stack rendered into a canvas.
- Lower-stack time, effect parameter time, and downstream post-effect time must stay separate.

Sampling rule:

Current contract to keep:

```text
comp_time = frame / comp_fps
first_pt = first ADBE Posterize Time in adjustment effects

if first_pt exists:
    lower_stack_time = quantize(comp_time, first_pt.fps)
else:
    lower_stack_time = comp_time

input_canvas = render_lower_stack_at(lower_stack_time)

for each adjustment effect:
    if first_pt exists and effect_index <= first_pt.index:
        param_time = first_pt.bucket_time
    else:
        param_time = comp_time
    output_canvas = render_effect(input_canvas, param_time)
```

Alpha/premult policy:

- Consumes the global substrate contract from `M19`; this pass found no new alpha/premult evidence.

Native implementation delta:

- The old `STK_030` over-posterize risk is resolved at the routing level: downstream effects after Posterize Time can evaluate animated params at live comp time while lower-stack input is frozen.
- Remaining `STK_030` error should be attributed first to Geometry2/Minimax/Turbulent/substrate formulas unless new adjustment telemetry contradicts this routing.
- `Time_Displace.aex` shows a stronger temporal pattern to preserve in telemetry: an effect may internally checkout source frames at times derived from its own map while its animated params are evaluated at the effect invocation time.

Test/probe needed next:

- Keep `STK_030` as composed regression only after isolated `M12`, `M13`, `M14`, `M19` checks.
- Add a non-commuting adjustment stack probe: `Geometry2 -> Posterize Time -> animated Minimax -> animated Turbulent`, with per-effect input/output hashes.
- Add a Time Displacement-style micro-probe later if temporal source checkout becomes a renderer feature.

Telemetry checkpoints:

- Source frame index: `source_time`, `source_frame_id`, `posterized_time`.
- Effect param time: `AdjustmentEffectTrace.param_time` and per-effect debug `effect_time`.
- Adjustment input hash: per-effect `input_hash`, plus `lower_stack_time`.
- Post-effect hashes: per-effect `output_hash`, and final frame hash.

### `M09` Property Expression Subset: Generated `edge_wobble`

Parameter mapping:

- Payload recognition maps generated transform-position expressions to `PositionExpression::EdgeWobble`.
- Recognized source fingerprint currently requires `var intro=`, `var outro=`, `var amp=`, `var freq=`, and `Math.exp(-2.4`.
- IR stores `intro`, `outro`, `amp`, `freq`, and original source string.

Coordinate/time/color space:

- Transform position expression; output is a 2D position delta/value in layer transform space.
- Time context must be layer-aware: AE host pointers include `BEE_CompToLayerTime` and `BEE_FpLongToTime`, so future generic expression evaluation must not treat `time` as a context-free comp scalar.

Sampling rule:

Current named shortcut behavior:

```text
t = max(0, comp_time - layer_start)
dur = max(layer_duration, frame_duration)

if t < intro:
    p = clamp(t / intro, 0, 1)
    env = sin(pi * p) * exp(-2.4 * p)
else if t > dur - outro:
    p = clamp((dur - t) / outro, 0, 1)
    env = sin(pi * p) * exp(-2.4 * p)
else:
    env = 0

x = amp * env * sin(2*pi*freq*t)
y = amp * 0.68 * env * cos(2*pi*(freq*0.82)*t)
position = base_position + [x, y]
```

Evaluator vs named shortcut responsibility:

- The named `edge_wobble` fingerprint is responsible for current `scenes_3rd` template coverage.
- The small expression evaluator is useful for generated scalar/Vec2 expressions, but it is not proof of AE Expression Engine v2 parity.
- Generic AE expression evaluator scope belongs behind a separate BEE/Scripting host contract: time conversion, layer context, property access, numeric/vector coercion, and delayed expression behavior.
- The current named shortcut is allowed to remain template-scoped, but its formula must be validated against AE property samples because existing docs note a mismatch risk: fixture JSX can be x-only, while native named mode emits both x and y offsets.

Alpha/premult policy:

- Not applicable directly; expression affects transform before raster/effect sampling.

Native implementation delta:

- `edge_wobble` should not be promoted as generic expression support.
- The next delta is not parser breadth; it is AE sample parity for generated `edge_wobble` values at the selected frames.

Test/probe needed next:

- Export AE property samples for `EXP_010` and the eight `scenes_3rd` footage layers: base position, expression result, final position, layer in/out points, and comp/layer time.
- Add a probe that distinguishes exact generated JSX `[x, 0]` from the named shortcut `[x, y]`.
- Add a time-context probe where layer start is nonzero to validate `time`, `inPoint`, `outPoint`, and comp-to-layer conversion.

Telemetry checkpoints:

- `property_expression_value`: source mode, input `value`, comp time, layer-local time, in/out points, context vars, raw Vec2/scalar.
- `edge_wobble_position`: base position, envelope, offset, sampled/final position, source fingerprint.

## Guardrail Findings

No blocker was found for the current `M15`/`M16` temporal routing model. The old `STK_030` global over-posterize failure is resolved as a routing issue, though `STK_030` still needs operator-level probes.

Posterize boundary finding:

- `Posterize_Time.aex` confirms floor/trunc bucket behavior, but does not confirm an explicit epsilon.
- This is a documentation/probe refinement, not a formula blocker.

Scoped expression blocker:

```text
BLOCKER:
  title: Generic AE Expression Engine v2 requires BEE/Scripting time host, not only named edge_wobble shortcut
  affected objects: M09; future generic expression work also affects M08 and any animated property expressions
  evidence paths:
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/09_Scripting_PTR_GetTimePalSuite_180e16918/data_target.md
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/10_Scripting_PTR_TDB_SecondsToTime_180e16958/data_target.md
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/11_Scripting_PTR_BEE_FpLongToTime_180e16cb0/data_target.md
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/14_Scripting_PTR_BEE_CompToLayerTime_180e16f60/data_target.md
    - docs/phase_reports/AE_REVERSE_TURBULENT_TEXT_EXPRESSION_GHIDRA.md
  exact address/function:
    - Scripting.aex .rdata 0x180e16918 PTR_GetTimePalSuite -> pointer value 0x01267774; incoming 0x180b46db4 caller FUN_180b46c70
    - Scripting.aex .rdata 0x180e16958 PTR_TDB_SecondsToTime -> pointer value 0x0126752e; incoming 0x180949f91 caller FUN_180949e80
    - Scripting.aex .rdata 0x180e16cb0 PTR_BEE_FpLongToTime -> pointer value 0x012601c2; many incoming callers including FUN_1800cb3c0, FUN_1803c7160, FUN_180520500, FUN_180547bb0
    - Scripting.aex .rdata 0x180e16f60 PTR_BEE_CompToLayerTime -> pointer value 0x012639aa; incoming 0x1806729d6 caller FUN_180672980
  what assumption broke: Arbitrary property expression evaluation cannot be treated as the current named/fingerprint subset plus a plain comp-time scalar.
  hypotheses:
    - AE expression time is converted through BEE layer context before host variables are exposed.
    - Numeric/vector coercion and property access live in Scripting.aex / AfterFXLib host bindings, not in raw ExtendScript alone.
    - Named edge_wobble can remain a shortcut, but generic evaluator v2 must wait for BEE/TDB host contract.
  smallest probe/test needed:
    - AE property-sample export for generated edge_wobble with nonzero layer start and explicit inPoint/outPoint.
    - A second expression using valueAtTime or thisLayer time access to prove whether comp-to-layer conversion is required.
  can continue on unrelated work: yes
  can continue named edge_wobble shortcut: yes, as template-scoped approximate behavior
```

## Recommendation

Final recommendation: `blocked_by_guardrail` for generic Expression Engine v2; `needs_probe` for `M09` named `edge_wobble`, `M15` boundary parity, and `M16` adjustment-stack confirmation.

Ready-to-use temporal contract for other agents:

- Ordinary layer/source time: evaluate Posterize Time before source/precomp sampling and before downstream effect param evaluation on that layer.
- Source time: `source_start + (posterized_layer_time - layer_start)` for footage, clamped at zero; precomp source time is `posterized_layer_time - layer_start`, clamped at zero.
- Adjustment lower-stack time: first Posterize Time in the adjustment stack chooses lower-stack resample time.
- Adjustment effect param time: effects up to and including first Posterize Time use bucket time; effects after it use live comp time.
- Downstream effect hashes and `effect_time` telemetry are mandatory before tuning composed `STK_030`.
- Expression work may tune the named `edge_wobble` shortcut against AE samples, but must not claim broader AE expression parity until the BEE/Scripting time bridge is resolved.

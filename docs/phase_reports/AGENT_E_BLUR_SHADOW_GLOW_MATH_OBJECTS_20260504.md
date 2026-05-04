# Agent E Blur / Drop Shadow / Glow Math Objects

Status date: 2026-05-04

## Scope

Assigned objects:

- `M10` Drop Shadow.
- `M11` Glow.
- Shared blur-kernel subset of `M19`.

Write scope for this pass was limited to this report. Evidence root:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

Substrate dependency: formulas below reference `docs/phase_reports/AGENT_A_SUBSTRATE_MATH_OBJECTS_20260504.md`. Agent A explicitly says `M19` is not locked and alpha/composite formula tuning should pause. Therefore any alpha, premult, final composite, or final-pixel formula here is marked `substrate-dependent` or `blocked` instead of ready for tuning.

Orchestrator update honored: Agent A's global `M19` blocker applies directly to Box Blur final alpha/RGB, Drop Shadow final color/opacity/composite, and Glow threshold/composite/final RGB tuning. Agent F reported the same final-RGB-only tuning hazard for Minimax/Turbulent; Agent E treats Drop Shadow and Glow the same way. This report still provides parameter mapping, kernel routing, radius/softness hypotheses, based-on/threshold hypotheses, and probe plans, but does not mark alpha/composite-sensitive pieces as `ready_for_formula_tuning`.

Confidence labels: `confirmed`, `inferred`, `blocked`, `unknown`.

## Evidence Table

| Evidence | Address / function | Finding | Confidence | Affected modules |
| --- | --- | --- | --- | --- |
| `box_blur_aex/index.md` | `180005b70`, `FUN_180005b70`; `18000b5d1`, delay import | Box Blur CPU render candidate calls `GF::FastBoxBlur`; the AEX has a delay-load thunk for `GF::FastBoxBlur`. | confirmed | shared blur, `M19` |
| `box_blur_aex/05_BoxBlur_cpu_render_candidate_180005b70/decompile.c` | `180005f4c`, `1800063ff`, `180006430`, `180006928` | Checkout path resolves radius, iterations, dimensions, repeat-edge flag, builds `GF::BoxBlurOptions::StandardOptions`, then calls `GF::FastBoxBlur`. | confirmed | Box Blur |
| `target/reverse/ae_2026/effects_blur_glow_shadow/Box_Blur.aex` strings | plugin strings | UI strings include `Blur Radius`, `Blur Dimensions`, `Repeat Edge Pixels`, and `Iterations`; current native only models radius/iterations. | confirmed | Box Blur, param coverage |
| `blur_gpufoundation_kernels/06_GF_FastBoxBlur_180031c30/disassembly.txt` | `180031c76`, `180031c89` | `GF::FastBoxBlur` uses `VROUNDSS ... 0x2` on horizontal/vertical radius fields before integer conversion: effective radius quantization is ceil-like, not plain nearest-round. | confirmed | shared blur |
| `blur_gpufoundation_kernels/06_GF_FastBoxBlur_180031c30/decompile.c` | `180031c30`, `GF::FastBoxBlur` | Option bits `0x20`/`0x40` choose one-pass vs two-pass temporary path; some padded paths fill transparent black. | confirmed | shared blur, `M19` |
| `blur_gpufoundation_kernels/03..05_GF_BoxBlurOptions_*` | `180025150`, `180025280`, `180025350` | `BoxBlurOptions` stores dest alpha type at `+8`, src alpha type at `+4`; alpha-only blur clears bits `0xe` and sets low value `1`. | confirmed | shared blur, `M19` |
| `drop_shadow_aex/index.md` | `180005bb0`, `18000c393`, `18000c3a5`, `18000c3c9` | Drop Shadow has delay-load thunks for `GF::BoxBlurOptions::StandardOptions`, `SetBlurAlphaChannelOnly`, and `GF::FastBoxBlur`. | confirmed | `M10`, shared blur |
| `drop_shadow_aex/01_DropShadow_core_candidate_5bb0_180005bb0/decompile.c` | `180005f40`, `180005f4d`, `18000610e` | Drop Shadow builds box-blur options, explicitly switches alpha-channel-only blur, then calls `GF::FastBoxBlur`. | confirmed | `M10`, `M19` |
| `drop_shadow_aex/01_DropShadow_core_candidate_5bb0_180005bb0/data_refs.tsv` | `180005ecd`, `180005f00`, `18000614e`, `180006155` | Softness scale constants include `1.4`, `1.0`, and divisor about `2.71`; after blur, code loads effect-specific GPU kernel `DropShadow` / `CompositeShadowMask`. | confirmed | `M10`, `M19` |
| `drop_shadow_aex/01_DropShadow_core_candidate_5bb0_180005bb0/disassembly.txt` | `180005fb8`, `180005fe1` | Offset floats are converted with `VCVTTSS2SI`, so shadow placement uses truncation toward zero after upstream direction/distance math. | confirmed | `M10` |
| `docs/reverse_engineering/effect_math_blur_glow_shadow.md` | Round 5 probe notes | `direction=135`, `distance=28` gives raw AE shadow offset approximately `dx=+19`, `dy=+19`; `softness=18` expands bbox by about 10 px. | inferred/probe-backed | `M10` |
| `target/reverse/ae_2026/effects_blur_glow_shadow/Drop_Shadow.aex` strings | plugin strings | UI strings confirm `Shadow Color`, `Opacity`, `Direction`, `Distance`, `Softness`, and `Shadow Only`. | confirmed | `M10` |
| `target/reverse/ae_2026/effects_blur_glow_shadow/Glow.aex` strings | plugin strings/import strings | UI strings confirm `Glow Based On`, `Alpha Channel\|Color Channels`, threshold/radius/intensity/composite/operation/color controls; imports include `IR_GaussianBlur`, `IR_CompositeWithBlendMode`, `ImageRenderer.dll`. | confirmed | `M11`, `M19` |
| `objdump -p target/reverse/ae_2026/effects_blur_glow_shadow/Glow.aex` | import table | Glow imports `ImageRenderer.dll` functions `IR_GaussianBlur` and `IR_CompositeWithBlendMode`. | confirmed | `M11`, `M19` |
| `imagerenderer_gaussian_composite/02_IR_GaussianBlur_wrapper_1800a0e40/decompile.c` | `1800a0e40`, `IR_GaussianBlur` | Wrapper forwards to `18009fca0`, the ImageRenderer Gaussian implementation. | confirmed | `M11`, `M19` |
| `imagerenderer_gaussian_composite/01_IR_GaussianBlur_impl_18009fca0/callees.tsv` | `18009ff53..1800a0274`, `1800a079a`, `1800a0a92` | Gaussian path uses `exp`, `sin`, `cos`, transient pixel buffers, and task executor calls; not the same primitive as Box Blur. | confirmed | `M11`, `M19` |
| `docs/phase_reports/AGENT_A_SUBSTRATE_MATH_OBJECTS_20260504.md` | Agent A `M19` passport | Straight `RGBA8` + normal source-over is only an approximation; alpha/composite formula tuning should pause until alpha-ramp probes resolve `M19`. | confirmed blocker | `M10`, `M11`, `M19` |

## Object Passports

### Shared Blur Kernel Subset of M19

Parameter mapping:

- `confirmed`: `GF::FastBoxBlur` receives a `BoxBlurOptions` struct whose radius fields are read at option offsets equivalent to `param_15[4]` and `param_15[5]`, iteration/count-like field at `param_15[3]`, and option bits at `param_15[0]`.
- `confirmed`: `GetSrcAlphaType` reads `BoxBlurOptions + 4`; `GetDestAlphaType` reads `BoxBlurOptions + 8`.
- `confirmed`: `SetBlurAlphaChannelOnly` sets low alpha-option value `1` after clearing bits `0xe`.
- `inferred`: `StandardOptions` packs pass direction, repeat/padding behavior, iteration count, horizontal radius, vertical radius, and alpha options.

Coordinate/time/color space:

- Pixel-space blur over integer image bounds and strides.
- No time dependency in the kernel itself; animated effect params are sampled by the caller before options are built.
- Color/alpha interpretation is blocked by Agent A's `M19` substrate contract.

Sampling rule:

- `confirmed`: `GF::FastBoxBlur` is separable and toggles `0x20`/`0x40` direction bits between passes.
- `confirmed`: radius quantization in `GF::FastBoxBlur` is ceil-like (`VROUNDSS imm=0x2`) before integer conversion.
- `confirmed`: some padded/intermediate paths fill transparent black.
- `unknown`: exact edge-repeat vs transparent extension semantics for each option combination.

Alpha/premult policy:

- `confirmed`: blur has explicit source alpha type, dest alpha type, and alpha-channel-only modes.
- `blocked`: whether normal Box Blur should blur straight RGBA, premultiplied RGB, or a converted format depends on Agent A `M19`.

Formula / pseudocode:

```text
rx = ceil(options.radius_x)
ry = ceil(options.radius_y)
n  = options.iteration_count

if options has both direction bits:
    allocate temporary
    pass_x = blur_1d(src, tmp, rx, alpha_options, edge_options)
    pass_y = blur_1d(tmp, dst, ry, alpha_options, edge_options)
else:
    pass_one_axis_only(...)
```

Native implementation delta:

- Current native `round(radius)` should be treated as suspect; `ceil(radius)` is a better kernel hypothesis.
- Current native `clip_to_layer_bounds` is not proven; repeat-edge/transparent-black options must be split before tuning.

Next probe:

- Impulse near center, edge, and corner with radius `0`, `0.49`, `0.5`, `1`, `1.01`, `10`, `18`; iterations `1/2/3`; repeat-edge on/off; transparent edge RGB vs opaque edge RGB.

### Box Blur / Fast Box Blur

Parameter mapping:

- `confirmed`: AEX plugin is `ADBE Box Blur2` / Fast Box Blur.
- `confirmed`: Render checkout path covers radius, iterations, blur dimensions, and repeat edge pixels.
- `confirmed`: UI strings identify `Blur Radius`, `Blur Dimensions`, `Repeat Edge Pixels`, and `Iterations`.
- `inferred`: Current payload mapping `0001=radius`, `0002=iterations` remains usable only for the simplified native subset. AE's full native control set also includes dimensions and repeat-edge controls; if payloads carry those, current native silently misses them.

Coordinate/time/color space:

- Pixel-space separable blur on source layer frame.
- Radius is adjusted by frame/pixel-aspect-like scale before `StandardOptions` (`FUN_180005b70` computes `fVar21` from checkout radius and frame ratios).
- Alpha/color interpretation is substrate-dependent on Agent A `M19`.

Sampling rule:

- `confirmed`: AEX delegates to `GF::FastBoxBlur` at `180006928`.
- `confirmed`: dimensions popup is forwarded into pass-enable booleans (`sVar18 != 3`, `sVar18 != 2`), consistent with horizontal/vertical selection.
- `confirmed`: repeat-edge boolean is forwarded into `StandardOptions`.
- `confirmed`: fractional radius should quantize ceil-like in the shared kernel.
- `unknown`: whether AE applies additional pre-scaling or clamp limits before `StandardOptions` for all bpc/formats.

Alpha/premult policy:

- `blocked`: normal Box Blur does not call `SetBlurAlphaChannelOnly`; it relies on `StandardOptions` alpha types and Agent A's unresolved `M19` contract.

Formula / pseudocode:

```text
radius_scaled = effect_radius * pixel_frame_scale
iterations = checked_out_iterations
dimensions = {both, horizontal, vertical}
repeat_edge = checked_out_bool

options = GF::BoxBlurOptions::StandardOptions(
    horizontal_enabled = dimensions != vertical_only,
    vertical_enabled   = dimensions != horizontal_only,
    repeat_edge,
    iterations,
    radius_scaled,
    radius_scaled
)

dst = GF::FastBoxBlur(src, options)
```

Native implementation delta:

- Change candidate after probes: `ceil` radius, full dimensions/repeat-edge coverage, and shared alpha options.
- Do not tune final alpha/color deltas until Agent A resolves `M19`.

Next probe:

- Same shared blur probe above, plus one dimensions probe (`H/V`, `H`, `V`) and repeat-edge on/off.

### M10: Drop Shadow

Parameter mapping:

- `confirmed`: UI strings map to `Shadow Color`, `Opacity`, `Direction`, `Distance`, `Softness`, `Shadow Only`.
- `probe-backed`: current numbered native mapping `0001=color`, `0002=opacity`, `0003=direction`, `0004=distance`, `0005=softness`, `0006=shadow_only` matches prior fixture/probe notes.
- `blocked/inferred`: opacity units are still not locked globally. Round 5 supports byte-style `180/255`, but final scaling goes through effect kernel/composite plus Agent A `M19`.

Coordinate/time/color space:

- Pixel-space shadow generated from source alpha mask.
- Coordinate sign convention from Round 5: for `direction=135`, `distance=28`, AE raw offset is approximately `dx=+19`, `dy=+19`.
- `confirmed`: the AEX truncates float offsets toward zero when placing the blur source (`VCVTTSS2SI`).

Sampling rule:

- `inferred`: offset formula consistent with probes:

```text
dx = trunc(-cos(direction_degrees) * distance)
dy = trunc( sin(direction_degrees) * distance)
```

- `confirmed`: softness blur path calls `GF::BoxBlurOptions::StandardOptions`, then `SetBlurAlphaChannelOnly`, then `GF::FastBoxBlur`.
- `confirmed`: softness is scaled before blur:

```text
scale = 1.4 for one observed pixel-format branch, otherwise 1.0
radius_soft = ceil(softness * scale / 2.71)
```

For the observed 8bpc-like path, `softness=18` gives `ceil(18 * 1.4 / 2.71) = 10`, matching Round 5 bbox expansion.

Alpha/premult policy:

- `confirmed`: shadow blur is alpha-channel-only inside `GF::FastBoxBlur`.
- `blocked`: final colored shadow, opacity, and source-over behavior go through Agent A `M19` and a hidden effect kernel.

Formula / pseudocode:

```text
offset = trunc((-cos(theta), sin(theta)) * distance)
shadow_mask = translate(source_alpha, offset)

if softness > 0:
    options = StandardOptions(..., radius_soft_x, radius_soft_y)
    options.SetBlurAlphaChannelOnly()
    shadow_mask = GF::FastBoxBlur(shadow_mask, options)

final = CompositeShadowMask(
    source,
    shadow_mask,
    color,
    opacity,
    shadow_only,
    substrate_alpha_flags
)
```

Native implementation delta:

- Current `round(softness / 2) + 1` is an approximation that happens to match `18 -> 10`; the AEX evidence supports `ceil(softness * 1.4 / 2.71)` for the observed path.
- Current normal source-over composite is not safe for tuning because native AEX loads `DropShadow` / `CompositeShadowMask`.

Next probe:

- Direction grid `0/45/90/135/180/225/270/315`, distances that produce fractional projections, softness `0/1/2/8/18/32`, opacity `50/100/128/180/255`, colored shadow over partial-alpha source. Capture raw offset mask, blurred mask, and final composite separately.

### M11: Glow

Parameter mapping:

- `confirmed`: AEX plugin is `ADBE Glo2`.
- `confirmed`: UI strings include `Glow Based On`, `Alpha Channel|Color Channels`, `Glow Threshold`, `Glow Radius`, `Glow Intensity`, `Composite Original`, `Glow Operation`, color-loop controls, and `Glow Dimensions`.
- `blocked`: current native docs say `0001=1` maps to color channels and `0001=2` maps to alpha channel. Native AEX string order implies the one-based popup order is likely `1=Alpha Channel`, `2=Color Channels`. This is a parameter/enum guardrail until confirmed with a two-pixel AE probe.

Coordinate/time/color space:

- Pixel-space effect over source layer frame.
- `confirmed`: Glow imports `ImageRenderer.dll` functions `IR_GaussianBlur` and `IR_CompositeWithBlendMode`.
- `confirmed`: `IR_GaussianBlur` is a true ImageRenderer Gaussian path using floating math (`exp`, `sin`, `cos`) and transient buffers; it is not the Box Blur primitive.
- Color/alpha interpretation is blocked by Agent A `M19`.

Sampling rule:

- `confirmed`: setup exposes threshold, radius, intensity, dimensions, composite original, operation, and color controls; the selected predecoded render wrapper routes through helpers not fully included in the bundle.
- `probe-backed`: Round 5 outputs prove `0001=1` and `0001=2` are materially different; default/absent `0001` differs from both enum-specific outputs.
- `blocked`: exact threshold source is not recovered: luma, max RGB, alpha, premultiplied RGB, unpremultiplied RGB, or combined default remain possible.
- `blocked`: exact radius mapping into `IR_GaussianBlur` is not recovered from selected Glow helpers. Native `radius / 2` remains a weak approximation.

Alpha/premult policy:

- `blocked`: thresholding and composite must wait for Agent A `M19` plus direct Glow intermediate probes.
- `confirmed`: final blend/composite route can use `IR_CompositeWithBlendMode`, and Glow has a `Glow Operation` popup with many blend modes. Native normal source-over is therefore too small a model for AE Glow.

Formula / pseudocode:

```text
based_on = popup_value  # enum order currently blocked
threshold_source = threshold(source, based_on, threshold, substrate_pixel_model)

blurred = IR_GaussianBlur(
    threshold_source,
    radius,
    dimensions,
    pixel_format_alpha_flags
)

colored_or_scaled = apply_intensity_and_color_controls(
    blurred,
    intensity,
    glow_colors,
    color_looping
)

final = IR_CompositeWithBlendMode(
    original,
    colored_or_scaled,
    composite_original,
    glow_operation,
    substrate_alpha_flags
)
```

Native implementation delta:

- Current native Box Blur glow kernel is likely the wrong primitive for AE Glow; AEX imports `IR_GaussianBlur`.
- Current based-on enum mapping may be reversed.
- Current normal source-over final composite is too narrow because AE Glow imports `IR_CompositeWithBlendMode` and exposes `Glow Operation`.

Next probe:

- Two-pixel probe: dark/high-alpha `[32,32,32,255]` and bright/low-alpha `[240,240,240,64]`, threshold `120`, radius `0`, intensity `1`, absent `0001`, `0001=1`, `0001=2`.
- Then radius probes `1/10/35` with `Glow Operation=Normal/Add/Screen`, `Composite Original=On Top/Behind/None`, and `Glow Dimensions=H/V/H/V` to isolate Gaussian spread and composite route.

## Guardrail Findings

### BLOCKER

title: Agent A `M19` substrate is not locked for alpha/composite tuning

affected objects: `M10`, `M11`, shared blur subset of `M19`; also final template pixel thresholds.

evidence paths:

- `docs/phase_reports/AGENT_A_SUBSTRATE_MATH_OBJECTS_20260504.md`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/blur_gpufoundation_kernels/03_GF_BoxBlurOptions_GetDestAlphaType_180025150/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/blur_gpufoundation_kernels/04_GF_BoxBlurOptions_GetSrcAlphaType_180025280/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/blur_gpufoundation_kernels/05_GF_BoxBlurOptions_SetBlurAlphaChannelOnly_180025350/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/imagerenderer_gaussian_composite/01_IR_GaussianBlur_impl_18009fca0/decompile.c`

exact address/function:

- `180025150`, `GF::BoxBlurOptions::GetDestAlphaType`
- `180025280`, `GF::BoxBlurOptions::GetSrcAlphaType`
- `180025350`, `GF::BoxBlurOptions::SetBlurAlphaChannelOnly`
- `18009fca0`, ImageRenderer Gaussian implementation

what assumption broke:

- Current straight `RGBA8` plus normal source-over is not confirmed enough for alpha-sensitive formula tuning.

hypotheses:

- Effects may still output AE-equivalent pixels in a common 8bpc straight path, but only after hidden conversion/alpha flags.
- Drop Shadow and Glow may differ mostly through shared substrate/composite, not local kernel math.

smallest probe/test needed:

- Agent A alpha-ramp probe, plus per-effect intermediate hashes for Drop Shadow mask/blur/final and Glow threshold/blur/final.

can continue on unrelated work: yes. Continue kernel shape and enum probes; do not tune final alpha/composite constants.

### BLOCKER

title: Glow `Glow Based On` enum appears inconsistent with current native assumption

affected objects: `M11`

evidence paths:

- `target/reverse/ae_2026/effects_blur_glow_shadow/Glow.aex`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/glow_aex/03_Glow_render_candidate_5f50_180005f50/decompile.c`
- `docs/reverse_engineering/effect_math_blur_glow_shadow.md`

exact address/function:

- `180005f50`, `FUN_180005f50` parameter setup
- Glow AEX string: `$$$/AE/Glow/LStr/0002=Alpha Channel|Color Channels`

what assumption broke:

- Current native note maps `0001=1` to color channels and `0001=2` to alpha channel. The native AEX popup string order implies `1=Alpha Channel`, `2=Color Channels` unless AE stores the popup with an inverted value map.

hypotheses:

- H1: Native enum is reversed and must be fixed before Glow threshold tuning.
- H2: AE popup display order differs from stored numeric value, so current native mapping could still be right.

smallest probe/test needed:

- Radius `0`, intensity `1`, threshold `120` two-pixel probe with `0001=1` and `0001=2`; compare which output keeps dark/high-alpha vs bright/low-alpha pixel.

can continue on unrelated work: yes, but Glow source threshold tuning is blocked.

### Hidden Pipeline Findings

- `Box Blur`: confirmed delegation to `GF::FastBoxBlur`; this is expected and in Agent E/Agent A shared scope.
- `Drop Shadow`: confirmed delegation to `GF::FastBoxBlur` plus hidden effect kernel `DropShadow` / `CompositeShadowMask`; final composite formula is blocked until this kernel or an intermediate probe is available.
- `Glow`: confirmed import of `IR_GaussianBlur` and `IR_CompositeWithBlendMode`; current native Box Blur + normal source-over route is not AE-shaped enough for formula tuning.

## Recommendation

`blocked_by_guardrail`

Reasons:

- Agent A `M19` is explicitly not locked, so alpha/composite-sensitive tuning for Drop Shadow and Glow should pause.
- Glow based-on enum mapping has a concrete contradiction risk.
- Box Blur kernel work is ready for probes, not formula tuning: change candidates are `ceil` radius quantization, dimensions/repeat-edge support, and shared alpha option coverage.

Safe next actions:

- Add focused AE probe packs for Box Blur edge/fraction/dimensions, Drop Shadow offset/softness/opacity, and Glow based-on/radius/composite.
- Add native telemetry checkpoints for `threshold_source`, `raw_shadow_mask`, `blurred_shadow_mask`, `blurred_glow`, and `final_composite`.
- Only after Agent A resolves `M19`, tune final alpha/color/composite constants.

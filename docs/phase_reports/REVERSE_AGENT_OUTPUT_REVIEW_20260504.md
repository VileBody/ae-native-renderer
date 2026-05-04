# Reverse Agent Output Review

Status date: 2026-05-04.

This review separates completed evidence from useful-but-incomplete checkpoint
work. The UI-level agent status is less important than whether the artifact has
addresses, confidence labels, and native implications.

## Executive Summary

| Agent / area | Artifact quality | Main value | Risk |
| --- | --- | --- | --- |
| A: Core Alpha / Composite | Useful checkpoint | Blend enum map, AlphaGain fast paths, TransferDescriptor opacity state. | Exact normal composite math and premult flags still not locked. |
| B: Geometry / Collapse | Strongest completed slice | Geometry2 property mapping, matrix order, sampler enum shape, validation diff breakdown. | True collapse/text deferred rasterization is not solved. |
| C: Temporal / Posterize / Motion | Strong for Posterize, good target map for motion | Posterize bucket formula and negative-time truncation are confirmed; BEE scheduler RVAs collected. | Motion blur shutter sampling still pending BEE decompile. |
| D: Blur / Glow / Shadow / Minimax | Very useful checkpoint | BoxBlur options/edge policy, Drop Shadow alpha-only path, ImageRenderer Gaussian constants, Minimax enum surface. | Kernel weights, Glow blend mapping, Minimax neighborhood still need fixtures/decompile. |
| E: Turbulent / Text / Expression | Mixed; Turbulent became strong after follow-up | Turbulent wrapper contract now confirmed; text uses TXT/CoolType; Expression is generic ExtendScript plus AE host targets. | Text animator and expression formulas are not recovered yet. |

## Agent A: Core Alpha / Composite

Strong evidence:

- `GPUFoundation.dll` owns central alpha/composite primitives.
- `AlphaGain` has confirmed transparent-black and memcpy fast paths.
- `Composite@GF` dispatches by `IR_BlendMode`; enum `0x00..0x1d` is mapped to
  concrete kernel names.
- Normal blend is enum `0x12`.
- `TransferDescriptor` stores opacity at `+0x0c` and blend table pointer at
  `+0x18`.

What not to claim yet:

- Exact normal source-over formula.
- Meaning of the two trailing composite bool flags.
- Global straight/premult policy.
- Pixel-center/OOB sampler policy.

Actionable next step:

- Decompile `Composite@GF` normal kernel or CPU fallback and resolve
  `param_13`/`param_14`. This should precede final alpha/premult tuning for all
  effects.

## Agent B: Geometry / Collapse

Strong evidence:

- Geometry2 payload numeric keys are AE property indices:
  `0008` is Rotation, while `ADBE Geometry2-0008` is Opacity.
- Current matrix order should remain:

```text
T(position) * S(1/par,1) * R(-axis) * Hx(-tan(skew)) *
R(axis) * R(rotation) * S(scale) * S(par,1) * T(-anchor)
```

- GPUFoundation sampler enum shape is now useful:
  nearest, bilinear, bicubic/lanczos, bicubic-area, sharp/smooth, area/no-area.
- Validation split is clear:
  `EFF_040` close, `GPH_010` collapse/text failure, `CMP_010` RGB/composite
  mismatch with alpha matching.

What not to claim yet:

- True AE collapse transformations.
- Pixel-center/OOB exact boundary.
- Motion blur sample timestamps/weights.
- Geometry2 opacity/shutter parity.

Actionable next step:

- Isolate pixel-center/OOB/bilinear rounding on `EFF_040`, while keeping
  `GPH_010` in the collapse/text bucket instead of tuning Geometry2 against it.

## Agent C: Temporal / Posterize / Motion

Strong evidence:

- Posterize Time frame rate is PF fixed 16.16.
- Bucket formula is confirmed:

```text
fps = frame_rate_fixed / 65536.0
bucket_size_ticks = time_scale / fps
bucket_time_ticks = trunc(current_time_ticks / bucket_size_ticks) * bucket_size_ticks
```

- Negative/pre-roll bucket conversion truncates toward zero.
- Posterize itself does not do alpha/pixel math; it re-checks out input at a
  quantized time.
- BEE.dll temporal scheduler targets and RVAs are collected.

What not to claim yet:

- Full adjustment-layer downstream scheduling policy.
- Motion blur shutter sample generation.
- Layer/precomp cache equivalence policy.

Actionable next step:

- Decompile BEE shutter/scheduler targets:
  `GetShutterSampleInfo`, `GetShutterStartTime`, `GetShutterDuration`,
  `BEE_GetLayerROAndTimeFromCompRO`, and checkout functions.

## Agent D: Blur / Glow / Shadow / Minimax

Strong evidence:

- `GF::BoxBlurOptions` layout is mostly anchored.
- Box blur channel bits and repeat-edge bit are recovered.
- Radius is rounded with a ceil-like op; V2 receives rounded radius plus
  fractional delta.
- Drop Shadow blurs alpha only and starts from transparent black mask setup.
- Glow delegates blur to `IR_GaussianBlur` and composite to
  `IR_CompositeWithBlendMode`.
- ImageRenderer Gaussian is separable recursive with recovered constants.
- Minimax exposes operation/channel/direction/edge parameters and has 8/16/32
  bpc CPU callbacks plus GPU spatial-tree kernels.

What not to claim yet:

- Exact BoxBlur V2 fractional kernel weights.
- Exact Drop Shadow softness scale.
- Glow operation-to-blend enum mapping.
- Exact Gaussian coefficient placement/sign/order.
- Minimax radius rounding/neighborhood and edge-shrink details.

Actionable next step:

- Implement instrumentation/prototypes, not parity locks:
  BoxBlur fractional-radius fixtures, DropShadow alpha-mask telemetry, Glow
  recursive-Gaussian impulse fixtures, and Minimax callback-focused decompile.

## Agent E: Turbulent / Text / Expression

Strong evidence:

- Turbulent Displace wrapper contract is now strong after follow-up:
  state block `0x8130`, GPU param block `0x405c`, 64-row table, `FracAll` vs
  `Frac1D`, H/V lookup buffers, fixed16 slots, complexity split.
- Turbulent Noise exposes `ADBE_AIF_Perlin_Noise_3D`, suggesting a shared noise
  family, but not enough for formula parity.
- Text is routed through TXT/CoolType, not direct raw font metrics.
- CoolType/TXT evidence confirms glyph IDs, widths, bboxes, baseline deltas,
  feature/glyph access interfaces.
- Expression evidence confirms generic ExtendScript and points toward
  `Scripting.aex` / `AfterFXLib.dll` host bindings.

What not to claim yet:

- Turbulent kernel noise basis and sampler math.
- Text animator selector formulas.
- Exact glyph rasterization/antialias policy.
- AE expression builtins such as `thisLayer`, `valueAtTime`, `wiggle`,
  `seedRandom`.

Actionable next step:

- For Turbulent, implement the recovered two-path state/lookup model and add
  lookup hashes before final formula tuning.
- For text, add CoolType-shaped telemetry.
- For expressions, reverse AE host binding tables, not generic ExtendScript.

## Recommended Order

1. Lock composite substrate: normal blend math, premult flags, hidden RGB policy.
2. Tune Geometry2 sampler/OOB on `EFF_040`.
3. Implement AE-shaped Turbulent state/lookup telemetry/model.
4. Move DropShadow/BoxBlur/Glow from approximate to instrumented prototypes.
5. Start text parity with CoolType-shaped telemetry before touching selector
   formula tuning.

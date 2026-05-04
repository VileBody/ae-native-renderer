# Agent E Blur / Drop Shadow / Glow Round 9

Date: 2026-05-04

Scope: M10 Drop Shadow, M11 Glow, shared blur subset of M19. I only touched
blur/drop-shadow/glow source/tests plus this report.

## Ghidra Evidence Used

- `blur_gpufoundation_kernels/06_GF_FastBoxBlur_180031c30/decompile.c` rounds
  horizontal and vertical radii with `vroundss_avx(..., 2)` before computing
  the halo as rounded radius times iterations.
- `blur_gpufoundation_kernels/02_GF_BoxBlur_1DImgOpInfo_ctor_180024cc0`
  stores the float radius separately from the rounded radius, also using
  `vroundss_avx(..., 2)`. I treat mode `2` as ceil / round toward positive
  infinity.
- `blur_gpufoundation_kernels/05_GF_BoxBlurOptions_SetBlurAlphaChannelOnly`
  clears RGB channel bits and leaves alpha bit `0x01`.
- `drop_shadow_aex/01_DropShadow_core_candidate_5bb0_180005bb0` calls
  `GF::BoxBlurOptions::StandardOptions`, then
  `GF::BoxBlurOptions::SetBlurAlphaChannelOnly`, then `GF::FastBoxBlur`, then
  the `DropShadow/CompositeShadowMask` kernel. This confirms the softness blur
  is alpha-only before final composite.
- `imagerenderer_gaussian_composite/01_IR_GaussianBlur_impl_18009fca0` and
  `03_IR_composite_worker_select_1800769b0` confirm Glow uses ImageRenderer
  Gaussian/composite plumbing. The final alpha/premult/blend contract is still
  an M19/Agent A blocker.

## Implemented

- Shared `blur_radius` now uses ceil quantization, matching the recovered
  GPUFoundation radius rounding for the integer subset we emulate.
- Box Blur and Glow already sampled animated params at effect time; added render
  tests proving output changes across `EffectContext.time`, not only params.
- Drop Shadow already used the AE-probed sign convention and truncating integer
  offset. Added a fractional-distance test to pin truncation.
- Drop Shadow softness blur now follows `SetBlurAlphaChannelOnly`: blur the
  shadow alpha mask, then re-apply the shadow RGB color before final composite.
- No final composite, premult, ImageRenderer blend, or Glow operation tuning was
  changed.

## Added Primitive Tests

- Box Blur fractional radius quantization around integer boundaries:
  `0.01`, `0.5`, `0.99`, `1.0`, `1.01`, `1.5`, and clamp cases.
- Box Blur clipped edge behavior with exact 3-pixel RGBA averages.
- Box Blur render-time animated radius smoke test.
- Drop Shadow fractional offset truncation test.
- Drop Shadow alpha-only softness blur test that proves RGB is not diluted by
  transparent neighbors.
- Glow render-time animated radius smoke test.
- Glow half-radius mapping test showing shared ceil quantization after
  `radius / 2`.

## Glow Based On Probe Checklist

Use one raw-threshold-source probe before any blur or final composite tuning.

Common params:

```text
threshold / 0002 = 120
radius / 0003 = 0
intensity / 0004 = 1.0
operation/blend/defaults unchanged
```

Input frame `GBON_4PX`, one row of four pixels:

```text
x0 = [ 32,  32,  32, 255]  dark RGB, high alpha
x1 = [240, 240, 240,  64]  bright RGB, low alpha
x2 = [240, 240, 240, 255]  bright RGB, high alpha
x3 = [ 32,  32,  32,  64]  dark RGB, low alpha
```

Probe `0001` values:

| Case | `0001` value | Native hypothesis for raw `threshold_source_rgba` |
| --- | --- | --- |
| `GBON_DEFAULT` | absent | x0 pass, x1 pass, x2 pass, x3 zero |
| `GBON_0` | `0` if AE accepts it | classify; native currently maps to combined |
| `GBON_1` | `1` | x0 zero, x1 pass, x2 pass, x3 zero |
| `GBON_2` | `2` | x0 pass, x1 zero, x2 pass, x3 zero |
| `GBON_3` | `3` if AE accepts it | classify; native currently maps to combined |

Expected raw source pixels for the three hypotheses:

```text
combined/default:
  [[32,32,32,255], [240,240,240,64], [240,240,240,255], [0,0,0,0]]

color_channels:
  [[0,0,0,0], [240,240,240,64], [240,240,240,255], [0,0,0,0]]

alpha_channel:
  [[32,32,32,255], [0,0,0,0], [240,240,240,255], [0,0,0,0]]
```

Decision order:

1. Compare AE/raw probe to `threshold_source_rgba`.
2. Only after that matches, compare `blurred_glow_rgba`.
3. Only after blur matches, compare `intensity_scaled_glow_rgba`.
4. Keep `final_rgba` out of formula decisions until M19 alpha/premult/composite
   is fixed by Agent A.

## Blocked On M19 / Agent A

- Drop Shadow final `CompositeShadowMask` alpha/premult behavior.
- Glow `IR_CompositeWithBlendMode` operation mapping and premult/straight-alpha
  handling.
- Any final-pixel tuning that tries to compensate for composite substrate
  differences.

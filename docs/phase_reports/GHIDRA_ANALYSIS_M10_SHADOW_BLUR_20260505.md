# GHIDRA Analysis M10 Shadow / Blur

Date: 2026-05-05

## Status

Short pass over fresh extraction only:
`target/reverse/predecoded/20260505_153013_blocker_modules_round2`.

Implementation code was not touched. Scope is Drop Shadow plus the local
BoxBlur/GPUFoundation blur evidence needed to unblock M10 probes.

## Inputs

- `drop_shadow_aex`: `FUN_180005bb0`, `EffectMainExtra`, delay thunks for
  `GF::BoxBlurOptions::StandardOptions`,
  `GF::BoxBlurOptions::SetBlurAlphaChannelOnly`, and `GF::FastBoxBlur`.
- `box_blur_aex`: `FUN_1800034d0`, `FUN_180005b70`, `xGPUFilterEntry`.
- `blur_gpufoundation_kernels`: `GF::FastBoxBlur`,
  `GF::BoxBlurOptions::{GetSrcAlphaType,GetDestAlphaType,SetBlurAlphaChannelOnly}`,
  `GF::BoxBlur_1DImgOpInfo` ctor.
- Prior local probe notes in `docs/reverse_engineering/effect_math_blur_glow_shadow.md`
  for the `direction=135`, `distance=28`, `softness=18` AE observations.

## Findings

1. Direction/distance to offset:
   - Fresh Ghidra bundle does not include the upstream producer
     `FUN_1800073a0`, so it does not expose the single path that computes
     `dx/dy` from UI `direction` and `distance`.
   - The Drop Shadow GPU consumer does confirm the final integer conversion:
     `param_3[0]` and `param_3[1]` are converted with `VCVTTSS2SI`, then added
     to source origin. That is truncation toward zero, not floor/round-nearest.
   - Accepted working formula remains probe-backed:
     `dx = trunc(-cos(direction_degrees) * distance)`,
     `dy = trunc( sin(direction_degrees) * distance)`.
     For `135/28`, this gives approximately `+19/+19`, matching Round 5 bbox
     evidence.

2. Softness/radius mapping:
   - Drop Shadow builds `GF::BoxBlurOptions::StandardOptions`, then immediately
     calls `SetBlurAlphaChannelOnly`, then `GF::FastBoxBlur`.
   - Ghidra constants in Drop Shadow are `1.4`, `1.0`, and divisor `2.71`.
     The call site passes horizontal/vertical softness-derived floats:
     `radius = softness_component * factor / 2.71`; the same call also passes
     an int-like count of `1` or `3` depending on `param_1 + 0xc0`.
   - `GF::FastBoxBlur` and `GF::BoxBlur_1DImgOpInfo` both quantize radius with
     `VROUNDSS/VROUNDSD imm=0x2`, i.e. ceil before integer conversion.
   - Therefore the Ghidra-backed local candidate is:
     `effective_radius_axis = ceil(softness_axis * factor / 2.71)`, with the
     `factor/count` branch still needing a runtime probe.
   - Rejected as Ghidra-backed wording: plain `round(softness / 2) + 1`.
     It may fit one bbox, but the fresh code points at `2.71` plus ceil.

3. Alpha-only blur and alpha type:
   - `SetBlurAlphaChannelOnly` clears option bits `0xe` and sets bit/value `1`.
   - `GetSrcAlphaType` reads `BoxBlurOptions + 4`.
   - `GetDestAlphaType` reads `BoxBlurOptions + 8`.
   - Drop Shadow explicitly uses alpha-channel-only blur; normal Box Blur does
     not call this setter in the inspected path and should remain separate.

4. Drop Shadow local composite/shadow alpha policy:
   - After alpha-only blur, Drop Shadow loads GPU kernel
     `DropShadow / CompositeShadowMask`.
   - The dispatch passes the blurred mask, source frame, source/dest geometry,
     a `shadow_only` boolean, and packed shadow color/opacity-ish fields copied
     from the Drop Shadow param block.
   - This supports an effect-local policy: shadow mask is generated/blurred in
     alpha only, then `CompositeShadowMask` colors/composites it locally.
   - No global alpha/composite rule should be inferred from this. Any attempt to
     apply this as renderer-wide alpha policy is `ORCHESTRATOR_BLOCKER`.

## Accepted / Rejected / Unknown

Accepted:

- Offset integer conversion is truncation toward zero at Drop Shadow GPU
  consumer boundary.
- Working direction sign convention remains
  `dx=-cos(deg)*distance`, `dy=sin(deg)*distance`, then truncation.
- Drop Shadow routes softness through `GF::FastBoxBlur`.
- Drop Shadow explicitly sets alpha-channel-only blur.
- `BoxBlurOptions` src alpha type is at `+4`; dest alpha type is at `+8`.
- `GF::FastBoxBlur` radius quantization is ceil-like.
- Drop Shadow final shadow coloring/composite is local to
  `CompositeShadowMask`.

Rejected:

- Do not mark `round(radius)` or `floor(radius)` as matching the recovered blur
  kernel.
- Do not use Drop Shadow alpha-only blur as evidence for normal Box Blur global
  alpha/composite behavior.
- Do not promote a global premult/straight alpha policy from this bundle.

Unknown:

- Exact upstream direction/distance computation path, because `FUN_1800073a0`
  was not in this extraction.
- Which `param_1 + 0xc0` branch is active for the relevant AE render modes and
  how the `factor/count` pair should collapse in native M10.
- Exact weights/edge policy inside the blur span kernel after radius ceil.
- Exact alpha math inside `CompositeShadowMask` for colored translucent shadow,
  partial source alpha, and overlap with source.

## Missing Probes

- Direction sweep, shadow-only, softness `0`: directions
  `0/45/90/135/180/225/270/315`, plus fractional projections such as `30`,
  `60`, distances `1/2/7/28`. Measure bbox offset to lock sign and truncation.
- Fractional offset probe: choose cases where projected value is `+/-N.49` and
  `+/-N.51` to distinguish truncation from floor/nearest for negative dx/dy.
- Softness sweep, shadow-only: `0/1/2/3/8/18/32`, with bpc/quality variants if
  available, to identify the active `1.4 x 1` versus `1.0 x 3` branch.
- Alpha-only probe: source with RGB noise under varying alpha, shadow color
  fixed, to confirm RGB is ignored during blur and only mask alpha spreads.
- Composite probe: colored translucent shadow over partial-alpha source,
  including overlap and `shadow_only=0/1`, to isolate
  `CompositeShadowMask` alpha/color math without changing global composite.

## Next Implementation Candidate

Keep implementation unchanged until probes land. The next candidate, if probes
match the Ghidra path, is effect-local M10 only:

```text
dx = trunc(-cos(direction_degrees) * distance)
dy = trunc( sin(direction_degrees) * distance)

radius_axis = ceil(softness_axis * active_factor / 2.71)
blur shadow mask alpha only with GF/FastBoxBlur-like pass settings
color/composite via Drop Shadow local policy, not renderer-wide alpha policy
```

Any implementation change that touches shared/global alpha or composite beyond
Drop Shadow should be blocked as `ORCHESTRATOR_BLOCKER`.

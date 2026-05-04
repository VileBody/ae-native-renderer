# AE Effect Math: Blur / Glow / Drop Shadow

Date: 2026-05-04

Scope: reverse notes only for the blur/glow/drop-shadow family:

- `ADBE Box Blur2` / Fast Box Blur-ish behavior.
- Text animator `ADBE Text Blur` only where it reuses blur math.
- `ADBE Drop Shadow`.
- `ADBE Glo2` / Glow.

## Source Inventory

### Binaries / Symbols / Ghidra

Round 9 uses the prepared Ghidra bundle:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

Relevant decoded targets:

| Target | Relevance |
| --- | --- |
| `blur_gpufoundation_kernels` | `GF::FastBoxBlur`, `GF::BoxBlur_1DImgOpInfo`, `BoxBlurOptions::SetBlurAlphaChannelOnly`; used for ceil radius and alpha-only blur policy. |
| `box_blur_aex` | Box Blur wrapper and delayed `GF::FastBoxBlur` calls. |
| `drop_shadow_aex` | Drop Shadow offset/mask/softness wrapper and `CompositeShadowMask` dispatch. |
| `glow_aex` | Glow wrapper and `ImageRenderer` blur/composite path. |
| `imagerenderer_gaussian_composite` | Glow Gaussian/composite plumbing; final blend/premult policy still blocked by M19. |

Current Box Blur and Drop Shadow intermediate math is now partly binary-derived.
Glow threshold-source branches are instrumented and probe-backed, while final
Glow composite remains blocked on ImageRenderer/M19 behavior.

### Existing Native Functions / Trace Points

These are current native implementation/trace anchors, useful as target names
for parity work:

| Area | Native functions / trace fields |
| --- | --- |
| Box Blur | `BoxBlur2::render`, `BoxBlurParams::from_json`, `box_blur_debug_trace`, `blur_canvas_iterations`, `blur_pass_canvases`, `blur_radius`, trace hashes: `input_rgba`, `horizontal_pass_rgba`, `first_iteration_rgba` in newer traces, `output_rgba`; trace params include `radius`, `iterations`, `iterations_applied`, `kernel_radius`, `edge_policy`. |
| Drop Shadow | `DropShadow::render`, `DropShadowParams::from_json`, `drop_shadow_debug_trace`, `drop_shadow_offset`, `drop_shadow_blur_radius`, `raw_offset_shadow_canvas`, `alpha_mask_canvas`; trace hashes: `source_alpha_rgba`, `raw_offset_shadow_rgba`, `blurred_shadow_rgba`, `final_rgba`. |
| Glow | `Glow::render`, `GlowParams::from_json`, `GlowBasedOn::from_params`, `glow_debug_trace`, `glow_source`, `scale_canvas`; trace hashes: `threshold_source_rgba`, `blurred_glow_rgba`, `intensity_scaled_glow_rgba`, `final_rgba`. |
| Text Animator Blur | `ADBE Text Blur` appears in conformance JSX as `[10,10]`; current docs describe it as a per-unit splat blur approximation, not the same as effect Box Blur. |

## Parameters

Compatibility baseline is AE-style numbered params.

| matchName | Params inferred/used |
| --- | --- |
| `ADBE Box Blur2` | `0001` radius, `0002` iterations. Native also accepts named `radius`, `iterations`. Legacy native fallback can treat `0002` as radius only when no radius param exists. |
| `ADBE Drop Shadow` | `0001` color RGBA, `0002` opacity, `0003` direction degrees, `0004` distance, `0005` softness, `0006` shadow only. |
| `ADBE Glo2` | `0001` Glow Based On, `0002` threshold, `0003` radius, `0004` intensity. Current native maps `0001=1` to color channels, `0001=2` to alpha channel, absent `0001` to legacy combined source. |
| `ADBE Text Blur` | Text animator property value is a 2D vector, observed as `[10,10]` in `TXT_030`; current native applies a weighted per-unit splat radius. Exact AE glyph blur is still unknown. |

## Fixture / Probe Evidence

### Conformance Cases

| Case | Fixture | Params |
| --- | --- | --- |
| `EFF_010` | Drop Shadow on alpha square | color black, opacity `180`, direction `135`, distance `28`, softness `18`, shadow only `0`. |
| `EFF_020` | Glow on luma ramp | threshold `120`, radius `35`, intensity `1.25`; no `0001` based-on param. |
| `EFF_030` | Box Blur on impulse | radius `18`, iterations `3`. |
| `EFF_070` | Animated Box Blur + animated Glow | blur radius `1 -> 28`, iterations `2`; glow radius `10 -> 55`, threshold `160`, intensity `0.5`. |
| `STK_010` | Two Drop Shadows | first: opacity `210`, dir `135`, distance `8`, softness `8`; second: opacity `160`, dir `45`, distance `22`, softness `18`. |
| `STK_020` | Blur/Minimax order gate | Box Blur radius `10`, iterations `2` on one side before Minimax and on the other side after Minimax. |
| `TXT_030` | Text animator blur | `ADBE Text Blur` set to `[10,10]` with position/scale/rotation animator. |

Goldens exist under `fixtures/ae_conformance_pack/ae_goldens/png/`.

### Round 5 Glow / Drop Shadow AE Probe Outputs

Imported AE output root:

`fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/glow_shadow/ae_probe_outputs`

The pack contains 18 PNG and 18 TIFF frame-0 outputs. The builder applies effect
params by numeric property index.

Drop Shadow bbox observations from PNG alpha:

| Case | Meaning | Alpha bbox / pixels |
| --- | --- | --- |
| `DSH_010` | Source alpha square | `[200,200]-[311,311]`, `12544` px |
| `DSH_020` | AE no-softness shadow-only | `[219,219]-[330,330]`, `12544` px |
| `DSH_030` | AE no-softness final composite | `[200,200]-[330,330]`, `16439` px |
| `DSH_040` | AE softened shadow-only | `[209,209]-[340,340]`, `17292` px |
| `DSH_050` | AE softened final composite | `[200,200]-[340,340]`, `19260` px |
| `DSH_060` | Manual candidate dx=-20, dy=+20 | `[180,220]-[291,331]` |
| `DSH_061` | Manual candidate dx=+20, dy=+20 | `[220,220]-[331,331]` |

Key inference: for `direction=135`, `distance=28`, AE's raw shadow offset is
approximately `dx=+19`, `dy=+19`; `softness=18` expands the raw bbox by about
10 px each side.

Glow bbox observations from PNG alpha:

| Case | Meaning | Alpha bbox / pixels |
| --- | --- | --- |
| `GLO_010` | Opaque luma ramp source | `[96,146]-[415,365]`, `70400` px |
| `GLO_020` | Luma >= 120 mask candidate | `[256,146]-[415,365]`, `35200` px |
| `GLO_030` | Box-blurred luma mask candidate | `[166,56]-[505,455]`, `125756` px |
| `GLO_040` | Intensity-scaled candidate | `[166,56]-[505,455]`, `125756` px |
| `GLO_050` | AE final, opaque ramp | `[63,113]-[448,398]`, `109364` px |
| `GLO_060` | Alpha/luma split source | `[122,152]-[389,379]`, `30976` px |
| `GLO_070` | Alpha-split luma-rule mask candidate | `[122,152]-[389,379]`, `23232` px |
| `GLO_080` | Alpha-split alpha-rule mask candidate | `[122,152]-[389,379]`, `23232` px |
| `GLO_090` | AE final, default based-on on split source | `[89,119]-[422,412]`, `86283` px |
| `GLO_100` | AE final, `0001=1` | `[89,119]-[422,412]`, `74038` px |
| `GLO_101` | AE final, `0001=2` | `[89,152]-[422,412]`, `60404` px |

Key inference: `0001` is a real Glow branch point. `0001=1` and `0001=2`
produce distinct final alpha/RGB coverage on the alpha/luma split probe. The
default case also differs from both enum-specific outputs in coverage, so the
default may be combined/legacy behavior or an AE default not exactly mirrored by
native yet.

## Current Formula Model

### Box Blur / Fast Box Blur-ish

Current native model:

```text
radius_px = ceil(radius).clamp(0, 64)
iterations_applied = round(iterations).clamp(1, 64)

for each iteration:
  horizontal[x,y] = average(input[x-radius_px .. x+radius_px, y])
  output[x,y] = average(horizontal[x, y-radius_px .. y+radius_px])

edge policy = clipped sample window at layer bounds
average = integer floor(sum / count), per RGBA channel
```

Evidence:

- `EFF_030` with radius `18`, iterations `3` is exact on RGB and
  background-alpha-normalized metrics in earlier conformance runs, but this is a
  weak kernel proof because the fixture is degenerate.
- Current debug sidecars expose `horizontal_pass_rgba`; newer implementation
  also exposes first-iteration/final iteration distinction.
- Animated radius sampling is active: `EFF_070` frame 30 resolves blur radius
  `14.5`, kernel radius `15`, iterations `2`.

Unknowns:

- AE Fast Box Blur edge behavior: Adobe prose says repeat-edge style behavior,
  while current native trace says `clip_to_layer_bounds`.
- Whether AE's `iterations` are repeated full separable passes exactly as native
  now does, or if there are internal radius/quality adjustments.
- Fractional-radius kernel weights in GF FastBoxBlur V2. Native now uses the
  recovered ceil quantization for the integer subset, but does not emulate the
  V2 fractional delta path.
- Whether Glow/Drop Shadow internally reuse exactly the Box Blur2 kernel.

### Drop Shadow

Current native target after Round 5 ingest:

```text
opacity_norm =
  if opacity > 100: clamp(opacity / 255, 0, 1)
  else:             clamp(opacity / 100, 0, 1)

dx = trunc(-cos(direction_degrees) * distance)
dy = trunc( sin(direction_degrees) * distance)

for each source pixel with alpha > 0:
  dst = (x + dx, y + dy)
  shadow_alpha = round(source_alpha * opacity_norm * color_alpha)
  shadow_rgb = color_rgb
  if multiple source pixels map to dst, keep max alpha

if softness > 0:
  blur_radius = ceil(softness / 2).clamp(0,64) + 1
  shadow_alpha = box_blur(shadow.alpha, blur_radius)
  shadow = color_rgb with shadow_alpha
else:
  blur_radius = 0

if shadow_only:
  output = shadow
else:
  output = normal_composite(source over shadow)
```

Evidence:

- `DSH_020` raw AE shadow bbox `[219,219]-[330,330]` from source
  `[200,200]-[311,311]` resolves to `dx=+19`, `dy=+19`.
- `DSH_040` softened bbox `[209,209]-[340,340]` implies radius about `10` for
  `softness=18`.
- Native sidecar after ingest for `EFF_010` resolves `dx=19`, `dy=19`,
  `blur_radius=10`, `opacity_normalized=0.705882`.
- Focused conformance moved `EFF_010` RGB mean from `0.1635` to `0.1301`; still
  close but not exact.

Unknowns:

- Softness kernel: AE expansion matches radius 10, but exact box/gaussian-ish
  weights and iteration count are not proven.
- Opacity units: byte-style `180/255` fits the fixture, but UI percent vs
  payload byte values need more cases (`50`, `100`, `128`, `255`).
- Subpixel offsets/rounding for non-integer direction-distance tuples.
- Composite/premult behavior for colored translucent shadows and overlapping
  source/shadow pixels.
- Layer expansion behavior when shadow extends outside the original canvas.

### Glow

Current native model:

```text
threshold = clamp(param_0002, 0, 255)
kernel_radius = ceil(radius / 2).clamp(0, 64)
intensity = max(param_0004, 0)

source pixel passes when:
  combined/default: luminance >= threshold OR alpha >= threshold
  color_channels:   luminance >= threshold
  alpha_channel:    alpha >= threshold

luminance = 0.2126*r + 0.7152*g + 0.0722*b

threshold_source = original pixel if passes else transparent
blurred = box_blur(threshold_source, kernel_radius)
scaled.rgb = round(blurred.rgb * intensity)
scaled.a   = round(blurred.a * clamp(intensity, 0, 8))
output = normal_composite(input over scaled_glow)
```

Evidence:

- `EFF_020` native sidecar resolves threshold `120`, radius `35`, kernel radius
  `18`, intensity `1.25`; with absent `0001`, native
  `threshold_source_rgba == input_rgba`.
- Round 5 AE probes show `0001=1` and `0001=2` are distinct on final output.
- Default AE final on alpha/luma split (`GLO_090`) has broader/different
  coverage than `0001=1` (`GLO_100`) and `0001=2` (`GLO_101`), so native's
  default combined branch is plausible but unproven.
- `EFF_020` remains the largest isolated effect mismatch: RGB mean `12.6578`,
  RGB RMSE `32.1739` in the recent metrics.

Unknowns:

- Exact default `Glow Based On` semantics for absent `0001`.
- Whether AE thresholds luma, max RGB, premultiplied RGB, unpremultiplied RGB,
  alpha, or a combined mask at each enum value.
- Exact radius mapping: native uses `radius / 2`; AE final spread does not yet
  prove this.
- Intensity scaling and clamp constants.
- Final blend mode/composite order. Native uses normal source-over; AE Glow may
  use additive/screen-like color in some regimes.
- Color management / 8bpc premult-straight differences in TIFF/PNG export.

### Text Animator Blur

Current native model is separate from effect blur:

```text
per_unit_blur = animator_blur * selector_weight
render-core applies a bounded per-unit splat around text alpha/glyph bounds
```

Evidence:

- `TXT_030` uses `ADBE Text Blur` value `[10,10]`.
- Earlier native bug divided/clamped this too aggressively; later pass reports
  weighted radii in the `0..10` range and frame-0 bbox moved closer to AE.

Unknowns:

- Whether AE text blur is separable, gaussian-ish, glyph-mask-space, or
  post-transform layer-space.
- How the two vector components map to horizontal/vertical blur.
- Exact ordering relative to per-character transform, opacity, and source
  compositing.

## Proposed Native Implementation Targets

1. Keep Box Blur as the shared primitive but split edge policy behind an enum:
   `clip_to_layer_bounds` vs `repeat_edge`. Add tests before flipping default.
2. Preserve current Drop Shadow offset/softness fixes as the working target:
   `dx=trunc(-cos*d)`, `dy=trunc(sin*d)`, `softness -> round(s/2)+1`.
3. Add Drop Shadow fixture sweeps before further tuning:
   direction `0/45/90/135/180/225/270/315`, distances with fractional projected
   components, softness `0/1/2/8/18/32`, opacity `50/100/128/180/255`, and
   colored translucent shadow.
4. For Glow, do not tune radius/intensity/blend until `threshold_source_rgba`
   is proven against AE for absent `0001`, `0001=1`, and `0001=2`.
5. Add a tiny two-pixel AE Glow probe matching the native unit test:
   dark/high-alpha `[32,32,32,255]` and bright/low-alpha `[240,240,240,64]`,
   threshold `120`, radius `0`, intensity `1`.
6. Convert Round 5 Glow/Drop Shadow PNG observations into conformance fixtures
   or metadata so future tuning can compare named intermediates, not only final
   pixels.
7. Treat Text Animator Blur as its own module until a probe proves it reuses
   Box Blur2/Glow kernels.

## Needed Fixtures / Goldens

High priority:

- Box Blur impulse/edge pack:
  - impulse centered and near each edge/corner;
  - radius `0/0.49/0.5/1/2/10/18`;
  - iterations `1/2/3`;
  - transparent edge vs opaque edge to separate RGB/alpha policy.
- Glow source-mask pack:
  - two-pixel dark-high-alpha / bright-low-alpha source;
  - absent `0001`, `0001=1`, `0001=2`;
  - threshold `0/64/120/255`;
  - radius `0` first, then `1/10/35`;
  - straight and premult export hashes if AE can provide both.
- Drop Shadow geometry pack:
  - no-softness shadow-only for direction/distance rounding;
  - shadow-only softened for kernel mapping;
  - final composite for premult/source-over behavior;
  - colored shadow and partial-alpha source.

Medium priority:

- `STK_010` per-effect sidecars for both Drop Shadows so stack behavior is not
  inferred from final pixels.
- `STK_020` blur-before/after morphology sidecars for Box Blur edge policy.
- `TXT_030` text blur micro-scenes with single glyph, no transform, then
  transform, then animator weights, to separate glyph blur from selector math.

## Current Confidence

| Area | Confidence | Reason |
| --- | --- | --- |
| Match names and param ids | High | Confirmed across docs, JSX, and native parser contracts. |
| Drop Shadow direction and softness mapping for `135/28/18` | Medium-high | Direct Round 5 AE bbox evidence; exact kernel still unknown. |
| Box Blur separable box structure | Medium | Native fixture is exact on one impulse case, but edge/fraction/iteration behavior remains underconstrained. |
| Glow `0001` branch existence | High | Round 5 AE outputs for `GLO_090/100/101` differ materially. |
| Glow default/source/radius/intensity/blend math | Low-medium | Strong diagnostics exist, but first AE intermediate is not isolated yet. |
| Text Animator Blur exact math | Low | Current implementation is explicitly a splat approximation. |

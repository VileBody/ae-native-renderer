# Agent A Substrate / Global Guardrails Passport

Status date: 2026-05-04

## Scope

Assigned objects:

- `M01` Timeline layer activity, z-order, opacity compositing.
- `M02` Footage source-time sampling and media frame selection.
- `M19` Color, alpha, sampling, gamma assumptions.

Write scope for this pass was limited to this report. Evidence root:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

Primary objective: recover the shared pixel/alpha contract around
`GPUFoundation.dll` and `ImageRenderer.dll` enough to decide whether current
native straight `RGBA8` + normal source-over assumptions are safe for formula
tuning.

Conclusion: `M19` is not locked. Current native straight `RGBA8` can remain an
implementation approximation for non-alpha-sensitive scaffolding, but the
reverse evidence shows shared alpha/pixel-format paths that are richer than a
single straight `RGBA8` model. Alpha/composite formula tuning should pause until
the smallest alpha-ramp probe below is run. Per Agent D's text/glyph report,
text animator blur is also a direct consumer of this substrate contract; its
top-spread/blur mismatch should not be tuned as a local `M07` formula until the
alpha/premult/composite policy is stated.

Confidence labels used below: `confirmed`, `inferred`, `unknown`.

## Evidence Table

| Evidence | Address / function | Finding | Confidence | Affected modules |
| --- | --- | --- | --- | --- |
| `core_alpha_gpufoundation/01_GF_Composite_18002af50/decompile.c` | `18002af50`, `GF::Composite` | Dispatches by `IR_BlendMode` (`param_12`) across cases `0..0x1d`; mode `0x12` has a memcpy fast path when opacity is <= epsilon, otherwise loads a blend kernel. Two bool-like flags are passed into the kernel argument block. | confirmed | `M01`, `M19`, blend/effect modules |
| `core_alpha_gpufoundation/05_GF_AlphaGain_180022570/decompile.c` | `180022570`, `GF::AlphaGain` | Gain <= epsilon fills transparent black; gain approximately >= 1 performs device memcpy; otherwise dispatches alpha-gain kernel with float gain. | confirmed | `M01`, `M10`, `M11`, `M19` |
| `core_alpha_gpufoundation/06_GF_PackedAlphaGain_180022cc0/decompile.c` | `180022cc0`, `GF::PackedAlphaGain` | Same branch structure as `AlphaGain`, but separate packed-alpha kernel. | confirmed | `M01`, `M19` |
| `core_alpha_gpufoundation/07_GF_Unpremultiply_180022fc0/decompile.c` | `180022fc0`, `GF::Unpremultiply` | Dedicated unpremultiply kernel; stride logic recognizes component depths 8, 10, 16, 24, and 32 bits plus packed/planar pixel-format flags. Takes a `float4` parameter. | confirmed | `M19`, all pixel math |
| `core_alpha_gpufoundation/08_GF_BlendUnpackedAlpha_18002c620/decompile.c` | `18002c620`, `GF::BlendUnpackedAlpha` | Dedicated unpacked-alpha blend kernel; final boolean parameter is forwarded into the kernel args. | confirmed | `M01`, `M19` |
| `blur_gpufoundation_kernels/03_GF_BoxBlurOptions_GetDestAlphaType_180025150/decompile.c` | `180025150`, `GetDestAlphaType` | Destination alpha type is stored at `BoxBlurOptions + 8`. | confirmed | `M10`, `M11`, `M13`, `M19` |
| `blur_gpufoundation_kernels/04_GF_BoxBlurOptions_GetSrcAlphaType_180025280/decompile.c` | `180025280`, `GetSrcAlphaType` | Source alpha type is stored at `BoxBlurOptions + 4`. | confirmed | `M10`, `M11`, `M13`, `M19` |
| `blur_gpufoundation_kernels/05_GF_BoxBlurOptions_SetBlurAlphaChannelOnly_180025350/decompile.c` | `180025350`, `SetBlurAlphaChannelOnly` | Clears option bits `0xe` and sets bit `0x1`; this strongly suggests alpha-channel-only mode is enum/value `1` in the low alpha-option field. | confirmed/inferred | shared blur users |
| `blur_gpufoundation_kernels/06_GF_FastBoxBlur_180031c30/decompile.c` | `180031c30`, `GF::FastBoxBlur` | Uses option bits `0x20` and `0x40` to choose one-pass vs two-pass blur and toggles those bits between passes; transparent black fill appears for some padded alpha cases. | confirmed | shared blur users |
| `blur_gpufoundation_kernels/07_GF_GaussianBlur_180036430/decompile.c` | `180036430`, `GF::GaussianBlur` | Direction/radius gates choose copy/fill vs horizontal/vertical kernels. `param_13`/`param_15` are forwarded as flags. Pixel-format alpha bits `(& 0xe) == 6` select alternate kernels. | confirmed | `M10`, `M11`, `M19` |
| `imagerenderer_gaussian_composite/01_IR_GaussianBlur_impl_18009fca0/decompile.c` | `18009fca0`, `ImageRenderer` Gaussian impl | Validates source/dest pixel-format alpha groups using bits `0x30`, `0xc`, and `0x1c0`; mismatched alpha layout returns error `-4`. Sets MXCSR flags before float math. | confirmed | `M19`, blur/effect users |
| `imagerenderer_gaussian_composite/03_IR_composite_worker_select_1800769b0/decompile.c` | `1800769b0`, composite worker select | Selects one of eight CPU composite workers based on three bool fields at offsets `+8`, `+9`, `+10`. Called by `IR_Composite` and `IR_CompositeWithBlendMode`. | confirmed | `M01`, `M19` |
| `imagerenderer_gaussian_composite/06_IR_pixel_format_map_180079800/decompile.c` | `180079800`, pixel-format map | Maps and sometimes normalizes pixel-format codes (`4`, `0x104`, `0x805`, `0x905`, `0xa05`, `0xb05`, `0x1004`, `0x1104`) based on source format and depth-like parameter. Called by `IR_CompositeToBlack` and `IR_CompositeToBlackAndTestSourceAlpha`. | confirmed | `M19` |
| `docs/MATH_PARITY_STATUS.md` | module registry | Current native state is `implemented approximate`; `M19` currently says straight `RGBA8` canvas, normal composite, deterministic PNG output, with premult/straight audit and gamma decision still next steps. | confirmed | all |

## Object Passports

### M01: Timeline Layer Activity, Z-order, Opacity Compositing

Parameter mapping:

- `confirmed`: Current project status says deterministic layer activity, reverse
  layer order, normal alpha composite, and render logs exist.
- `confirmed`: `GF::Composite` signature includes source pointer/stride, dest
  pointer/stride, `PixelFormat`, width/height, float opacity, `IR_BlendMode`,
  and two boolean flags.
- `inferred`: `param_11` is opacity/gain-like float in `GF::Composite`; `param_12`
  is `IR_BlendMode`; `param_13` and `param_14` are alpha/composite behavior flags
  passed to kernels.

Coordinate/time/color space:

- `confirmed`: Composite operates over explicit integer width/height and strides.
- `unknown`: Native AE layer activity timing is not refined by this evidence.
- `unknown`: Whether layer compositing should occur in straight or premult color
  for the current AE templates remains unresolved.
- `unknown`: No direct gamma/linear-light proof was found in `GF::Composite`; the
  existence of `ImageRenderer` pixel-format map means channel format conversion
  cannot be assumed absent globally.

Sampling rule:

- `confirmed`: Composite is a per-pixel kernel dispatch; device workgroup shape
  changes by GPU/device fields.
- `unknown`: CPU worker formulas behind the eight `ImageRenderer` composite
  paths were not part of this bundle.

Alpha/premult policy:

- `confirmed`: `GF::Composite` is not one hard-coded normal source-over routine;
  it dispatches blend kernels for modes `0..0x1d`.
- `confirmed`: `GF::AlphaGain` and `GF::PackedAlphaGain` treat zero opacity as
  transparent black and approximately full opacity as a copy.
- `confirmed`: There are separate packed/unpacked alpha and unpremultiply paths.
- `unknown`: Exact formula for AE normal source-over, whether source RGB is
  interpreted straight or premult at the layer boundary, and how the two boolean
  composite flags map to AE switches.

Formula / pseudocode:

```text
if width <= 0 or height <= 0:
    return ok

if blend_mode == 0x12 and opacity <= eps:
    copy src -> dst
else:
    kernel = load_kernel_for(blend_mode, device, pixel_format)
    if kernel missing:
        return unsupported
    dispatch kernel(src, dst, pixel_format, width, height, opacity, flag_a, flag_b)
```

Native implementation delta:

- Current native `normal alpha composite` is still an approximation relative to
  the recovered substrate because the mode enum, packed/unpacked alpha handling,
  and premult/straight boundary are not locked.

Next probe:

- 2x2 and 4x1 alpha-ramp layer over opaque black, opaque white, and transparent
  background with RGB greater than alpha (`rgba=(255,0,0,64)` and
  `(128,255,0,128)`), opacity `0`, `0.5`, `1.0`, and one overlap. Log native
  source/dest sample values before and after composite and compare to AE PNG.

### M02: Footage Source-Time Sampling And Media Frame Selection

Parameter mapping:

- `confirmed`: `docs/MATH_PARITY_STATUS.md` says current native has
  `source_start`, activity windows, sequential decode/cache, and media plan logs.
- `unknown`: This evidence bundle did not include a source-time selection
  function; no additional frame-index formula was recovered here.

Coordinate/time/color space:

- `unknown`: No new source-time or frame-rate boundary evidence in the assigned
  folders.
- `inferred`: `M02` can continue independently from the alpha guardrail for
  frame-index telemetry, because the blocker is pixel interpretation after a
  frame is selected, not which source frame is selected.

Sampling rule:

- `unknown`: Current sequential decode/cache behavior remains documented as
  approximate; exact AE frame rounding/boundary behavior needs a numbered-frame
  source fixture.

Alpha/premult policy:

- `unknown`: Source footage decode may enter the shared pixel model as straight,
  premult, or format-tagged pixels; this bundle does not prove the import-side
  contract.

Formula / pseudocode:

```text
source_frame_index = current_native_approx(
    comp_time,
    layer_in_out,
    source_start,
    media_plan
)

# Pixel interpretation after decode is blocked by M19.
```

Native implementation delta:

- No new delta recovered for frame-time math. `M02` remains `implemented
  approximate`.

Next probe:

- Numbered-frame footage fixture with alpha-coded pixels. Record selected source
  frame index separately from post-decode pixel values so time sampling and
  alpha/premult import do not get conflated.

### M19: Color, Alpha, Sampling, Gamma Assumptions

Parameter mapping:

- `confirmed`: Shared GPUFoundation alpha functions take `PixelFormat` and
  dimensions/strides, not just raw `RGBA8`.
- `confirmed`: `BoxBlurOptions` includes separate source alpha type at offset
  `+4` and destination alpha type at offset `+8`.
- `confirmed`: `SetBlurAlphaChannelOnly` forces the low option field to value
  `1` after clearing bits `0xe`.
- `inferred`: Option bits `0x20` and `0x40` in `FastBoxBlur` represent pass
  direction/intermediate alpha handling; both set causes a two-pass temporary
  path.

Coordinate/time/color space:

- `confirmed`: Shared kernels handle device-dependent tiling (`1`, `16`, `64`
  width groups depending on device/pixel-format state).
- `confirmed`: `Unpremultiply` has stride logic for 8/10/16/24/32-bit component
  layouts.
- `confirmed`: `ImageRenderer` maps several pixel-format codes and rejects
  mismatched alpha-layout groups in Gaussian blur.
- `unknown`: sRGB vs linear-light compositing/gamma conversion is not directly
  proven. However, the pixel-format map means hidden channel conversion remains
  plausible and must be probed.

Sampling rule:

- `confirmed`: Gaussian blur has a transparent-black fill path when both radii
  are inactive and source/dest extents differ; otherwise it copies with offsets.
- `confirmed`: Fast box blur can fill transparent black around an intermediate
  buffer when alpha/padding options require it.
- `unknown`: OOB/sampler policy for effect-specific kernels is not fully
  recovered here; this pass only establishes that shared blur has explicit alpha
  options.

Alpha/premult policy:

- `confirmed`: The shared substrate contains explicit `Unpremultiply`,
  `BlendUnpackedAlpha`, `AlphaGain`, and `PackedAlphaGain` entry points.
- `confirmed`: Alpha gain semantics include transparent-black at zero and memcpy
  at full gain.
- `confirmed`: Blur has explicit source/destination alpha types and an
  alpha-channel-only option.
- `unknown`: The global contract at renderer boundaries is not yet known:
  straight import, premult import, packed alpha, or conversion before/after
  effects are all still possible for different pixel formats.

Formula / pseudocode:

```text
pixel_format = format_tagged_value
alpha_type = {src_alpha_type, dest_alpha_type, option_bits}

if operation == alpha_gain:
    if gain <= eps: fill transparent black
    elif gain >= 1 - eps: copy
    else: dispatch gain kernel

if operation == blur:
    choose copy/fill/kernel from radii, direction, alpha option bits, pixel_format

if operation == composite:
    choose blend kernel by IR_BlendMode and pixel_format
```

Native implementation delta:

- Current straight `RGBA8` + normal source-over model is not safe enough for
  formula tuning of alpha-sensitive modules, including text animator blur.
  It may remain as a temporary implementation approximation for scaffolding
  and non-pixel-contract work.

Next probe:

- Same as `M01`, plus one blur probe: isolated alpha impulse and RGB impulse
  with source alpha type variants if exposed. Expected output should tell
  whether blur spreads RGB independently, premultiplies before blur, or blurs
  alpha only in the shared path.
- Include a text-raster blur variant from Agent D's `M07` case: glyph or simple
  text shape with partially covered edges and animator blur/top-spread enabled,
  logging pre-blur coverage/alpha, post-blur intermediate, and final composite.
  This separates glyph geometry from substrate alpha/composite policy.

## Guardrail Findings

### BLOCKER

title: `M19` straight `RGBA8` / normal composite contract is not locked

affected objects: `M19`, `M01`; formula tuning for text animator blur (`M07`),
`M10`, `M11`, `M13`, `M17` collapsed/precomp composites, `M18` premult
accumulation, and final template pixel thresholds should wait for this
guardrail. `M05` glyph metrics remain primarily Agent D scope, but glyph edge
coverage/final PNG comparison consumes the `M19` alpha/color contract.

evidence paths:

- `target/reverse/predecoded/20260504_222748_full_predecode_round2/core_alpha_gpufoundation/01_GF_Composite_18002af50/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/core_alpha_gpufoundation/05_GF_AlphaGain_180022570/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/core_alpha_gpufoundation/06_GF_PackedAlphaGain_180022cc0/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/core_alpha_gpufoundation/07_GF_Unpremultiply_180022fc0/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/core_alpha_gpufoundation/08_GF_BlendUnpackedAlpha_18002c620/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/blur_gpufoundation_kernels/03_GF_BoxBlurOptions_GetDestAlphaType_180025150/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/blur_gpufoundation_kernels/04_GF_BoxBlurOptions_GetSrcAlphaType_180025280/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/blur_gpufoundation_kernels/05_GF_BoxBlurOptions_SetBlurAlphaChannelOnly_180025350/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/imagerenderer_gaussian_composite/01_IR_GaussianBlur_impl_18009fca0/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/imagerenderer_gaussian_composite/06_IR_pixel_format_map_180079800/decompile.c`

exact address/function:

- `18002af50`, `GF::Composite`
- `180022570`, `GF::AlphaGain`
- `180022cc0`, `GF::PackedAlphaGain`
- `180022fc0`, `GF::Unpremultiply`
- `18002c620`, `GF::BlendUnpackedAlpha`
- `180025150`, `GF::BoxBlurOptions::GetDestAlphaType`
- `180025280`, `GF::BoxBlurOptions::GetSrcAlphaType`
- `180025350`, `GF::BoxBlurOptions::SetBlurAlphaChannelOnly`
- `18009fca0`, `ImageRenderer` Gaussian blur implementation
- `180079800`, `ImageRenderer` pixel-format map

what assumption broke:

- The current parity document records `M19` as straight `RGBA8` canvas and
  normal composite, but the shared native substrate has explicit
  unpremultiply, packed/unpacked alpha, source/destination alpha type, alpha
  channel only blur, multiple pixel-format codes, and component-depth handling.
  That does not prove AE output is premultiplied, but it does break the idea
  that a single straight `RGBA8` model is confirmed enough for formula tuning.

hypotheses:

- H1: AE/ImageRenderer stores pixels in a tagged format and converts between
  straight and premult only around specific operations.
- H2: The current templates usually land in an `RGBA8` straight path, but blur
  and composite use alpha-option flags that change RGB treatment under partial
  alpha.
- H3: `IR_BlendMode == 0x12` may be copy/normal-like only under a particular
  opacity/flag combination; normal source-over enum mapping still needs proof.
- H4: Pixel-format map codes encode bpc and alpha layout; gamma/linear-light is
  still unknown, not disproven.
- H5: Text animator blur/top-spread mismatch may be caused by substrate alpha
  treatment around glyph edge coverage and blur, not by `M07` animator geometry
  alone.

smallest probe/test needed:

- Render a tiny AE project with two layers:
  - bottom: opaque black, opaque white, and transparent cells;
  - top: straight-looking RGB > alpha cells, for example `(255,0,0,64)`,
    `(0,255,0,128)`, and `(255,255,255,0)`;
  - opacity: `0`, `50`, `100`;
  - mode: Normal only first.
- Add one isolated blur probe: alpha impulse with colored RGB under partial
  alpha, radius `1` and `4`, plus alpha-only source if available.
- Add one glyph/text blur probe from Agent D's `M07` case with partially covered
  glyph edges, animator blur/top-spread, and the same pre/post alpha telemetry.
- Native telemetry must dump decoded source pixel, pre-composite/effect input,
  post-effect intermediate, and final pixel in integer and normalized float
  forms.

can continue on unrelated work: yes. Safe work includes `M02` frame-index
telemetry, matrix/UV/temporal/glyph/selector structural passports, and effect
parameter mapping that does not claim final RGB/alpha parity. Do not tune
alpha-sensitive final formulas, including text animator blur/top-spread, or
thresholds until this probe resolves `M19`.

### Shared Contract For Other Agents Until Probe Resolves

- `confirmed`: Treat current native straight `RGBA8` as an approximation, not a
  recovered AE contract.
- `confirmed`: Shared blur/composite paths have explicit alpha policy knobs.
- `confirmed`: Effects using `GPUFoundation.dll` or `ImageRenderer.dll` are
  cross-object dependencies on `M19`.
- `confirmed`: Text raster/animator blur is also a consumer of `M19` whenever
  glyph coverage alpha is blurred or composited into the final canvas.
- `unknown`: sRGB vs linear, exact normal blend enum, exact source-over formula,
  and exact import/export premult status.
- `safe to analyze now`: source-time frame selection (`M02`), transform matrices
  and UV fields (`M03`, `M12`), keyframe timing (`M04`), text layout/selector
  weights (`M05`-`M08`) excluding final blurred edge/pixel tuning,
  temporal/expression timing (`M09`, `M15`, `M16`), collapse graph topology
  (`M17`), and motion-blur sample schedule (`M18`).
- `not safe for formula tuning yet`: layer composite (`M01`), Drop Shadow
  composite/blur mask (`M10`), Glow threshold/composite (`M11`), text animator
  blur/top-spread (`M07`), glyph edge final PNG thresholds where alpha/color
  coverage is the measured output (`M05`), Minimax alpha behavior (`M13`),
  collapsed-precomp final composite (`M17`), motion-blur accumulation (`M18`),
  and template-level final pixel thresholds.

## Recommendation

`blocked_by_guardrail`

`M19` should be resolved before alpha/composite formula tuning. The blocker is
small and testable: run an alpha-ramp/composite probe plus one blur alpha-policy
probe and the `M07` text blur variant, then update the substrate contract. `M02`
can continue toward `instrumented/testable` independently because this pass
found no new source-time contradiction.

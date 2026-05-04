# AE Reverse: Blur / Glow / Shadow / Minimax Ghidra Checkpoint

Status: checkpoint, 2026-05-04. This report intentionally captures the
intermediate result before formula tuning. Ghidra is the primary evidence;
probes/conformance are validation only.

## Scope

Agent D scope:

- Box Blur / Gaussian Blur radius mapping, edge policy, kernel shape.
- Drop Shadow offset, softness, opacity, alpha handling, composite order.
- Glow threshold/radius/intensity/blend/source mask.
- Minimax operation/channel/direction/neighborhood behavior.
- Time-aware animated effect params.

Raw outputs live under `target/reverse/agent_blur_glow_minimax/`.
Ghidra projects live under `target/reverse/ghidra_projects/agent_blur_glow_minimax/`.
The triage script is
`target/reverse/agent_blur_glow_minimax/scripts/AgentDBlurGlowMinimaxTriage.java`.

## Binaries

| Binary | SHA-256 |
| --- | --- |
| `target/reverse/ae_2026/effects_blur_glow_shadow/Box_Blur.aex` | `aa8f2eb1489ebb492ee2d37873bfec394cfeb7dcfbcf6fa8f29af0e4a7d46b98` |
| `target/reverse/ae_2026/effects_blur_glow_shadow/Gaussian_Blur.aex` | `28be268bc78f2b3784dc3bee9d4977618ed61ddd7169f9949862b135a83aaf6e` |
| `target/reverse/ae_2026/effects_blur_glow_shadow/Glow.aex` | `bb8f60143cbb342fdb3c074a129647b65c5e593772c305593707e0dee9a9b2d4` |
| `target/reverse/ae_2026/effects_blur_glow_shadow/Drop_Shadow.aex` | `f5ef57b00fa3607125c5ede95766af6d84b5612fdadfb9de3b6d6c1495d0f9fe` |
| `target/reverse/ae_2026/effects_temporal_noise_distort/Minimax.aex` | `95afa7a3b4a539e8389c60901149f285e8cd3e809d9ad11a08f80d979fa19b32` |

`GPUFoundation.dll` and `ImageRenderer.dll` are available locally:

- `target/reverse/ae_2026/GPUFoundation.dll`
- `target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll`
- `target/reverse/ae_2026/core_composite_alpha/ImageRenderer.dll`

## Executive Result

The main result of this round: the effect `.aex` files are not always the
place where the core math lives.

- Box Blur delegates the real blur to `GPUFOUNDATION.DLL::GF::FastBoxBlur`
  through `GF::BoxBlurOptions::StandardOptions`.
- Drop Shadow also delegates softness blur to `GF::FastBoxBlur`, but with
  `GF::BoxBlurOptions::SetBlurAlphaChannelOnly`, so alpha-only filtering is
  confirmed.
- Glow delegates blur and composite to `IMAGERENDERER.DLL::IR_GaussianBlur`
  and `IMAGERENDERER.DLL::IR_CompositeWithBlendMode`; our current native
  box-blur-plus-normal-composite model is structurally wrong for AE parity.
- Gaussian Blur Legacy acquires `FLT Blur Suite`; the legacy kernel path is
  suite-provided, not fully local in `Gaussian_Blur.aex`.
- Minimax has enough local evidence to expand the native enum surface now:
  four operations, five channel modes, three direction modes, and a
  "Don't Shrink Edges" flag. Exact neighborhood/edge behavior still needs
  callback/kernel decompile.

## Progress Chunks

### Chunk 1: shared-DLL targets started

Time: 2026-05-04, after checkpoint acceptance.

Targeted Ghidra script added:

- `target/reverse/agent_blur_glow_minimax/scripts/AgentDCoreMathTargeted.java`

New raw logs:

- `target/reverse/agent_blur_glow_minimax/gpufoundation_targeted.log`
- `target/reverse/agent_blur_glow_minimax/imagerenderer_targeted.log`
- `target/reverse/agent_blur_glow_minimax/minimax_targeted.log`

First launch failed because the new project directories did not exist. This was
fixed with:

```text
mkdir -p \
  target/reverse/ghidra_projects/agent_blur_glow_minimax/GPUFoundation \
  target/reverse/ghidra_projects/agent_blur_glow_minimax/ImageRenderer \
  target/reverse/ghidra_projects/agent_blur_glow_minimax/Minimax_Targeted
```

Current runs launched:

```text
GPUFoundation.dll -> GF::FastBoxBlur / BoxBlurOptions targets
ImageRenderer.dll -> IR_GaussianBlur / IR_CompositeWithBlendMode targets
Minimax.aex      -> CPU callbacks and setup struct targets
```

Shared binary hashes now fixed:

| Binary | SHA-256 |
| --- | --- |
| `target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll` | `36ea8222bc1e3abae148771cbea03eb60cd4bbcc708e145af5dd9451782d2466` |
| `target/reverse/ae_2026/core_composite_alpha/ImageRenderer.dll` | `daaf1ec0996126fec10159490ea5eb93323e399e8ee4030bfde3259586887883` |

Fast string scan before Ghidra completion already confirms relevant symbol and
kernel names in `GPUFoundation.dll`:

- `GF.FastBoxBlurV2`
- `mBlurRadiusVert=`
- `mBlurRadiusHoriz=`
- `BoxBlurKernelWithPremultiply`
- `FastBoxBlurPremultiply`
- `BoxBlurKernel`
- `FastBoxBlur`
- `BoxBlurKernelHalfV2`
- `BoxBlurKernelFloatV2`
- exported/demangled strings for `GF::FastBoxBlur`,
  `GF::BoxBlurOptions::StandardOptions`,
  `GF::BoxBlurOptions::StandardOptionsExt`,
  `GF::BoxBlurOptions::SetBlurAlphaChannelOnly`,
  `GF::BoxBlurOptions::GetRepeatEdge`,
  `GF::BoxBlurOptions::GetSrcAlphaType`,
  `GF::BoxBlurOptions::GetDestAlphaType`.

Fast string scan also confirms relevant `ImageRenderer.dll` exports:

- `IR_GaussianBlur`
- `IR_CompositeWithBlendMode`
- `IR_Composite`
- `IR_SetAlphaChannel`
- `IR_UnPremultiplyAgainstMatteColor`
- `IR::ResampleKernel::*`

Immediate interpretation:

- Box/Drop Shadow next math is definitely in `GPUFoundation.dll`.
- Glow next math is definitely in `ImageRenderer.dll`.
- The next useful evidence is decompile output, not another AE probe pack.

### Chunk 2: Minimax CPU/GPU targeted decompile

Time: 2026-05-04, after Chunk 1.

Additional helper and raw logs:

- `target/reverse/agent_blur_glow_minimax/scripts/AgentDExactAddresses.java`
- `target/reverse/agent_blur_glow_minimax/minimax_targeted.log`
- `target/reverse/agent_blur_glow_minimax/minimax_exact_helpers.log`

What this revealed:

- The dispatcher at `FUN_18000c2a0` routes by pixel depth:
  - 8 bpc -> `FUN_1800060b0`
  - 16 bpc -> `FUN_180004ef0`
  - 32 bpc -> `FUN_180007270`
- The callback param block carries at least:
  - horizontal/vertical radius fields,
  - operation/comparator byte,
  - channel bitmask,
  - direction/axis byte,
  - edge/shrink flag byte,
  - pass/phase flag for the two-pass operations.
- The active radius is coerced to at least `1`; window width is built as
  `2 * radius + 1`.
- Channel handling is a bitmask, not a simple enum branch. The CPU callbacks
  allocate/use per-channel queues only for enabled channel bits.
- Helper `FUN_180001db0` is an ROI/edge helper. It expands or contracts the
  processing rectangle by horizontal/vertical radius and then intersects it
  against source/dest bounds. This is the current best evidence for
  "Don't Shrink Edges" behavior.
- Exact comparator helpers:
  - `FUN_18000d710(byte*, byte*)` returns `lhs < rhs`.
  - `FUN_18000d720(ushort*, ushort*)` carries the same less-than predicate for
    16 bpc, with Ghidra typing noise in the returned high byte.
  - `FUN_18000d730(float*, float*)` returns `lhs < rhs`.
- GPU path `FUN_18000c4b0` loads:
  - `AEFX_Minimax / MinimaxBuildSTreeLeafNodesKernel`
  - `AEFX_Minimax / MinimaxBuildSTreeNonLeafNodesKernel`
  - `AEFX_Minimax / MinimaxTraverseSTreeKernel`
  and builds a spatial-tree window around `2 * radius + 1`, clamped to the
  active dimension.

Current interpretation:

- Min/Max polarity is now localized to comparator selection and pass order, not
  hidden in per-pixel arithmetic.
- Native implementation should model Minimax as separable/directional
  morphology over selected channels with an explicit edge policy.
- Exact enum polarity still needs one more setup-pass read: which AE operation
  value selects less-than vs greater-than and which order is used for
  min-then-max / max-then-min.

Confidence:

- High: depth dispatch, channel bitmask existence, less-than comparator helper.
- Medium/high: radius window is `2r + 1`.
- Medium: "Don't Shrink Edges" is ROI expansion/clipping policy.
- Low/medium: final operation enum polarity until setup struct writes are fully
  named.

Next Minimax target:

1. Decompile setup writes immediately before `PF_IterateGeneric`.
2. Map AE enum values to comparator polarity and pass order.
3. Validate with `EFF_050` split cases: one channel at a time, one direction at
   a time, then two-pass operations.

### Chunk 3: ImageRenderer Glow/composite targeted decompile

Time: 2026-05-04, after Chunk 1.

Raw log:

- `target/reverse/agent_blur_glow_minimax/imagerenderer_targeted.log`

Additional exact helper completed:

- `target/reverse/agent_blur_glow_minimax/scripts/AgentDImageRendererExact.java`
- `target/reverse/agent_blur_glow_minimax/imagerenderer_exact.log`
- `target/reverse/agent_blur_glow_minimax/imagerenderer_exact_full.log`
- `target/reverse/agent_blur_glow_minimax/imagerenderer_constants.log`

What this revealed so far:

- `IR_GaussianBlur` at `1800a0e40` is a thin wrapper; the actual implementation
  target is `FUN_18009fca0`.
- `IR_CompositeWithBlendMode` at `1800770d0` has a clear special route:
  - blend mode `0x12` calls `IR_Composite` directly.
  - blend mode `6` with more than one channel first calls `FUN_18005cee0`.
  - blend modes below `0x41` route through `FUN_1800769b0`.
  - higher blend modes route through `FUN_1800a1650`.
- `IR_CompositeWithBlendMode` sets MXCSR bits with `| 0x8020`, so denorm/flush
  behavior may matter for float parity.
- `IR_SetAlphaChannel` revealed pixel-format-specific alpha offsets:
  - 8-bit packed formats use byte offset `+3`,
  - 16-bit packed formats use byte offset `+6`,
  - float formats use byte offset `+0xc`.
- `IR_UnPremultiplyAgainstMatteColor`, `IR_CompositeToBlack`, and
  `IR_CompositeToBlackAndTestSourceAlpha` are present and active in the same
  composite/alpha family.

Current interpretation:

- Glow formula tuning must wait for `FUN_18009fca0`; the public
  `IR_GaussianBlur` symbol alone is not enough.
- Glow operation parity needs numeric mapping from AE's Glow Operation enum to
  ImageRenderer blend codes. The current strongest anchor is blend code `0x12`
  as the direct normal-composite route.
- Native should keep Glow alpha/premult telemetry separate from RGB metrics:
  ImageRenderer clearly has multiple alpha conversion/composite helpers.

Confidence:

- High: Glow uses ImageRenderer Gaussian and ImageRenderer blend composite.
- High: `IR_GaussianBlur` delegates to `FUN_18009fca0`.
- Medium: `0x12` is the direct normal-composite route.
- Low/medium: full Glow Operation enum mapping until `FUN_1800769b0` /
  `FUN_1800a1650` are read.

Next ImageRenderer target:

1. Finish exact-address decompile of `FUN_18009fca0`.
2. Read `FUN_1800769b0`, `FUN_1800a1650`, and `FUN_18005cee0`.
3. Turn the results into a Glow recipe with explicit source mask, blur,
   intensity, blend, and composite-original stages.

### Chunk 4: GPUFoundation BoxBlur targeted decompile

Time: 2026-05-04, after Chunk 1.

Raw log:

- `target/reverse/agent_blur_glow_minimax/gpufoundation_targeted.log`

What this revealed:

- `GF::BoxBlurOptions` layout is now mostly anchored:
  - flags at offset `0x00`,
  - source alpha type at `0x04`,
  - destination alpha type at `0x08`,
  - iterations at `0x0c`,
  - horizontal radius at `0x10`,
  - vertical radius at `0x14`,
  - `isUsingV2` at `0x18`,
  - `forceV2ForVertical` at `0x19`,
  - `use16BitCompute` at `0x1a`.
- Channel/edge bits in `BoxBlur_1DImgOpInfo` flags:
  - alpha channel bit: `0x01`,
  - red channel bit: `0x02`,
  - green channel bit: `0x04`,
  - blue channel bit: `0x08`,
  - repeat-edge bit: `0x10`.
- `SetBlurAlphaChannelOnly` clears RGB bits and sets alpha:
  `flags = (flags & ~0x0e) | 0x01`.
- `StandardOptions` checks the feature flag `GF.FastBoxBlurV2` and stores the
  V2 toggle in the options struct.
- `BoxBlur_1DImgOpInfo::Init` rounds blur radius with `vroundss` mode `2`;
  this appears to be ceil/round-to-positive-infinity. The rounded radius is
  stored separately from the float radius.
- `GF::FastBoxBlur` computes the input halo from rounded horizontal/vertical
  radii multiplied by iterations.
- `SetupSpans` confirms the edge model:
  - repeat-edge false clips/shrinks spans to source bounds,
  - repeat-edge true keeps the full requested span.
- Kernel routes are explicit:
  - V2 path: `FastBoxBlurV2 / BoxBlurKernelFloatV2` or
    `FastBoxBlurV2 / BoxBlurKernelHalfV2`.
  - direct non-premult path: `FastBoxBlur / BoxBlurKernel`.
  - premult/matting path: `FastBoxBlurPremultiply /
    BoxBlurKernelWithPremultiply`.
- V2 path passes both rounded radius and the fractional delta
  `rounded_radius - float_radius`, so V2 is not just an integer box kernel.

Current interpretation:

- Box Blur and Drop Shadow should share a `GpuFoundationBoxBlurSemantics`
  native model: same radius rounding, same repeat-edge semantics, same
  premult/alpha-only channel policy.
- Drop Shadow alpha blur is now stronger than a policy guess: AEX calls
  `SetBlurAlphaChannelOnly`, and GPUFoundation shows that this leaves only
  bit `0x01` enabled.
- Current native clipped separable box blur is structurally close only for the
  non-repeat, non-premult, integer-radius subset.

Confidence:

- High: flags layout, alpha-only bit behavior, repeat-edge bit, kernel routing.
- Medium/high: radius ceil behavior; confirm with one focused radius fixture.
- Medium: V2 fractional-radius semantics; exact kernel weights still live in
  compiled GPU kernels.
- Low/medium: exact premultiply/unpremultiply numeric policy until kernel or
  probe validation confirms channel math.

Next GPUFoundation target:

1. Create BoxBlur fixtures for fractional radii around integer boundaries:
   `0.25`, `0.5`, `0.99`, `1.0`, `1.01`, `1.5`, `2.0`.
2. Split repeat-edge true/false and alpha-only/RGB-only cases.
3. Use probes to validate the recovered semantics before tuning kernel weights.

### Chunk 5: ImageRenderer Gaussian exact follow-up

Time: 2026-05-04, after Chunk 3.

Raw logs:

- `target/reverse/agent_blur_glow_minimax/imagerenderer_exact_full.log`
- `target/reverse/agent_blur_glow_minimax/imagerenderer_constants.log`

What this revealed:

- `FUN_18009fca0` is the real `IR_GaussianBlur` implementation.
- The blur is separable and recursive, not an explicit sampled Gaussian kernel
  table:
  - it computes coefficients with `exp`, `cos`, and `sin`,
  - it builds horizontal and vertical coefficient blocks independently,
  - it dispatches `BlurHorizontal` and `BlurVertical` task objects.
- Direction mapping is now anchored:
  - direction `1` enables horizontal blur,
  - direction `2` enables vertical blur,
  - direction `3` enables both axes.
- If both axes are enabled, ImageRenderer may allocate a transient pixel buffer
  between the horizontal and vertical passes. If source/dest format and storage
  conditions allow it, it can avoid the extra buffer.
- For small dimensions the implementation uses direct helper calls; larger
  dimensions go through `FUN_1800a1650`, which runs the task across CPU workers.
- `IR_GaussianBlur` sets MXCSR `| 0x8020`, then restores MXCSR on exit.
- It validates compatible input/output pixel format classes before doing math;
  incompatible formats return negative error codes.
- If blur is effectively disabled, the function falls back to format conversion
  / centered copy behavior instead of running the recursive blur.

Recovered constants used in the coefficient setup:

| Address | Value |
| --- | --- |
| `1801ad9fc` | `0.000008` as f32 epsilon |
| `1801ada00` | `1.0` as f32 |
| `1801adff0` | `-0.0` / sign-mask style value |
| `1801b1620` | `0.9629` as f32 |
| `1801b1628` | `0.1` as f64 fallback radius |
| `1801b1638` | `0.8447999954` as f64 |
| `1801b1640` | `0.9628999829` as f64 |
| `1801b1648` | `1.0` as f64 |
| `1801b1650` | `1.942` as f32 |
| `1801b1658` | `1.9420000315` as f64 |
| `1801b1660` | `2.0` as f64 |
| `1801b1668` | `-1.2599999905` as f64 |
| `1801b1670` | `-2.0` as f64 |
| `1801b1678` | `-2.5199999809` as f64 |

Current coefficient skeleton:

```text
axis_radius = requested_radius if axis enabled else 0.1

e1 = exp(-1.26 / axis_radius)
e2 = exp(-2.52 / axis_radius)
c1 = cos(0.8448 / axis_radius)
s1 = sin(0.8448 / axis_radius)

normalizers use 0.9629, 1.942, 2.0, 1.0 and sign-mask operations.
```

This is strong evidence for an AE/ImageRenderer recursive Gaussian family. The
exact assignment of these intermediate coefficients into the horizontal and
vertical task structs still needs either typed decompile cleanup or a small
impulse-response fixture to lock signs/order.

Composite/blend follow-up from exact decompile:

- `FUN_1800769b0` chooses one of eight composite worker families based on three
  task flags at offsets `+8`, `+9`, and `+10`.
- `FUN_1800a1650` is the multithreaded task executor wrapper.
- `FUN_18005cee0` is a jump-table/special route; Ghidra recovered it poorly as
  `srand(1)`, so this needs disassembly/jumptable recovery rather than trusting
  the decompiler C output.
- `FUN_180079800` maps pixel formats to conversion/composite function pointers
  and normalized working pixel formats such as `0x805`, `0x905`, `0xa05`,
  `0xb05`, `0x104`, `0x1004`, and `0x1104`.

Current interpretation:

- Glow blur can move from "placeholder approximate" to "implemented,
  instrumented recursive-Gaussian approximation" once the coefficient block is
  ported and validated.
- Formula tuning should use impulse fixtures first: one bright pixel on
  transparent and opaque backgrounds, horizontal-only/vertical-only/both,
  radii around `0.1`, `0.5`, `1`, `2`, `5`, `10`, with alpha/RGB split metrics.
- Blend operation tuning still needs a numeric AE Glow Operation -> ImageRenderer
  blend-code map.

Confidence:

- High: ImageRenderer Gaussian is separable recursive blur.
- High: direction mapping and transient-buffer/two-pass structure.
- Medium/high: coefficient constants.
- Medium: coefficient formula skeleton.
- Low/medium: exact coefficient placement/sign/order until impulse validation.

## Box Blur AEX

Log: `target/reverse/agent_blur_glow_minimax/Box_Blur/ghidra.log`.

What the AEX revealed:

- Match name/string: `ADBE_Box_Blur2`.
- Parameter/resource strings found:
  - `Blur Radius`
  - `Iterations`
- Imported suite/core calls:
  - `GPUFOUNDATION.DLL::GF::BoxBlurOptions::StandardOptions`
  - `GPUFOUNDATION.DLL::GF::FastBoxBlur`
  - also contains `FLT Blur Suite` acquisition in fallback/helper paths.
- Ghidra xrefs show `GF::FastBoxBlur` called from analyzed functions around
  `FUN_1800034d0` and `FUN_180005b70`.

Edge/sampling hints:

- The AEX does not expose the blur kernel itself. The presence of
  `BoxBlurOptions::StandardOptions` means edge mode, radius scaling,
  iterations, and alpha/premult policy are likely encoded inside
  `GPUFoundation.dll`, not inside `Box_Blur.aex`.
- Current native implementation uses a clipped separable box blur. Ghidra does
  not confirm that this is AE's production kernel.

Constants/hints:

- No reliable numeric kernel constants were recovered from `Box_Blur.aex`.
  The meaningful constants are likely in `GF::BoxBlurOptions` and
  `GF::FastBoxBlur`.

Confidence:

- High: Box Blur delegates to GPUFoundation FastBoxBlur.
- Medium: parameter labels are correct; label resource IDs are not guaranteed
  to be AE property indices.
- Low: exact radius mapping, edge policy, premult policy from this AEX alone.

Exact next target:

1. Import/decompile `target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll`.
2. Target symbols:
   - `GF::FastBoxBlur`
   - `GF::BoxBlurOptions::StandardOptions`
3. Answer there: kernel shape, radius-to-window mapping, iteration handling,
   clamp/mirror/transparent edge behavior, premult/unpremult policy.

## Gaussian Blur AEX

Log: `target/reverse/agent_blur_glow_minimax/Gaussian_Blur/ghidra.log`.

What the AEX revealed:

- Match names/strings:
  - `ADBE_Gaussian_Blur`
  - `ADBE Gaussian Blur`
  - `Gaussian Blur (Legacy)`
- Parameter/resource strings found:
  - `Blurriness`
  - `Horizontal and Vertical|Horizontal|Vertical`
  - `Blur Dimensions`
- Error/resource strings found:
  - `Couldn't allocate Gaussian kernel`
  - `Couldn't allocate Gaussian lookup table`
  - `Couldn't allocate scratch scanline`
- Suite name:
  - `FLT Blur Suite`

Edge/sampling hints:

- The AEX repeatedly acquires `FLT Blur Suite` and calls through suite
  function pointers. The local plugin code is mostly a wrapper around AE's
  blur suite.

Constants/hints:

- The strings prove that a Gaussian kernel and lookup table exist somewhere in
  the suite path, but the actual coefficients are not recovered from this AEX.

Confidence:

- High: legacy Gaussian Blur uses `FLT Blur Suite`.
- Medium: Gaussian-specific allocation strings indicate real Gaussian kernel
  machinery.
- Low: exact sigma/radius/edge formula from this AEX alone.

Exact next target:

1. Find the provider of `FLT Blur Suite` in AE shared binaries.
2. If the suite provider is not tractable, use fixture probes only after
   narrowing the parameter contract from the suite call sites.

## Drop Shadow AEX

Log: `target/reverse/agent_blur_glow_minimax/Drop_Shadow/ghidra.log`.

What the AEX revealed:

- Parameter/resource strings found:
  - `Shadow Color`
  - `Opacity`
  - `Direction`
  - `Distance`
  - `Softness`
  - `Shadow Only`
- Imported suite/core calls:
  - `GPUFOUNDATION.DLL::GF::FastBoxBlur`
  - `GPUFOUNDATION.DLL::GF::BoxBlurOptions::StandardOptions`
  - `GPUFOUNDATION.DLL::GF::BoxBlurOptions::SetBlurAlphaChannelOnly`
  - `GF::FillWithTransparentBlack`
- Internal string:
  - `CompositeShadowMask`

Edge/sampling hints:

- Alpha-only blur is confirmed: after `StandardOptions`, the AEX calls
  `SetBlurAlphaChannelOnly`.
- Transparent-black initialization is confirmed through
  `FillWithTransparentBlack`.
- The AEX appears to build a shadow mask, blur the mask's alpha, then composite
  it. That is different from blurring a colored RGBA shadow layer naively.

Constants/hints:

- The softness call path computes a blur argument shaped like:

```text
blur_arg ~= selected_scale * softness / DAT_180011a10
```

- `DAT_180011a00`, `DAT_180011a08`, and `DAT_180011a10` appear in the
  `StandardOptions` argument setup. Ghidra's current types are weak here:
  tiny float-looking values such as `1.4013e-45` / `4.2039e-45` are probably
  enum bits mis-recovered as floats, not literal blur radii.

Confidence:

- High: alpha-only blur and transparent-black mask setup.
- High: blur softness is delegated to `GPUFoundation.dll`.
- Medium: parameter labels and processing order.
- Low-to-medium: exact softness scale until `DAT_180011a*` and
  `StandardOptions` layout are resolved.

Native implications:

- Current native should not blur full RGBA for shadow softness.
- Native shadow should be restructured as:

```text
source alpha -> offset mask -> alpha-only blur -> color/opacity apply -> composite
```

Exact next target:

1. Decompile `GF::BoxBlurOptions::StandardOptions` in `GPUFoundation.dll`.
2. Decompile `GF::BoxBlurOptions::SetBlurAlphaChannelOnly`.
3. Resolve `DAT_180011a00`, `DAT_180011a08`, `DAT_180011a10` values and map
   them to pixel aspect/depth/quality branches.
4. Then tune Drop Shadow against `EFF_020`, `EFF_030`, `STK_010`, `STK_020`.

## Glow AEX

Log: `target/reverse/agent_blur_glow_minimax/Glow/ghidra.log`.

What the AEX revealed:

- Match name/string:
  - `ADBE_Glo2`
- Parameter/resource strings found:
  - `Glow Based On`
  - `Alpha Channel|Color Channels`
  - `Glow Threshold`
  - `Glow Radius`
  - `Glow Intensity`
  - `Composite Original`
  - `Glow Operation`
  - blend enum:
    `None|Normal|Add|Multiply|Dissolve|Screen|Overlay|Soft Light|Hard Light|Darken|Lighten|Difference|Hue|Saturation|Color|Luminosity|Color Dodge|Color Burn|Exclusion|Stencil Alpha|Stencil Luma|Silhouette Alpha|Silhouette Luma|Luminescent Premultiply|Alpha Add`
  - `Glow Colors`
  - `Original Colors|A & B Colors|Arbitrary Map`
  - `Color Looping`
  - `Glow Dimensions`
- Imported core calls:
  - `IMAGERENDERER.DLL::IR_GaussianBlur`
  - `IMAGERENDERER.DLL::IR_CompositeWithBlendMode`
- Pixel depth guard:
  - accepts 8, 16, and 32 bpc worlds; unexpected depth sets an error.

Edge/sampling hints:

- Glow blur is ImageRenderer Gaussian, not BoxBlur.
- Composite is ImageRenderer blend-mode composite, not necessarily normal
  source-over.
- The `Glow Based On` enum is explicitly two-way: alpha channel or color
  channels.

Constants/hints:

- Ghidra shows radius/scale prep before `IR_GaussianBlur`; one recovered term
  uses pixel aspect/depth-like values and a constant `DAT_180011e98`.
- The current decompile is not typed enough to treat `DAT_180011e98` as final
  formula evidence.
- A branch maps local glow operation values to blend mode codes:
  current evidence suggests operation value `1 -> blend code 2` and
  `2 -> blend code 1`, but this needs `IR_CompositeWithBlendMode` enum
  reverse before implementation.

Confidence:

- High: Glow delegates blur to `IR_GaussianBlur`.
- High: Glow delegates final operation to `IR_CompositeWithBlendMode`.
- High: additional AE parameters exist beyond our current native subset.
- Medium: pixel-format and radius-prep observations.
- Low: exact Gaussian sigma/radius and blend enum mapping until
  `ImageRenderer.dll` is reversed.

Native implications:

- Current native `Glow` is only a placeholder approximation.
- Next implementation should expose at least:
  - based-on alpha/color,
  - threshold,
  - radius,
  - intensity,
  - composite original,
  - glow operation/blend mode,
  - dimensions.
- Formula tuning should wait for `IR_GaussianBlur` and
  `IR_CompositeWithBlendMode`.

Exact next target:

1. Import/decompile `target/reverse/ae_2026/core_composite_alpha/ImageRenderer.dll`.
2. Target symbols:
   - `IR_GaussianBlur`
   - `IR_CompositeWithBlendMode`
3. Answer there: Gaussian kernel/sigma mapping, edge policy, color space,
   premult policy, blend enum numeric mapping.

## Minimax AEX

Log: `target/reverse/agent_blur_glow_minimax/Minimax/ghidra.log`.

What the AEX revealed:

- Parameter/resource strings found:
  - `Operation`
  - `Minimum|Maximum|Minimum Then Maximum|Maximum Then Minimum`
  - `Radius`
  - `Channel`
  - `Color|Alpha and Color|Red|Green|Blue|Alpha`
  - `Direction`
  - `Horizontal & Vertical|Just Horizontal|Just Vertical`
  - `Don't Shrink Edges`
- CPU suite call:
  - `PF.DLL::PF_IterateGeneric`
- CPU per-depth callbacks:
  - 8 bpc callback: `FUN_1800060b0`
  - 16 bpc callback: `FUN_180004ef0`
  - 32 bpc callback: `FUN_180007270`
- GPU/kernel strings:
  - `AEFX_Minimax`
  - `MinimaxBuildSTreeLeafNodesKernel`
  - `MinimaxBuildSTreeNonLeafNodesKernel`
  - `MinimaxTraverseSTreeKernel`

Edge/sampling hints:

- Direction is a first-class parameter, so native square-only behavior is
  incomplete.
- `Don't Shrink Edges` is a first-class flag, so native clipped-neighborhood
  behavior is incomplete.
- CPU and GPU paths likely share the same semantic model but different
  implementations: `PF_IterateGeneric` callbacks for CPU, spatial-tree kernels
  for GPU/accelerated path.

Constants/hints:

- No final radius-rounding constant recovered yet.
- The setup path passes a compact param struct into per-depth callbacks. The
  struct appears to include operation, radius/direction expansion, channel, and
  edge flag fields.

Confidence:

- High: operation/channel/direction enum surfaces.
- High: 8/16/32 bpc CPU dispatch exists.
- Medium: GPU path uses spatial tree kernels.
- Low-to-medium: exact neighborhood inclusion, radius rounding, and edge
  shrink behavior until callback functions are decompiled directly.

Native implications:

- Current native Minimax is under-specified:
  - only min/max, but AE has min, max, min-then-max, max-then-min;
  - only RGB/alpha, but AE has color, alpha+color, red, green, blue, alpha;
  - no H/V/HV direction;
  - no Don't Shrink Edges.

Exact next target:

1. In `Minimax.aex`, decompile and type the per-depth callbacks:
   - `FUN_1800060b0`
   - `FUN_180004ef0`
   - `FUN_180007270`
2. Decompile setup functions around the `PF_IterateGeneric` call and recover
   the param struct layout.
3. Then inspect GPU kernels only to corroborate CPU semantics.

## Cross-Cutting Required Questions

### Parameter Mapping

- Box Blur: resource labels confirm radius/iterations; property indices should
  still come from AE payload/property dumps, not LStr numbers alone.
- Gaussian Blur: resource labels confirm blurriness and dimensions.
- Drop Shadow: labels confirm color, opacity, direction, distance, softness,
  shadow-only. LStr IDs are label IDs, not guaranteed property IDs.
- Glow: AEX reveals a larger parameter surface than native currently supports:
  based-on, threshold, radius, intensity, composite original, operation, colors,
  color looping, dimensions.
- Minimax: AEX confirms operation/radius/channel/direction/don't-shrink surface
  and enum labels.

### Time Semantics

- These plugins receive already-evaluated PF params from the host. No evidence
  suggests arbitrary per-pixel time expression logic inside these effect kernels.
- Native implication: animated effect params must be evaluated at effect time
  before invoking effect math. This aligns with current `param_*_at_any` direction,
  but all newly added params must use the same time-aware path.

### Sampling Rules

- Box Blur: AEX delegates to `GF::FastBoxBlur`; GPUFoundation now confirms
  radius ceil, iteration halo, repeat-edge span behavior, and kernel routing.
  Exact GPU kernel weights/fractional treatment still need fixture validation.
- Gaussian Blur: unresolved in AEX; delegated to `FLT Blur Suite`.
- Drop Shadow: alpha-only blur is confirmed; exact blur sampling delegated to
  `GF::FastBoxBlur`, with the same BoxBlur semantics above.
- Glow: `IR_GaussianBlur` is a separable recursive Gaussian with recovered
  constants, direction mapping, and horizontal/vertical task stages.
- Minimax: direction, channel bitmask, ROI helper, `2r + 1` window, and depth
  callbacks are recovered; operation polarity/pass order remains to be named.

### Alpha / Premult Policy

- Drop Shadow: confirmed alpha-only blur after transparent-black mask setup.
- Glow: based-on alpha/color source is confirmed; premult/composite policy is
  inside ImageRenderer.
- Box Blur: channel flags and premultiply/matting kernel route are confirmed in
  GPUFoundation; exact numeric premult/unpremult policy still needs validation.
- Minimax: alpha and channel modes are explicit; current native alpha handling
  is incomplete.

### Color / Numeric Policy

- Glow checks for 8/16/32 bpc worlds before render.
- Minimax dispatches separate CPU callbacks for 8, 16, and 32 bpc.
- Native can keep PNG/u8 conformance for now, but formula design should avoid
  baking in u8-only assumptions.

## Validation Cases

Relevant conformance IDs for this block:

- `EFF_010`: Box/Gaussian blur basics.
- `EFF_020`: Drop Shadow alpha/softness/composite.
- `EFF_030`: Glow threshold/radius/intensity/composite.
- `EFF_050`: Minimax operations/channel/direction.
- `EFF_070`: animated effect params/time-aware behavior.
- `STK_010`, `STK_020`: stacked adjustment/effect interactions.

Validation was not re-run in this checkpoint. Attempted runtime image:

```text
docker run --rm --entrypoint sh -v "$PWD:/work" -w /work ae-native-renderer:round2-dev -lc 'cargo run -p render-cli -- conformance-pack ...'
```

Blocker:

```text
sh: 1: cargo: not found
```

This is not a math blocker, only a local validation-runner blocker for this
checkpoint. Existing AE golden fixtures remain the validation target.

## Exact Next Targets

Priority order:

1. Box/Drop Shadow validation from `GPUFoundation.dll` findings:
   - fractional-radius fixtures,
   - repeat-edge true/false fixtures,
   - alpha-only/RGB-only fixtures,
   - premult/matting validation.
2. Glow validation from `ImageRenderer.dll` findings:
   - impulse-response fixtures for recursive Gaussian coefficients,
   - Glow Operation -> ImageRenderer blend-code map,
   - alpha/RGB metric split.
3. `Minimax.aex`
   - `FUN_1800060b0`
   - `FUN_180004ef0`
   - `FUN_180007270`
   - setup struct around `PF_IterateGeneric`
   - needed for operation/channel/direction/edge parity.
4. `FLT Blur Suite` provider
   - needed for Legacy Gaussian if it remains in template scope.

## Current Confidence Board

| Area | Status | Confidence | Why |
| --- | --- | --- | --- |
| Box blur formula | Partially recovered | Medium/High | `BoxBlurOptions` layout, radius ceil, iterations halo, kernel routes recovered; GPU kernel weights pending |
| Box edge policy | Recovered at policy level | Medium/High | repeat-edge false shrinks/clips spans, repeat-edge true keeps full spans |
| Drop Shadow alpha blur | Recovered at policy level | High | `SetBlurAlphaChannelOnly` + transparent-black setup |
| Drop Shadow softness scale | Partially seen | Medium | GPUFoundation radius/iteration semantics recovered; AEX softness scale still needs fixture lock |
| Glow blur type | Structurally recovered | Medium/High | `IR_GaussianBlur` resolves to separable recursive Gaussian with constants and direction mapping |
| Glow composite | Recovered at policy level | High | Direct `IR_CompositeWithBlendMode` import/call; worker task family recovered |
| Glow blend enum mapping | Partially seen | Medium | normal route `0x12` and worker dispatch recovered; AE operation map pending |
| Minimax enum surface | Recovered | High | strings expose operation/channel/direction/edge flag |
| Minimax neighborhood | Partially recovered | Medium | CPU callbacks, radius window, channel bitmask, ROI helper, and GPU spatial-tree path recovered |

## Native Work Not Yet Authorized By This Report

Do not parity-lock formulas solely from this checkpoint. Safe native changes
after this checkpoint are now implementation/instrumentation prototypes, with
fixture validation required before claiming parity:

- add missing Glow params and telemetry,
- add missing Minimax enum params and telemetry,
- restructure Drop Shadow around alpha-only blur,
- implement recovered BoxBlur/ImageRenderer Gaussian semantics behind
  conformance tests,
- keep exact glow blend and minimax operation polarity gated until the remaining
  enum maps are locked.

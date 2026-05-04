# Agent B Geometry / Motion Math Objects

Status date: 2026-05-04

Predecoded evidence root:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

## Scope

Assigned objects:

| Object | Area | Status in this pass |
| --- | --- | --- |
| `M03` | 2D transform matrix, anchor/position/scale/rotation sampling | matrix order confirmed through `GF::TransformToMatrix`; sampler edge policy still needs probe |
| `M04` | Keyframe hold/linear/cubic Bezier/ease | no direct Bezier evidence in assigned binaries; probe shape defined |
| `M12` | `ADBE Geometry2` | parameter namespace and core matrix confirmed; sampler/pixel-center/OOB needs probe |
| `M17` | Collapse transformations / deferred rasterization | host/render-graph owned; decision table defined from current evidence and existing native contract |
| `M18` | Motion blur | transform sample builder and GPU matrix-count path narrowed; exact endpoint timing/weights need probe |

Write scope honored: only this report was created.

## Evidence Table

| Object | Source | Function / address | Finding | Confidence | Affected modules |
| --- | --- | --- | --- | --- | --- |
| `M03`, `M12` | `geometry_transform_gpufoundation/index.md` | `GF::TransformToMatrix`, `180074eb0`; `GF::TransformsToMatrices`, `1800757c0`; `GF::TransformedBounds`, `180075160` | Transform math is in `GPUFoundation.dll`, not in `Transform.aex`. | high | `M03`, `M12`, `M18` |
| `M03`, `M12` | `geometry_transform_gpufoundation/04_GF_TransformToMatrix_180074eb0/decompile.c` | `180074eb0` | Internal `GF::Transformation` fields are consumed as anchor, position, scale factors, rotation, skew, skew axis; matrix is built with pixel-aspect compensation, rotation, skew, scale, and anchor translation. | high for order/sign, medium for field names | `M03`, `M12` |
| `M03`, `M12` | `geometry_transform_gpufoundation/05_GF_TransformedBounds_180075160/decompile.c` | `180075160` | Point mapping uses `x' = x*m00 + y*m01 + m02`, `y' = x*m10 + y*m11 + m12`; bounds are forward-transformed. | high | `M03`, `M12`, `M17` |
| `M12`, `M18` | `geometry_transform_wrapper/01_Geometry2_render_wrapper_180009a20/decompile.c` | `180009a20` | Wrapper receives a vector of prebuilt `GF::Transformation` samples, converts through `GF::TransformsToMatrices`, creates `GF::TransformOperation`, then calls `GF::TransformWithMotionBlur`. | high | `M12`, `M18` |
| `M12`, `M18` | `geometry_transform_gpufoundation/08_GF_TransformWithMotionBlur_180076ed0/decompile.c` and `callees.tsv` | `180076ed0` | GPU path uploads inverted matrices as `9 * matrix_count` float buffers, branches by `TransformOperation::Quality`, and calls sampler kernels. | high | `M03`, `M12`, `M18`, `M19` |
| `M12`, `M18` | `geometry_transform_gpufoundation/08_GF_TransformWithMotionBlur_180076ed0/callees.tsv` | calls `IsMotionBlur`, `NumMatrices`, `Quality`, `Matrices`, `InvertedMatrices`, `MatrixRadii`, `GetOpacityMultiplier` | Motion/sampler path depends on precomputed matrix vector, opacity multiplier, sampler quality, and matrix radii. | high | `M12`, `M18`, `M19` |
| `M12` | `docs/phase_reports/AE_REVERSE_GEOMETRY2_GHIDRA.md` | property dump summary | AE property indices are confirmed: property key `0003` is Uniform Scale, `0004` Scale Height, `0008` Rotation; matchName `ADBE Geometry2-0008` is Opacity at property index 9. | high | `M12`, payload import |
| `M18` | `geometry_transform_wrapper/02_Geometry2_mb_candidate_33a0_1800033a0/decompile.c`, `data_refs.tsv` | `1800033a0` | Candidate sample builder uses `T_MulTime`, `T_AddTime`, `T_ConvTime`, fixed constants `5760`, `65536`, `1/65536`, and caller-supplied sample count `0x11`. | medium | `M18`, `M04`, temporal stack |
| `M18` | `geometry_transform_wrapper/03_Geometry2_mb_candidate_4440_180004440/decompile.c` | `180004440` | Reads motion-related fixed-point fields at `param_1+0xf8` and `param_1+400`, calls sample builder, then transforms bounds for matrix vector. | medium | `M18`, `M12` |
| `M18` | `geometry_transform_wrapper/04_Geometry2_mb_candidate_4ab0_180004ab0/decompile.c` | `180004ab0` | Same sample builder appears in render/PF fallback branch; can route through `PF_TransformWorld` when GPU path is not used. | medium | `M18`, `M12`, `M19` |
| `M18`, `M19` | `docs/phase_reports/AE_REVERSE_CORE_ALPHA_SAMPLING_GHIDRA.md`; `target/reverse/agent_core_alpha/20260504_211758_bounded/GPUFoundation_dll.dump.txt` | `GF::Motion`, `18003eac0` | `GF::Motion` owns separate affine/composite/opacity flow and calls `Composite` or `AlphaGain`; opacity/composite is a substrate dependency. | medium | `M18`, `M19`, `M01` |
| `M17` | `docs/MATH_PARITY_STATUS.md`; `docs/phase_reports/AGENT_ROUND4_COLLAPSE_DEFER_RASTER.md`; `docs/phase_reports/AE_REVERSE_GEOMETRY_COLLAPSE_GHIDRA_ROUND2.md` | native collapse reports | Collapse is not implemented by `Transform.aex`; current native can flatten text/solid-only collapsed precomps, but true deferred text/vector rasterization is not parity-locked. | medium | `M17`, `M05`, `M07`, `M19` |
| `M04` | `docs/MATH_PARITY_STATUS.md` | module registry | Bezier/ease is already `instrumented/testable`, but assigned reverse folders do not contain direct tangent/ease math. | high for status, low for formula | `M04`, animated params in all modules |

## Object Passports

### `M03` - 2D Transform Matrix / Sampling

Parameter mapping:

- Internal `GF::Transformation` layout inferred from `GF::TransformToMatrix`:
  `anchor.x`, `anchor.y`, `position.x`, `position.y`, normalized `scale.x`,
  normalized `scale.y`, `rotation_degrees`, `skew_degrees`, `skew_axis_degrees`.
- AE UI scale percentages must be divided by `100` before entering this core
  function.

Coordinate/time/color space:

- 2D image coordinates, origin at upper-left in render space.
- Forward matrix maps source/layer coordinates into output coordinates.
- Matrix storage is effectively:

```text
x' = x*m00 + y*m01 + m02
y' = x*m10 + y*m11 + m12
```

- `GF::TransformToMatrix` computes in double; `GF::TransformWithMotionBlur`
  uploads inverted matrices as float GPU buffers.
- Color/alpha are not part of `TransformToMatrix`; opacity/composite live in
  `TransformOperation`, `TransformWithMotionBlur`, and `GF::Motion`.

Sampling rule:

- Render path samples backward: output pixel -> inverse matrix -> source UV.
- Confirmed GPU path uploads `InvertedMatrices`, not forward matrices, to the
  sampler kernels.
- Exact pixel-center convention and OOB threshold are not locked. The assigned
  predecode reaches kernel argument packing, not the kernel source.

Alpha/premult policy:

- Transform matrix is alpha-independent.
- `TransformOperation` carries an opacity multiplier and the GPU path passes it
  into kernels. Premult/straight behavior remains owned by `M19`.

Formula:

```text
M =
  T(position)
  * S(1 / pixel_aspect, 1)
  * R(-skew_axis)
  * Hx(-tan(skew_degrees * pi / 180))
  * R(skew_axis)
  * R(rotation_degrees)
  * S(scale_x, scale_y)
  * S(pixel_aspect, 1)
  * T(-anchor)
```

If skew is zero, the skew block is skipped. Rotation uses degrees modulo
`360.0`; matrix-diff epsilon observed in the operation path is `1e-12`.

Native implementation delta:

- Native Geometry2 matrix now follows this order and inverse-samples with
  bilinear transparent OOB.
- Native uses `f32` matrices in `crates/effects/src/geometry.rs`; AE core builds
  in double and uploads inverse matrices as floats.
- Native sampler/OOB is still approximate: continuous UV guard is
  `uv < 0 || uv > width-1/height-1`, with bilinear sampling. AE kernel
  boundary/pixel-center remains unknown.

Test/probe needed next:

- Coordinate-field transform probe with AE-exported forward/inverse matrix and
  per-pixel UV sidecar if possible.
- 1-pixel impulse and checkerboard around `-0.5`, `0`, `0.5`,
  `width-1`, `width-0.5`, `width` to lock pixel center and OOB.
- Separate layer transform probe from Geometry2 effect probe, so M03 layer
  transform and M12 effect transform are not accidentally conflated.

### `M04` - Keyframes / Bezier / Ease

Parameter mapping:

- No direct Bezier/ease mapping was recovered from the assigned
  `Transform.aex`/`GPUFoundation.dll` evidence.
- Existing native status is `instrumented/testable`, not formula tuning.

Coordinate/time/color space:

- Time-domain object. It affects transform, Geometry2 animated params, opacity,
  and any effect parameter sampled during motion blur.
- It must be tested at comp time and shutter sample time, not only at whole
  frames.

Sampling rule:

- Hold and linear can be checked against property samples directly.
- Cubic Bezier/ease must compare AE property value samples and rendered output,
  because easing errors can be hidden by later alpha/composite differences.

Alpha/premult policy:

- None directly. Rendered probes should use opaque high-contrast geometry first
  to avoid M19 contaminating the timing signal.

Formula or pseudocode:

```text
for property P with keys k0, k1:
  export AE value P(t) at dense times around key interval
  compare native hold/linear/ease value before raster/composite
  only then compare rendered centroid/edge location
```

Native implementation delta:

- Native currently has scalar/Vec2 keyframes and compact cubic-ease
  approximation.
- No evidence in this pass proves AE tangent influence/speed mapping, temporal
  continuity flags, or exact solver epsilon.

Required probes/tests:

- `INT_BEZ_010`: position X from `0 -> 100`, easy ease, sample every 1/8 frame;
  export AE property values through JSX and native trace values.
- `INT_BEZ_020`: asymmetric temporal ease in/out influence/speed; include
  vector position and opacity.
- `INT_BEZ_030`: same animated property under motion blur with samples inside
  shutter interval; verify params evaluate at sample time.
- Keep probes opaque and unblurred first, then add alpha/composite version only
  after `M19` is locked.

### `M12` - Geometry2

Parameter mapping:

There are two numeric namespaces. This is the high-risk mapping point.

| Payload key / AE property index | AE matchName | UI label | Native handling |
| --- | --- | --- | --- |
| `0001` | `ADBE Geometry2-0001` | Anchor Point | anchor |
| `0002` | `ADBE Geometry2-0002` | Position | position |
| `0003` | `ADBE Geometry2-0011` | Uniform Scale | checkbox/legacy fallback if no width/height |
| `0004` | `ADBE Geometry2-0003` | Scale Height | native `scaleY` |
| `0005` | `ADBE Geometry2-0004` | Scale Width | native `scaleX` |
| `0006` | `ADBE Geometry2-0005` | Skew | skew degrees |
| `0007` | `ADBE Geometry2-0006` | Skew Axis | axis degrees |
| `0008` | `ADBE Geometry2-0007` | Rotation | rotation degrees |
| `0009` | `ADBE Geometry2-0008` | Opacity | not currently applied by native Geometry2 |
| `0010` | `ADBE Geometry2-0009` | Use Composition's Shutter | not currently applied by native Geometry2 |
| `0011` | `ADBE Geometry2-0010` | Shutter Angle | not currently applied by native Geometry2 |
| `0012` | `ADBE Geometry2-0012` | Sampling | native currently assumes bilinear |

Answer for the requested AE `0003`/`0004`/`0008` names:

- MatchName suffix `ADBE Geometry2-0003` = Scale Height.
- MatchName suffix `ADBE Geometry2-0004` = Scale Width.
- MatchName suffix `ADBE Geometry2-0008` = Opacity.
- Payload key `"0008"` is not opacity; it is property index 8, Rotation.

Coordinate/time/color space:

- Geometry2 uses the same GF transform coordinate convention as `M03`.
- Animated Geometry2 samples are built upstream of `GF::TransformsToMatrices`.
  The wrapper receives a vector of concrete transformations, so per-sample time
  evaluation happens before the GPUFoundation transform call.
- Opacity/composite details are outside the pure matrix path and touch `M19`.

Sampling rule:

- Internal `SampleQuality` method map in `GF::TransformWithMotionBlur`:

| Method field | Kernel family |
| ---: | --- |
| `0` | `NearestNeighbor` |
| `1` | `Bilinear` |
| `2` | `BicubicLanczos` |
| `3` | `BicubicAreaSample` |

- UI strings show `Sampling = Bilinear|Bicubic`, but the exact UI Bicubic
  assignment to internal Lanczos vs AreaSample remains unconfirmed.
- `MatrixRadii` drives area/no-area branch and optional radius buffers.
- Exact edge policy/OOB/pixel-center is not confirmed by Ghidra.

Alpha/premult policy:

- `ADBE Geometry2-0008` opacity is a transform-operation opacity multiplier in
  AE evidence, but native Geometry2 currently does not apply the effect opacity
  slot.
- Premult/straight and composite behavior should reference Agent A/M19 before
  formula tuning.

Formula:

Same as `M03`, with scale width/height resolved to normalized factors before
calling `GF::TransformToMatrix`.

Native implementation delta:

- Native mapping now uses property index `0004` as height, `0005` as width, and
  `0008` as rotation.
- Native still ignores Geometry2 opacity, shutter controls, and Sampling
  beyond bilinear.
- Native does not implement bicubic/area/lanczos sampler selection.

Test/probe needed next:

- `EFF_GEO_PARAM_010`: set payload `0003`, `0004`, `0005`, `0008`, `0009`
  separately and assert AE-side resolved property names and rendered effect.
- `EFF_GEO_SAMPLE_020`: Bilinear vs Bicubic UI at high scale-down and high
  scale-up, with checkerboard and single-pixel impulse.
- `EFF_GEO_EDGE_030`: transparent source edge and alpha ramp around exact UV
  boundaries; record output alpha and RGB.
- `EFF_GEO_OPACITY_040`: Geometry2 opacity slot only, no layer opacity, to
  locate opacity multiplier and premult boundary.

### `M17` - Collapse / Deferred Rasterization

Parameter mapping:

- Collapse transformations is not owned by `Transform.aex`. Evidence points to
  AE host/render graph ownership.
- Current native collapse planning supports text/solid-only flattening with
  barriers for effects, footage, adjustment layers, non-collapsed nested
  precomps, missing targets, and cycles.

Coordinate/time/color space:

- Collapse composes parent/precomp/child transforms in parent/output space.
- Source time must be evaluated after precomp layer timing and before child
  transform evaluation.
- Text layout should remain in source/layer units; rasterization should happen
  at final effective matrix scale for true deferred rasterization.

Sampling rule:

- True collapse should defer vector/text rasterization until the final effective
  matrix is known.
- Current native path rasterizes collapsed text at a scale hint
  (`matrix_scale_hint(...).clamp(1,4)`) and inverse-scales it back. That is a
  useful approximation, not AE parity.

Alpha/premult policy:

- Collapse itself should not change premult policy, but any forced raster
  barrier creates a composite boundary. Coordinate with `M19` for alpha and
  hidden RGB behavior.

Decision table:

| Layer/content at precomp boundary | Collapse decision | Reason / next requirement |
| --- | --- | --- |
| Text layer, no effects/mattes/barriers | Defer rasterization | Carry text primitive, source time, child transform, parent matrix, opacity, and final effective matrix to renderer. |
| Vector/shape layer, no effects/mattes/barriers | Defer rasterization | Same as text; final raster backend must receive final matrix/scale. |
| Solid layer | Can flatten as transformable primitive if no barriers | Native already handles solid-like flattened layers; still verify alpha/composite at boundary. |
| Footage layer | Rasterize at precomp boundary | Footage is already raster content and has source-frame/time sampling; flattening through it can change sampling/OOB policy. |
| Adjustment layer | Raster barrier | It depends on accumulated lower stack and effect order. |
| Layer with effects | Raster barrier | Effects operate on raster input/output unless effect is proven transform-defer-safe. |
| Non-collapsed nested precomp | Raster barrier | Boundary intentionally rasterizes. |
| Collapsed nested precomp with only defer-safe descendants | Continue deferral and compose matrices | Must carry nested boundary path and source time. |
| Missing target or cycle | Rasterize/fail closed | Avoid invalid graph flattening. |

Native implementation delta:

- Native has planning/telemetry and matrix composition but still lacks a
  render-facing deferred primitive contract for final text/vector rasterization.
- `GPH_010` should remain a collapse/text regression, not a Geometry2 matrix
  test.

Test/probe needed next:

- AE pair: same precomp rendered once collapsed and once pre-rasterized, with
  text/vector sharpness metric and identical parent transforms.
- Telemetry: one record per deferred primitive with parent matrix, child matrix,
  final matrix, source time, raster scale, fallback reason.
- Barrier tests: footage, adjustment, effect, non-collapsed nested precomp, and
  nested collapsed precomp.

### `M18` - Motion Blur

Parameter mapping:

- `GF::TransformWithMotionBlur` does not create shutter times. It consumes
  `TransformOperation`, whose matrix vector has already been built from a
  vector of `GF::Transformation` samples.
- Wrapper candidates show a sample builder:
  - `FUN_1800033a0` uses `T_MulTime`, `T_AddTime`, `T_ConvTime`;
  - callers pass `DAT_180023268 = 0x11`, likely 17 samples;
  - motion fields at `param_1+0xf8` and `param_1+400` are multiplied by
    `1/65536`, consistent with fixed-point shutter angle fraction and shutter
    phase fraction.

Coordinate/time/color space:

- Motion blur for transform is temporal supersampling of concrete transform
  matrices.
- The GPU sampler receives inverse matrices and a matrix count.
- `GF::Motion` is a separate affine/composite path and can call `Composite` or
  `AlphaGain`; alpha/composite ownership crosses into `M19`.

Sampling rule:

- Confirmed: when motion blur is active, matrix count comes from
  `TransformOperation::NumMatrices`.
- Confirmed: inverse matrices are converted double -> float and uploaded as
  `0x24` bytes per matrix.
- Confirmed: kernels receive `matrix_count`, opacity multiplier,
  interpolation bool, and optional area radii buffers.
- Weight clues: no explicit per-sample weight array was seen in the wrapper or
  `TransformWithMotionBlur`; likely uniform averaging by matrix count, modified
  by opacity multiplier, but this is not locked.

Alpha/premult policy:

- `GetOpacityMultiplier()` is passed into transform kernels.
- `GF::Motion` can route normal blend through `AlphaGain` or composite through
  `Composite`. Treat final accumulation/premult as `M19`-dependent.

Formula/hypothesis:

```text
sample_count = 17              # observed caller constant 0x11
shutter_ratio = fixed16_field(param_1 + 0xf8)
phase_ratio   = fixed16_field(param_1 + 400)
shutter_span  = frame_duration * shutter_ratio
step          = shutter_span / (sample_count - 1)
for each sample:
  sample_time = base_time + phase_offset + endpoint_or_step_offset
  evaluate Geometry2/transform params at sample_time
  append GF::Transformation
```

Important: endpoint semantics are not locked. `FUN_1800033a0` appears to add
the step inside the loop before evaluation, so the exact first/last sample
offset must be probed rather than inferred from decompiler shape.

Native implementation delta:

- Native currently uses midpoint temporal samples inside shutter interval and
  equal weights (`1/samples`) scaled by opacity.
- AE evidence suggests a 17-sample Transform effect path and possibly endpoint
  or non-midpoint sample placement.
- Native supports global `MotionBlurSettings.samples`; AE Transform/Geometry2
  may have debug/config sample count and per-effect shutter controls.

Test/probe needed next:

- `MB_XFORM_010`: moving 1px vertical bar, Geometry2/layer transform only,
  export frames with shutter angle 180 and phase -90/0/90. Fit blur centroid
  and support width to infer endpoints.
- `MB_XFORM_020`: animated rotation around off-center anchor, record per-sample
  native matrices and compare AE rendered arc.
- `MB_XFORM_030`: set Geometry2 `Use Composition's Shutter` and `Shutter Angle`
  slots separately; determine whether effect shutter overrides comp shutter.
- `MB_WEIGHT_040`: non-uniform opacity animation during shutter; distinguish
  uniform matrix weights from opacity-at-sample accumulation.
- Add native trace fields: matrix sample index, sample time, weight, forward
  matrix, inverse matrix, opacity multiplier, sampler method.

## Guardrail Findings

No hard `BLOCKER` was found in the assigned evidence. Continue on unrelated
work is yes.

Guardrails that remain open:

| Guardrail | Finding | Action |
| --- | --- | --- |
| Sampling/OOB policy | GPU path reaches sampler kernel selection, but not kernel source. Pixel-center and transparent/clamp boundary are not locked. | Run Geometry2 edge/impulse probes before formula tuning `M03`/`M12`. |
| Coordinate origin/handedness | No contradiction found. Matrix order/sign matches existing Geometry2 notes and native direction after inverse sampling. | Keep current matrix order. |
| Time semantics | Motion blur samples animated transform state upstream of `GF::TransformsToMatrices`; exact endpoint timestamps are unknown. | Treat `M18` as `needs_probe`; coordinate with temporal agent for Posterize/effect-param sample times. |
| Alpha/premult | Transform math is alpha-independent, but opacity/composite crosses into `GF::Motion`, `Composite`, and `AlphaGain`. | Reference `M19` substrate before tuning opacity/composite pixels. |
| AE hidden pipeline call | `Transform.aex` delegates to `GPUFoundation.dll`, which is in assigned scope. XForm kernel bodies were not in the predecoded bundle. | Not a blocker for passport; kernel/OOB needs targeted shader/kernel extraction or black-box probe. |
| Cross-object dependency | Collapse text/vector parity depends on text/glyph rasterization and alpha/composite. | Do not tune `GPH_010` as Geometry2 matrix failure. |

## Recommendation

Final recommendation: `needs_probe`.

Ready to carry forward:

- Keep the recovered matrix order for `M03`/`M12`.
- Treat payload numeric keys as AE property indices. Most importantly,
  payload `"0008"` is Rotation, while matchName `ADBE Geometry2-0008` is
  Opacity.
- Use inverse-matrix sampling for transform/Geometry2.
- Keep current collapse plan as an approximation but do not mark it parity-ready.

Before formula tuning:

1. Lock Geometry2 sampler/OOB/pixel-center with impulse/checkerboard probes.
2. Lock Geometry2 opacity and Sampling/Bicubic UI mapping.
3. Recover or probe motion blur sample endpoints, weights, and effect shutter
   override behavior.
4. Export AE Bezier/ease value samples before changing native keyframe formulas.
5. Define deferred text/vector raster primitives for collapse, then test
   barrier cases separately from Geometry2 matrix tests.

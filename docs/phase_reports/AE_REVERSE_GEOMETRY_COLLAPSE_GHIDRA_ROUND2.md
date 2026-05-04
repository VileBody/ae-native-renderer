# AE Reverse Geometry / Collapse Ghidra Round 2

Status date: 2026-05-04.

Agent scope: Geometry2 / layer transform / collapse / motion matrix. This round
uses Ghidra as the primary evidence source. Probes, decoded shaders, and
conformance outputs are used only as validation/corroboration.

## Inputs

Local Adobe binaries are ignored under `target/reverse/ae_2026/`.

| Binary | Path | SHA-256 |
| --- | --- | --- |
| GPUFoundation.dll | `target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll` | `36ea8222bc1e3abae148771cbea03eb60cd4bbcc708e145af5dd9451782d2466` |
| Transform.aex | `target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Transform.aex` | `21661b33ef1b6aefdc9a686316c0be5d29642683eb94848cdc58d6ad38ef17ba` |
| Transform.aex | `target/reverse/ae_2026/Transform.aex` | `21661b33ef1b6aefdc9a686316c0be5d29642683eb94848cdc58d6ad38ef17ba` |

Raw outputs:

- Ghidra project: `target/reverse/ghidra_projects/agent_geometry_collapse/`
- Ghidra logs: `target/reverse/agent_geometry_collapse/ghidra_gpu_targeted.log`
- Ghidra logs: `target/reverse/agent_geometry_collapse/ghidra_transform_hints.log`
- Ghidra logs: `target/reverse/agent_geometry_collapse/ghidra_gpu_sampler_index.log`
- Ghidra logs: `target/reverse/agent_geometry_collapse/ghidra_gpu_sampler_deep_narrow.log`
- Ghidra logs:
  `target/reverse/agent_geometry_collapse/ghidra_transform_wrapper_deep_narrow.log`
- Ghidra raw dump:
  `target/reverse/agent_geometry_collapse/geometry_sampler_deep_narrow.txt`
- Ghidra raw dump:
  `target/reverse/agent_geometry_collapse/transform_wrapper_deep_narrow.txt`
- Validation outputs: `target/reverse/agent_geometry_collapse/validation/`
- Diff bbox summary: `target/reverse/agent_geometry_collapse/visual_diff_summary.json`

Commands used:

```sh
env LC_ALL=C LANG=C GHIDRA_HOME=/Applications/ghidra_12.0_PUBLIC \
  /Applications/ghidra_12.0_PUBLIC/support/analyzeHeadless \
  /Users/ergin/Desktop/ae-native-renderer/target/reverse/ghidra_projects/agent_geometry_collapse \
  agent_geometry_collapse \
  -import /Users/ergin/Desktop/ae-native-renderer/target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll \
  -overwrite \
  -postScript DumpGpuTransformTargeted.java \
  -scriptPath /Users/ergin/Desktop/ae-native-renderer/target/reverse/ghidra_scripts \
  -log /Users/ergin/Desktop/ae-native-renderer/target/reverse/agent_geometry_collapse/ghidra_gpu_targeted.log

env LC_ALL=C LANG=C GHIDRA_HOME=/Applications/ghidra_12.0_PUBLIC \
  /Applications/ghidra_12.0_PUBLIC/support/analyzeHeadless \
  /Users/ergin/Desktop/ae-native-renderer/target/reverse/ghidra_projects/agent_geometry_collapse \
  agent_geometry_collapse \
  -import /Users/ergin/Desktop/ae-native-renderer/target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Transform.aex \
  -overwrite \
  -postScript DumpTransformHints.java \
  -scriptPath /Users/ergin/Desktop/ae-native-renderer/target/reverse/ghidra_scripts \
  -log /Users/ergin/Desktop/ae-native-renderer/target/reverse/agent_geometry_collapse/ghidra_transform_hints.log

docker run --rm --entrypoint /work/target/debug/render-cli \
  -v /Users/ergin/Desktop/ae-native-renderer:/work \
  -w /work ae-native-renderer:round2-dev \
  conformance-pack --pack fixtures/ae_conformance_pack \
  --out target/reverse/agent_geometry_collapse/validation \
  --case EFF_040 --case GPH_010 --case CMP_010

env LC_ALL=C LANG=C GHIDRA_HOME=/Applications/ghidra_12.0_PUBLIC \
  /Applications/ghidra_12.0_PUBLIC/support/analyzeHeadless \
  /Users/ergin/Desktop/ae-native-renderer/target/reverse/ghidra_projects/agent_geometry_collapse \
  agent_geometry_collapse \
  -process GPUFoundation.dll \
  -noanalysis \
  -postScript DumpGeometrySamplerDeepNarrow.java \
  /Users/ergin/Desktop/ae-native-renderer/target/reverse/agent_geometry_collapse/geometry_sampler_deep_narrow.txt \
  -scriptPath /Users/ergin/Desktop/ae-native-renderer/target/reverse/agent_geometry_collapse \
  -log /Users/ergin/Desktop/ae-native-renderer/target/reverse/agent_geometry_collapse/ghidra_gpu_sampler_deep_narrow.log

env LC_ALL=C LANG=C GHIDRA_HOME=/Applications/ghidra_12.0_PUBLIC \
  /Applications/ghidra_12.0_PUBLIC/support/analyzeHeadless \
  /Users/ergin/Desktop/ae-native-renderer/target/reverse/ghidra_projects/agent_geometry_collapse \
  agent_geometry_collapse \
  -process Transform.aex \
  -noanalysis \
  -postScript DumpTransformWrapperDeepNarrow.java \
  /Users/ergin/Desktop/ae-native-renderer/target/reverse/agent_geometry_collapse/transform_wrapper_deep_narrow.txt \
  -scriptPath /Users/ergin/Desktop/ae-native-renderer/target/reverse/agent_geometry_collapse \
  -log /Users/ergin/Desktop/ae-native-renderer/target/reverse/agent_geometry_collapse/ghidra_transform_wrapper_deep_narrow.log
```

## Ghidra Findings

### Transform.aex Ownership

`Transform.aex` is mostly the AE effect wrapper/registration surface. Ghidra
resolved delay-load/import references to `GPUFoundation.dll`:

- `GF::TransformToMatrix`
- `GF::TransformsToMatrices`
- `GF::TransformOperation::TransformOperation`
- `GF::TransformWithMotionBlur`
- `GF::TransformedBounds`
- `GF::TransformedBoundsUnion`

Evidence:

- `target/reverse/agent_geometry_collapse/ghidra_transform_hints.log`
- external refs around `EXTERNAL:00000074` through `EXTERNAL:0000007f`
- delay-load stubs around `18000ea5f` through `18000eb25`

Confidence: confirmed.

Progress chunk 2026-05-04, Transform.aex wrapper boundary:

The wrapper xref pass confirms that the primary GPU render function
`FUN_180009a20` is the callsite that constructs `GF::TransformOperation` and
then calls `GF::TransformWithMotionBlur`.

Recovered call shape:

```text
matrices = GF::TransformsToMatrices(incoming_transform_vector, pixel_aspect)
operation = GF::TransformOperation(
  sample_quality = *(effect_state + 0x60),
  matrices = copied_matrix_vector,
  should_interpolate_end_matrices = true,
  source_geometry,
  dest_geometry,
  dest_rect,
  mask_geometry,
  opacity_multiplier / effect value,
  motion_blur_allowed = true
)
GF::TransformWithMotionBlur(device, src, dst, pixel_format, operation)
```

Implications:

- `Transform.aex` does not implement the sampler kernels itself; it packs
  effect state and delegates to GPUFoundation.
- The GPU render path uses `TransformWithMotionBlur` as the common transform
  backend; the motion/non-motion split is decided inside
  `TransformOperation::Calculate`.
- The UI/effect-state to internal `SampleQuality` mapping is now narrowed to
  the effect-state field at offset `0x60`, but the exact assignment site from
  `Sampling=Bilinear|Bicubic` to the internal enum still needs one more wrapper
  pass.

### Parameter Mapping

There are two numeric namespaces:

| Payload key | AE property index meaning | AE matchName | UI label | Default/value evidence |
| --- | --- | --- | --- | --- |
| `"0001"` | property index 1 | `ADBE Geometry2-0001` | Anchor Point | default `[128,128]` in property dump |
| `"0002"` | property index 2 | `ADBE Geometry2-0002` | Position | default `[128,128]` |
| `"0003"` | property index 3 | `ADBE Geometry2-0011` | Uniform Scale | default `1`; value is effectively boolean |
| `"0004"` | property index 4 | `ADBE Geometry2-0003` | Scale Height | default `100` |
| `"0005"` | property index 5 | `ADBE Geometry2-0004` | Scale Width | default `100` |
| `"0006"` | property index 6 | `ADBE Geometry2-0005` | Skew | default `0` degrees |
| `"0007"` | property index 7 | `ADBE Geometry2-0006` | Skew Axis | default `0` degrees |
| `"0008"` | property index 8 | `ADBE Geometry2-0007` | Rotation | default `0` degrees |
| `"0009"` | property index 9 | `ADBE Geometry2-0008` | Opacity | default `100` percent |
| `"0010"` | property index 10 | `ADBE Geometry2-0009` | Use Composition's Shutter Angle | default `1` |
| `"0011"` | property index 11 | `ADBE Geometry2-0010` | Shutter Angle | default `0` |
| `"0012"` | property index 12 | `ADBE Geometry2-0012` | Sampling | default `1` |

Ghidra corroboration from `Transform.aex` strings:

```text
$$$/AE/Transform/LStr/0003=Scale Height
$$$/AE/Transform/LStr/0004=Scale Width
$$$/AE/Transform/LStr/0005=Rotation
$$$/AE/Transform/LStr/0006=Opacity
$$$/AE/Transform/LStr/0009=Skew
$$$/AE/Transform/LStr/0010=Skew Axis
$$$/AE/Transform/LStr/0011=Shutter Angle
$$$/AE/Transform/LStr/0012=Use Composition's Shutter Angle
$$$/AE/Transform/LStr/0013=Uniform Scale
$$$/AE/Transform/LStr/0015=Sampling
$$$/AE/Transform/LStr/0016=Bilinear|Bicubic
```

Important implication: `ADBE Geometry2-0008` is Opacity, but payload key
`"0008"` is property index 8 and therefore Rotation. This is confirmed by the
AE property dump at
`fixtures/ae_conformance_pack/ae_goldens/metadata/effect_property_dump.json`.

Confidence: confirmed for property index/matchName mapping; likely for enum
value `Sampling=1 -> Bilinear` because the dump only shows value `1` and the
string list is `Bilinear|Bicubic`.

### Matrix Layout And Formula

`GPUFoundation.dll` owns the transform math. Target functions:

- `GF::TransformToMatrix` at `180074eb0`
- matrix multiply helper at `1800727b0`
- rotation helper at `180074c50`
- scale helper at `180075b80`
- `GF::TransformsToMatrices` at `1800757c0`

Internal `GF::Transformation` layout inferred from `TransformToMatrix`
accesses:

```text
0 anchor.x
1 anchor.y
2 position.x
3 position.y
4 scale.x
5 scale.y
6 rotation_degrees
7 skew_degrees
8 skew_axis_degrees
```

Scale values entering `GF::TransformToMatrix` are already normalized factors,
not UI percentages.

Observed matrix construction:

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

The rotation helper normalizes degrees with `fmod(degrees, 360.0)` and uses
`pi / 180`. The skew helper uses `tan(skew * pi / 180)` and flips the sign via
the negative-zero/xor pattern visible in decompile.

Matrix storage/point mapping remains the same as the previous round:

```text
x' = x*m00 + y*m01 + m02
y' = x*m10 + y*m11 + m12
```

Evidence:

- `ghidra_gpu_targeted.log`, `GF::TransformToMatrix` body around log lines
  `451-536`.
- `ghidra_gpu_targeted.log`, helpers `1800727b0`, `180074c50`, `180075b80`.

Confidence: confirmed for order/sign/pixel-aspect placement; inferred for
semantic field names from access order plus validation, because PDB field names
are not available.

### Time Semantics

Ghidra confirms that `GF::TransformsToMatrices` converts a vector of
`GF::Transformation` structs into a vector of matrices. Therefore motion blur
time sampling is upstream of `TransformToMatrix`: each shutter sample is already
a concrete transformation by the time GPUFoundation builds matrices.

`GF::TransformOperation::TransformOperation` accepts:

```text
SampleQuality
vector<MatrixT<double>>
bool
FrameGeometry source
FrameGeometry dest
Rect<int>
optional<MaskGeometry>
double opacity_multiplier
bool
```

`GF::TransformOperation::Calculate` marks motion blur active when any matrix
element differs from the first matrix by more than the observed epsilon
`1e-12`, then builds inverted matrices.

`GF::TransformWithMotionBlur` uses:

- `TransformOperation::IsMotionBlur`
- `TransformOperation::NumMatrices`
- `TransformOperation::Quality`
- `TransformOperation::Matrices`
- `TransformOperation::InvertedMatrices`
- debug key `GPU.MotionBlur.UsingLanczosLowPass`

`Transform.aex` also references `TransformEffectNumSamples`, likely a debug
database override for sample count. This round did not recover the exact
comp-time/shutter-time formula or where `TransformEffectNumSamples` is applied.

Progress chunk 2026-05-04, motion blur accessors:

The deep narrow pass confirms the `TransformOperation` accessor layout used by
the GPU path:

| Accessor | Offset / return | Native implication |
| --- | --- | --- |
| `Quality()` | copies 8 bytes from `this + 0x00` | `SampleQuality` is two packed ints: method and sharpness. |
| `GetOpacityMultiplier()` | `double` at `this + 0x80` | Opacity multiplier is passed into every transform kernel. |
| `IsMotionBlur()` | byte at `this + 0x88` | Active when `Calculate()` detected matrix deltas. |
| `ShouldInterpolateEndMatrices()` | `this[0x89] == 0` | The GPU kernels receive an end-matrix interpolation bool. |
| `MatrixRadii()` | vector at `this + 0xb8` | Area-sampling radii are precomputed per matrix. |

`TransformWithMotionBlur` passes these values into every loaded kernel:

```text
matrix_count
opacity_multiplier
should_interpolate_end_matrices
area/no-area flag and optional radii buffers
tile/source/dest dimensions
```

`TransformEffectNumSamples` was rechecked in `Transform.aex`. The recovered
function around `180003630` is effect/PF setup and debug database registration,
not the shutter scheduler itself. Therefore sample count is visible as a
debug/config key, but the exact shutter sample timestamps/weights are still
not recovered from the inspected GPUFoundation/Transform wrapper path.

The wrapper pass adds one important boundary: `FUN_180009a20` receives an
already-built vector of `GF::Transformation` samples as `param_6`, immediately
converts it through `GF::TransformsToMatrices`, and passes the matrix vector to
`GF::TransformOperation`. Therefore shutter timing and animated-property
sampling happen before the GPU render call. Candidate upstream builders seen in
the wrapper dump include `FUN_1800033a0`, `FUN_180004440`, and `FUN_180004ab0`;
these are the next targets for sample timestamp recovery.

Confidence:

- confirmed that matrix generation is per pre-evaluated transformation sample;
- confirmed that motion blur branches on matrix differences;
- confirmed that kernels receive equal-sized matrix arrays and a matrix count;
- unknown for AE shutter sample timestamps, shutter phase, and param evaluation
  time; these now appear to live upstream of `FUN_180009a20`.

### Sampling Rules

Confirmed Ghidra facts:

- `Transform.aex` exposes UI string `Sampling` with `Bilinear|Bicubic`.
- `GPUFoundation.dll` contains transform kernels/branches named:
  - `XFormMotionBlur_kSamplingMethod_Bilinear_*`
  - `XFormMotionBlur_kSamplingMethod_BicubicAreaSample_*`
  - `XFormMotionBlur_kSamplingMethod_NearestNeighbor_*`
  - `XFormMotionBlur_kSamplingMethod_BicubicLanczos_*`
- `GF::TransformWithMotionBlur` uploads inverted matrices as float buffers,
  9 floats / 0x24 bytes per matrix.
- It branches through `TransformOperation::Quality(this)`.

Progress chunk 2026-05-04, sampler/quality branch:

The narrow Ghidra pass confirms that `SampleQuality` is copied into
`TransformOperation` at offset `0x00` and returned by
`TransformOperation::Quality`. In `GF::TransformWithMotionBlur`, Ghidra names
the copied fields as `local_5a0` and `local_59c`.

Internal sampler method map:

| `SampleQuality` method field | Kernel branch | Evidence |
| ---: | --- | --- |
| `0` | `NearestNeighbor` | `iVar12 == 0` loads `XFormMotionBlur_kSamplingMethod_NearestNeighbor_*`. |
| `1` | `Bilinear` | `iVar12 == 1` loads `XFormMotionBlur_kSamplingMethod_Bilinear_*`. |
| `2` | `BicubicLanczos` | `iVar12 == 2` loads `XFormMotionBlur_kSamplingMethod_BicubicLanczos_*`. |
| `3` | `BicubicAreaSample` | `iVar12 == 3` loads `XFormMotionBlur_kSamplingMethod_BicubicAreaSample_*`. |

Internal sharpness/area map:

| Field/flag | Meaning | Evidence |
| --- | --- | --- |
| `local_59c == 2` | Sharp bicubic branch | Lanczos/AreaSample branches with `local_59c == 2` load `SamplingSharpness_Sharp`. |
| `local_59c != 2` | Smooth bicubic branch | Lanczos/AreaSample branches otherwise load `SamplingSharpness_Smooth`. |
| `local_628 == 0` | Area sampling enabled | Bilinear/AreaSample branches load `_Area` kernels and upload the radii buffers. |
| `local_628 != 0` | No area sampling | Branches load `_NoArea` kernels and pass null area buffers. |

Area/no-area selection is computed from `TransformOperation::MatrixRadii`.
`local_628` starts as no-area and is cleared when the per-sample matrix radii
exceed a small threshold. When area sampling is enabled, two float buffers are
uploaded, one radius pair per matrix. The exact threshold constants are still
unnamed in this pass.

`GPU.MotionBlur.UsingLanczosLowPass` is a debug-database switch in
`TransformWithMotionBlur`. When enabled and the sampled middle matrix matches
the inspected sign/scale condition, the code rewrites `method=2` and
`sharpness=1`, forcing the Lanczos low-pass path. This is a runtime override,
not the normal effect UI mapping.

Not confirmed by Ghidra in this round:

- exact pixel-center convention;
- exact OOB edge behavior;
- exact UI enum mapping beyond `Bilinear|Bicubic`; internal method enum is now
  known, and the value is handed to GPUFoundation from effect-state offset
  `0x60`, but the assignment site that maps Geometry2 `Sampling=2` into
  `BicubicAreaSample` vs `BicubicLanczos` still needs a state-builder pass;
- whether CPU/PF fallback uses the same kernel families. The GPU render path
  inspected does route through `TransformWithMotionBlur` even when
  `TransformOperation::IsMotionBlur()` is false.

Validation corroboration:

- Current native `EFF_040` uses bilinear transparent OOB.
- `EFF_040` is now very close after alpha policy fix: RGB mean `0.0491`,
  RGBA mean `0.0603`.
- Residual diff bbox is `[192,191,512,512]`, mostly transformed-field edge
  area, so pixel-center/OOB/sampler tuning is still the likely remaining
  Geometry2 blocker.

Confidence: confirmed for internal sampler family/quality/area branch
selection in `GF::TransformWithMotionBlur`; likely for bilinear default;
inferred for transparent OOB from validation, not from Ghidra.

### Pixel Center / OOB Threshold

Progress chunk 2026-05-04:

The inspected Ghidra functions reach kernel selection and argument packing, but
not the shader/kernel body where pixel-center and OOB checks are implemented.
The decoded OpenCL sources present under `PTX/CL_decoded/` do not include the
`XFormMotionBlur_*` kernels, so this pass cannot honestly lock the exact
integer-vs-half-pixel convention from source.

Current native implementation in `crates/render-core/src/layer_eval.rs` does:

```text
dst point = (x, y)
src = inverse_matrix * dst - local_origin
OOB guard = round(src_x/src_y) inside [0, width/height)
sampler = bilinear at fractional src
```

This is suspicious: a rounded OOB guard mixed with fractional bilinear sampling
can keep or discard edge pixels differently from an AE kernel that tests
continuous UV, half-pixel centers, or filter footprint bounds. The remaining
`EFF_040` diff bbox `[192,191,512,512]` sits on transformed edges, which is
consistent with a pixel-center/OOB mismatch rather than a matrix-order failure.

Next validation probe should isolate a 1-pixel opaque impulse and a small
checkerboard at source coordinates around:

```text
-0.5, -0.0001, 0.0, 0.4999, 0.5,
width - 1.5, width - 1.0, width - 0.5, width - 0.0001, width
```

Required readout: source UV, selected integer neighbors, output alpha, and
whether AE returns transparent black or a partial bilinear footprint.

### Alpha / Premult Policy

Ghidra does not show alpha compositing inside `TransformToMatrix`; the matrix
function is pure geometry. Alpha/composite is delegated through
`TransformOperation` / `TransformWithMotionBlur` and AE/PF integration.

Evidence:

- `Transform.aex` imports `PF_TransformWorld` and `PF_CompositeModePlus`.
- `GF::TransformOperation::TransformOperation` accepts a `double`
  opacity multiplier.
- `GPUFoundation.dll` exports `GF::TransformOperation::GetOpacityMultiplier`.

Validation after the core alpha fix shows the background alpha policy is no
longer polluting Geometry2:

- `EFF_040`: `background_corner.rgb_matches_alpha_differs_count = 0`.
- `CMP_010`: alpha metrics are exactly zero diff; RGB still differs.

Confidence: confirmed that transform math itself is alpha-independent; inferred
that opacity/composite happens in the transform operation or PF wrapper; exact
premult boundary is owned by Agent A/core-composite, not this Geometry report.

### Color / Numeric Policy

Confirmed:

- matrix math is double precision in `TransformToMatrix`;
- GPU path converts inverted matrices to float before dispatch;
- constants include `pi/180`, `360.0`, identity `1.0`, zero, and matrix-diff
  epsilon `1e-12`;
- `TransformWithMotionBlur` receives a `dvamediatypes::PixelFormat`.

Not recovered:

- 8/16/32 bpc dispatch mapping;
- linear/nonlinear color branch;
- exact integer rounding/clamping policy in transform kernels.

Confidence: confirmed for matrix numeric precision; unknown for pixel numeric
policy.

### Collapse / Deferred Raster Boundary

Progress chunk 2026-05-04:

The AE binary string pass finds UI/warning ownership in After Effects core:

```text
Can't open layers with collapsed transformations.
Continuously rasterized 3D layers
collapsed 3D precomposition layers
Shows bounding box wireframes for components of collapsed precompositions
```

This confirms that collapse is not owned by `Transform.aex`; the plugin only
owns the Transform effect. The actual precomp graph/raster barrier logic is
higher in the AE host/render graph. No reusable collapse formula or matrix
propagation function was recovered from `Transform.aex` in this pass.

Current native boundary:

| Native layer | Behavior |
| --- | --- |
| `precomp.rs::deferred_raster_plan_for_precomp_layer` | Collapse is allowed only for vector/text-only targets with no barriers. |
| `precomp.rs::collect_deferred_raster_barriers` | Barriers are effects, footage, adjustment layers, non-collapsed nested precomps, missing targets, and nested cycles. |
| `layer_eval.rs::render_collapsed_precomp` | Evaluates the precomp layer transform at parent time and starts parent matrix propagation. |
| `layer_eval.rs::render_layer_with_parent_matrix` | Composes `parent_matrix.mul(child_matrix)` for solids, text, and nested collapsed precomps. |
| `layer_eval.rs::collapsed_text_raster` | Text is still rasterized at `matrix_scale_hint(...).clamp(1,4)`, then inverse-scaled back. |

Native implication: the matrix propagation shape is now in the right family,
but it is not AE parity yet. True collapse needs deferred vector/text
rasterization all the way to final composed matrix, with no intermediate text
bitmap except at the final render backend. The current `matrix_scale_hint`
approach reduces blur but is still an approximation and explains why `GPH_010`
should remain a collapse/text regression, not a Geometry2 matrix test.

## Validation Results

Command output:

```text
conformance-pack.done ok=true cases=3
report=target/reverse/agent_geometry_collapse/validation/report.json
```

Summary:

| Case | Frames | RGBA mean | RGB mean | Alpha mean | Max diff | Interpretation |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `EFF_040` | 1 | `0.0603` | `0.0491` | `0.0938` | `61` | Geometry2 is close; residual likely sampler/OOB/pixel-center edge tuning. |
| `GPH_010` | 4 | `14.5753` | `14.1704` | `15.7899` | `255` | Collapse/text slice remains non-parity; not a Geometry2 primitive failure. |
| `CMP_010` | 1 | `4.3968` | `5.8625` | `0.0000` | `188` | Alpha now matches; RGB sampling/composite color policy still differs. |

Diff bbox review from `visual_diff_summary.json`:

| Case | Frames | Diff bbox | Note |
| --- | --- | --- | --- |
| `EFF_040` | `00000` | `[192,191,512,512]` | Edge of transformed coordinate field; good target for sampler/OOB tuning. |
| `GPH_010` | `00000,00015,00030,00045` | `[0,192,512,259]` | Horizontal text/collapse band; likely glyph/collapse raster parity. |
| `CMP_010` | `00000` | `[129,128,384,384]` | RGB-only mismatch in central probe; alpha channel diff is zero. |

## Native Implications

1. Keep current Geometry2 matrix order:

```text
T(position) * S(1/par,1) * R(-axis) * Hx(-tan(skew)) *
R(axis) * R(rotation) * S(scale) * S(par,1) * T(-anchor)
```

2. Treat payload numeric keys as AE property indices, not matchName suffixes.
   Specifically, payload `"0008"` is Rotation; `ADBE Geometry2-0008` is Opacity.

3. Do not tune `GPH_010` as a Geometry2 matrix case. Use it only as a
   collapse/text regression until text raster parity is closer.

4. Next Geometry2 tuning should isolate:
   - pixel center: integer vs half-pixel;
   - OOB threshold: exact transparent boundary at `uv < 0`, `uv > width-1`,
     or a half-pixel convention;
   - bilinear rounding/clamp policy;
   - `Sampling=2` Bicubic branch.

5. Motion blur still needs a focused Ghidra/probe pass:
   - recover upstream runtime shutter sample generation in
     `FUN_1800033a0` / `FUN_180004440` / `FUN_180004ab0`;
   - map Geometry2 shutter slots `0010/0011`;
   - verify whether animated Geometry2 params are sampled at shutter sample
     time or frame time.

6. Implement native sampler quality as a structured enum, not ad hoc booleans:

```text
SampleQuality {
  method: Nearest | Bilinear | BicubicLanczos | BicubicAreaSample,
  sharpness: Smooth | Sharp,
  area_sampling: runtime MatrixRadii/no-area decision
}
```

7. Do not lock collapse parity from current `GPH_010`. The current native path
   composes matrices but still rasterizes text before final backend sampling.

## Remaining Unknowns

- exact UI `Sampling` enum mapping from Geometry2 wrapper into the internal
  `SampleQuality` enum beyond confirmed `Bilinear|Bicubic` UI labels and
  effect-state offset `0x60`;
- exact CPU/PF fallback transform sampler path;
- exact pixel-center convention;
- exact OOB threshold/transparent boundary in the transform kernels;
- exact premult boundary inside transform/filter kernels;
- exact motion-blur sample timestamps and weights;
- exact area-sampling radii threshold constants;
- collapse/deferred-raster matrix propagation in AE host code, beyond the
  current Transform.aex/GPUFoundation boundary.

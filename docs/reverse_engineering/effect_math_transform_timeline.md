# AE Transform / Timeline Math Reverse Notes

Status date: 2026-05-04.

Scope: transforms, precomp/collapse transformations, motion blur sampling, and
keyframe interpolation. This file is derived from
`docs/phase_reports/AE_REVERSE_GEOMETRY2_GHIDRA.md` plus nearby telemetry and
conformance notes. Do not treat it as an effect-wide reverse document.

No new Ghidra/headless run was performed for this note. If future work does run
Ghidra/headless against the local AE binaries, use:

```sh
flock /tmp/ae-native-renderer-ghidra.lock <ghidra-headless-command>
```

or a separate Ghidra project directory. Do not run concurrent headless imports
against the same project without that lock.

## Recovered Native Entrypoints

`Transform.aex` looks like registration/wrapper code for `ADBE Geometry` and
`ADBE Geometry2`. The transform math of interest is in `GPUFoundation.dll`:

```text
GF::TransformToMatrix
GF::TransformsToMatrices
GF::TransformOperation::Calculate
GF::TransformWithMotionBlur
GF::TransformedBounds
GF::TransformedBoundsUnion
```

The currently useful reverse facts are:

- `GF::TransformToMatrix` consumes an internal `GF::Transformation` record with
  nine doubles.
- `GF::TransformedBounds` confirms the matrix point-transform convention.
- `GF::TransformOperation::Calculate` compares matrix elements with
  `1e-12` to decide whether an operation is motion blurred.
- `GF::TransformWithMotionBlur` consumes vectors of inverted matrices and
  uploads 9-float matrices to GPU buffers when blur is active.

## Matrix Convention

`dvacore::geom::MatrixT<double>` storage observed in AE is column-major:

```text
[ m00, m10, m20,
  m01, m11, m21,
  m02, m12, m22 ]
```

The affine point transform observed in `GF::TransformedBounds` is:

```text
x' = x*m00 + y*m01 + m02
y' = x*m10 + y*m11 + m12
```

When written as row-major math, this is the familiar:

```text
[ x' ]   [ m00 m01 m02 ] [ x ]
[ y' ] = [ m10 m11 m12 ] [ y ]
[ 1  ]   [ 0   0   1   ] [ 1 ]
```

The helper at `1800727b0` left-multiplies:

```text
M = New * M
```

So formulas below are written in evaluation order from left to right as the
final matrix product applied to a source-space point.

## Geometry2 Transform Formula

Recovered internal `GF::Transformation` double layout:

| Index | Meaning |
| ---: | --- |
| 0 | anchor.x |
| 1 | anchor.y |
| 2 | position.x |
| 3 | position.y |
| 4 | scale.x, already normalized factor |
| 5 | scale.y, already normalized factor |
| 6 | rotation degrees |
| 7 | skew degrees |
| 8 | skew axis degrees |

This is an internal layout, not proof of AE stream/property ids.

Approximate recovered forward matrix:

```text
M =
  T(position)
  * S(1 / pixel_aspect, 1)
  * R(-skew_axis)
  * Hx(-tan(skew_degrees * pi / 180))
  * R(skew_axis)
  * R(rotation)
  * S(scale_x, scale_y)
  * S(pixel_aspect, 1)
  * T(-anchor)
```

If skew is zero, the skew block is skipped. Constants observed:

```text
pi / 180 = 0.017453292519943295
full turn = 360.0
identity = 1.0
matrix-diff epsilon = 1e-12
```

AE UI scale percentages must be converted before entering this formula:

```text
scale_factor = ui_percent / 100
```

The native layer transform subset, without Geometry2 skew/pixel-aspect handling,
is currently:

```text
M_layer = T(position) * R(rotation) * S(scale / 100) * T(-anchor)
```

This is the non-skew 2D subset to keep aligned with `GF::TransformToMatrix`,
but AE layer transform parity still needs goldens for pixel aspect,
continuously-rasterized vector/text cases, negative scale, and subpixel
sampling.

## Helper Formulas To Preserve

Rotation helper as recovered:

```text
angle = fmod(degrees, 360.0) * pi / 180

matrix storage =
[ cos,  sin, 0,
 -sin,  cos, 0,
  tx,   ty,  1 ]

tx = (1 - cos) * pivot_x + sin * pivot_y
ty = (1 - cos) * pivot_y - sin * pivot_x
```

Scale helper as recovered:

```text
matrix storage =
[ sx, 0,  0,
  0,  sy, 0,
  (1 - sx) * pivot_x,
  (1 - sy) * pivot_y,
  1 ]
```

For renderer implementation, always be explicit about whether a matrix is in AE
column-major storage or native row-major arrays. The numeric transform is the
same; only indexing changes.

## Parent, Precomp, And Collapse Order

Working matrix rule for collapsed/nested content:

```text
M_world_child = M_parent_boundary * M_child_local
```

For a chain of collapsed precomps:

```text
M_effective =
  M_root_precomp_layer_at_parent_time
  * M_nested_precomp_boundary_at_nested_time
  * ...
  * M_source_layer_at_source_time
```

The Round 5 deferred-raster contract records transform steps in this order:

```text
root precomp boundary
nested collapsed precomp boundaries
source layer
```

and source-time steps as:

```text
child_time = max(parent_time - precomp_layer.start, 0.0)
```

Open reverse questions:

- Does AE clamp collapsed precomp time at zero in every relevant mode, or does
  time remap / stretch / negative start require a separate branch?
- Where exactly are parent-layer matrices composed relative to precomp boundary
  matrices for mixed parented + collapsed scenes?
- Which layer types are true deferred-raster primitives under collapse:
  text, shape/vector, solids, continuously rasterized Illustrator, footage,
  adjustment layers, effects, masks, mattes?
- Does opacity compose as layer opacity after deferred raster, or as a property
  sampled on the source primitive before parent opacity?

Implementation invariant to preserve while tuning:

```text
collapse should defer raster only when every crossed layer is a supported
vector/text primitive and no raster barrier exists.
```

Known raster barriers in current diagnostics: footage, adjustment layers,
effects, missing targets, cycles, and non-collapsed nested precomps.

## Motion Blur Sampling Model

Recovered GF behavior:

- `TransformOperation::Calculate` marks an operation as blurred when any matrix
  element changes by more than `1e-12`.
- Both forward and inverted matrix vectors are retained.
- `TransformWithMotionBlur` uses `TransformOperation::NumMatrices()` when blur
  is active.
- Inverted matrices are converted from double to float, 9 floats per matrix,
  for GPU sampling.
- Sampling mode strings seen nearby include `Bilinear`, `BicubicAreaSample`,
  `NearestNeighbor`, and `BicubicLanczos`; `GPU.MotionBlur.UsingLanczosLowPass`
  is referenced.

Current native diagnostic model:

```text
sample_count defaults to 17 for AE Transform/Geometry2-style motion blur
exposure = frame_duration * shutter_angle / 360
open     = frame_time + frame_duration * shutter_phase / 360
close    = open + exposure

sample_i = open + exposure * ((i + 0.5) / sample_count)
weight_i = 1 / contributing_sample_count
```

Samples are clamped by implementation limits to `1..64`. This is testable, but
not yet proven AE parity. Reverse/probe questions still open:

- Is AE's sample placement midpoint, endpoint-inclusive, adaptive, or quality
  dependent for each renderer mode?
- Is shutter phase sign/origin exactly `frame_time + phase/360 * frame_duration`
  for AE 2026 export settings?
- Are weights uniform, shutter-shaped, or filtered by the Lanczos low-pass path?
- Does AE skip static layers before sampling, and is the `1e-12` matrix epsilon
  the only static-test threshold?
- Are opacity, effects, text raster, and source frame selection evaluated per
  sample or partially cached outside the sample loop?
- How do Posterize Time and motion blur interact when samples fall in multiple
  posterize buckets?

## Timeline And Interpolation Functions To Find

Search targets for the next reverse pass:

- Property value evaluator: maps comp time/layer time to stream value.
- Keyframe span search: handles `time <= first`, `time >= last`, duplicate
  times, and exact keyframe equality.
- Hold/linear/Bezier branch: native payload has observed interpolation ids
  `6614` for hold and `6613` for Bezier, but these are not yet mapped to AE
  binary functions.
- Temporal ease mapper: converts AE `KeyframeEase(speed, influence)` into
  interpolation. Current native approximation uses influence only:

```text
x1 = clamp(out_influence / 100, 0.05, 0.95)
y1 = 0
x2 = clamp(1 - in_influence / 100, 0.05, 0.95)
y2 = 1
```

- Cubic solver: current native solves `BezierX(u)=linear_time` with six Newton
  iterations and returns `BezierY(u)`.
- Per-dimension ease: position/scale may carry separate temporal ease arrays;
  current native uses only the first ease entry.
- Spatial interpolation: paths, tangents, roving keyframes, auto Bezier, and
  separated dimensions are out of current scope but must not be confused with
  scalar temporal ease.
- Posterize Time routing: exact order for layer time, source time, adjustment
  lower-stack time, effect param time, and motion-blur sample time.
- Time remap, stretch, nested comp frame rounding, and display-start offsets.

Native current keyframe behavior to compare against AE:

```text
if no keyframes:
  fallback static value
if time <= first.time:
  first.value
for each span [a,b]:
  if time <= b.time:
    if a.hold or b.time <= a.time:
      a.value
    else:
      t = (time - a.time) / (b.time - a.time)
      t = ease(a.ease, t) or linear t
      lerp(a.value, b.value, t)
after last:
  last.value
```

## Property Ids And Unknowns

Do not conflate these layers of identity:

- AE UI stream names and matchNames.
- Numeric effect parameter ids such as `0001`.
- Generator payload keys such as `tf_position`.
- Internal `GF::Transformation` slots.

Tentative/currently consumed mappings:

| Area | Current/native names | Status |
| --- | --- | --- |
| Layer anchor | `tf_anchor`, `ADBE Anchor Point` payload match name | payload-level only |
| Layer position | `tf_position`, `ADBE Position` payload match name | payload-level only |
| Layer scale | `tf_scale`, `ADBE Scale` payload match name | payload-level only |
| Layer rotation | `tf_rotation`, `ADBE Rotate Z`/rotation payload names | payload-level only |
| Layer opacity | `tf_opacity`, `layer_opacity`, `ADBE Opacity` | payload-level only |
| Layer motion blur | `layer_meta.motion_blur` / `motionBlur` | payload metadata, AE switch id unknown |
| Collapse transformations | `layer_meta.collapseTransformation` | payload metadata, AE switch id unknown |
| Geometry2 anchor | `anchor`, `anchorPoint`, `Anchor Point`, `0001` | needs JSX/property dump confirmation |
| Geometry2 position | `position`, `Position`, `0002` | needs confirmation |
| Geometry2 scale/uniform | `scale`, `scaleX`, `scaleY`, `uniformScale`, `0003`, `0004`, `0005` | highest-risk mapping; `0003` may be uniform flag or uniform value depending payload |
| Geometry2 rotation | `rotation`, `Rotation`, `0008` | needs confirmation |
| Geometry2 skew | `skew`, `Skew`, `0009` | needs confirmation |
| Geometry2 skew axis | `skewAxis`, `skew_axis`, `Skew Axis`, `0010` | needs confirmation |
| Geometry2 pixel aspect | `pixelAspect`, `pixelAspectRatio` aliases | numeric AE id unknown |

Important unknowns:

- Whether `ADBE Geometry2` exposes opacity and, if so, its numeric id and order
  relative to rotation/skew in AE 2026.
- Whether `0003` in Geometry2 is a uniform-scale checkbox, a uniform scale
  value, or fixture-specific shorthand.
- Whether scale width/height order is `0004`/`0005` in all locales/builds.
- Sampling quality enum ids for Geometry2 and layer transform resampling.
- Edge behavior id/default for transparent, repeat, clamp, or mirror modes.
- Exact composition/layer motion blur setting ids: sample count, shutter angle,
  shutter phase, adaptive sample limit, and renderer quality mode.

## Implementation Gaps

Transform / Geometry2:

- Confirm Geometry2 numbered parameter ids with an AE property dump.
- Add matrix telemetry for normal layer transforms, Geometry2 effects, and
  collapsed primitives in the same row/column convention.
- Compare forward matrix, inverse matrix, bounds, and representative UV samples
  against AE before pixel tuning.
- Replace Geometry2 nearest-round sampling with AE sampling-mode semantics.
- Audit half-pixel/pixel-center convention and edge policy.
- Verify pixel-aspect behavior for normal layer transform and precomp boundary
  transforms, not only Geometry2.

Precomp / collapse:

- Consume the deferred-raster contract end-to-end for every eligible primitive.
- Emit per-step transform/time telemetry for collapse chains.
- Keep text layout in source units unless an AE-matched scale-aware layout mode
  is explicitly selected.
- Add parented-layer + collapse fixtures; current collapse tests are too narrow.
- Distinguish "collapse supported" from "rasterize first" in final-frame
  reports and template blocker summaries.

Motion blur:

- Compare AE shutter window and sample placement before changing formulas.
- Record per-sample forward/inverse matrices and bounds, not only times.
- Audit premultiplied-alpha accumulation; AE TIFF exports are premultiplied.
- Skip static layers only after matching AE's matrix/effect static criteria.
- Confirm whether effects and source sampling are evaluated for every sample.

Keyframes / timeline:

- Export AE scalar/Vec2 property samples, not only PNGs, for linear/hold/Bezier
  cases.
- Use speed + influence for AE temporal ease; current compact cubic ignores
  speed and multidimensional value velocity.
- Preserve per-dimension ease arrays.
- Add boundary probes at `key_time - epsilon`, `key_time`, and
  `key_time + epsilon`.
- Keep Posterize Time and motion blur sample-time routing visible in telemetry.

## Fixtures And Goldens Needed

Existing useful pack cases:

| Case | Purpose |
| --- | --- |
| `INT_010` | linear/hold position and opacity interpolation |
| `INT_020` | Bezier/ease position and opacity interpolation |
| `EFF_040` | Geometry2 coordinate-field transform |
| `TMP_030` | motion blur velocity and shutter window |
| `STK_030` | Geometry2 -> Posterize Time -> Minimax -> Turbulent stack |
| `GPH_010` | nested precomp and collapse-transform text sharpness |
| `CMP_010` | alpha/composite/sampling audit |

Additional focused goldens:

- `TRN_MATRIX_010`: static anchor/position/scale/rotation on coordinate field;
  export AE matrix-equivalent point samples if possible.
- `TRN_MATRIX_020`: negative scale, non-zero anchor, rotation, half-pixel
  translation.
- `GEO2_IDS_010`: Geometry2 every parameter set one at a time, with JSX dumping
  `propertyIndex`, `matchName`, display name, default, min/max, and value type.
- `GEO2_SAMPLER_010`: coordinate field with nearest/bilinear/bicubic quality
  settings if AE exposes them for Transform/Geometry2.
- `GEO2_SKEW_PAR_010`: skew + skew axis + non-1 pixel aspect, with UV telemetry
  from representative output points.
- `PARENT_010`: parented 2D layers with non-zero parent anchor and child anchor.
- `COLLAPSE_PARENT_010`: parented precomp layer with collapse enabled and nested
  source layer transform.
- `COLLAPSE_TIME_010`: nested precomp with non-zero start times, source start,
  and animated child transform.
- `MB_SHUTTER_010`: shutter angle 0/90/180/360 and phase -180/-90/0/90 on the
  same moving square.
- `MB_STATIC_010`: static transform but animated opacity/effect params to learn
  what AE considers motion-blur-worthy.
- `MB_POSTERIZE_010`: motion blur samples crossing Posterize Time bucket
  boundaries on numbered frames.
- `INT_EASE_VALUES_010`: JSX/ExtendScript dump of `valueAtTime()` for scalar,
  2D position, and separated dimensions with known speed/influence.
- `INT_BOUNDARY_010`: hold/linear/Bezier exact keyframe and one-subframe
  boundary samples.

Minimum telemetry/golden payload per case:

```text
frame/time
layer id and source id
raw property ids/names/values
sampled property value at comp/layer/source time
forward and inverse matrices
representative source UVs
motion-blur sample times/weights/matrices when enabled
precomp boundary time and transform steps when nested/collapsed
AE PNG/TIFF reference plus native PNG and diff metrics
```

## Next Reverse Pass Checklist

1. Dump AE Geometry2 property metadata via JSX before trusting numeric ids.
2. Search `GPUFoundation.dll` for callers that build `GF::Transformation`
   arrays from AE stream values.
3. Search for the matrix-vector generation feeding
   `GF::TransformWithMotionBlur`; identify sample count and time placement.
4. Find or probe the temporal ease conversion from AE speed/influence to curve
   samples.
5. Export AE `valueAtTime()` tables for the interpolation fixtures and compare
   them to native telemetry before tuning pixels.
6. Add collapse/precomp parent-chain goldens with both rasterized and collapsed
   variants.

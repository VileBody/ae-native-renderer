# AE Reverse Geometry2 Ghidra Notes

Status date: 2026-05-04.

This is a local reverse/probe note for Adobe After Effects 2026 binaries pulled
from the reserved `85.239.48.31` AE node. Do not commit or redistribute Adobe
binaries; local copies are under `target/reverse/ae_2026/`.

## Inputs

- `Transform.aex`
  - local path: `target/reverse/ae_2026/Transform.aex`
  - sha256: `21661b33ef1b6aefdc9a686316c0be5d29642683eb94848cdc58d6ad38ef17ba`
- `GPUFoundation.dll`
  - local path: `target/reverse/ae_2026/GPUFoundation.dll`
  - sha256: `36ea8222bc1e3abae148771cbea03eb60cd4bbcc708e145af5dd9451782d2466`

Ghidra headless scripts/logs:

- `target/reverse/ghidra_scripts/DumpTransformHints.java`
- `target/reverse/ghidra_scripts/DumpGpuFoundationHints.java`
- `target/reverse/ghidra_scripts/DumpGpuTransformTargeted.java`
- `target/reverse/ae_2026/ghidra_transform_dump_deep.log`
- `target/reverse/ae_2026/ghidra_gpufoundation_dump.log`
- `target/reverse/ae_2026/ghidra_gpufoundation_targeted.log`

## Main Finding

`Transform.aex` appears to be mostly effect registration / wrapper code for
`ADBE Geometry` and `ADBE Geometry2`. The transform math is exported from
`GPUFoundation.dll`:

```text
GF::TransformToMatrix
GF::TransformsToMatrices
GF::TransformOperation::Calculate
GF::TransformWithMotionBlur
GF::TransformedBounds
GF::TransformedBoundsUnion
```

## Inferred Transformation Layout

`GF::TransformToMatrix` receives a `GF::Transformation` with nine doubles.
Based on the matrix construction sequence:

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

Important: this is the internal `GF::Transformation` layout, not proof of AE
stream property ids. Property id mapping still needs JSX/property-dump
confirmation.

## Matrix Layout

`dvacore::geom::MatrixT<double>` is stored column-major:

```text
[ m00, m10, m20,
  m01, m11, m21,
  m02, m12, m22 ]
```

Point transform observed in `GF::TransformedBounds`:

```text
x' = x*m00 + y*m01 + m02
y' = x*m10 + y*m11 + m12
```

The helper at `1800727b0` left-multiplies the current matrix:

```text
M = New * M
```

## Geometry2 Matrix Formula

`GF::TransformToMatrix` builds approximately:

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
negative sign flip uses -0.0 bit xor
```

Scale fields in `GF::Transformation` are already normalized factors. AE UI
percent values still need conversion before this core formula.

## Rotation And Scale Helpers

Rotation helper:

```text
angle = fmod(degrees, 360.0) * pi / 180
matrix = [
  cos,  sin, 0,
 -sin,  cos, 0,
  tx,   ty,  1
]

tx = (1 - cos) * pivot_x + sin * pivot_y
ty = (1 - cos) * pivot_y - sin * pivot_x
```

Scale helper:

```text
matrix = [
  sx, 0,  0,
  0,  sy, 0,
  (1 - sx) * pivot_x,
  (1 - sy) * pivot_y,
  1
]
```

## Motion Blur Notes

`GF::TransformOperation::Calculate` marks an operation as motion-blurred when
any matrix element differs by more than `1e-12`. It keeps both forward and
inverted matrix vectors.

`GF::TransformWithMotionBlur`:

- uses `TransformOperation::NumMatrices()` when motion blur is active;
- converts inverted matrices from double to float GPU buffers, 9 floats per
  matrix;
- references `GPU.MotionBlur.UsingLanczosLowPass`;
- branches by sampling method strings including `Bilinear`,
  `BicubicAreaSample`, `NearestNeighbor`, and `BicubicLanczos`.

## Native Implementation Update

`crates/effects/src/geometry.rs` now mirrors the recovered
`GF::TransformToMatrix` order for the native Geometry2 matrix:

- `T(position)`;
- pixel-aspect compensation before/after transform math;
- rotation;
- skew and skew axis through `Hx(-tan(skew))`;
- normalized percent scale;
- `T(-anchor)`;
- generic affine inverse for source-UV sampling and telemetry.

Tests added in the same module cover:

- recovered skew/pixel-aspect matrix order;
- skew moving y into x using the observed negative shear sign;
- pixel-aspect affecting the rotation basis;
- forward/inverse matrix multiplication sanity.

The conformance runner now records the full resolved Geometry2 state, including
skew, skew axis, pixel aspect, per-sample source UV, rounded sample index, and
sampled RGBA. It also reports RGB/alpha split metrics plus RGB composited over
native and AE background colors, so the known background-alpha mismatch does not
hide matrix/sampler progress.

Remote AE property dump from
`fixtures/ae_conformance_pack/ae_goldens/metadata/effect_property_dump.json`
confirmed that the JSX numeric keys are AE property indices:

```text
1  Anchor Point                  ADBE Geometry2-0001
2  Position                      ADBE Geometry2-0002
3  Uniform Scale                 ADBE Geometry2-0011
4  Scale Height                  ADBE Geometry2-0003
5  Scale Width                   ADBE Geometry2-0004
6  Skew                          ADBE Geometry2-0005
7  Skew Axis                     ADBE Geometry2-0006
8  Rotation                      ADBE Geometry2-0007
9  Opacity                       ADBE Geometry2-0008
10 Use Composition's Shutter     ADBE Geometry2-0009
11 Shutter Angle                 ADBE Geometry2-0010
12 Sampling                      ADBE Geometry2-0012
```

## Remaining Renderer Implications

Current native `ADBE Geometry2` is still approximate:

- confirm whether bilinear is the correct AE sampling mode for the isolated
  Geometry2 fixture, and whether bicubic/quality settings alter this path;
- implement/verify Geometry2's own opacity, shutter, motion blur, and sampling
  controls only if templates actually use those slots;
- compare native matrix/UV telemetry against AE goldens before tuning final
  pixels.

# Effect Math Reverse Notes: Temporal / Noise / Distort

Status date: 2026-05-04

Scope: `ADBE Posterize Time`, `ADBE Minimax`, `ADBE Turbulent Displace`, and
the adjustment-layer/time routing clues that affect those effects. This note
only summarizes already-present repo evidence: copied binaries, Ghidra logs,
phase reports, AE probe packs, generated AE goldens, sidecars, and current
native debug traces. No Ghidra/headless run was started for this pass.

## Evidence Index

- Temporal/Posterize: `docs/phase_reports/PHASE_2_TEMPORAL_POSTERIZE.md`,
  `AGENT_2_TEMPORAL_GRAPH_NATIVE_DIFF.md`,
  `AGENT_TEMPORAL_STACK_TELEMETRY.md`,
  `AGENT_ROUND2_TEMPORAL_STACK.md`,
  `AGENT_ROUND7_TEMPORAL_MOTION.md`,
  `AGENT_ROUND8_TEMPORAL_MOTION.md`.
- Minimax: `docs/phase_reports/AGENT_ROUND4_MINIMAX.md`,
  `AGENT_ROUND5_MINIMAX_PROBE_PACK.md`,
  `fixtures/ae_probe_pack/minimax/*`, and rendered AE probe bundle under
  `fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/minimax`.
- Turbulent Displace: `docs/phase_reports/AGENT_ROUND3_TURBULENT.md`,
  `AGENT_ROUND4_TURBULENT_FIELD_PROBES.md`,
  `AGENT_ROUND5_TURBULENT_FIELD_PROBE_PACK.md`,
  `TURBULENT_CONTROLS_PASS.md`, `fixtures/ae_probe_pack/turbulent_field/*`,
  rendered AE probe bundle under
  `fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field`,
  and `target/ae_agents/round8_os_integration/turbulent_native_compare_round8_all.json`.
- Native trace sidecars: examples in `target/ae_probe_ingest/**/effects_debug`
  and `target/ae_agents/**/effects_debug`.
- Binaries/Ghidra: `docs/phase_reports/AE_REVERSE_GEOMETRY2_GHIDRA.md` and
  `target/reverse/**`.

## Binaries, Symbols, Functions

Present local AE binary copies:

| Binary | Path | SHA-256 | Relevance |
| --- | --- | --- | --- |
| `Transform.aex` | `target/reverse/ae_2026/Transform.aex` | `21661b33ef1b6aefdc9a686316c0be5d29642683eb94848cdc58d6ad38ef17ba` | Geometry/transform wrapper. Not direct Posterize/Minimax/Turbulent evidence. |
| `GPUFoundation.dll` | `target/reverse/ae_2026/GPUFoundation.dll` | `36ea8222bc1e3abae148771cbea03eb60cd4bbcc708e145af5dd9451782d2466` | Transform/motion-blur GPU math, useful for time pipeline and sampler naming. |

Already dumped Ghidra logs/scripts:

- `target/reverse/ae_2026/ghidra_transform_dump*.log`
- `target/reverse/ae_2026/ghidra_gpufoundation_*.log`
- `target/reverse/ghidra_scripts/DumpTransformHints.java`
- `target/reverse/ghidra_scripts/DumpGpuFoundationHints.java`
- `target/reverse/ghidra_scripts/DumpGpuTransformTargeted.java`

Recovered/exported transform symbols from `GPUFoundation.dll`:

```text
GF::TransformToMatrix
GF::TransformsToMatrices
GF::TransformOperation::Calculate
GF::TransformWithMotionBlur
GF::TransformedBounds
GF::TransformedBoundsUnion
```

Motion/sampler clues from `GF::TransformWithMotionBlur`:

- Uses `TransformOperation::NumMatrices()` when motion blur is active.
- Converts inverted matrices to float GPU buffers, 9 floats per matrix.
- References `GPU.MotionBlur.UsingLanczosLowPass`.
- Branch strings include `Bilinear`, `BicubicAreaSample`, `NearestNeighbor`,
  and `BicubicLanczos`.

No copied `.aex`/`.dll` found in this repo currently names `ADBE Posterize
Time`, `ADBE Minimax`, or `ADBE Turbulent Displace` directly. The direct effect
math evidence below therefore comes from AE property dumps, rendered probe
goldens, and native/AE telemetry, not decompiled effect binaries.

## Posterize Time

### Inferred Params

| AE match/name | Native key(s) | Meaning |
| --- | --- | --- |
| `ADBE Posterize Time-0001` / Frame Rate | `0001`, `frameRate`, `frame_rate`, `Frame Rate` | Posterize FPS. Values `<= 0` or non-finite behave as pass-through. |

### True Temporal Behavior Inferred

Isolated layer/source case `TMP_020` confirms AE-like behavior for 6 fps
Posterize in a 30 fps comp: frames `0..4` sample bucket `0.0`, frame `5`
samples `1/6`, frames `10,11` sample `1/3`, etc. RGB matched exactly after
ignoring the known background-alpha substrate.

Core bucket formula:

```text
bucket_id = floor(time * posterize_fps + epsilon)
bucket_time = bucket_id / posterize_fps
epsilon ~= 1e-9 for floating boundary tolerance
```

Layer/source routing:

```text
comp_time = frame / comp_fps
posterized_layer_time = apply_posterize_effects_in_order(layer.effects, comp_time)

footage_source_time = max(0, source_start + (posterized_layer_time - layer_start))
precomp_source_time = max(0, posterized_layer_time - layer_start)
effect_time_for_non_adjustment_layer = posterized_layer_time
```

Canvas-stage `ADBE Posterize Time` is a no-op because temporal sampling already
happened before pixels are passed to stateless effects.

### Adjustment Layer / Stack Clues

Earlier native behavior over-posterized `STK_030`: the adjustment layer used one
quantized time for lower-stack resampling and all downstream effects, making
native frames `0` and `1` byte-identical inside the same 6 fps bucket while AE
frames still differed.

Current inferred routing after telemetry:

```text
time = comp_time
first_pt = first ADBE Posterize Time in adjustment effect stack

if first_pt exists:
    lower_stack_time = quantize(time, first_pt.fps)
else:
    lower_stack_time = time

input_canvas = render_lower_stack_at(lower_stack_time)

for effect_index, effect in adjustment.effects:
    if first_pt exists and effect_index <= first_pt.index:
        param_time = first_pt.bucket_time
    else:
        param_time = comp_time
    canvas = effect.render(canvas, param_time)
```

This preserves isolated `TMP_020` bucket behavior while allowing downstream
animated effects after Posterize Time, including Minimax radius or Turbulent
evolution, to remain live within the same lower-stack bucket.

### Unknowns

- Exact AE behavior for multiple Posterize Time effects in one stack is only
  approximated by sequential quantization for ordinary layers and first
  Posterize routing for adjustment layers.
- Interaction with motion blur is traceable but not AE-parity proven. Existing
  trace shows shutter-phase-shifted sample times can collapse into one
  posterize bucket, but AE shutter sample weights and source-frame rounding are
  still open.

## Minimax

### Inferred Params

AE property dump from the Round 5 probe bundle:

| Index | Match name | UI name | Observed role/value |
| ---: | --- | --- | --- |
| 1 | `ADBE Minimax-0001` | Operation | Popup scalar. Values `1` and `2` probed. |
| 2 | `ADBE Minimax-0002` | Radius | Radius scalar. Values `0` and `12` probed. |
| 3 | `ADBE Minimax-0003` | Channel | Popup scalar. Values `1` and `2` probed. |
| 4 | `ADBE Minimax-0004` | Direction | Default `1`; not yet varied. |
| 5 | `ADBE Minimax-0005` | Don't Shrink Edges | Default `0`; not yet varied. |
| 6 | `ADBE Effect Built In Params` | Compositing Options | Built-in group. |

AE scripting did not expose popup labels; enum meaning is inferred from
rendered pixels.

### AE Probe Matrix Summary

Source: 256x256 precomp with centered 128x128 opaque white square, placed into a
512x512 comp at position `[256,256]`, scale `[70,70]`. Pre-effect bbox:
`[211,211,300,300]`. Row samples are at `y=256`.

| Case | Params | BBox | Key row behavior |
| --- | --- | --- | --- |
| `MINIMAX_PRE` | none | `[211,211,300,300]` | x211/x300 antialias `[205,205,205,204]`, interior white. |
| `OP1_CH1_R0` | `0001=1,0002=0,0003=1` | same as pre | Radius 0 identity. |
| `OP1_CH1_R12` | `0001=1,0002=12,0003=1` | same extent | RGB dark edge ring, alpha mostly preserved: x212 `[0,0,0,255]`, x219 `[102,102,102,255]`, x222 white. |
| `OP1_CH2_R0` | `0001=1,0002=0,0003=2` | same as pre | Radius 0 identity. |
| `OP1_CH2_R12` | `0001=1,0002=12,0003=2` | `[219,219,292,292]` | Erodes alpha/RGB; outside becomes background alpha 0; edge x219 `[105,105,106,102]`. |
| `OP2_CH1_R0` | `0001=2,0002=0,0003=1` | same as pre | Radius 0 identity. |
| `OP2_CH1_R12_EFF050` | `0001=2,0002=12,0003=1` | same as pre | No visible change from pre for this white-square probe. |
| `OP2_CH2_R0` | `0001=2,0002=0,0003=2` | same as pre | Radius 0 identity. |
| `OP2_CH2_R12` | `0001=2,0002=12,0003=2` | `[202,202,309,309]` | Dilates/expands alpha/RGB. |

Most likely enum mapping from this probe:

```text
operation 1 = minimum
operation 2 = maximum
channel 1   = RGB/color channels
channel 2   = alpha
```

`EFF_050` tuple `{0001:2, 0002:12, 0003:1}` is therefore `maximum + RGB`, not
the older suspected `minimum + alpha`. On a solid white square this appears
unchanged because max RGB over white/background does not alter the white
interior or antialiased source extent in this specific probe.

### Formula / Pseudocode

Current useful model for channel/operation:

```text
radius_px = round(radius)
radius_px = clamp(radius_px, 0, 32)   # native clamp; AE max still not proven

if radius_px == 0:
    return input

for output pixel p:
    neighborhood = pixels within radius around p
    if operation == minimum:
        extrema = per-channel min(neighborhood)
    else:
        extrema = per-channel max(neighborhood)

    out = input[p]
    if channel == RGB:
        out.rgb = extrema.rgb
    if channel == Alpha:
        out.a = extrema.a
        # AE rendered PNG also changes premultiplied RGB where alpha changes.
```

Observed AE alpha-channel operation affects RGB in exported premultiplied PNGs
because alpha and premultiplication/composite are coupled in the final render.
Native implementation should separate internal straight/premult math from
final PNG interpretation before overfitting row colors.

### Unknowns

- `Direction` (`0004`) neighborhood shape/orientation is unprobed. Current
  native uses a square neighborhood; AE may use horizontal/vertical/directional
  kernels depending on `0004`.
- `Don't Shrink Edges` (`0005`) is unprobed and likely changes edge handling.
- Radius rounding for fractional radii and AE radius limits are unprobed.
- Neighborhood shape for direction default `1` is inferred only from one
  square-source matrix; need impulse/ramp probes.
- Internal alpha vs premult rules are not isolated because current evidence is
  final PNG/TIFF, not effect intermediate buffers.

## Turbulent Displace

### Inferred Params

AE property dump from `turbulent_field_builder_log.txt`:

| Index | Match name | UI name | Default/observed |
| ---: | --- | --- | --- |
| 1 | `ADBE Turbulent Displace-0001` | Displacement | Default `1`; variants `1..9` rendered. |
| 2 | `ADBE Turbulent Displace-0002` | Amount | Default `50`; sweeps `0,1,10,45,100`. |
| 3 | `ADBE Turbulent Displace-0003` | Size | Default `100`; AE rejects `1`, range starts at `2`; sweeps attempted. |
| 4 | `ADBE Turbulent Displace-0004` | Offset (Turbulence) | Point, default `[256,256]` in AE dump/probe builder. |
| 5 | `ADBE Turbulent Displace-0005` | Complexity | Default `1`; sweeps `1,2,3,4,6`. |
| 6 | `ADBE Turbulent Displace-0006` | Evolution | Degrees scalar/keyframes. |
| 7 | `ADBE Turbulent Displace-0007` | Evolution Options | Group/unreadable. |
| 8 | `ADBE Turbulent Displace-0008` | Cycle Evolution | Default `0`; unmodeled. |
| 9 | `ADBE Turbulent Displace-0009` | Cycle (in Revolutions) | Default `1`; unmodeled. |
| 10 | `ADBE Turbulent Displace-0010` | Random Seed | Numeric seed; variants `0,1,2,10,999` rendered. |
| 11 | `ADBE Turbulent Displace-0011` | Random Seed | Unreadable/group or duplicate entry; unmodeled. |
| 12 | `ADBE Turbulent Displace-0012` | Pinning | Default `3`; AE rejects `0`, accepts `1`; native currently clamps. |
| 13 | `ADBE Turbulent Displace-0013` | Resize Layer | Boolean-ish `0/1`. |
| 14 | `ADBE Turbulent Displace-0014` | Antialiasing for Best Quality | Default `1`; unmodeled. |
| 15 | `ADBE Effect Built In Params` | Compositing Options | Built-in group. |

### AE Field Probe Evidence

Coordinate-field source decodes sampled source coordinates from rendered RGB:

```text
R = source_x mod 256
G = source_y mod 256
B = 32px tile parity
A = 255

dx = decoded_source_x - output_x
dy = decoded_source_y - output_y
```

Important observed probe facts:

- `amount=0` is exact pass-through across 2502 sampled points.
- `amount` roughly scales displacement magnitude, but native sine turbulence is
  not AE's field. Round 8 all-case comparison summary: mean vector error
  `13.70`, max p95 vector error `75.66`.
- `size=1` was rejected by AE with range `2..1000`; implementation should clamp
  at least to `2`.
- Displacement type variants materially change basis:
  - type 1 center at amount 45/size 65/evolution 0: AE `(dx,dy)=(8,-6)`.
  - type 2 center: `(-12,-3)`.
  - type 5 center: `(27,-7)`.
  - type 6 center: `(6,10)`.
  - type 7/8 center: `(-10,6)`.
  - type 9 center: `(0,5)`.
- Static evolution changes field and is not simply native sine phase parity:
  at `E360`, center AE `(-22,4)` while current native model predicts about
  `(5,8)` for the same type-1 fixture.
- Animated evolution `0 -> 180` over 2 seconds changes every frame; e.g. center
  moves from `(8,-6)` at frame 0 toward `(5,-16)` by frame 59 in the AE decode.
- Seed variants materially change the field; seed 1 center AE `(-8,4)`.
- Sampler-check cases have much smaller aggregate vector error than formula
  sweeps, suggesting decode/sampler trust is usable before field fitting.
- Default/pinned coordinate-field renders keep full alpha at edges; native
  transparent OOB was too harsh. Later native control pass switched default
  pinning to clamped edge behavior for `pinning > 0` or `resize_layer=true`.

### Current Native Approximation

Native model currently resolves:

```text
amount = clamp(0002, 0, 200)
size = clamp(0003, 2, 1000)
complexity = round(0005), clamped 1..6
evolution_degrees = sample 0006 at effect time
random_seed = round(0010)
pinning = round(0012)
resize_layer = bool(0013)
amplitude = amount * 0.25
phase = radians(evolution_degrees) + random_seed * 0.618034
offset = 0004 point
```

Current field is deterministic but explicitly approximate:

```text
nx = (x - offset.x) / size
ny = (y - offset.y) / size

noise_x = turbulence(nx + 17 + seed*0.137,
                     ny      + seed*0.071,
                     phase,
                     complexity)
noise_y = turbulence(nx      + seed*0.113,
                     ny + 29 + seed*0.193,
                     phase + 1.7,
                     complexity)

base = [noise_x, noise_y] * amplitude
scalar = ((noise_x + noise_y) * 0.5) * amplitude
radial = normalize([x - offset.x, y - offset.y])
tangent = [-radial.y, radial.x]

type 1: base
type 2: radial * scalar * 1.5
type 3/4: tangent * scalar * 1.5
type 5: [base.x * 1.45, base.y * 0.35]
type 6: [base.x * 0.35, base.y * 0.85]
type 7/8: [base.y * 0.85, base.x * 0.85]
type 9: [0, scalar]
```

Sampling:

```text
source_uv = [x + dx, y + dy]
sample_xy = round(source_uv)

if outside:
    if pinning > 0 or resize_layer:
        sample_xy = clamp_to_source_bounds(sample_xy)
    else:
        output = transparent
```

This is not yet AE math. The Round 8 signed permutation scan over type-1 cases
found only a tiny improvement from flipping signs (`~0.7%` mean-vector
improvement), so the main mismatch is the procedural noise basis/frequency, not
a simple axis swap or sign error.

### Unknowns

- AE procedural noise basis: gradient/value/simplex/fractal implementation,
  hash tables, interpolation curve, normalization, and octave weights.
- Exact `Amount` scale and whether scale is linear across all types.
- Exact `Size` frequency normalization, coordinate origin, and offset handling.
- Evolution phase units, period, cycle-evolution behavior (`0008`/`0009`), and
  how animated evolution is sampled with Posterize Time/motion blur.
- Seed mapping: coordinate offset, table reseed, phase perturbation, or a mix.
- Displacement type formulas for all 1..9 labels.
- Antialiasing/best-quality sampler (`0014`), subpixel interpolation, pixel
  center convention, and how it interacts with rendered bit depth.
- Exact pinning enum values. AE rejected `0` for `Pinning`, accepts `1`, and
  defaults to `3`; labels are still unknown.
- Resize-layer extent semantics beyond current clamped-edge approximation.

## Proposed Native Implementation Targets

1. Keep `Posterize Time` formula as `floor(time*fps+1e-9)/fps` for ordinary
   layer/source sampling. Preserve adjustment routing: lower stack at first
   Posterize bucket, downstream effects after Posterize at live comp time.
2. Add explicit Minimax support for:
   - `0001`: `1=minimum`, `2=maximum`;
   - `0003`: `1=RGB`, `2=Alpha`;
   - `0004` direction and `0005` Don't Shrink Edges before tuning radius
     kernel shape.
3. For Minimax, add impulse/ramp telemetry to choose neighborhood shape and
   edge policy before changing composed `EFF_050` behavior.
4. For Turbulent Displace, expose a Rust/native vector telemetry CLI/API for
   arbitrary sample points so AE decoded coordinate-field JSON can be compared
   without maintaining Python mirrors.
5. Fit Turbulent in this order:
   - sampler/pixel-center using `TD_SAMPLER_CHECK_*`;
   - amount scale/sign using `TD_AMOUNT_SWEEP_A010/A045/A100`;
   - size/frequency/origin using `TD_SIZE_SWEEP_*` and offset variants;
   - base type-1 noise basis;
   - complexity octave weights;
   - evolution period/phase and animated sampling;
   - seed mapping;
   - displacement type branches;
   - pinning/resize/antialiasing.
6. Keep stack-level `STK_030` as a regression only after isolated Geometry2,
   Minimax, and Turbulent vectors/params are close. It is too composed for first
   formula fitting.

## Needed Fixtures / Goldens

Posterize/time:

- Multiple Posterize Time effects in one layer stack: different FPS values and
  effect order, with temporal sidecar expectations.
- Adjustment layer with Posterize in positions 0, middle, and last, plus an
  animated downstream effect after it.
- Posterize Time + motion blur AE fixture that records/infers shutter sample
  bucket behavior, source frame id, and weight policy.

Minimax:

- AE property screenshot/UI labels for Operation, Channel, Direction, and
  Don't Shrink Edges.
- Impulse, 1D ramp, hard alpha edge, soft alpha ramp, and premult/straight
  color-alpha split sources.
- Sweep `0004` Direction values and `0005` Don't Shrink Edges off/on.
- Fractional radius sweep: `0, 0.25, 0.5, 0.75, 1, 1.5, 2, 12`, with bbox and
  row/column samples.
- 16 bpc or EXR/TIFF intermediate-style renders if possible, to separate alpha
  math from PNG premultiplication.

Turbulent Displace:

- Offset sweep for `0004`: center, zero, quarter-frame, non-integer offsets.
- 16 bpc coordinate-field renders for the existing Round 5 pack.
- Full per-sample JSON for selected cases with parity mismatches included,
  especially near modulo seams and edges.
- Evolution cycle cases for `0008` and `0009`.
- Pinning enum sweep across accepted values `1..17` with hard-edge/alpha-ramp
  source.
- Antialiasing `0014` off/on on unique-checkerboard source.
- Additional small-size and large-size cases after AE range clamp, especially
  `2,3,4,8,16,256,512,1000`.

## Current Native Debug Targets To Reuse

- `effects_debug/<case>/<frame>/*ADBE_Minimax.json`: params, input/output hash,
  effect time.
- `EFF_060/effects_debug/frame_*/...turbulent_displace.json`: resolved params,
  field hash, sampler mode, edge policy, OOB count, nine sample vectors.
- `adjustment_effects.jsonl`: per-adjustment-effect comp time, layer time,
  lower-stack time, param time, input/output hash, Posterize bucket info.
- `temporal_telemetry.jsonl`: layer/source/posterize/motion-blur checkpoints.

These sidecars are the lowest-noise comparison surface; final PNGs should be
used for confirmation after isolated math is fit.

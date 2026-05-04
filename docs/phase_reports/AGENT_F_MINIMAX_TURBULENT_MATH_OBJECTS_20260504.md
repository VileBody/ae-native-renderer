# Agent F: Minimax / Turbulent Displace Math Objects

Status date: 2026-05-04

Predecoded evidence root:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

## Scope

Assigned objects:

| Object | AE effect | Module status entering pass | This pass outcome |
| --- | --- | --- | --- |
| `M13` | `ADBE Minimax` | `implemented approximate` | CPU callback shape recovered; operation/channel/direction surface implemented in native from AEX strings plus callback shape; fractional radius and edge-shrink still need targeted probes. |
| `M14` | `ADBE Turbulent Displace` | `instrumented/testable` | Wrapper/state model substantially confirmed; exact kernel body is hidden behind loaded GPU kernels, so final formula tuning is blocked until kernel extraction or field probes. |

Initial passport pass made no code changes. This follow-up updated
`crates/effects/src/minimax.rs`, `docs/EFFECTS.md`,
`docs/MATH_PARITY_STATUS.md`, and this report only; no Turbulent formula change
was made.

## Evidence Table

| Evidence | Path / function / address | Confidence | Affected modules |
| --- | --- | --- | --- |
| Agent TZ and output contract | `docs/phase_reports/MATH_OBJECT_AGENT_TZ_20260504.md` | confirmed | `M13`, `M14`, guardrails |
| Minimax predecoded function index | `target/reverse/predecoded/20260504_222748_full_predecode_round2/minimax_aex/index.md` | confirmed | `M13` |
| Minimax 8 bpc CPU callback | `minimax_aex/02_Minimax_callback_8bpc_1800060b0/decompile.c`, `FUN_1800060b0` | confirmed for callback body | `M13`, `M19` |
| Minimax 16 bpc CPU callback | `minimax_aex/01_Minimax_callback_16bpc_180004ef0/decompile.c`, `FUN_180004ef0` | confirmed for callback body | `M13`, `M19` |
| Minimax 32 bpc CPU callback | `minimax_aex/03_Minimax_callback_32bpc_180007270/decompile.c`, `FUN_180007270` | confirmed for callback body | `M13`, `M19` |
| Minimax compare functions | `FUN_18000d710`, `FUN_18000d720`, `FUN_18000d730` | confirmed | `M13` |
| Minimax GPU path | `minimax_aex/04_Minimax_gpu_path_18000c4b0/decompile.c`, `GF::LoadKernel("AEFX_Minimax", "MinimaxTraverseSTreeKernel")` | confirmed wrapper, kernel body hidden | `M13`, `M19` |
| Minimax AE enum probe ingest | `docs/phase_reports/AE_PROBE_ROUND5_INGEST.md`; `fixtures/ae_probe_pack/.../minimax/ae_goldens/metadata/minimax_measurements.json` | confirmed by rendered AE probes | `M13` |
| Turbulent predecoded function index | `turbulent_displace_aex/index.md` | confirmed | `M14` |
| Turbulent property/state setup | `turbulent_displace_aex/02_Turbulent_param_setup_180003e70/decompile.c`, `FUN_180003e70` | confirmed for wrapper setup | `M14`, `M15` |
| Turbulent render/dispatch wrapper | `turbulent_displace_aex/06_Turbulent_render_18000ad10/decompile.c`, `FUN_18000ad10` | confirmed for dispatch topology | `M14`, `M19` |
| Turbulent zero amount checkout | `turbulent_displace_aex/07_Turbulent_amount_checkout_18000d070/decompile.c`, `FUN_18000d070` | confirmed | `M14` |
| Turbulent pixel stride helper | `turbulent_displace_aex/03_Turbulent_pixel_format_stride_180006ca0/decompile.c`, `FUN_180006ca0` | confirmed, pixel-format plumbing | `M14`, `M19` |
| Turbulent 1D lookup noise | `turbulent_displace_aex/08_Turbulent_lookup_noise_18000d230/decompile.c`, `FUN_18000d230` | confirmed for lookup helper | `M14` |
| Turbulent 64x64 table build | `turbulent_displace_aex/09_Turbulent_table_build_18000e340/decompile.c`, `FUN_18000e340` | confirmed | `M14` |
| Turbulent kernel/texture names | `turbulent_displace_aex/06_Turbulent_render_18000ad10/data_refs.tsv` refs to `TDHaxisHTexture`, `TDVaxisHTexture`, `TurbulentDisplaceFracAllKernel`, `TurbulentDisplaceFrac1DKernel` | confirmed | `M14`, hidden GPU path |
| Turbulent prior focused reverse note | `docs/phase_reports/AE_REVERSE_TURBULENT_TEXT_EXPRESSION_GHIDRA.md` | corroborating | `M14`, text/expression adjacency |
| Temporal/noise/distort summary | `docs/reverse_engineering/effect_math_temporal_noise_distort.md` | corroborating | `M13`, `M14`, `M15`, `M16` |
| Current module registry | `docs/MATH_PARITY_STATUS.md` | confirmed status source | `M13`, `M14` |

## Object Passport: M13 Minimax

### Parameter Mapping

AE property mapping from Round 5 probes:

| AE index | Match name | Role | Confidence |
| ---: | --- | --- | --- |
| `1` | `ADBE Minimax-0001` | Operation enum: `1 = minimum`, `2 = maximum`, `3 = minimum then maximum`, `4 = maximum then minimum` | `1/2` confirmed by probes; `3/4` from AEX popup label order |
| `2` | `ADBE Minimax-0002` | Radius | confirmed |
| `3` | `ADBE Minimax-0003` | Channel enum: `1 = Color`, `2 = Alpha and Color`, `3 = Red`, `4 = Green`, `5 = Blue`, `6 = Alpha` | label order recovered; `1` confirmed by probe; `2` needs RGB/alpha isolation before treating as alpha-only |
| `4` | `ADBE Minimax-0004` | Direction enum: `1 = Horizontal & Vertical`, `2 = Just Horizontal`, `3 = Just Vertical` | recovered from AEX popup label order; needs AE impulse probe |
| `5` | `ADBE Minimax-0005` | Don't Shrink Edges | known property, behavior not mapped |
| `6` | `ADBE Effect Built In Params` | Compositing Options group | confirmed property dump |

Internal callback structure:

- `param_1[0]` / `param_1[1]`: integer radius-like values used by pass orientation; callback receives an already-integer radius.
- `param_1[2]`: operation selector feeding the comparison direction.
- `param_1[3]`: channel bit mask; decompiled callbacks split four component lanes via shifted bits.
- `param_1[9]`: internal pass/orientation selector, not directly the AE UI direction enum.
- byte at `param_1 + 10`: edge sentinel / "do not shrink edges" style gate; exact AE property alignment not proven.

### Coordinate / Time / Color Space

- Coordinate space: integer pixel grid over PF source/output worlds.
- Time: no temporal sampling inside Minimax callback; animated radius/enum must already be checked out by the effect wrapper at the effect sample time. If Minimax is after Posterize Time on an adjustment layer, use the `M15/M16` temporal contract for param time.
- Color space: per-component extrema on the native component storage; no gamma/linear conversion evidence in the callback.
- Bit depth:
  - 8 bpc callback operates on byte components, pixel stride 4 bytes.
  - 16 bpc callback operates on 16-bit components, pixel stride 8 bytes.
  - 32 bpc callback operates on float components, pixel stride 16 bytes.

### Sampling Rule

Confirmed callback shape is not a naive 2D square loop. It is a sliding-window extrema pass:

```text
diameter = 2 * integer_radius + 1
for each selected component lane:
  maintain monotonic queue of candidate (position, value)
  expire samples whose position is outside radius window
  write selected extrema into destination lane
```

Evidence:

- queue capacity allocated as `2 * radius + 1` in all bpc callbacks;
- selected component lanes each get an independent queue;
- compare helpers are typed by bpc:
  - 8 bpc: `FUN_18000d710(byte*, byte*)` returns `a < b`;
  - 16 bpc: `FUN_18000d720(ushort*, ushort*)` returns low byte boolean plus high byte artifact in decompile, but call sites use the boolean result;
  - 32 bpc: `FUN_18000d730(float*, float*)` returns `a < b`.

Neighborhood shape:

- Confirmed: one-dimensional radius window per pass.
- Implemented native mapping: Direction `1` composes horizontal then vertical
  one-dimensional passes, Direction `2` runs horizontal only, and Direction `3`
  runs vertical only.
- Still needs probe: confirm that AE's UI popup order exactly matches the
  resource label order and that GPU and CPU paths agree at borders.

### Alpha / Premult Policy

- Color channel mode preserves alpha in AE Round 5 probes.
- Alpha-affecting modes change alpha and therefore change exported
  premultiplied RGB in final PNG/TIFF. That final RGB should not be used as
  proof of hidden RGB morphology.
- No direct contradiction to straight internal canvas assumptions was found in CPU callbacks, but final-frame tuning still depends on the global `M19` alpha/export contract.

### Formula / Pseudocode

AE-shaped operator for the CPU path:

```text
radius_i = checkout_and_quantize_radius(param[2])  # exact rounding still unknown
if radius_i <= 0:
  return input

lanes =
  if channel == Color:           component lanes R, G, B
  if channel == Alpha and Color: component lanes R, G, B, A
  if channel == Red/Green/Blue:  that single RGB lane
  if channel == Alpha:           component lane A

cmp =
  if operation == minimum: min comparator
  if operation == maximum: max comparator
  if operation == min then max: minimum stage then maximum stage
  if operation == max then min: maximum stage then minimum stage

for pass in direction_to_passes(direction):
  for each row/column in pass:
    for each selected component lane:
      out_lane[p] = extrema(input_lane[p - radius_i ... p + radius_i], cmp, edge_policy)
    for each unselected lane:
      out_lane[p] = input_lane[p] or previous-pass lane
```

Edge policy:

- Out-of-source rows/columns are zero-filled when the destination row maps outside source bounds.
- Within-row/column OOB behavior is gated by the edge byte; when the gate is enabled, the decompiled code keeps feeding a sentinel candidate instead of simply shortening the window.
- Exact UI behavior of `Don't Shrink Edges` is not yet proven. Name suggests edge extension / full-window behavior, while disabled mode likely shrinks/clips the window.

### Native Implementation Delta

Current native `crates/effects/src/minimax.rs` now uses AE-shaped
one-dimensional pass composition:

```text
for each operation stage:
  run horizontal and/or vertical pass according to Direction
```

Remaining delta:

- Native assumes `round(radius).clamp(0, 32)`; AE wrapper quantization is not recovered because the setup function feeding `FUN_180004ef0/60b0/7270` is outside the predecoded target set.
- Native edge behavior still shrinks/clips to the image extent, not the
  callback's sentinel/full-window policy. `0005` is parsed and reported but not
  applied.
- Channel `0003=2` is now implemented as `Alpha and Color` per AEX popup order;
  previous Round 5 white-square probes only proved that it affects alpha.

### Field-Level Comparison Checkpoints

Do not compare final pixels only. Add per-effect checkpoints:

| Checkpoint | Purpose |
| --- | --- |
| `resolved_params` | operation enum, channel enum, raw radius, integer radius, direction, edge flag, bpc |
| `active_lanes` | Color / Alpha and Color / single-channel lane selection before morphology |
| `pass_plan` | horizontal/vertical/both/unknown, source and destination extents |
| `edge_policy` | shrink-window vs sentinel/extend behavior at x/y borders |
| `pass1_hash` | after first 1D pass, before second pass if any |
| `pass2_hash` | final morphology output before composite/export |
| `lane_samples` | center row/column samples for R/G/B/A separately |

### Test / Probe Needed Next

Smallest probes:

1. Fractional radius probes: `r = 0.49, 0.50, 0.51, 1.49, 1.50, 1.51, 11.49, 11.50, 11.51`.
2. Direction enum probes over a sparse impulse plus horizontal/vertical ramps, varying `0004`.
3. `Don't Shrink Edges` probes over an impulse near each border, varying `0005 = 0/1`.
4. 16/32 bpc probe with a sub-byte/sub-float ramp to catch compare precision and final export quantization.

## Object Passport: M14 Turbulent Displace

### Parameter Mapping

PF indices confirmed from `FUN_180003e70` and wrapper calls:

| PF index | Role | Evidence / notes | Confidence |
| ---: | --- | --- | --- |
| `1` | Displacement enum | read in render/setup; UI values are internalized, e.g. values `4/8` remap down by one in some cases | confirmed index, internal mapping partially unknown |
| `2` | Amount | `FUN_18000d070(param, 2, state)`; zero amount copies source to output | confirmed |
| `3` | Size | fixed-point scalar, coordinate scale; AE rejects size `< 2` in probes | confirmed |
| `4` | Offset (Turbulence) | X/Y fixed-point point stored at state `[4]`, `[5]` | confirmed |
| `5` | Complexity | split into integer octaves and fractional remainder | confirmed |
| `6` | Evolution | fixed-point evolution, normalized by `90 * 65536` | confirmed |
| `7` | Evolution Options group | UI/group; no direct numeric checkout in setup | inferred from resource strings |
| `8` | Cycle Evolution toggle | boolean gate around cycle wrapping | confirmed as boolean index, label alignment needs one property dump check |
| `9` | Cycle / cycle-related scalar | used with cycle range when cycle is enabled | confirmed index, label ordering with 10 needs verification |
| `10` | Cycle / cycle-related scalar | read before index 9 in setup; default constants include `90 * 65536` and larger range | confirmed index, label ordering with 9 needs verification |
| `11` | UI/resource slot under Evolution Options | not directly seen as checkout in focused setup | unknown |
| `12` | Pinning enum | drives internal mode/edge flags; supports modes beyond simple UI labels | confirmed |
| `13` | Resize Layer | not clearly checked out in `FUN_180003e70`; Round 5 AE property dump and native payload mapping expose it | inferred / needs verification in setup |
| `14` | Antialiasing for Best Quality | read and defaulted when render quality is not best | confirmed index, label alignment needs verification |

Resource/string ordinals from earlier Ghidra note align as:

```text
Displacement, Amount, Size, Offset, Complexity, Evolution,
Evolution Options, Cycle Evolution, Cycle (in Revolutions),
Random Seed, Pinning, Resize Layer, Antialiasing for Best Quality
```

Important namespace warning: resource ordinals, AE match-name indices, PF checkout indices, and payload keys are not always identical. Public payload mapping should not be renamed from this report alone.

### Coordinate / Time / Color Space

- Coordinate space: output pixel grid maps to source sampling coordinates via a displacement field.
- Amount scale:
  - raw fixed16 amount uses `1 / 65536`;
  - wrapper multiplies by `0.01` and later by project/source scaling constants.
- Size scale:
  - reciprocal size-like factor stored in state `[1]`;
  - smoother modes multiply by approximately `0.35355339`.
- Evolution:
  - raw evolution is fixed16;
  - normalized by `90 * 65536 = 5,898,240`;
  - split into integer cycle and fractional phase state.
- Time:
  - no source-time sampling is recovered inside the AEX beyond param checkout;
  - animated Evolution must use the effect param time supplied by the stack. For adjustment stacks after Posterize Time, this depends on `M15/M16`.
- Color:
  - displacement math is coordinate/sample based; final pixel color depends on source sampler and output pixel format.

### Sampling Rule

Wrapper-level model:

```text
state = malloc(0x8130)
gpu_params = operator_new(0x405c)

state.amount = checkout(param[2]) * fixed16_scale
if state.amount == 0:
  copy_source_to_output()

state.displacement_mode = internalize(param[1])
state.size = checkout(param[3])
state.offset_xy = checkout(param[4])
state.complexity_int, state.complexity_frac = split(checkout(param[5]))
state.evolution_phase = split_evolution(param[6], param[8..10])
state.pin_mode = checkout(param[12])
state.aa_quality = checkout(param[14]) or best-quality default

state.noise_table_64x64 = FUN_18000e340(...)
gpu_params[0x0000..0x3fff] = float32(state.noise_table_64x64)
gpu_params[0x4000..0x4058] = float32/int scalar state
```

Dispatch topology:

```text
if internal_mode in {9, 10, 11}:
  build H lookup when mode uses horizontal axis
  build V lookup when mode uses vertical axis
  bind TDHaxisHTexture / TDVaxisHTexture resources
  dispatch TurbulentDisplaceFrac1DKernel
else:
  dispatch TurbulentDisplaceFracAllKernel
```

Confirmed lookup sizes:

- H lookup allocation uses source width plus 4 intermediate slots and state count `width + 2` in the state model.
- V lookup allocation uses source height plus 4 intermediate slots and state count `height + 2`.
- Render code packs lookup floats into GPU textures with zero padding lanes.

### Lookup Table Construction

`FUN_18000e340` builds a `64 x 64` double table later converted to float in `gpu_params`.

Confirmed constants:

| Constant | Value | Use |
| --- | ---: | --- |
| fixed16 scale | `1 / 65536` | param checkout |
| amount scalar | `0.01` | amount normalization |
| smoother factor | `0.35355339` | smoother displacement modes |
| evolution cycle base | `90 * 65536` | evolution normalization |
| octave amplitude base | `0.25` | fractal amplitude |
| octave amplitude decay | `0.7` | fractal amplitude decay |
| lookup frequency multiplier | `1.77` | `FUN_18000d230` |
| smoothstep polynomial | `(3 - 2t) * t * t` | table interpolation |
| H phase offset | `7913.17` | H 1D lookup |
| V phase offset | `9711.73` | V 1D lookup |
| cycle table range default | `49152` | cycle/noise table range |

`FUN_18000d230` samples the table fractally:

```text
sum = 0
amp = 0.25
for octave in 1..complexity_int:
  coord *= 1.77
  i = floor(coord)
  t = coord - i
  w = (3 - 2*t) * t*t
  sum += lerp(table_hash(i), table_hash(i+1), w) * amp
  amp *= 0.7

if complexity_frac > 0:
  sum += partial_octave(coord * 1.77) * amp * complexity_frac
```

The hash addressing uses table dimensions `64`, bit masks `0x3f`, and xor constant `0x440`.

### Internal Displacement Modes

Confirmed:

- Internal modes `9`, `10`, `11` dispatch `TurbulentDisplaceFrac1DKernel`.
- Mode `9` builds/binds H lookup.
- Mode `10` builds/binds V lookup.
- Mode `11` builds/binds both H and V lookup.
- All other internal modes dispatch `TurbulentDisplaceFracAllKernel`.
- UI enum values `1..9` are not guaranteed to equal internal mode values. Setup adjusts at least values in the `4/8` family by subtracting one.

Unresolved:

- Exact vector mapping for UI modes:
  - Turbulent
  - Bulge
  - Twist
  - Turbulent Smoother
  - Bulge Smoother
  - Twist Smoother
  - Vertical Displacement
  - Horizontal Displacement
  - Cross Displacement

The wrapper proves branch topology, not the final vector formula inside the kernels.

### Pinning / Resize / AA Policy

Confirmed:

- Pinning is checked out at PF index `12`.
- Internal pinning supports many modes, including axis, side, and locked variants.
- Setup writes pin flags and edge limits into scalar state around offsets `0x34..0x44`, `0x4030..0x4044` in the GPU parameter block.
- Antialiasing / quality-like control is checked out at PF index `14`; if render quality is not Best, setup forces a default quality value.
- Workgroup size varies by backend/device flags (`1x1`, `64x1`, or `16x16`), which is performance plumbing rather than math.

Needs verification:

- Exact Resize Layer checkout path for public AE index `13`.
- Which pin enum maps to each side/axis/locked behavior.
- Whether AA Low/High changes source sampler only, displacement field evaluation, or both.

### Alpha / Premult Policy

- AEX wrapper operates by sampling source pixels through GPU kernels. It does not reveal straight vs premult color math.
- AE Round 5 output module logs show premultiplied TIFF/PNG export for probe packs. Therefore final RGB at low alpha must not be used as displacement evidence.
- Turbulent formula tuning must compare decoded coordinate fields, UVs, OOB masks, and sampler results before final compositing.

### Native Implementation Delta

Current native `crates/effects/src/turbulent_displace.rs` is a deterministic approximate sine/noise field with telemetry slots inspired by prior Ghidra work.

Delta:

- Native does not implement AE's `0x405c` GPU parameter block or exact `64 x 64` table generation.
- Native rounds/clamps complexity to integer octaves for actual field generation; AE splits integer octaves and fractional remainder.
- Native mode handling is approximate; AE uses internalized modes and a hard split between `FracAll` and `Frac1D`.
- Native seed/evolution/cycle semantics are not AE-equivalent; AE uses fixed16 evolution normalization and cycle wrapping controls.
- Native edge/pinning/resize behavior is simplified; AE writes multiple pin flags and edge limits into kernel args.
- Native sampler/AA policy is not proven against `Low/High` best-quality behavior.

### Field-Level Comparison Checkpoints

Do not tune M14 from final PNG diffs. Required checkpoints:

| Checkpoint | Purpose |
| --- | --- |
| `resolved_params` | PF index values 1..14, payload keys, UI labels, render quality |
| `internal_mode` | UI displacement enum after AE internalization |
| `dispatch_path` | `FracAll` vs `Frac1D`, and H/V lookup presence |
| `fixed_state` | amount fixed16, size scale, offset, evolution fixed16, seed/cycle values |
| `complexity_split` | integer octaves and fractional remainder |
| `noise_table_hash` | hash and selected samples of the 64x64 table before float conversion |
| `h_lookup_hash` / `v_lookup_hash` | lookup length, phase offset, selected samples |
| `pin_state` | pin mode, edge limits, resize flag, AA quality |
| `uv_field` | source coordinate vector at corners, edge midpoints, center, 32 px grid, dense center patch |
| `oob_mask` | out-of-source samples before pin/resize/clamp |
| `sampler_samples` | nearest/bilinear/AA identification over checkerboard source |
| `post_sample_hash` | sampled source before composite/export |

### Extraction / Probe Plan

Do not imitate the hidden kernels from final pixels. Work in this order:

1. Wrapper/state capture:
   - log PF indices `1..14`, AE popup values, render quality, layer size,
     source/output extent, and bit depth;
   - record fixed16 amount, size, offset, evolution, random seed, cycle toggle,
     cycle revolutions/range, complexity integer/fraction, pinning, resize, and
     AA scalar exactly as the wrapper prepares them.
2. CPU/GPU branch identification:
   - record whether AE dispatched CPU iterate callbacks, GPU
     `TurbulentDisplaceFracAllKernel`, or GPU
     `TurbulentDisplaceFrac1DKernel`;
   - for `Frac1D`, record `internal_mode`, H/V lookup presence, H/V lookup
     lengths, texture names, and lookup hashes before any source sampling.
3. Coordinate normalization probes:
   - render coordinate-field sources where RGB encodes source `x/y`, with
     opaque alpha and a separate checkerboard/impulse source for sampler
     identification;
   - sweep amount `0/1/10/45/100`, size `2/8/16/32/65/128/256`,
     evolution `0/45/90/180/360/720`, seeds `0/1/2/10/999`, and complexity
     `1/1.25/1.5/1.75/2/2.75/3`.
4. Field-map outputs:
   - decode and store dense `uv_field`, `dx/dy`, `noise_or_lookup_sample`,
     `oob_mask`, `pin_clamp_mask`, and `post_sample_hash`;
   - always include corners, edge midpoints, center, a 32 px grid, and a dense
     center patch. Keep alpha and hidden RGB separate from final export RGB.
5. Kernel extraction path:
   - extract/decode the GPUFoundation kernel payloads for
     `TurbulentDisplaceFracAllKernel` and `TurbulentDisplaceFrac1DKernel`;
   - compare extracted scalar/vector formulas against field-map probes before
     changing `crates/effects/src/turbulent_displace.rs`.

BLOCKER: any mismatch in hidden kernel body, coordinate normalization,
source-sampler/OOB policy, alpha/premult handling, or CPU/GPU branch choice must
stop formula tuning and update this report before native implementation changes.

### Test / Probe Needed Next

Smallest probes:

1. Live AE property dump specifically for indices `8`, `9`, `10`, `13`, `14` with current AE version.
2. Coordinate-field decode for each displacement mode `1..9` at amount `45`, size `65`, complexity `2`, evolution `0`, seed `0`.
3. Complexity split probes: `1.0`, `1.25`, `1.5`, `1.75`, `2.0`, `2.75`, `3.0`.
4. Evolution/cycle probes:
   - evolution `0`, `45`, `90`, `180`, `360`, `720`;
   - cycle off/on;
   - cycle revolutions `1`, `2`, `4`;
   - seeds `0`, `1`, `2`, `10`, `999`.
5. Pinning/resize/AA probes over coordinate field and unique checkerboard:
   - every pin enum AE accepts;
   - Resize Layer off/on;
   - AA Low/High;
   - border band and OOB mask.

## Guardrail Findings

### BLOCKER: Turbulent Displace Kernel Body Hidden

```text
BLOCKER:
  title: Turbulent Displace final vector/sampler math is inside hidden GPU kernels
  affected objects: M14, M19 sampler/OOB policy, M15/M16 when animated evolution is stacked after Posterize Time
  evidence paths:
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/turbulent_displace_aex/06_Turbulent_render_18000ad10/decompile.c
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/turbulent_displace_aex/06_Turbulent_render_18000ad10/data_refs.tsv
  exact address/function:
    - FUN_18000ad10 at 0x18000ad10
    - GF::LoadKernel("TurbulentDisplace", "TurbulentDisplaceFracAllKernel") at decompile line with address ref 0x18000bc8e
    - GF::LoadKernel("TurbulentDisplace", "TurbulentDisplaceFrac1DKernel") at decompile line with address ref 0x18000be72
  what assumption broke: wrapper decompile alone cannot prove exact displacement vector mapping, source sampler, AA, or OOB behavior
  hypotheses:
    - kernel assets are embedded/loaded through GPUFoundation-compatible resources and need extraction/decode
    - coordinate-field probes can constrain kernel behavior enough for a CPU reimplementation without full kernel decompile
  smallest probe/test needed:
    - run coordinate-field decode for modes 1..9 plus sampler/AA/pinning cases and compare uv_field/noise_table/lookup hashes, not final pixels
  can continue on unrelated work: yes
```

### Minimax GPU Path Caveat

Minimax also has a GPU path:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2/minimax_aex/04_Minimax_gpu_path_18000c4b0/decompile.c
FUN_18000c4b0 at 0x18000c4b0
GF::LoadKernel("AEFX_Minimax", "MinimaxTraverseSTreeKernel")
```

This is not a full blocker for the CPU-shaped passport because 8/16/32 bpc callbacks are present and decompiled. It is a tuning caveat: AE goldens may use the GPU path depending on project/device/render settings, so conformance should record whether AE used CPU or GPU if possible. If GPU/CPU diverge at edges or Direction modes, escalate before formula changes.

### Minimax Radius Rounding / Direction / Edge Setup Gap

The callback receives integer radius/pass state. The setup function referenced as caller `FUN_18000c2a0` was not included in the predecoded target set:

```text
minimax_aex/01_Minimax_callback_16bpc_180004ef0/callers.tsv
minimax_aex/02_Minimax_callback_8bpc_1800060b0/callers.tsv
minimax_aex/03_Minimax_callback_32bpc_180007270/callers.tsv
```

This leaves exact AE radius quantization and the UI edge/sentinel mapping
unresolved. Direction `1/2/3` is implemented from popup label order, but still
needs an AE impulse/ramp probe before using it as a final-pixel tuning anchor.

### Temporal Dependency

Turbulent Evolution and Minimax Radius can be animated. In adjustment stacks, formula tuning must use the `M15/M16` temporal contract:

- lower-stack source time may be posterized;
- downstream effect params after Posterize Time may still evaluate at comp time;
- field checkpoints must include effect param time for each frame.

### Alpha / Premult Dependency

Both objects should avoid final-RGB-only tuning:

- Minimax alpha-channel morphology changes exported premultiplied RGB through alpha, not necessarily through RGB extrema.
- Turbulent coordinate-field decoding fails when alpha/export premultiplication hides source RGB.

Use `M19` substrate conclusions for final thresholds; use internal field/lane checkpoints for formula changes.

## Recommendation

| Object | Recommendation | Reason |
| --- | --- | --- |
| `M13 Minimax` | `needs_probe` | Operation/channel/direction native surface now follows AEX evidence, but fractional radius quantization, edge-shrink policy, and GPU/CPU path parity still need probes before broad formula tuning. |
| `M14 Turbulent Displace` | `blocked_by_guardrail` | Param/state/dispatch model is ready, but exact vector/sampler math lives in hidden `FracAll`/`Frac1D` GPU kernels. Continue with field probes or kernel extraction before formula tuning. |

Implementation order after orchestrator accepts this passport:

1. Add/verify telemetry checkpoints listed above.
2. For Minimax, implement pass-plan/direction and edge-policy probes before changing neighborhood math.
3. For Turbulent, implement AE-shaped state/table/lookup telemetry first, then fit field vectors from coordinate goldens.
4. Only after `M19` alpha/export and `M15/M16` temporal routing are stable should final-pixel thresholds be used as acceptance criteria.

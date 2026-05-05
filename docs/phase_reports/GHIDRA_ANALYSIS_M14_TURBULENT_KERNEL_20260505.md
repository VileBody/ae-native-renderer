# Ghidra Analysis M14 Turbulent Kernel 20260505

## Status

Reviewed the fresh `turbulent_displace_aex` predecode bundle only. No implementation
code was changed.

Ghidra is enough to confirm the Turbulent Displace control flow, table/lookup
setup, and GPU dispatch split. It is not enough to implement the final AE field
formula safely: the actual `TurbulentDisplaceFracAllKernel` and
`TurbulentDisplaceFrac1DKernel` bodies remain opaque behind GPUFoundation kernel
loads.

## Inputs

- Bundle: `target/reverse/predecoded/20260505_153013_blocker_modules_round2/turbulent_displace_aex`
- Key functions:
  - `01_Turbulent_FilterMain_180001f30`
  - `02_Turbulent_param_setup_180003e70`
  - `03_Turbulent_pixel_format_stride_180006ca0`
  - `04_Turbulent_dispatch_all_180006e40`
  - `05_Turbulent_dispatch_1d_180007040`
  - `06_Turbulent_render_18000ad10`
  - `07_Turbulent_amount_checkout_18000d070`
  - `08_Turbulent_lookup_noise_18000d230`
  - `09_Turbulent_table_build_18000e340`
- Prior context checked for property order/probe state:
  - `docs/phase_reports/AE_REVERSE_TURBULENT_TEXT_EXPRESSION_GHIDRA.md`
  - `docs/phase_reports/AGENT_ROUND5_TURBULENT_FIELD_PROBE_PACK.md`
  - `docs/phase_reports/TURBULENT_CONTROLS_PASS.md`

## Findings

### Lookup table construction / noise kernel hints

- `FUN_18000e340` builds a `64 x 64` double table, later packed as float into
  the first `0x4000` bytes of a `0x405c` GPU parameter block.
- Table seeds use an LCG shape: initial `0xbc8cb5`, multiplier `0x41c64e6d`,
  increment `0x3039`, with high 15 bits scaled by `1 / 16384` then shifted into
  roughly `[-1, 1]`.
- Evolution/cycle state seeds additional neighboring LCG streams from
  `(evolution + k * 90 * 65536) mod period`, then blends through a helper
  (`FUN_18000d120`). This strongly suggests table-driven temporal interpolation,
  not a simple inline Perlin implementation.
- `FUN_18000d230` is a 1D fBm-style table lookup helper:
  - base amplitude `0.25`;
  - amplitude decay `0.7`;
  - frequency multiplier `1.77`;
  - cubic smoothstep weight `(3 - 2t) * t * t`;
  - table address wraps through `floor(coord) & 0x3f` plus a hashed second axis
    `((floor(coord) ^ 0x440) >> 6) & 0x3f`;
  - integer complexity plus fractional remainder are both handled.

### Amount / size / complexity / evolution / time flow

- Param `0002` Amount is read through `FUN_18000d070`, converted from fixed
  `16.16` by multiplying by `1 / 65536`. Render short-circuits to copy/pass
  through when the resolved amount is `0.0`.
- Param `0003` Size is also fixed `16.16`. Setup uses it in both amplitude and
  coordinate scale:
  - amount is first scaled by `size * 0.01`;
  - coordinate scale is derived from `(0.5 / size)` with a pixel-aspect/render
    scale divisor;
  - internal modes `1..3` additionally multiply coordinate scale by
    `0.35355339`.
- Param `0005` Complexity is split into integer octave count and fractional
  remainder. The fractional remainder feeds the packed params and the lookup
  helper; it is not just rounded away.
- Param `0006` Evolution is normalized by `90 * 65536 = 5898240`, split into
  whole-cycle and fractional parts, then passed into table construction.
- Cycle-related params `0009`/`0010` and the `0008` cycle boolean affect the
  table period/seed path. When cycling is enabled and the cycle count is
  positive, setup replaces the default period with `cycle_count * 90 * 65536 * 4`.
- Param `0004` Offset (Turbulence) is read as fixed-point x/y and stored into
  the packed block; it participates in 1D lookup coordinate generation.

### All-vs-1D dispatch and coordinate field scaling

- `FUN_18000ad10` loads two GPU kernels by name:
  - `TurbulentDisplaceFracAllKernel`
  - `TurbulentDisplaceFrac1DKernel`
- Internal displacement modes `9`, `10`, and `11` dispatch through
  `Frac1D`; other modes dispatch through `FracAll`.
- Modes `9` and `11` build/bind a horizontal `width + 2` float lookup. Modes
  `10` and `11` build/bind a vertical `height + 2` float lookup.
- The 1D lookup coordinate uses source/destination origin offsets, fixed-point
  turbulence offset, the derived coordinate scale, and axis phase constants:
  - horizontal phase approx `7913.17`;
  - vertical phase approx `9711.73`.
- `FracAll` uses a smaller GPU arg wrapper (`0x30`); `Frac1D` uses a larger one
  (`0x50`) because it passes extra axis lookup resources.
- Pixel-format stride is resolved by `FUN_180006ca0` from GPU pixel format flags;
  dispatch also computes backend-dependent thread group dimensions (`1`, `16`,
  or `64`-wide shapes depending on device/format flags).

### OOB / sampler policy hints

- Setup computes displacement reach/bounds from amount, complexity amplitude
  sum, and image dimensions:
  - x/y reach are capped to half image dimensions;
  - inverse reach and max legal coordinates are packed into params.
- Pinning/edge-style params produce packed flags and boundary values, but the
  exact sampler behavior lives in the GPU kernels, not in the visible CPU code.
- Render passes source/destination origins, dimensions, row byte strides, pixel
  format, and bound textures into GPUFoundation. The decompile does not expose
  whether source sampling is clamp, transparent, repeat, or mixed by mode.
- Prior probe evidence says default/pinned outputs can preserve alpha near
  edges, so a transparent-OOB assumption should stay rejected unless a new probe
  proves it for a specific mode.

## Accepted/Rejected/Unknown

- Accepted: property/control order from prior probes remains consistent with
  this decompile: Displacement, Amount, Size, Offset, Complexity, Evolution,
  cycle/seed controls, Pinning, Resize Layer.
- Accepted: table-driven noise state is real and should be represented as a
  packed `64 x 64` table if/when the field implementation is updated.
- Accepted: all-vs-1D dispatch split is mode-based: internal `9..11` use 1D
  axis lookups; other displacement modes use the all-field kernel.
- Rejected: implementing a guessed Perlin/simple-noise candidate from this
  Ghidra pass. The visible lookup helper is only part of the pipeline.
- Rejected: assuming transparent OOB globally. Ghidra does not prove it, and
  prior AE probe notes point away from that as a default.
- Unknown: exact `FracAll` vector basis per displacement type.
- Unknown: exact sign convention from displacement vector to source coordinate.
- Unknown: exact GPU sampler address/filter policy.
- Unknown: exact pinning/resize-layer interaction at image edges.

## Missing Probes

- Decode AE vector fields for the existing `TD_AMOUNT_SWEEP`, `TD_SIZE_SWEEP`,
  `TD_COMPLEXITY_SWEEP`, and `TD_EVOLUTION_*` cases.
- Add focused internal-mode probes for Horizontal, Vertical, and Cross
  displacement to validate modes `9`, `10`, and `11` against the 1D path.
- Run sampler/OOB probes over coordinate-field and unique-checkerboard sources:
  subpixel amount, border band, alpha ramp, and pinned/unpinned variants.
- Compare two or more `Random Seed` / cycle-evolution cases to verify the table
  period and seed path before porting table construction.

## Next Implementation Candidate

Proceed only with a conservative scaffold, not a final formula:

1. Preserve existing public control parsing.
2. Add an internal representation for the AE packed params/table shape
   (`64 x 64` table, integer/fractional complexity, evolution cycle state, and
   mode dispatch flags) behind tests or telemetry.
3. Use AE vector-field probe results to fit sign, vector basis, sampler policy,
   and OOB behavior before changing rendered pixels.

Formula implementation should wait for AE vector-field probe evidence or a
separate extraction of the GPU kernel bodies.

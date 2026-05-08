# P1 M14 Turbulent Pass

Date: 2026-05-08

## Scope

Advance M14 Turbulent Displace with a broad but evidence-backed branch selection,
without metric-only fitting.

Owned changes:

- `crates/effects/src/turbulent_displace.rs`

Reference inputs reviewed:

- `docs/phase_reports/P1_STK030_STAGE_GATE_20260507.md`
- `docs/phase_reports/P1_TURBULENT_BUCKET_WORLD_20260507.md`
- `docs/phase_reports/GHIDRA_ANALYSIS_M14_TURBULENT_KERNEL_20260505.md`
- `docs/phase_reports/M14_RUST_VECTOR_EXPORT_SINE_FIT_V1_20260506.md`
- `docs/EFFECTS.md`
- current `crates/effects/src/turbulent_displace.rs`

## Branch Decision

Selected branch: `Frac1D lookup`.

Evidence:

- Ghidra/static evidence in
  `target/reverse/predecoded/20260507_010219_turbulent_field_to_uv_20260507`
  shows public displacement types routed through internal modes `9`, `10`, and
  `11` use the `Frac1D` kernel path.
- `Turbulent_runtime_cmd11_prepare_3410_180003410` builds horizontal and
  vertical lookup buffers with `width + 2` / `height + 2` active entries,
  coordinate biases `7913.17` and `9711.73`, coordinate scale from size, and
  `FUN_18000d230`.
- `Turbulent_lookup_noise_18000d230` gives the 1D lookup formula: amplitude
  `0.25`, decay `0.7`, frequency multiplier `1.77`, smoothstep interpolation,
  table index `(ix & 0x3f, ((ix ^ 0x440) >> 6) & 0x3f)`, integer complexity,
  and fractional complexity.
- `Turbulent_pixel_kernel_frac1d_core_5330_180005330` uses H lookup for
  internal modes `9/11`, V lookup for `10/11`, and applies H to Y displacement
  and V to X displacement.
- Round 5 dynamic probes include displacement type `1..9`, and the current
  telemetry already reports the `Frac1D` split and lookup lengths.

Branches not implemented in this pass:

- Antialiasing footprint: static code exposes footprint sizing branches, but
  the exact sampler footprint contract still needs a focused native/AE probe.
- Resize-layer edge helpers: probes show `0013` changes edge/output behavior,
  but helper semantics are not yet isolated enough to alter pixels safely.
- Cycle evolution: setup/table evidence exists and native records the controls,
  but non-default-cycle parity still needs dynamic validation.
- Nonzero seed: seed is wired into the table path and Round 5 proves it changes
  AE output, but no new seed formula change was needed here.

## Implementation

Native Turbulent now builds recovered `Frac1D` axis buffers for public types
`7`, `8`, and `9`:

- type `7` / internal `9`: horizontal lookup, Y displacement only;
- type `8` / internal `10`: vertical lookup, X displacement only;
- type `9` / internal `11`: both lookups, cross-axis displacement.

The all-field `FracAll` path used by `EFF_060` and `STK_030` remains unchanged.
Pinning is applied to the recovered Frac1D noise lanes before amount scaling.

Added focused unit coverage:

- `frac1d_modes_use_recovered_axis_lookup_buffers`

## Probe Plan For Remaining Branches

Minimal next probes before further pixel changes:

1. Antialiasing footprint: render type `9` on coordinate/checker sources with
   Best Quality `0014` on/off, capture pixel-core callback footprint args, and
   compare edge/subpixel displacement spread.
2. Resize-layer edge helpers: render `TD_RESIZE_LAYER_OFF/ON` with transparent
   border bands, trace `FUN_180007840`/edge helper inputs, and record source UV
   plus sampled RGBA at each border.
3. Cycle evolution: render `cycle_evolution=true` with cycle revolutions
   `1`, `2`, and fractional values over `0/90/180/360/720` evolution, then dump
   table prefix hashes from the setup block.
4. Nonzero seed: dump table prefixes for seeds `0`, `1`, `2`, `10`, `999` and
   compare against Round 5 decoded coordinate vectors before changing the seed
   table formula further.

## Verification

Rust:

```text
cargo fmt -p effects
cargo test -p effects turbulent -- --nocapture
```

Result: `16 passed`.

Focused native gate:

```text
target/ae_agents/p1_m14_turbulent_frac1d_pass_20260508
```

Primary metric is `rgb_straight_source_over_ae_background.mean_abs_diff`.

| Case | Primary mean |
| --- | ---: |
| `EFF_060` | `0.166828` |
| `STK_030` | `0.332818` |
| `STK_030_S00_BASE` | `0.000000` |
| `STK_030_S01_GEOMETRY2` | `0.310652` |
| `STK_030_S02_POSTERIZE` | `0.310652` |
| `STK_030_S03_MINIMAX` | `0.310909` |
| `STK_030_S04_TURBULENT` | `0.317022` |

Selected frame means:

| Case | Frame 0 | Frame 1 | Frame 59 |
| --- | ---: | ---: | ---: |
| `STK_030` | `0.338142` | `0.322614` | `0.319851` |
| `STK_030_S04_TURBULENT` | `0.319536` | `0.319322` | `0.312208` |

Master gate:

```text
target/ae_agents/p1_m14_turbulent_frac1d_master_gate_20260508
```

```text
case_count=19
accepted=2
approximate=16
tuning=1
regression_count=0
missing_count=0
```

Formatting note: `cargo fmt -p effects --check` passed immediately after the
M14 edit/format pass. A later final check was blocked by concurrent unrelated
`crates/effects/src/glow.rs` formatting drift, which this pass did not edit or
revert.

## Status

P1 remains not blocked by the old Turbulent frame-spike. The selected Frac1D
branch advances M14 coverage for displacement types `7..9` and did not change
the FracAll P1 metrics. Remaining Turbulent work is branch coverage/parity for
AA footprint, resize-layer edge behavior, cycle evolution, and seed validation,
not a current P1 blocker for `STK_030`.

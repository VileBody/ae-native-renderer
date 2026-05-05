# M14 Turbulent Displace Hypothesis Results

Date: 2026-05-05

Scope: M14 `ADBE Turbulent Displace` only. No M10/M11/M12/M13 files were
edited. No global sampler or M12 Geometry2 code was changed.

## Inputs Read

- `docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md`, M14 section.
- `docs/reverse_engineering/effect_math_temporal_noise_distort.md`, Turbulent
  Displace and STK_030 routing notes.
- `crates/effects/src/turbulent_displace.rs`, current native implementation and
  unit tests.
- Existing sidecar emitters in `crates/render-cli/src/conformance_pack.rs` and
  `crates/render-core/src/layer_eval.rs` were inspected as read-only context.

## Finite Hypotheses

H1. Field generator path.

- Candidate observed: `native_sine_turbulence_approximation`.
- Evidence: current field sidecars label this model explicitly; reverse notes
  record Round 8 vector mismatch and AE center vectors that do not match a
  simple sign or axis permutation.
- Result: rejected as an AE-parity field generator, retained only as an
  instrumented baseline. Needs new AE field-map or kernel-derived discriminator
  before a replacement generator can be accepted.

H2. Evolution/time mapping.

- Candidate observed: evolution degrees sampled at effect time, converted to
  radians, then offset by seed phase.
- Evidence: EFF_060 sidecars show evolution `0,45,90,135` at frames
  `0,15,30,45`; STK_030 sidecars show downstream Turbulent evolution remains
  live after Posterize Time, including frame 1 at `3.000000238` degrees.
- Result: accepted for native telemetry/time routing. AE phase-period semantics
  still need a new evolution/cycle probe.

H3. Complexity/octaves.

- Candidate observed: rounded `0005`, clamped `1..6`; wrapper telemetry splits
  `complexity_octaves` and `complexity_fraction`; current field only consumes
  integer octaves.
- Evidence: EFF_060 and STK_030 both report `complexity_octaves=2` and
  `complexity_fraction=0.0`.
- Result: accepted as instrumentation, not accepted as AE octave weighting.
  Fractional complexity and octave amplitude/frequency need a discriminator.

H4. H/V lookup and interpolation.

- Candidate observed: internal displacement mode maps types `7/8/9` to
  Frac1D-style modes `9/10/11`; type 1 uses FracAll and no H/V lookup.
- Evidence: EFF_060 and STK_030 type 1 sidecars report
  `TurbulentDisplaceFracAllKernel`, `h_lookup_len=0`, `v_lookup_len=0`; existing
  Rust tests cover type 9 as Frac1D with `width+2` and `height+2` lookup sizes.
- Result: dispatch/lookup shape instrumentation accepted. Interpolation policy
  remains needs_new_probe.

H5. Coordinate-field displacement vs final-pixel tuning.

- Candidate observed: native displacement is `source_uv = output_xy +
  displacement`, sampled with `nearest_round`, with clamped edges when pinning
  is enabled.
- Evidence: sidecars include full-frame field hash, sample grid,
  `source_uv`, `sample_xy`, OOB counts, and guardrail
  `do_not_tune_from_final_png_only`.
- Result: accepted guardrail. Final PNG tuning is rejected for M14. STK_030 is
  only a stack regression until AE field/checkpoint data can separate M12/M13,
  M14 field, sampler, and composite error.

## Hypothesis-Pack Commands

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M14 \
  --candidate current_native_sine_field_eff060 \
  --status instrumented \
  --gate isolated \
  --question "Does the current Turbulent Displace field telemetry distinguish AE field-generator/evolution/octave/lookup hypotheses before final pixel tuning?" \
  --hypothesis "Current native sine turbulence is only an instrumented baseline; EFF_060 sidecars expose field hash/sample grid but do not accept the field generator." \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md \
  --evidence docs/reverse_engineering/effect_math_temporal_noise_distort.md \
  --note "field-first guardrail: do_not_tune_from_final_png_only" \
  --case EFF_060 \
  --out target/ae_agents/m14_eff060_current_field
```

Result: `ok=true`, report
`target/ae_agents/m14_eff060_current_field/hypothesis_report.json`.

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M14 \
  --candidate current_native_sine_field_stk030 \
  --status needs-new-probe \
  --gate stack \
  --question "Can STK_030 separate M14 Turbulent field error from Geometry2, Minimax, Posterize, adjustment routing, and sampler/composite errors?" \
  --hypothesis "STK_030 is a composed regression only; current trace can locate the Turbulent stage but cannot accept/reject the field generator without an AE field-map/checkpoint probe." \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md \
  --evidence docs/reverse_engineering/effect_math_temporal_noise_distort.md \
  --note "Do not tune M14 from STK_030 final PNG; require field/checkpoint discriminator." \
  --case STK_030 \
  --out target/ae_agents/m14_stk030_current_stack
```

Result: `ok=true`, report
`target/ae_agents/m14_stk030_current_stack/hypothesis_report.json`.

## Field Hash And Sample Grid Summary

EFF_060 isolated field sidecars:

| frame | time | evolution | field hash | OOB | kernel | center displacement | center source uv | center sample |
| ---: | ---: | ---: | --- | ---: | --- | --- | --- | --- |
| 0 | 0.0 | 0.0 | `4964183d7a7d1118` | 5233 | FracAll | `[10.1461,-0.6295]` | `[266.1461,255.3705]` | `[266,255]` |
| 15 | 0.5 | 45.0 | `7a1e66fbb0bf25ed` | 5154 | FracAll | `[3.7621,-8.1699]` | `[259.7621,247.8301]` | `[260,248]` |
| 30 | 1.0 | 90.0 | `2aedd879810580ac` | 5183 | FracAll | `[-4.8256,-10.9256]` | `[251.1744,245.0744]` | `[251,245]` |
| 45 | 1.5 | 135.0 | `5f8042714ddfae18` | 5117 | FracAll | `[-10.5865,-7.2809]` | `[245.4135,248.7191]` | `[245,249]` |

EFF_060 final PNG metrics are recorded only as confirmation, not formula
tuning evidence: case mean abs diff `2.7991786003`, max `250`,
RGB-over-AE-background mean `2.8989289602`.

STK_030 Turbulent adjustment trace sidecars:

| frame | effect time | evolution | field hash | OOB | center displacement | center sample |
| ---: | ---: | ---: | --- | ---: | --- | --- |
| 0 | 0.0 | 0.0 | `ea5d5a6dd577a54c` | 2785 | `[-2.1145,1.1035]` | `[254,257]` |
| 1 | 0.0333333333 | 3.000000238 | `99a40db649632c08` | 2826 | `[-2.0508,1.2548]` | `[254,257]` |
| 5 | 0.1666666667 | 15.0 | `0f865563e11a3bd1` | 2795 | `[-1.7421,1.8191]` | `[254,258]` |
| 10 | 0.3333333333 | 30.0 | `ea88a895c0b73b9a` | 2768 | `[-1.2508,2.4098]` | `[255,258]` |
| 15 | 0.5 | 45.0 | `3d2f0ef4ecef5c9d` | 2731 | `[-0.6745,2.8364]` | `[255,259]` |
| 20 | 0.6666666667 | 60.0 | `5d4b1bc326edf8d5` | 2717 | `[-0.0522,3.0704]` | `[256,259]` |
| 30 | 1.0 | 90.0 | `b0750897d89dcb3c` | 2802 | `[1.1606,2.9071]` | `[257,259]` |
| 45 | 1.5 | 135.0 | `1a09bf8fd4d70622` | 2727 | `[2.3159,1.2754]` | `[258,257]` |
| 59 | 1.9666666667 | 177.0 | `28e127a46e98cd1a` | 2785 | `[2.1723,-0.9507]` | `[258,255]` |

STK_030 final PNG metrics are stack-regression-only: case mean abs diff
`59.2162659963`, max `255`, RGB-over-AE-background mean `40.0527429934`.
The temporal contract passed, but this does not isolate Turbulent field error.

## Accepted / Rejected / Needs New Probe

Accepted:

- Native sidecars expose field-first M14 telemetry: field hash, sample grid,
  resolved params, fixed16 wrapper slots, FracAll/Frac1D dispatch labels,
  lookup dimensions, sampler mode, edge policy, and OOB counts.
- EFF_060 isolated telemetry is usable as the current native baseline.
- STK_030 trace localizes the Turbulent stage after Geometry2, Posterize Time,
  and Minimax, with downstream Turbulent params sampled at live comp time.

Rejected:

- Current `native_sine_turbulence_approximation` as an AE-parity field
  generator.
- Any M14 tuning from final PNG metrics, including STK_030 final pixels.
- A new local candidate switch in this pass. Existing telemetry is enough for
  baseline measurement; adding candidate paths before AE field-map goldens would
  create open-ended formula search rather than a finite discriminator.

Needs new probe:

- AE field-map/checkpoint goldens for Turbulent displacement vectors before
  sampling.
- Evolution cycle probes for `0008` and `0009`.
- Fractional complexity and octave-weight probes.
- H/V Frac1D interpolation probes for displacement types `7`, `8`, and `9`.
- Pinning/resize/antialiasing probes on an alpha-ramp or hard RGB edge source.

## Next Discriminator

Build or import an AE coordinate-field probe that writes per-sample vector JSON
for the same points emitted by native sidecars, plus at least one dense grid or
field-map hash. The first discriminator should compare type 1, amount 45,
size 65, complexity 2, seed 0, offset center, evolution frames
`0/45/90/135`, before final PNG sampling. Once that matches or names a
specific formula family, repeat for type `9` to force the Frac1D H/V lookup
path.

If AE cannot export direct field maps, the blocker is
`ORCHESTRATOR_BLOCKER: M14 requires AE coordinate-field vector goldens or kernel
extraction before field-generator acceptance`.

## Verification Commands

```bash
cargo fmt --all
cargo test -p effects turbulent
cargo test -p render-cli conformance_pack
```

Results:

- `cargo fmt --all`: pass.
- `cargo test -p effects turbulent`: pass, 11 tests passed. The first run
  exposed stale EFF_060 regression `field_hash` constants; only those M14-local
  constants were updated to match the fresh sidecar baseline above.
- `cargo test -p render-cli conformance_pack`: pass, 15 tests passed.

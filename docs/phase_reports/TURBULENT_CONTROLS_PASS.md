# Turbulent Controls Pass

This pass used the Round 5 AE probe evidence to move `M14 / ADBE Turbulent
Displace` from "amount/size/evolution only" toward a real AE control surface.

## Implemented

`crates/effects/src/turbulent_displace.rs` now parses and resolves:

| AE param | Native field | Notes |
| --- | --- | --- |
| `0001` | `displacement_type` | values clamped to `1..9`; non-1 variants now produce distinct vector bases |
| `0002` | `amount` | unchanged amplitude scale for default type |
| `0003` | `size` | clamped to AE-observed minimum `2` |
| `0004` | `offset` | array/string point support, used as turbulence coordinate origin |
| `0005` | `complexity` | unchanged octave count |
| `0006` | `evolution` | still supports existing keyframes/expression subset |
| `0010` | `random_seed` | deterministic phase/coordinate seed input |
| `0012` | `pinning` | default `3`, clamped edge sampling when enabled |
| `0013` | `resize_layer` | clamped edge sampling path when enabled |

Telemetry now emits the new resolved controls through the EFF_060 sidecar:

`target/ae_probe_ingest/turbulent_controls_conformance/EFF_060/effects_debug/frame_00000/EFF_060_field_turbulent_00_turbulent_displace.json`

## Probe Evidence Used

Round 5 AE logs exposed the Turbulent Displace property order:

```text
0001 Displacement
0002 Amount
0003 Size
0004 Offset (Turbulence)
0005 Complexity
0006 Evolution
0010 Random Seed
0012 Pinning
0013 Resize Layer
```

The returned AE frames showed:

- `amount=0` is pass-through.
- seed variants materially change the coordinate field.
- displacement type variants materially change the vector basis.
- default/pinned coordinate-field outputs keep full alpha near edges, so native
  transparent-out-of-bounds was too harsh for AE-like default behavior.
- AE rejected size `1`, so native size now clamps at `2`.

## Conformance

Command output:

`target/ae_probe_ingest/turbulent_controls_conformance/report.json`

| Case | Before RGB mean | After RGB mean | Before RGB RMSE | After RGB RMSE | Before BG-alpha mean | After BG-alpha mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `EFF_060` | `2.8989` | `2.8989` | `16.4709` | `16.4709` | `2.8006` | `2.8006` |
| `STK_030` | `3.7145` | `3.6582` | `21.1593` | `21.1522` | `17.8368` | `3.8174` |

Interpretation:

- isolated field formula did not move yet, which is expected because this pass
  did not tune AE noise math;
- composed stack alpha/composite behavior improved substantially because
  pinning/edge handling no longer drops out-of-bounds samples to transparent.

## Verification

- `cargo test -p effects -- --nocapture`: 39 passed.
- `cargo fmt -p effects -p render-cli -- --check`: passed.
- `render-cli conformance-pack --case EFF_060 --case STK_030`: ok.

## Next

The next Turbulent step is formula tuning against decoded coordinate-field
vectors:

1. fit sampler policy from `TD_SAMPLER_CHECK_*`;
2. fit amplitude/sign against `TD_AMOUNT_SWEEP_*`;
3. fit frequency/origin against `TD_SIZE_SWEEP_*` and `0004` offset probes;
4. fit octave/evolution/seed behavior separately;
5. only then retest `STK_030` as a composed regression.

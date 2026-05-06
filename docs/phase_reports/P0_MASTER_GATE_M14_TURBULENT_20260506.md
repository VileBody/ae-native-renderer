# P0 Master Gate + M14 Turbulent Pass

Date: 2026-05-06.

## What Changed

P0 now has a single master conformance gate:

```text
fixtures/ae_conformance_pack/master_gate_policy.json
scripts/run_master_conformance_gate.py
docs/MASTER_CONFORMANCE_GATE.md
```

The gate selects key isolated, composed stack, and template-frame cases and
classifies them with per-case thresholds against the primary M19-visible metric:

```text
rgb_straight_source_over_ae_background
```

M14 now records the missing Turbulent Displace AE control slots in resolved and
debug telemetry:

```text
0008 cycle_evolution
0009 cycle_revolutions
0010 random_seed
0014 antialiasing_best_quality
```

These controls are recorded but do not yet alter the native sine approximation.
That keeps the current formula stable while making the next AE-shaped field
experiment observable.

## Gate Result

Run:

```text
target/ae_agents/p0_master_gate_m14_20260506/dashboard.md
```

Summary:

```text
case_count=19
accepted=1
approximate=18
regression=0
missing=0
```

Template dashboards:

```text
template_4th approximate
impulse_2nd  approximate
scenes_3rd   approximate
```

Important cases:

| Case | Status | Primary |
| --- | --- | ---: |
| `TMP_020` | accepted | `0.000000` |
| `EFF_041` | approximate | `0.043613` |
| `EFF_050` | approximate | `0.073972` |
| `EFF_060` | approximate | `2.898929` |
| `STK_030` | approximate | `3.741755` |

## M14 Evidence

Focused run:

```text
target/ae_agents/m14_property_telemetry_20260506/report.json
```

The warps/fields evidence check passed:

```text
python3 scripts/check_warps_fields_evidence.py \
  --out target/ae_agents/m14_property_telemetry_20260506
```

The new EFF_060 sidecar confirms the property telemetry is emitted under
`trace.resolved`, `trace.ae_wrapper`, and `trace.field_state`:

```text
target/ae_agents/m14_property_telemetry_20260506/effects_debug/EFF_060/0/EFF_060_field_turbulent_0_ADBE_Turbulent_Displace.json
```

Current metrics after this instrumentation-only pass:

| Case | Primary | Raw RGBA | Max |
| --- | ---: | ---: | ---: |
| `EFF_040` | `0.054426` | `0.100533` | `250` |
| `EFF_060` | `2.898929` | `2.799179` | `250` |
| `STK_030` | `3.741755` | `7.201522` | `255` |

## Turbulent Vector Baseline

Fresh round5 vector measurement and native comparison:

```text
target/ae_agents/m14_turbulent_vectors_20260506/turbulent_vector_measurements.json
target/ae_agents/m14_turbulent_vectors_20260506/turbulent_native_comparison.json
target/ae_agents/m14_turbulent_vectors_20260506/turbulent_native_comparison.csv
```

Summary:

```text
case_frames=102
samples=253549
mean_abs_dx=8.447227
mean_abs_dy=8.226792
mean_abs_magnitude=7.453155
mean_vector_error=13.700092
max_p95_vector_error=75.663730
```

The signed flip/swap scan only improves type-1 basis error by about `0.7%`.
That means amplitude scaling is not the first-order blocker. The next real M14
work is the noise basis/origin/field state, then amount/size, then branch and
temporal controls.

## Next M14 Order

1. Replace the Python mirror with a Rust field-sample export API or CLI so the
   vector comparison cannot drift from native code.
2. Fit the type-1 noise basis/origin against coordinate-field vectors.
3. Fit amount on `A010/A045/A100` after the basis is closer.
4. Fit size/frequency, excluding `S001` if AE rejects/clamps it specially.
5. Fit displacement branch formulas, keeping type 9 separate.
6. Fit seed phase/coordinate offsets.
7. Fit complexity octave/fraction behavior.
8. Fit evolution/cycle behavior and only then revisit resize/pinning edges.

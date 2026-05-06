# M14 Rust Vector Export + Sine Fit v1

Date: 2026-05-06.

## Scope

This pass moved Turbulent Displace from telemetry-only comparison into actual
formula tuning. The exact AE GPU kernels are still not recovered, so this is not
parity locked. The goal was to make the native Rust implementation measurable at
arbitrary AE probe points, then fit the current sine/noise approximation against
decoded AE coordinate-field vectors.

## Code Changes

- `render-cli turbulent-samples` accepts a JSON request and exports native
  Turbulent Displace field samples from Rust.
- `fixtures/ae_probe_pack/turbulent_field/scripts/export_turbulent_native_samples.py`
  builds those requests from Round 5 AE vector measurements.
- `compare_turbulent_native_vectors.py` now prefers Rust-exported samples and
  keeps the Python mirror only as fallback.
- Missing AE `0004` Offset (Turbulence) now resolves to the input center during
  rendering/telemetry, matching AE default dumps.
- The active native model is `native_sine_turbulence_fit_v1`.

## Artifacts

```text
target/ae_agents/m14_sine_fit_v1_vectors_20260506/native_samples.json
target/ae_agents/m14_sine_fit_v1_vectors_20260506/turbulent_native_comparison_rust.json
target/ae_agents/m14_sine_fit_v1_20260506/report.json
target/ae_agents/m14_sine_fit_v1_gate_20260506/dashboard.json
target/ae_agents/m14_sine_fit_v1_gate_20260506/dashboard.md
```

## Vector Metrics

Round 5 all-case vector comparison:

| Metric | Before | After |
| --- | ---: | ---: |
| Case frames | `102` | `102` |
| Samples | `253549` | `253549` |
| Mean vector error | `13.700092` | `11.129620` |
| Mean abs dx error | `8.447227` | `6.340522` |
| Mean abs dy error | `8.226792` | `6.173414` |

## Conformance Metrics

| Case | Before primary | After primary | Before raw | After raw | Before alpha | After alpha |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `EFF_060` | `2.898929` | `1.424711` | `2.799179` | `1.191211` | `2.004444` | `0.395130` |
| `STK_030` | `3.741755` | `3.424827` | `7.201522` | `6.849510` | `7.088274` | `6.351250` |

Master gate after the fit:

```text
case_count=19
accepted=1
approximate=18
regression=0
missing=0
```

## Guardrails

- This is a fitted approximation, not a claim that the hidden
  `TurbulentDisplaceFracAllKernel` or `TurbulentDisplaceFrac1DKernel` has been
  recovered.
- `0008` cycle evolution and `0009` cycle revolutions are still telemetry-only
  for the sine model.
- Continue tuning from decoded vector fields and Rust samples; do not tune this
  module from final PNG diffs first.

## Next Order

1. Fit the field basis again with displacement type `1` only and split amount
   zero cases out of aggregate statistics.
2. Fit amount scale on `A010/A045/A100`.
3. Fit size/frequency/origin using size and offset sweeps.
4. Fit seed and evolution phase separately.
5. Fit complexity octave/fraction behavior.
6. Fit displacement branches, pinning, resize-layer, and antialiasing.

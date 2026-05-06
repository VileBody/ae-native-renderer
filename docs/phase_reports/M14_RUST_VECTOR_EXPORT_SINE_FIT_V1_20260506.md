# M14 Rust Vector Export + Sine Fit v1

Date: 2026-05-06.

## Scope

This pass moved Turbulent Displace from telemetry-only comparison into actual
formula tuning. The exact AE GPU kernels are still not recovered, so this is not
parity locked. The goal was to make the native Rust implementation measurable at
arbitrary AE probe points, then fit the current sine/noise approximation against
decoded AE coordinate-field vectors.

Process note: `native_sine_turbulence_fit_v1` is a transitional baseline, not
the ongoing methodology. Future M14 formula changes must be reverse-first:
Frida/Ghidra evidence should recover kernel inputs, tables, uniforms, buffer
layouts, and control flow before constants or branches change. Probes and
decoded vectors validate the recovered candidate; they are not the discovery
mechanism.

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
- Do not continue by optimizing constants against vector or final PNG metrics.
  Continue by capturing the AE kernel/table contract through Frida/Ghidra
  evidence, then use decoded vectors and Rust samples to validate it.

## Next Order

1. Trace or dump the `TurbulentDisplaceFracAllKernel` and
   `TurbulentDisplaceFrac1DKernel` execution path with Frida/Ghidra/coverage.
2. Recover the field basis/table contract for displacement type `1`: coordinate
   normalization, offset origin, lookup table layout, interpolation, and sampler.
3. Recover amount and size units from the traced params/uniforms, then validate
   on `A010/A045/A100` and size sweeps.
4. Recover seed and evolution/cycle handling from traced state and table updates.
5. Recover complexity octave/fraction behavior.
6. Recover displacement branches, pinning, resize-layer, and antialiasing.

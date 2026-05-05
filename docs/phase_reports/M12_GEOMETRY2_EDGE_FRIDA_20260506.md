# M12 Geometry2 Edge Sampler + Frida Path

Date: 2026-05-06

## Goal

Close the next Geometry2 uncertainty with facts instead of visual guessing:

- isolate pixel-center and out-of-bounds sampler behavior;
- apply only the sampler policy that the isolated AE probe supports;
- verify whether the no-GPU AE85 node uses GPU or CPU execution for Geometry2;
- record exact artifacts and commands so the workflow can be scripted later.

## Inputs

- Probe pack: `fixtures/ae_probe_pack/geometry2_edge`
- AE node: `http://85.239.48.31:8000`
- Baseline case: `EFF_040`
- Frida trace folders:
  - `target/dynamic_tools_85/geometry2_edge_trace_20260506`
  - `target/dynamic_tools_85/geometry2_edge_targeted_trace_20260506`

## Commands

```bash
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/m12_eff040_20260506_baseline \
  --case EFF_040

python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/geometry2_edge \
  --entry-script jsx/build_geometry2_edge_probe_project.jsx \
  --node http://85.239.48.31:8000 \
  --job-id geometry2_edge_20260506 \
  --poll-interval-s 5 \
  --timeout-s 1800

python3 fixtures/ae_probe_pack/geometry2_edge/scripts/measure_geometry2_edge_probe.py \
  --pack fixtures/ae_probe_pack/geometry2_edge \
  --png-root target/ae_remote/geometry2_edge_20260506/extracted/geometry2_edge_20260506/geometry2_edge/ae_goldens/png8 \
  --output-json target/ae_remote/geometry2_edge_20260506/geometry2_edge_measurements.json \
  --output-csv target/ae_remote/geometry2_edge_20260506/geometry2_edge_samples.csv \
  --no-json-samples

python3 scripts/summarize_ae_frida_trace.py \
  target/dynamic_tools_85/geometry2_edge_trace_20260506
```

## AE Probe Result

- Cases measured: 48/48.
- Pixel center: `integer`.
- Out-of-bounds candidate winner: `partial_footprint_transparent`.
- Sampling `0012`: produced a distinct AE branch in the edge probe.
- Hidden transparent RGB marker evidence: no marker samples found.

Candidate scores:

| Candidate | Mean MAE |
| --- | ---: |
| partial-footprint transparent | 58.47818 |
| clamp/edge extend | 75.960358 |
| whole-sample transparent black | 95.6961 |

## Native Change

`ADBE Geometry2` now samples bilinear taps independently:

- inside-source taps contribute normal source pixels;
- outside-source taps contribute transparent black;
- a sample is fully out of bounds only when the whole bilinear footprint cannot touch the source.

Telemetry labels:

- `sampler_mode`: `bilinear_partial_footprint_transparent`
- `edge_policy`: `partial_footprint_transparent_out_of_bounds`

## Conformance Check

`EFF_040` metrics after the sampler change:

| Metric | Value |
| --- | ---: |
| RGBA mean abs diff | 0.06029224395751953 |
| RGB visible source-over mean | 0.045085906982421875 |
| Max abs diff | 61 |
| Changed pixels | 19874 |

The isolated edge behavior changed native telemetry (`out_of_bounds_count`
`91194 -> 90181`), but `EFF_040` final pixels did not move. This means the
remaining `EFF_040` mismatch is not owned by full-canvas OOB clipping. The next
tuning target is the hidden Sampling/quality branch or deeper CPU sampler
behavior.

## Frida Path Result

AE85 has no usable GPU for this path. The trace confirms the active path is:

```text
Transform.aex EffectProc at +0x5f30
  -> GPUFoundation TransformToMatrix / TransformsToMatrices / TransformedBounds helpers
  -> BEE/PIN output frame path
  -> PF world allocation/copy/reference helpers
```

Observed in broad trace:

- `gf_transforms_to_matrices`: 2 calls per case
- `gf_transform_to_matrix`: 2 calls per case
- `gf_transformed_bounds_union`: 3 calls per case
- `gf_transformed_bounds`: 3 calls per case
- `Transform.aex+0x4440`: 1 hit per case
- `Transform.aex+0x4ab0`: 1 hit per case

Not observed:

- `gf_transform_with_motion_blur`
- `gf_transform_operation_quality`
- `gf_transform_operation_ctor`

## Decision

M12 advances from `implemented approximate` to `instrumented/testable`.

The current native implementation now matches the isolated evidence for
integer pixel center and partial-footprint transparent bilinear edges. Formula
tuning is still not locked because AE `0012` Sampling does not appear as a
normal PF parameter on the traced CPU path; it needs a deeper hook around the
CPU sampler/quality state before changing bicubic/high-quality behavior.

## Next

1. Hook around `Transform.aex+0x5f30`, `+0x4440`, and `+0x4ab0` with smaller
   one-case jobs and stack/register/memory snapshots.
2. Add a coordinate-field probe that separates `0012 = 1` and `0012 = 2` on
   non-edge subpixel samples, not only edge pixels.
3. Tune `EFF_040` only after the `0012` branch is mapped to a concrete sampler
   policy.

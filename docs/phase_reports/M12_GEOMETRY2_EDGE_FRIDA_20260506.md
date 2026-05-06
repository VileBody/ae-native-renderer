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

`ADBE Geometry2` now uses partial-footprint transparent bilinear sampling:

- inside-source taps contribute normal source pixels;
- outside-source taps contribute transparent black;
- a sample is fully out of bounds only when the whole bilinear footprint cannot touch the source.

A later alpha-step pass upgraded color accumulation to premultiplied sampling
with unpremultiply back to straight RGBA; the edge policy above is unchanged.

Telemetry labels:

- `sampler_mode`: `bilinear_premult_unpremultiply_partial_footprint_transparent`
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

## CPU Sampler Follow-up

After the first broad CPU trace, the Geometry2 CPU path was narrowed with three
smaller AE85 jobs:

| Trace | Local folder | S3 prefix | Result |
| --- | --- | --- | --- |
| render Stalker | `target/dynamic_tools_85/geometry2_cpu_sampler_stalker_trace_20260506` | `ae_dynamic_traces/geometry2_cpu_sampler_stalker/20260506_013443` | `Transform.aex+0x5f30` render calls `Transform.aex+0x5b20`. |
| inner wrapper | `target/dynamic_tools_85/geometry2_cpu_sampler_inner_trace_20260506` | `ae_dynamic_traces/geometry2_cpu_sampler_inner/20260506_013703` | `+0x5b20` is a PF wrapper/dispatcher: `rdx=PF_Cmd`, `r8=in_data`, `r9=out_data`, stack `+0x28=params`, stack `+0x38=output`. |
| param dump | `target/dynamic_tools_85/geometry2_param_dump_trace_20260506` | `ae_dynamic_traces/geometry2_param_dump/20260506_014212` | `PF_ParamDef[12]` is `Sampling`; its word offset `56` low `s32` is `1` for Bilinear and `2` for Bicubic. |

The key correction is that AE `0012` was not invisible host state. It was just
outside the first twelve-param dump and behind the `+0x5b20` wrapper ABI.

Native now records and parses:

- payload key `0012`
- matchName `ADBE Geometry2-0012`
- UI label `Sampling`
- resolved values: `1 = Bilinear`, `2 = Bicubic`

The first native `0012 = 2` implementation used a Catmull-Rom bicubic sampler
with the same transparent partial-footprint edge model. A later Sampling=2 fit
pass (`docs/phase_reports/M12_GEOMETRY2_SAMPLING2_FIT_20260506.md`) replaced
that with a Frida-gated Keys cubic kernel using `a=-0.7`.

## Decision

M12 advances from `implemented approximate` to `instrumented/testable`.

The current native implementation now matches the isolated evidence for
integer pixel center and partial-footprint transparent bilinear edges. Formula
tuning is still not locked because AE bicubic/high-quality sampling needs its
exact kernel and edge weighting fitted from isolated `0012 = 2` cases. The
parameter identity itself is now mapped.

## Next

1. Add alpha-specific sampler cases, separate from opaque kernel fitting.
2. Keep matrix tuning frozen while any residual diff is sampler/alpha-owned.
3. Re-check composed `0012 = 2` scenes to see whether any stack-level
   adjustment remains.

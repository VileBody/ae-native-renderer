# M12 Geometry2 Hypothesis Results

Status date: 2026-05-05.

Agent C scope: `M12` / `ADBE Geometry2`. No Docker was used. No source change
was made in this pass because `crates/effects/src/geometry.rs` already exposes
the required Geometry2 debug/passport telemetry: raw params, property mapping,
resolved transform, forward/inverse matrix, UV samples, sampler mode, edge
policy, and OOB count.

## Inputs Read

- `docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md`, section
  `M12 Geometry2`
- `docs/reverse_engineering/effect_math_transform_timeline.md`
- `docs/phase_reports/PHASE_4_GEOMETRY_TURBULENT.md`
- `docs/phase_reports/AGENT_ROUND3_GEOMETRY2.md`
- `docs/phase_reports/AGENT_B_GEOMETRY_MOTION_MATH_OBJECTS_20260504.md`
- `docs/phase_reports/AGENT_B_TRANSFORM_GEOMETRY_COLLAPSE_MOTION_FOLLOWUP_20260504.md`
- `docs/phase_reports/AE_REVERSE_GEOMETRY_COLLAPSE_GHIDRA_ROUND2.md`
- `docs/phase_reports/WORKER_D_STEP4_GEOMETRY_PROCEDURAL_ADJUSTMENT_20260505.md`
- `crates/effects/src/geometry.rs`

## Finite Hypotheses

| ID | Hypothesis | Result | Evidence |
| --- | --- | --- | --- |
| H1 | AE payload numeric keys are property indices: `0003` is Uniform Scale namespace, `0004` Scale Height, `0005` Scale Width, `0008` Rotation, `0009` Opacity. | accepted | AE property dump/Ghidra reports plus current `EFF_040` sidecar. `EFF_040` resolves raw `{0003:82,0004:120,0008:72,rotation:17}` to `scale=[120,120]`, `rotation=17`. |
| H2 | Reject legacy mapping where payload `0008` is Scale Height or matchName `ADBE Geometry2-0008` opacity. | rejected | Current property mapping records payload `0008 -> ADBE Geometry2-0007 -> Rotation`; matchName `ADBE Geometry2-0008` is payload `0009` opacity. |
| H3 | Matrix order is the recovered GF order: `T(position) * S(1/par,1) * R(-axis) * Hx(-tan(skew)) * R(axis) * R(rotation) * S(scale) * S(par,1) * T(-anchor)`. | accepted | Existing Geometry2 tests cover skew/pixel-aspect order. `EFF_040` forward matrix determinant is about `1.44`, matching `1.2^2`; STK matrix determinant is about `1.21`, matching `1.1^2`. |
| H4 | Anchor/position/scale/rotation are evaluated in layer/effect space, then inverse sampled per output pixel. | accepted | `EFF_040` center sample maps output `[256,256]` to source `[128.0,127.99999]` for anchor `[128,128]`, position `[256,256]`, scale `[120,120]`, rotation `17`. |
| H5 | Current integer destination pixel-center convention exactly matches AE. | needs_new_probe | Residual `EFF_040` diff is very small but still edge/alpha-sensitive. Current telemetry samples integer output pixels with no `+0.5`; no AE UV sidecar exists to distinguish integer vs half-pixel center. |
| H6 | Current sampler/OOB policy exactly matches AE: bilinear sample with transparent OOB at continuous `uv < 0 || uv > width-1/height-1`. | needs_new_probe | `EFF_040` uses `bilinear_transparent_out_of_bounds` and is close, but the remaining max diff `61` and alpha/RGB edge residual can still be filter-footprint or OOB threshold. |
| H7 | In `STK_030`, Geometry2 consumes the accumulated lower canvas and participates in the locked adjustment order before Posterize/Minimax/Turbulent. | accepted as composition telemetry | `STK_030` temporal contract is OK for all 9 frames; adjustment sidecar order is Geometry2 -> Posterize Time -> Minimax -> Turbulent Displace. Final stack pixels remain downstream-composed diagnostics, not isolated M12 proof. |

## Commands Run

```sh
cargo run -p render-cli -- hypothesis-pack \
  --module M12 \
  --candidate geometry2_current_mapping_matrix_sampler \
  --status instrumented \
  --gate isolated \
  --question "Does current Geometry2 mapping/matrix/sampler explain isolated EFF_040?" \
  --hypothesis "Payload 0003 is uniform-scale namespace, 0004 scale height, 0005 scale width, 0008 rotation; matrix uses recovered GF order; residual EFF_040 error, if any, is sampler/pixel-center/OOB rather than mapping/order." \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md#M12 \
  --evidence docs/reverse_engineering/effect_math_transform_timeline.md \
  --evidence docs/phase_reports/AE_REVERSE_GEOMETRY_COLLAPSE_GHIDRA_ROUND2.md \
  --note "Agent C M12 local Rust run; no Docker." \
  --pack fixtures/ae_conformance_pack \
  --case EFF_040 \
  --out target/ae_agents/m12_eff040_current
```

```sh
cargo run -p render-cli -- hypothesis-pack \
  --module M12 \
  --candidate geometry2_current_in_stk030 \
  --status instrumented \
  --gate composition \
  --question "Does current Geometry2 remain stack-safe inside STK_030?" \
  --hypothesis "Current Geometry2 mapping/matrix feeds the adjustment stack correctly; STK_030 residuals are composition diagnostics and cannot be assigned to M12 unless Geometry2 checkpoints diverge before M13/M14/M15." \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md#M12 \
  --evidence docs/phase_reports/WORKER_D_STEP4_GEOMETRY_PROCEDURAL_ADJUSTMENT_20260505.md \
  --note "Agent C M12 local Rust run; no Docker." \
  --pack fixtures/ae_conformance_pack \
  --case STK_030 \
  --out target/ae_agents/m12_stk030_current
```

Both hypothesis-pack runs completed with `ok=true`.

## Matrix And UV Metrics

### `EFF_040`

Artifacts:

- `target/ae_agents/m12_eff040_current/report.json`
- `target/ae_agents/m12_eff040_current/hypothesis_report.json`
- `target/ae_agents/m12_eff040_current/EFF_040/metrics.json`
- `target/ae_agents/m12_eff040_current/EFF_040/effects_debug/frame_00000/EFF_040_geometry_00_geometry2.json`

Metrics summary:

| Metric | Value |
| --- | ---: |
| RGBA mean abs diff | `0.06029224395751953` |
| RGB mean abs diff | `0.049138387044270836` |
| Alpha mean abs diff | `0.09375381469726562` |
| RGB source-over-AE-bg mean abs diff | `0.045085906982421875` |
| Max abs diff | `61` |
| Changed pixels | `19874` |
| Changed pixel ratio | `0.07581329345703125` |

Geometry2 sidecar:

```text
raw_params = {0001:[128,128], 0002:[256,256], 0003:82, 0004:120, 0008:72, rotation:17}
resolved  = anchor [128,128], position [256,256], scale [120,120], rotation 17, skew 0, pixel_aspect 1
forward   = [[1.1475658, -0.35084605, 154.01987],
             [0.35084605, 1.1475658, 64.20328],
             [0, 0, 1]]
inverse   = [[0.79692054, 0.24364305, -138.38428],
             [-0.24364305, 0.79692054, -13.639045],
             [0, 0, 1]]
sampler   = bilinear_transparent_out_of_bounds
OOB count = 91194
center UV = output [256,256] -> source [128.0,127.99999]
```

Interpretation: mapping, anchor/position, and matrix order are not the remaining
first-divergence owner for isolated Geometry2. Residual error is small and
concentrated in the sampler/edge/pixel-center family.

### `STK_030`

Artifacts:

- `target/ae_agents/m12_stk030_current/report.json`
- `target/ae_agents/m12_stk030_current/hypothesis_report.json`
- `target/ae_agents/m12_stk030_current/STK_030/metrics.json`
- `target/ae_agents/m12_stk030_current/STK_030/adjustment_effects.jsonl`
- `target/ae_agents/m12_stk030_current/effects_debug/STK_030/*/STK_030_adjustment_0_ADBE_Geometry2.json`

Aggregate metrics across 9 frames:

| Metric | Value |
| --- | ---: |
| RGBA mean abs diff | `59.2162659962972` |
| RGB mean abs diff | `42.69982217859339` |
| Alpha mean abs diff | `108.76559744940863` |
| RGB source-over-AE-bg mean abs diff | `40.05274299339012` |
| Max abs diff | `255` |
| Changed pixels | `1832673` |
| Temporal contract | `ok=true`, 9 checked frames |

Geometry2 adjustment sidecar is constant over all checked frames:

```text
raw_params = {0003:96, 0004:110, 0008:92}
resolved  = anchor [256,256], position [256,256], scale [110,110], rotation 92, skew 0, pixel_aspect 1
forward   = [[-0.03838941, -1.09932995, 820.8842],
             [1.09932995, -0.03838941, -23.401157],
             [0, 0, 1]]
inverse   = [[-0.03172678, 0.90853703, 47.304832],
             [-0.90853703, -0.03172678, 745.0613],
             [0, 0, 1]]
sampler   = bilinear_transparent_out_of_bounds
OOB count = 127592
```

Temporal/stack evidence:

- adjustment order is Geometry2 -> Posterize Time -> Minimax -> Turbulent Displace;
- frames `0,1,5,10,15,20,30,45,59` all pass
  `m15.m16.adjustment-posterize-bucket-live-split.v1`;
- Geometry2 and Posterize observe the bucket time; downstream effects consume
  bucketed input while using live comp time.

Interpretation: `STK_030` is stack-safe telemetry for M12, not a final M12
parity gate. Its large final-pixel error remains composed across M13/M14/M15/
M16/M19 and should not be tuned as a Geometry2 matrix failure.

## Accepted / Rejected / Needs New Probe

Accepted:

- H1 property-index mapping for `0003`/`0004`/`0005`/`0008`/`0009`.
- H3 recovered GF matrix order.
- H4 anchor/position/scale/rotation layer-space inverse sampling.
- H7 Geometry2 participation in the `STK_030` adjustment order and temporal
  contract.

Rejected:

- Treating payload `0008` as Scale Height.
- Treating payload `0008` as opacity because matchName `ADBE Geometry2-0008`
  exists.
- Reordering the current matrix to tune `EFF_040` final pixels.

Needs new probe:

- H5 pixel center: integer destination pixel vs half-pixel center.
- H6 sampler/OOB: continuous UV bounds vs half-pixel/filter-footprint bounds;
  transparent black vs partial footprint behavior at source edges.
- Geometry2 sampling enum `0012`: current default evidence supports bilinear;
  Bicubic/Lanczos/Area is not locked by these runs.

## Next Discriminator

Create a focused Geometry2 edge probe before changing any sampler:

```text
GEO2_EDGE_010:
  source: 1-pixel opaque impulse plus 2x/4x checkerboard and hard alpha border
  transforms: identity, subpixel translate, small rotation, scale 110/120
  UV targets: -0.5, -0.0001, 0, 0.4999, 0.5,
              width-1.5, width-1, width-0.5, width-0.0001, width
  readout: AE output alpha/RGB, inferred source UV, selected neighbors,
           native sidecar UV/OOB/sample RGBA
```

This distinguishes:

- integer vs half-pixel destination centers;
- OOB threshold at `uv < 0`, `uv < -0.5`, `uv > width-1`, or `uv >= width`;
- bilinear transparent footprint vs clamp/preserve-background edge behavior.

No `ORCHESTRATOR_BLOCKER` is marked in this pass. The remaining uncertainty may
be a shared transform/sampler convention, but the current evidence only proves a
Geometry2 edge discriminator is needed; it does not justify changing the global
sampler or pixel-center policy for all modules.

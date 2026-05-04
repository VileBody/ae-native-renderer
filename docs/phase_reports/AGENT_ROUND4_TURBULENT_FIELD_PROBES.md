# Agent Round 4 Turbulent Field Probes

Scope: M14 / `ADBE Turbulent Displace` field evidence and safe prep only.

Owned files touched:

- `crates/effects/src/turbulent_displace.rs`
- `docs/phase_reports/AGENT_ROUND4_TURBULENT_FIELD_PROBES.md`
- `target/ae_agents/round4_turbulent/**`

No Turbulent noise formula, sampler, edge policy, conformance pack, or shared
fixture files were changed.

## Current Evidence

Round 3 already split Turbulent into parameter mapping, field/noise,
displacement, sampler, and telemetry stages. This round preserved that runtime
behavior and added narrow tests that document the current model boundary.

Current modeled controls:

| Control | Status |
| --- | --- |
| Amount / `0002` | Modeled, clamped to `0..200`, displacement amplitude is `amount * 0.25`. |
| Size / `0003` | Modeled, clamped to minimum `1.0`, used as coordinate scale divisor. |
| Complexity / `0005` | Modeled as rounded octave count, clamped to `1..6`. |
| Evolution / `0006` | Modeled as time-varying scalar, converted from degrees to radians as procedural phase. |
| Sampler | Modeled as `nearest_round`. |
| Edge behavior | Modeled as transparent out-of-bounds samples. |
| Temporal semantics | Effect params are sampled at `ctx.time`; EFF_060 evolution keyframes resolve to `0`, `45`, `90`, `135` degrees at frames `0`, `15`, `30`, `45`. |

Known unmodeled or unproven AE controls:

| Control / behavior | Current native status |
| --- | --- |
| Displacement type / mode | Not modeled. Unknown params are preserved in telemetry `raw_params` but ignored by field resolution. |
| Random seed / noise seed | Not modeled. |
| Pinning | Not modeled. |
| Resize layer | Not modeled. |
| AE edge mode variants | Not modeled beyond transparent out-of-bounds. |
| AE procedural noise basis | Not known; native remains deterministic sine turbulence. |
| AE evolution internals | Not known beyond scalar keyframe sampling; phase mapping is native approximation. |
| Bilinear/subpixel sampler variants | Not modeled; current sampler is nearest-round. |

## Round4 Checks

Added tests:

- `resolved_params_capture_current_field_model_bounds`
  - locks current amount/size/complexity clamp behavior and amplitude mapping;
  - no formula change.
- `unmodeled_ae_controls_are_currently_ignored_but_preserved_in_raw_params`
  - verifies unknown AE-like controls do not silently alter the current field;
  - verifies telemetry still preserves those raw params for future probe-driven work.

Existing EFF_060 sidecar regression remains unchanged:

| Frame | Time | Evolution | Field hash | OOB | Center dx,dy | Center UV | Sample |
| ---: | ---: | ---: | --- | ---: | --- | --- | --- |
| `0` | `0.0` | `0` | `00fcc69566c90a44` | `5233` | `[10.146056,-0.629475]` | `[266.146057,255.370529]` | `[266,255]` |
| `15` | `0.5` | `45` | `aaec48f4e5d0a876` | `5154` | `[3.762150,-8.169911]` | `[259.762146,247.830093]` | `[260,248]` |
| `30` | `1.0` | `90` | `4c8ae70ab7d0aca1` | `5183` | `[-4.825585,-10.925628]` | `[251.174408,245.074371]` | `[251,245]` |
| `45` | `1.5` | `135` | `fda864d70f0c103a` | `5117` | `[-10.586543,-7.280874]` | `[245.413452,248.719131]` | `[245,249]` |

## Before / After Metrics

Baseline Round3 / Round2 evidence for EFF_060:

- RGB mean abs diff: `2.898928960164388`
- Background-alpha-normalized mean abs diff: `2.8005855083465576`
- RGB RMSE: `16.470856736302686`
- Alpha mean abs diff: `191.25`

Fresh Round4 output:

- Path: `target/ae_agents/round4_turbulent/EFF_060/metrics.json`
- RGB mean abs diff: `2.898928960164388`
- Background-alpha-normalized mean abs diff: `2.8005855083465576`
- RGB RMSE: `16.470856736302686`
- Alpha mean abs diff: `191.25`
- Conformance status: `ok=true`, `status=measured`

The unchanged metrics are expected. This round intentionally avoided
final-pixel tuning.

## AE Field Probe Spec

Do not add these to the shared pack until the team is ready to regenerate AE
goldens. The next fixture should be an AE field-identification micro-pack, not a
single final-pixel tuning scene.

Required render setup:

- Composition: `512x512`, `30 fps`, `2 s`, 8-bit and 16-bit output if possible.
- Source A: coordinate field image where RGB encodes source coordinate:
  - `R = x mod 256`, `G = y mod 256`, `B = checker/parity`, `A = 255`.
- Source B: sparse impulse grid:
  - one-pixel white impulses every `32 px`, colored axes at center lines.
- Source C: hard vertical/horizontal edge plus alpha ramp:
  - distinguishes transparent, clamp, wrap, and pinning edge behavior.
- Source D: checkerboard with unique colored cells:
  - distinguishes nearest, floor, bilinear, and half-pixel conventions.

Probe cases:

| Case | Params | Purpose |
| --- | --- | --- |
| `TD_AMOUNT_SWEEP` | amount `0,1,10,45,100`, fixed size `65`, complexity `2`, evolution `0` | Determine amplitude scale and sign convention. |
| `TD_SIZE_SWEEP` | size `1,8,16,32,65,128,256`, amount `45` | Identify noise frequency normalization and coordinate origin. |
| `TD_COMPLEXITY_SWEEP` | complexity `1,2,3,4,6`, amount `45`, size `65` | Identify octave accumulation and rounding semantics. |
| `TD_EVOLUTION_STATIC` | evolution `0,45,90,180,360,720`, fixed other params | Determine phase/evolution mapping and periodicity. |
| `TD_EVOLUTION_ANIM` | keyframed evolution `0 -> 180` over `2 s`; render frames `0,1,15,30,45,59` | Verify temporal interpolation and frame-time sampling. |
| `TD_DISPLACEMENT_TYPE` | same amount/size/evolution across every AE displacement type | Determine whether field vector basis changes by mode. |
| `TD_SEED_SWEEP` | seed/random controls `0,1,2,10,999` if exposed by AE | Determine seed field stability and value domain. |
| `TD_PINNING_EDGE` | pinning off/on variants over hard-edge and alpha-ramp source | Separate field generation from edge/pinning behavior. |
| `TD_RESIZE_LAYER` | resize layer off/on, large amount near edges | Determine canvas extent and out-of-bounds policy. |
| `TD_SAMPLER_CHECK` | small amount values causing subpixel UV shifts over unique checker | Determine nearest/floor/bilinear/pixel-center convention. |

For each AE render, derive a field estimate by decoding the coordinate-field
source at selected output pixels. Required sample grid:

- all corners, edge midpoints, center;
- every `32 px` grid point;
- a dense `17x17` center patch;
- a `16 px` border band for pinning/resize behavior.

For each sampled output pixel, record:

- output xy;
- observed source coordinate decoded from the coordinate field;
- inferred dx/dy;
- final RGBA;
- whether the pixel sampled outside the source image;
- effect params, frame, comp time, and AE color/depth settings.

Acceptance target before formula work:

1. Identify parameter mapping and enum values first.
2. Match AE field vectors on the coordinate-field source before inspecting
   footage/text final pixels.
3. Only then tune noise basis, evolution phase, sampler, and edge policy.

## Commands Run

```sh
docker run --rm -v "$PWD":/work -w /work rust:1-bookworm \
  sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; rustup component add rustfmt >/tmp/rustfmt-install.log && rustfmt crates/effects/src/turbulent_displace.rs'
```

```sh
docker run --rm -v "$PWD":/work -w /work rust:1-bookworm \
  sh -lc 'set -e; export PATH=/usr/local/cargo/bin:$PATH; export CARGO_HOME=/work/target/ae_agents/round4_turbulent/cargo-home; export CARGO_TARGET_DIR=/work/target/ae_agents/round4_turbulent/cargo-target; cargo test -p effects turbulent -- --nocapture'
```

Result: `10 passed; 0 failed; 26 filtered out`.

```sh
docker run --rm -v "$PWD":/work -w /work rust:1-bookworm \
  sh -lc 'set -e; export PATH=/usr/local/cargo/bin:$PATH; export CARGO_HOME=/work/target/ae_agents/round4_turbulent/cargo-home; export CARGO_TARGET_DIR=/work/target/ae_agents/round4_turbulent/cargo-target; apt-get update >/tmp/apt-update.log; apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev fontconfig >/tmp/apt-install.log; cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round4_turbulent --case EFF_060'
```

Result: `conformance-pack.done ok=true cases=1 report=target/ae_agents/round4_turbulent/report.json`.

## Status

M14 remains `implemented approximate`. The current native field model is now
guarded and sidecar-stable, but AE-equivalent field evidence is still missing.
The next safe step is generating the AE field probe pack above; changing the
noise basis or sampler from EFF_060 final pixels alone would be underdetermined.

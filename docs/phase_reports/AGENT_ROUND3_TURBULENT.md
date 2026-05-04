# Agent Round 3 Turbulent Displace

Date: 2026-05-03

Scope: M14 / `ADBE Turbulent Displace` only.

## Inputs

- Field sidecars:
  `target/ae_agents/round2_integrated_trace_smoke/EFF_060/effects_debug/frame_*/EFF_060_field_turbulent_00_turbulent_displace.json`
- Official Adobe references used only for parameter meaning/order:
  - https://helpx.adobe.com/in/after-effects/using/distort-effects.html#turbulent_displace_effect
  - https://ae-scripting.docsforadobe.dev/matchnames/effects/firstparty/

Adobe documents `ADBE Turbulent Displace` as a first-party Distort effect with
32 BPC support and describes it as fractal/turbulence-noise displacement driven
by controls such as Displacement, Amount, Size, Complexity, Evolution, seed,
pinning, and resize. The docs do not specify the exact procedural noise basis,
so no noise-formula tuning was made from final pixels.

## Code Changes

Changed `crates/effects/src/turbulent_displace.rs` only:

- Kept the current sine turbulence field formula unchanged.
- Split the implementation into explicit stages:
  - raw parameter extraction;
  - `map_params_to_field_model`;
  - `field_noise` / `displacement_from_noise` / `field_vector`;
  - `sample_nearest_round_transparent` for sampler and edge behavior.
- Kept sampler and edge labels as constants:
  `nearest_round` and `transparent_out_of_bounds`.
- Added a field-level EFF_060 regression test using the Round 2 integrated trace
  sidecar hashes, OOB counts, and center probe samples for frames 0, 15, 30, and
  45.

## Field Evidence

Round 3 conformance sidecars are byte-identical to
`round2_integrated_trace_smoke/EFF_060/effects_debug`:

```sh
diff -qr \
  target/ae_agents/round2_integrated_trace_smoke/EFF_060/effects_debug \
  target/ae_agents/round3_turbulent/EFF_060/effects_debug
# no output
```

Selected sidecar probes:

| Frame | Time | Evolution | Field hash | OOB | Center dx,dy | Center UV | Sample |
| ---: | ---: | ---: | --- | ---: | --- | --- | --- |
| 0 | 0.0 | 0 | `00fcc69566c90a44` | 5233 | `[10.1461,-0.6295]` | `[266.1461,255.3705]` | `[266,255]` |
| 15 | 0.5 | 45 | `aaec48f4e5d0a876` | 5154 | `[3.7621,-8.1699]` | `[259.7621,247.8301]` | `[260,248]` |
| 30 | 1.0 | 90 | `4c8ae70ab7d0aca1` | 5183 | `[-4.8256,-10.9256]` | `[251.1744,245.0744]` | `[251,245]` |
| 45 | 1.5 | 135 | `fda864d70f0c103a` | 5117 | `[-10.5865,-7.2809]` | `[245.4135,248.7191]` | `[245,249]` |

This supports a no-behavior-change refactor and regression coverage. It does
not identify a low-risk formula change.

## Metrics

Before: `target/ae_agents/round2_integrated_trace_smoke/EFF_060/metrics.json`.
After: `target/ae_agents/round3_turbulent/EFF_060/metrics.json`.

| Metric | Before | After |
| --- | ---: | ---: |
| `metrics.rgb.mean_abs_diff` | 2.898928960164388 | 2.898928960164388 |
| `metrics.rgb.rmse_abs_diff` | 16.470856736302686 | 16.470856736302686 |
| `metrics.rgb.max_abs_diff` | 200 | 200 |
| `metrics.background_alpha_normalized.mean_abs_diff` | 2.8005855083465576 | 2.8005855083465576 |
| raw `mean_abs_diff` | 49.98669672012329 | 49.98669672012329 |
| `metrics.alpha.mean_abs_diff` | 191.25 | 191.25 |

No final-pixel improvement was expected because the field formula, sampler, and
edge behavior were intentionally unchanged.

## Commands

Targeted tests:

```sh
docker run --rm \
  -e CARGO_HOME=/tmp/cargo_home \
  -e CARGO_TARGET_DIR=/tmp/ae_native_renderer_round3_turbulent_target \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  sh -lc 'set -e; export PATH=/usr/local/cargo/bin:$PATH; cargo test -p effects turbulent -- --nocapture'
```

Result: `8 passed; 0 failed`.

Conformance:

```sh
docker run --rm \
  -e HOST_UID=$(id -u) -e HOST_GID=$(id -g) \
  -e CARGO_HOME=/tmp/cargo_home \
  -e CARGO_TARGET_DIR=/tmp/ae_native_renderer_round3_turbulent_target \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  sh -lc 'set -e; apt-get update >/tmp/apt-update.log; apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev fontconfig >/tmp/apt-install.log; export PATH=/usr/local/cargo/bin:$PATH; cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round3_turbulent --case EFF_060; chown -R "$HOST_UID:$HOST_GID" /work/target/ae_agents/round3_turbulent'
```

Result:

```text
conformance-pack.done ok=true cases=1 report=target/ae_agents/round3_turbulent/report.json
```

Formatting note: host `cargo`, container `rustfmt`, and container `rustup` are
not installed in this environment, so `rustfmt` could not be run. The targeted
compiler test passed.

## Next Blocker

The current blocker is missing AE-equivalent field telemetry for the actual
Turbulent Displace noise basis and controls. Final PNG diff is not enough to
choose between noise basis, displacement type, evolution unit/progression,
pinning, resize-layer behavior, or sampler changes. The next useful fixture is a
field-level AE probe/export that isolates those controls before any formula
tuning.

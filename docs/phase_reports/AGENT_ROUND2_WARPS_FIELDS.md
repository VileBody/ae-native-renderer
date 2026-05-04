# Agent Round 2 Warps/Fields

Date: 2026-05-03.

Scope stayed on the Warps/Fields lane:

- fresh focused `EFF_040` / `EFF_060` conformance output;
- Geometry2 and Turbulent Displace sidecars under the Round 2 output tree;
- no Geometry2 or Turbulent Displace formula patch in this pass.

## Commands

Fresh conformance run:

```sh
docker run --rm \
  -e HOST_UID=$(id -u) -e HOST_GID=$(id -g) \
  -e CARGO_HOME=/tmp/cargo_home \
  -e CARGO_TARGET_DIR=/work/target/round2_cargo_target \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  sh -lc 'set -e; apt-get update >/tmp/apt-update.log; apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev fontconfig >/tmp/apt-install.log; export PATH=/usr/local/cargo/bin:$PATH; cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round2_warps_fields --case EFF_040 --case EFF_060; chown -R "$HOST_UID:$HOST_GID" /work/target/ae_agents/round2_warps_fields /work/target/round2_cargo_target'
```

Sidecar export:

- added a narrow conformance sidecar hook for `EFF_040` and `EFF_060`;
- rerunning the hook was blocked by unrelated current `render-core` compile
  errors in text/trace code (`record_*_trace` missing and
  `apply_text_animators` signature mismatch);
- generated the same sidecar shape with an inline Python/Numpy script mirroring
  the current Geometry2 and Turbulent Displace telemetry formulas.

Tests:

```sh
docker run --rm \
  -e HOST_UID=$(id -u) -e HOST_GID=$(id -g) \
  -e CARGO_HOME=/work/target/round2_cargo_home \
  -e CARGO_TARGET_DIR=/work/target/round2_cargo_target \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  sh -lc 'set -e; export PATH=/usr/local/cargo/bin:$PATH; cargo test -p effects geometry; cargo test -p effects turbulent; cargo test -p effects; chown -R "$HOST_UID:$HOST_GID" /work/target/round2_cargo_target /work/target/round2_cargo_home'
```

## Metrics

`EFF_040` improved after the prior axis-scale mapping fix. Old Round 1 report:
raw mean `55.817`, RGB mean all `15.302`, RGB mean content `49.315`, alpha mean
`177.360`.

Fresh Round 2 `EFF_040`:

| Metric | Value |
| --- | ---: |
| `metrics.rgb.mean_abs_diff` | `8.9347` |
| `metrics.rgb.rmse_abs_diff` | `33.8500` |
| `metrics.rgb.max_abs_diff` | `250` |
| `metrics.rgb.changed_pixel_ratio` | `0.3099` |
| raw `mean_abs_diff` | `51.0411` |

This is an RGB mean improvement of about `6.3673` absolute, or `41.6%`, from
the old Round 1 RGB-all estimate. The image moved in the right direction.

Fresh Round 2 `EFF_060`:

| Frame | Evolution | RGB mean | RGB RMSE | RGB max |
| ---: | ---: | ---: | ---: | ---: |
| `0` | `0 deg` | `2.9904` | `16.5674` | `200` |
| `15` | `45 deg` | `2.8646` | `16.3918` | `200` |
| `30` | `90 deg` | `2.8439` | `16.4609` | `200` |
| `45` | `135 deg` | `2.8968` | `16.4628` | `200` |

Summary `EFF_060 metrics.rgb.mean_abs_diff=2.8989`.

## Geometry2 Sidecar

Path:

```text
target/ae_agents/round2_warps_fields/EFF_040/effects_debug/frame_00000/EFF_040_geometry_00_geometry2.json
```

Resolved parameters:

```json
{
  "anchor": [128.0, 128.0],
  "position": [256.0, 256.0],
  "scale": [120.0, 72.0],
  "rotation": 17.0
}
```

Matrix evidence:

```text
forward = [[1.1475658, -0.2105076, 136.05655],
           [0.3508461,  0.6885394, 122.95866],
           [0.0,        0.0,       1.0]]

inverse = [[ 0.7969206, 0.2436431, -138.38429],
           [-0.4060718, 1.3282011, -108.06509],
           [ 0.0,       0.0,          1.0]]
```

Sampler/edge:

- `sampler_mode=nearest_round`
- `edge_policy=transparent_out_of_bounds`
- `out_of_bounds_count=116739 / 262144`

Probe examples:

| Output | Source UV | Sample | OOB |
| --- | --- | --- | --- |
| `[256,256]` | `[128.0000,128.0000]` | `[128,128]` | `false` |
| `[511,256]` | `[331.2148,24.4517]` | `[331,24]` | `false` |
| `[256,511]` | `[190.1290,466.6913]` | `[190,467]` | `false` |

The remaining `EFF_040` mismatch is too large for a sampler-only or pixel-center
patch. Native content bbox is now `(202,256)-(511,511)`, while AE is
`(192,192)-(511,511)`. The next Geometry2 target is matrix/anchor-position
convention, likely including effect-space versus comp-space assumptions after
layer placement.

## Turbulent Field Sidecars

Paths:

```text
target/ae_agents/round2_warps_fields/EFF_060/effects_debug/frame_00000/EFF_060_field_turbulent_00_turbulent_displace.json
target/ae_agents/round2_warps_fields/EFF_060/effects_debug/frame_00015/EFF_060_field_turbulent_00_turbulent_displace.json
target/ae_agents/round2_warps_fields/EFF_060/effects_debug/frame_00030/EFF_060_field_turbulent_00_turbulent_displace.json
target/ae_agents/round2_warps_fields/EFF_060/effects_debug/frame_00045/EFF_060_field_turbulent_00_turbulent_displace.json
```

Resolved constants across frames:

- `amount=45`
- `size=65`
- `complexity=2`
- `amplitude=11.25`
- sampler/edge: `nearest_round`, `transparent_out_of_bounds`

Frame probes at `[256,256]`:

| Frame | Evolution | Field hash | OOB count | `dx,dy` | Source UV |
| ---: | ---: | --- | ---: | --- | --- |
| `0` | `0` | `7e0b672d8b85f80d` | `5233` | `[10.1461,-0.6295]` | `[266.1461,255.3705]` |
| `15` | `45` | `0a08be4d10c00f52` | `5154` | `[3.7621,-8.1699]` | `[259.7621,247.8301]` |
| `30` | `90` | `b5e38dbbbc25dfd7` | `5183` | `[-4.8256,-10.9256]` | `[251.1744,245.0744]` |
| `45` | `135` | `b2d744fa464b360c` | `5117` | `[-10.5865,-7.2809]` | `[245.4135,248.7191]` |

`EFF_060` should remain field-only for now. The sidecars prove current native
evolution changes the displacement field deterministically, but the field is
still the known sine approximation. Do not tune Turbulent from final pixels;
the next M14 work should compare AE-equivalent field probes or isolate the AE
noise model/evolution semantics.

## Test Results

- `cargo test -p effects geometry`: `6 passed`
- `cargo test -p effects turbulent`: `7 passed`
- `cargo test -p effects`: `30 passed`

## Status

`M12` is ready for Geometry2 formula tuning with matrix/UV evidence. Start with
matrix order and anchor/position/effect-space convention before sampler or edge
policy.

`M14` remains field-only. Telemetry exists for the selected frames, but formula
tuning needs field-level comparison, not final PNG fitting.

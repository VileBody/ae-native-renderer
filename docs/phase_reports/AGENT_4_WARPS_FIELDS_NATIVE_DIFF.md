# Agent 4 Warps/Fields Native Diff

Date: 2026-05-03

## Scope

Agent 4 covers:

- `M12` Geometry2
- `M14` Turbulent Displace
- `M16` adjustment/effect stack interaction as a consumer of Posterize Time
- `M19` sampler, edge, color, and alpha assumptions

`STK_030` also depends on `M13` Minimax and `M15` Posterize Time, so it is a
composed diagnostic target, not a first tuning target.

## Commands

Host `cargo` was unavailable:

```sh
cargo --version
```

Result: `zsh:1: command not found: cargo`

Native conformance was run in Docker:

```sh
docker run --rm --entrypoint sh \
  -e HOST_UID=$(id -u) -e HOST_GID=$(id -g) \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'set -e; apt-get update; apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev fontconfig; export PATH=/usr/local/cargo/bin:$PATH; export CARGO_TARGET_DIR=/tmp/ae_native_renderer_cargo_target; cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/agent4_warps_fields --case EFF_040 --case EFF_060 --case STK_030; chown -R "$HOST_UID:$HOST_GID" /work/target/ae_agents/agent4_warps_fields'
```

Output root:

```text
target/ae_agents/agent4_warps_fields/
```

Result:

```text
conformance-pack.done ok=true cases=3 report=target/ae_agents/agent4_warps_fields/report.json
```

The run had no thresholds, so `ok=true` means all requested cases rendered and
were measured. It does not mean AE parity passed.

## Cases

| Case | Modules | Frames | Notes |
| --- | --- | --- | --- |
| `EFF_040` | `M12` | `0` | Geometry2 coordinate-field transform |
| `EFF_060` | `M14` | `0, 15, 30, 45` | Turbulent Displace over coordinate field, with checker below |
| `STK_030` | `M12`, `M13`, `M14`, `M15`, `M16`, `M19` | `0, 1, 5, 10, 15, 20, 30, 45, 59` | Geometry2 -> Posterize Time -> Minimax -> Turbulent Displace adjustment stack |

## Top Metrics

Summary from `metrics.json`:

| Case | Frames | Max abs | Mean abs | RMSE | Changed pixels |
| --- | ---: | ---: | ---: | ---: | ---: |
| `EFF_040` | 1 | 255 | 55.8168 | 113.3908 | 262144 / 262144 = 1.0000 |
| `EFF_060` | 4 | 255 | 49.9867 | 111.3358 | 1048576 / 1048576 = 1.0000 |
| `STK_030` | 9 | 255 | 51.9165 | 107.3930 | 2288981 / 2359296 = 0.9702 |

Largest frame-level offenders:

| Case | Frame | Time | Max abs | Mean abs | RMSE | Changed ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `EFF_040` | 0 | 0.0000 | 255 | 55.8168 | 113.3908 | 1.0000 |
| `STK_030` | 45 | 1.5000 | 255 | 52.2275 | 107.6021 | 0.9702 |
| `STK_030` | 30 | 1.0000 | 255 | 52.1793 | 107.7004 | 0.9701 |
| `STK_030` | 59 | 1.9667 | 255 | 51.9821 | 107.6943 | 0.9702 |
| `EFF_060` | 0 | 0.0000 | 255 | 50.0553 | 111.3465 | 1.0000 |

`EFF_060` stays nearly flat across evolution frames:

| Frame | Native effect time | Expected native evolution | Mean abs | RMSE |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 0.0000 | 0 deg | 50.0553 | 111.3465 |
| 15 | 0.5000 | 45 deg | 49.9610 | 111.3270 |
| 30 | 1.0000 | 90 deg | 49.9454 | 111.3347 |
| 45 | 1.5000 | 135 deg | 49.9851 | 111.3349 |

## Suspected First Divergent Primitive

### `EFF_040` / Geometry2

Suspected first divergence: parameter mapping before matrix/sampler tuning.

The recipe has:

```json
{
  "0001": [128, 128],
  "0002": [256, 256],
  "0003": 82,
  "0004": 120,
  "0008": 72,
  "rotation": 17
}
```

Current `Geometry2Params::scale_param` returns uniform `0003` as soon as it is
present, so native resolves scale as `(82, 82)` and ignores `0004`/`0008`.
The visual diff shows native using the smaller uniform transform while the AE
golden is wider and clipped on the right, consistent with AE consuming the
width/height scale controls. The first patch should therefore expose resolved
params and matrix telemetry before changing formulas:

- raw params and resolved `anchor`, `position`, `scale`, `rotation`
- forward matrix and inverse matrix
- sampled source UV for a small grid and for edge pixels
- out-of-bounds counts and edge policy
- sampler mode used for Geometry2

After mapping is confirmed, the next likely `M19` checks are pixel-center
convention, bilinear vs nearest sampling, and transparent-vs-clamped edge
behavior. Current Geometry2 samples by `round()` and skips out-of-bounds pixels.

### `EFF_060` / Turbulent Displace

Suspected first divergence: displacement field generation.

Current native Turbulent Displace is a deterministic sine turbulence
approximation:

- `amount` is clamped then scaled by `0.25`
- `size` is clamped to at least `1`
- `complexity` is rounded and clamped to `1..6`
- `evolution` is sampled from `0006` and converted to radians
- displaced UV is `(x + dx, y + dy)`
- source sampling uses `round()` and transparent out-of-bounds

The diff is full-frame for every selected frame, and the native output has
horizontal striation artifacts that are not a useful basis for formula tuning.
Do not tune Turbulent Displace from final pixels. The required next artifact is
field telemetry for the same frames:

- resolved `amount`, `size`, `complexity`, `evolution`
- `noise_x`, `noise_y` or equivalent scalar noise fields
- float `dx`, `dy`
- displaced source UV before rounding/sampling
- sampled source pixel or source UV hash
- out-of-bounds count and edge policy
- final native pixel hash only as a dependent output

Only after those field maps exist should any AE-noise/evolution/octave formula
patch be attempted.

### `STK_030` / Stack Interaction

`STK_030` cannot diagnose parity first because isolated `M12` and `M14` already
fail hard. It does reveal an additional stack-time question:

- native `STK_030_00000.png` and `STK_030_00001.png` are byte-identical
- AE `STK_030_00000.png` and `STK_030_00001.png` are different

Native currently computes one `effects_time` for the whole adjustment stack via
`posterized_time_for_effects(effects, time)`, resamples the lower stack at that
time, then applies every effect with that same time. For frame 1 at 30 fps and
Posterize Time 6 fps, this means both lower-stack sampling and Turbulent
Displace evolution are still evaluated at frame 0 time.

The suspected stack primitive is therefore time routing around Posterize Time in
an adjustment effect chain, not a final-pixel formula issue. Needed telemetry:

- per adjustment effect index: match name, input canvas hash, output canvas hash
- comp time, lower-stack resample time, effect param time, and source time
- whether effects before Posterize Time and after Posterize Time receive
  original time or quantized time
- Turbulent Displace resolved evolution inside `STK_030` per selected frame
- Minimax input/output hash to separate `M13` from warp/field failures

## Next Patch Needed

1. Add Geometry2 debug output for resolved params, matrix, inverse matrix,
   `sample_uv`, sampler mode, and out-of-bounds stats. Use it to verify the
   `0003` vs `0004`/`0008` mapping before any matrix/sampler formula change.
2. Add Turbulent Displace field telemetry for `EFF_060` frames `0`, `15`, `30`,
   and `45`: `noise`, `dx`, `dy`, displaced UV, source sample/hash, edge stats,
   and resolved evolution. This is the blocker before formula tuning.
3. Add adjustment-stack telemetry for `STK_030` showing per-effect time routing
   and canvas hashes around Geometry2, Posterize Time, Minimax, and Turbulent
   Displace. Coordinate with Agent 2 before changing Posterize semantics and
   with Agent 3 before treating Minimax output as settled.

No effect implementation files were changed in this pass.

# Agent 2 Temporal Graph And Motion Native Diff

Status date: 2026-05-03.

## Scope

Agent 2 covers `M02`, `M15`, `M16`, `M17`, and `M18`.

| Case | Modules | Frames |
| --- | --- | --- |
| `TMP_010` | `M02`, `M15` | 0, 5, 10, 15, 30, 45 |
| `TMP_020` | `M15` | 0, 1, 2, 3, 4, 5, 10, 11, 20, 21, 30, 31 |
| `TMP_030` | `M18` | 5, 10, 15, 20, 25, 30 |
| `STK_030` | `M12`, `M13`, `M14`, `M15`, `M16`, `M19` | 0, 1, 5, 10, 15, 20, 30, 45, 59 |
| `GPH_010` | `M17`, `M05`, `M19` | 0, 15, 30, 45 |

Native output:

```text
target/ae_agents/agent2_temporal_graph/
```

## Commands

Host cargo is unavailable:

```sh
cargo --version
# zsh:1: command not found: cargo
```

The successful native conformance run used Docker with explicit cargo `PATH`:

```sh
docker run --rm --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; export DEBIAN_FRONTEND=noninteractive; apt-get update; apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev libfreetype6-dev libharfbuzz-dev fontconfig; cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/agent2_temporal_graph --case TMP_010 --case TMP_020 --case TMP_030 --case STK_030 --case GPH_010'
```

Result:

```text
conformance-pack.done ok=true cases=5 report=target/ae_agents/agent2_temporal_graph/report.json
```

No thresholds were supplied, so each case status is `measured`; this is not an
AE parity pass.

## Top Metrics

Summary from `metrics.json`, sorted by overall mean diff:

| Case | Frames | Max abs | Mean abs | RMSE | Changed ratio | Elapsed ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `GPH_010` | 4 | 255 | 73.2563 | 134.9828 | 1.0000 | 2113.8494 |
| `TMP_030` | 6 | 255 | 63.2540 | 126.9352 | 0.9918 | 3757.5660 |
| `TMP_010` | 6 | 255 | 53.1250 | 116.3910 | 0.8333 | 1515.8043 |
| `STK_030` | 9 | 255 | 51.9165 | 107.3930 | 0.9702 | 23530.0065 |
| `TMP_020` | 12 | 255 | 47.8125 | 110.4182 | 0.7500 | 3757.5430 |

Alpha dominates most official metrics. A diagnostic RGB-only read of the same
native/AE PNGs shows the isolated temporal cases are much cleaner:

| Case | RGB max | RGB mean | RGB RMSE | RGB changed pixels | Alpha mean |
| --- | ---: | ---: | ---: | ---: | ---: |
| `TMP_010` | 0 | 0.000000 | 0.000000 | 0 | 212.500000 |
| `TMP_020` | 0 | 0.000000 | 0.000000 | 0 | 191.250000 |
| `TMP_030` | 132 | 0.105685 | 3.108858 | 2736 | 252.698771 |
| `STK_030` | 250 | 20.245413 | 54.002348 | 940336 | 146.929899 |
| `GPH_010` | 250 | 18.658145 | 64.921886 | 107504 | 237.050930 |

The shared substrate issue is the same one seen by other agents: native writes
background `[5, 5, 6, 255]` while AE goldens carry `[5, 5, 6, 0]` outside
rendered content. Agent 2 should not patch `M19`, but temporal diagnosis needs
alpha-normalized or masked metrics until that owner resolves it.

## Temporal Findings

### `TMP_010` / Source And Layer Time

Recipe:

```text
layer start = 0.25s
source_start = 0.75s
source_time = source_start + (comp_time - layer_start)
source frame = floor(source_time * 30fps)
```

Observed selected frames:

| Frame | Comp time | Expected state | Native/AE RGB |
| ---: | ---: | --- | --- |
| 0 | 0.0000 | inactive | match |
| 5 | 0.1667 | inactive | match |
| 10 | 0.3333 | source frame 25 | match |
| 15 | 0.5000 | source frame 30 | match |
| 30 | 1.0000 | source frame 45 | match |
| 45 | 1.5000 | source frame 60 | match |

`M02` layer/source-time mapping looks correct for these selected frames once the
background alpha channel is ignored.

### `TMP_020` / Posterize Buckets

Posterize Time is set to 6 fps in a 30 fps comp, so the expected bucket size is
5 comp frames. Native and AE RGB matched exactly on all selected frames.

| Frames | Bucket time | Expected source frame | Native/AE RGB |
| --- | ---: | ---: | --- |
| 0, 1, 2, 3, 4 | 0.0000 | 0 | match |
| 5 | 0.1667 | 5 | match |
| 10, 11 | 0.3333 | 10 | match |
| 20, 21 | 0.6667 | 20 | match |
| 30, 31 | 1.0000 | 30 | match |

This isolates `M15` layer/source quantization as healthy for the numbered-frame
case. No Posterize formula tuning is justified from `TMP_020`.

### `STK_030` / Adjustment Stack Time Routing

`STK_030` is the first Agent-2-owned divergence after alpha normalization.

The scene order is:

```text
Geometry2 -> Posterize Time 6fps -> Minimax -> Turbulent Displace
```

Native currently computes one `effects_time` for the whole adjustment layer,
re-renders the lower stack at that quantized time, then applies every effect
with that same time. The frame-pair probe exposes the problem:

| Pair | Native diff | AE diff |
| --- | --- | --- |
| `STK_030` frame 0 vs 1 | byte-identical, mean 0.000000 | max 255, mean 3.587873, RGB mean 2.389890, 59003 changed pixels |
| `TMP_020` frame 0 vs 1 | byte-identical | byte-identical |

Frames 0 and 1 are inside the same 6fps Posterize bucket. That is correct for
`TMP_020`, but in `STK_030` AE still changes between frames 0 and 1, most likely
because a downstream effect after Posterize Time, especially animated Turbulent
Displace evolution, is evaluated at comp time rather than the quantized bucket
time. Native appears to over-posterize the full adjustment output.

Suspected first Agent-2 divergent primitive: `M16` adjustment/effect-stack time
routing around `ADBE Posterize Time`, not the isolated `M15` bucket formula.

### `TMP_030` / Motion Blur

The generated scene enables composition motion blur and layer motion blur:

```text
samples = 8
shutter_angle = 180 deg
shutter_phase = -90 deg
frame_duration = 1 / 30
```

Native sample offsets relative to the rendered frame time are:

```text
-0.0072917s, -0.0052083s, -0.0031250s, -0.0010417s,
 0.0010417s,  0.0031250s,  0.0052083s,  0.0072917s
```

All samples currently use equal weight `1/8`. Official metrics are dominated by
background alpha (`alpha_mean=252.698771`). After ignoring alpha, the remaining
motion-blur signal is small but real: RGB mean `0.105685`, RGB max `132`, and
2736 RGB-changed pixels across six frames. Do not tune shutter/sample formulas
yet; first add per-sample telemetry with sample time, weight, evaluated matrix,
bbox, and premult/straight accumulation stats.

### `GPH_010` / Precomp Graph And Collapse

The native scene contains one child text comp and two root precomp layers:

```text
rasterized precomp: x=150, scale=180%, collapse_transformations=false
collapsed precomp:  x=362, scale=180%, collapse_transformations=true
```

The case is static; all selected frame metrics are identical. Both left and
right halves fail similarly, and a rough RGB ink bbox probe shows native text at
`y=228..291` while AE text is at `y=192..258` in both halves. That points first
to text/layout/rasterization and alpha substrate, not to a proven graph-order
failure.

Treat `GPH_010` text sharpness as shared with Agent 5. Agent 2 should only own
the graph/collapse telemetry: collapse mode, flattened layer list, child time,
parent matrix, child matrix, combined matrix, raster scale hint, and per-half
canvas hashes.

## Suspected First Divergent Primitive

Overall first observed primitive is the cross-agent `M19` background alpha
choice, because it fully explains `TMP_010` and `TMP_020` official diffs and
dominates `TMP_030` and `GPH_010`.

First Agent-2-owned primitive is the `STK_030` adjustment-layer time router:
native treats the quantized Posterize time as the time for the entire adjustment
effect stack, while AE evidence says downstream effects after Posterize Time can
still vary inside the same bucket.

## Next Patch Needed

1. Add temporal checkpoints to conformance output or `metrics.json`: comp time,
   layer time, source time, selected source frame id, Posterize fps, bucket id,
   bucket time, and per-effect input/output canvas hash.
2. Patch adjustment-stack telemetry before formulas: for each effect index,
   record original comp time, input resample time, effect param time, and output
   hash. Use `STK_030` frames 0 and 1 as the first regression probe.
3. After telemetry confirms AE ordering, split Posterize Time routing so the
   lower-stack/input resample can use bucket time without forcing all downstream
   effects to use the same bucket time. Validate against `TMP_020` so isolated
   layer/source quantization stays unchanged.
4. Add motion-blur telemetry for `TMP_030`: sample times, weights, per-sample
   transform matrix/bbox, skipped inactive samples, premult/straight accumulation
   hash, and final alpha policy.
5. Add graph/collapse telemetry for `GPH_010`, but leave text formula and
   sharpness ownership to Agent 5.

No renderer implementation files were changed in this pass.

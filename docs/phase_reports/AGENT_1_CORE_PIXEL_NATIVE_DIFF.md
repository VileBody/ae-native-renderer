# Agent 1 Core Pixel Native Diff

Date: 2026-05-03

Scope:

- `M01` timeline activity, z-order, opacity compositing
- `M03` transform matrix, anchor/position/scale/rotation sampling
- `M04` hold/linear/Bezier keyframe sampling
- `M19` color, alpha, sampling, gamma/matte assumptions

Cases rendered into:

```text
target/ae_agents/agent1_core_pixel/
```

Cases:

```text
PRI_010, CMP_010, INT_010, INT_020, EFF_040
```

`EFF_040` is treated here only as a coordinate/sampler probe for the core pixel
substrate. Geometry2 formula ownership remains with Agent 4.

## Commands

Host cargo was unavailable:

```sh
cargo --version
# zsh:1: command not found: cargo
```

Successful Docker command:

```sh
docker run --rm --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; apt-get update && apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev fontconfig && CARGO_TARGET_DIR=/tmp/ae_native_target cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/agent1_core_pixel --case PRI_010 --case CMP_010 --case INT_010 --case INT_020 --case EFF_040 && chown -R $(stat -c %u /work):$(stat -c %g /work) /work/target/ae_agents/agent1_core_pixel'
```

Result:

```text
conformance-pack.done ok=true cases=5 report=target/ae_agents/agent1_core_pixel/report.json
```

No thresholds were supplied, so `ok=true` means render/diff completed, not parity.

## Top Metrics

Case summaries from `report.json`:

| Case | Modules | Frames | Mean | RMSE | Max | Changed ratio |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `PRI_010` | `M01`, `M19` | 1 | 46.575 | 108.595 | 255 | 0.790 |
| `CMP_010` | `M19` | 1 | 52.209 | 112.325 | 255 | 0.998 |
| `INT_010` | `M04` | 6 | 62.457 | 126.001 | 255 | 0.986 |
| `INT_020` | `M04` | 7 | 63.626 | 127.197 | 255 | 0.998 |
| `EFF_040` | `M12` probe for `M03`/`M19` | 1 | 55.817 | 113.391 | 255 | 1.000 |

Top raw offending frames are dominated by alpha:

| Case/frame | Raw mean | RGB mean all | RGB mean content | Alpha mean | Suspected blocker |
| --- | ---: | ---: | ---: | ---: | --- |
| `INT_020` f0 | 63.750 | 0.000 | 0.000 | 255.000 | alpha/matte export |
| `INT_020` f59 | 63.747 | 0.000 | 0.034 | 254.987 | alpha/matte export |
| `INT_020` f5 | 63.745 | 0.074 | 5.541 | 254.759 | alpha/matte first, ease secondary |
| `INT_010` f30 | 62.744 | 0.110 | 4.274 | 250.646 | alpha/matte export |
| `EFF_040` f0 | 55.817 | 15.302 | 49.315 | 177.360 | alpha/matte first, matrix/UV secondary |
| `CMP_010` f0 | 52.209 | 5.862 | 23.450 | 191.250 | alpha/premult + opacity composite |
| `PRI_010` f0 | 46.575 | 0.529 | 1.902 | 184.715 | alpha/matte export |

## Diagnosis

The first divergent primitive is the exported alpha/matte model. Native scenes
currently serialize the comp background as `[5, 5, 6, 255]`, while the clean AE
goldens keep the same matte RGB with transparent alpha: top-left AE pixels are
`[5, 5, 6, 0]`. Because native starts with opaque alpha, every frame reports
full-frame alpha diffs up to `255`. This hides the useful per-module signal.

`PRI_010` confirms the source/layout substrate is mostly stable: quadrant sample
pixels match exactly at the probe centers, and the RGB content bbox differs only
by about one pixel. This does not look like a gross layer-order bug.

`CMP_010` has aligned RGB bboxes (`128,128` to `383,383`) and no centroid shift,
so z-order is probably not the first issue. After the alpha blocker, the
remaining RGB error is inside the stacked premult/opacity probe, so this should
be diagnosed as straight-vs-premult/matte compositing and opacity edge behavior.

`INT_010` linear/hold frame bboxes line up across all selected frames. The raw
max/mean failures are alpha-dominated; current evidence does not point to linear
keyframe timing as the first divergent primitive.

`INT_020` is also alpha-dominated, but after separating RGB there is a secondary
Bezier/ease signal: content centroid deltas are about `-5.5px` at frame 10,
`-8px` at frame 15, `0px` at frame 30, and `+9px` at frame 45. That matches the
existing recipe note that Bezier mapping is approximate.

`EFF_040` has a real coordinate-field problem after alpha is separated. Native
RGB content bbox is `(195,256)-(456,511)` while AE is `(192,192)-(511,511)`, with
centroid delta about `(-40.7,+13.2)`. This points to Geometry2 matrix/inverse UV,
pixel-center, edge, or sampler semantics. Agent 1 should not tune Geometry2
formula here, but this case confirms later coordinate work needs core sampler
telemetry.

## Next Patch Needed

Before formula tuning, add telemetry and/or a conformance-output patch that
splits matte RGB from alpha:

- log and compare per-channel metrics, especially RGB-only and alpha-only;
- record comp clear color, matte color, output alpha policy, and first/top-left
  pixel for every rendered conformance frame;
- compute native alpha against transparent output while preserving the AE matte
  RGB convention, or otherwise make the matte/export assumption explicit before
  changing renderer math;
- for `INT_020`, emit sampled position and opacity per selected frame, plus the
  Bezier/ease interpolation parameter used by native;
- for `EFF_040`, emit effect matrix, inverse matrix, destination pixel center,
  source UV, sampler mode, and edge behavior.

No renderer math was changed for this diagnostic pass.

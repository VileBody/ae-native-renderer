# Agent Round 3 Geometry2

Date: 2026-05-03.

Scope: `EFF_040` / `M12 Geometry2` only. `STK_030` was not tuned.

## Files Changed

- `crates/effects/src/geometry.rs`
- `docs/phase_reports/AGENT_ROUND3_GEOMETRY2.md`
- Fresh output under `target/ae_agents/round3_geometry2/`

## Evidence

Round 2 sidecar resolved:

```text
raw: 0001=[128,128], 0002=[256,256], 0003=82, 0004=120, 0008=72, rotation=17
resolved before: scale=[120,72], rotation=17
sampler: nearest_round
edge: transparent_out_of_bounds
```

AE/native image probes made the remaining error separable:

- AE alpha-255 coordinate fit was approximately
  `x = 0.796922*out_x + 0.243643*out_y - 271.557`,
  `y = -0.243643*out_x + 0.796923*out_y - 84.683`.
- Round 2 native fit was
  `x = 0.796909*out_x + 0.243638*out_y - 266.378`,
  `y = -0.406068*out_x + 1.328176*out_y - 236.056`.
- The X axis already matched `scale=120` and `rotation=17`; the Y axis matched
  AE after treating `0004` as the uniform scale value and not treating `0008`
  as scale height.
- The remaining translation matched Adobe's layer-space semantics: Transform
  effect control points are layer-space, so destination/source coordinates need
  the layer origin offset rather than comp origin.

Implemented corrections:

- `0008` is now a rotation fallback; named `rotation` still wins.
- numbered `0004` now resolves to uniform scale when uniform scale is enabled or
  only that numeric scale control is present.
- Geometry2 applies its matrix in layer-space using the first non-transparent
  input pixel as the layer-space origin for the current full-comp layer canvas.

## Metrics

Before: `target/ae_agents/round2_integrated_trace_smoke/EFF_040/metrics.json`.
After: `target/ae_agents/round3_geometry2/EFF_040/metrics.json`.

| Metric | Before | After |
| --- | ---: | ---: |
| `rgb.mean_abs_diff` | `8.9347279867` | `0.0676116943` |
| `rgb.rmse_abs_diff` | `33.8500307219` | `1.7056899049` |
| `rgb.changed_pixel_ratio` | `0.3099021912` | `0.0754013062` |
| `background_alpha_normalized.mean_abs_diff` | `7.0473718643` | `0.1453361511` |
| `background_alpha_normalized.rmse_abs_diff` | `30.6514529365` | `4.2593485170` |
| raw `mean_abs_diff` | `51.0410938263` | `44.3907566071` |
| raw `rmse_abs_diff` | `110.2629009925` | `106.3048490612` |

Shape evidence:

| Image | Before native bbox | After native bbox | AE bbox |
| --- | --- | --- | --- |
| RGB non-background | `(202,256)-(511,511)` | `(192,192)-(511,511)` | `(192,192)-(511,511)` |

After the patch, the alpha-255/content interior is effectively aligned:

```text
AE alpha255 + native content: mean RGB diff 0.0812, RMSE 0.2850, max 1
AE partial-alpha edge: mean RGB diff 14.3422, RMSE 31.0180, max 180
shape xor: 356 pixels
```

## Verification

```sh
docker run --rm ... rust:1-bookworm \
  sh -lc 'cargo test -p effects geometry -- --nocapture'
```

Result: `8 passed`.

```sh
docker run --rm ... rust:1-bookworm \
  sh -lc 'apt-get install ... gstreamer deps ...;
          CARGO_TARGET_DIR=/tmp/round3_geometry2_cargo_target
          cargo run -p render-cli -- conformance-pack
            --pack fixtures/ae_conformance_pack
            --out target/ae_agents/round3_geometry2
            --case EFF_040'
```

Result: `conformance-pack.done ok=true cases=1`.

## Next Blocker

The matrix/order/anchor-position convention is no longer the main `EFF_040`
blocker. Remaining RGB error is concentrated on partial-alpha/shape edge pixels,
while fully opaque interior pixels differ by at most `1`.

Do not tune `STK_030` from this pass. The next isolated Geometry2 work should
characterize AE edge antialiasing/background-alpha behavior and only then test
pixel-center or bilinear sampling. Current evidence does not justify changing
the sampler away from `nearest_round`.

Telemetry caveat: the existing render-cli sidecar does not expose the inferred
layer-space origin as its own field. I did not edit the conformance runner in
this block; if future debugging needs explicit origin/effective-comp matrices,
that should be a separate runner/telemetry change.

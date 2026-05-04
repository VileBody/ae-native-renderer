# Round 5 Text Animator Blur/Application

Scope: render-core text animator blur/application order for `TXT_030`, with `TXT_040`
checked for expression-selector regressions.

## Result

Implemented a focused render-core fix in `crates/render-core/src/layer_eval.rs`.

- Per-unit text animator blur now uses the weighted animator blur value in pixels.
- Removed the legacy `/8` shrink and `0..4` clamp that produced only `0/1` radii for `TXT_030` blur `[10, 10]`.
- Blur splats can now contribute when the transformed pixel center is just outside the canvas but the blur radius overlaps visible pixels.
- `TXT_040` output is unchanged; its blocker remains expression selector/application semantics.

No text-engine or effects code was changed.

## Evidence

Round 4 identified the actionable `TXT_030` issue:

- animator blur property: `[10.0, 10.0]`
- telemetry blur radii: only `0` or `1`
- frame 0 native bbox: `(0, 118, 512, 250)`
- frame 0 AE bbox: `(0, 106, 512, 204)`

The render-core path was:

```rust
((max_blur * weight * unit_scale) / 8.0).round().clamp(0, 4)
```

After this pass, `TXT_030` telemetry reports full weighted radii:

- frame 0 radius range: `1..10`
- frame 8 radius range: `0..9`
- frame 32 radius range: `0..10`
- frame 59 radius range: `0..0`

Frame 0 bbox moved toward AE:

| Source | Native bbox | AE bbox |
| --- | --- | --- |
| Before | `(0, 118, 512, 250)` | `(0, 106, 512, 204)` |
| After | `(0, 109, 512, 251)` | `(0, 106, 512, 204)` |

## Metrics

Before metrics are from `target/ae_agents/round4_final_integrated`.
After metrics are from `target/ae_agents/round5_text_animator_blur`.

| Case | Metric | Before | After |
| --- | --- | ---: | ---: |
| `TXT_030` | RGB RMSE | `34.99816861713527` | `31.691472943554803` |
| `TXT_030` | background-alpha-normalized RMSE | `43.819631249213835` | `44.62947023951768` |
| `TXT_030` | RGBA RMSE | `128.67979872658125` | `128.03553740425912` |
| `TXT_040` | RGB RMSE | `30.585968269602244` | `30.585968269602244` |
| `TXT_040` | background-alpha-normalized RMSE | `30.87372823366241` | `30.87372823366241` |
| `TXT_040` | RGBA RMSE | `129.4701491327945` | `129.4701491327945` |

`TXT_030` RGB improves on frames 0, 8, 16, 24, 32, and 45. Frame 59 is unchanged
because all selector weights are zero.

## Validation

Focused render-core tests:

```bash
docker run --rm -e HOST_UID=$(id -u) -e HOST_GID=$(id -g) \
  -v "$PWD:/work" -w /work rust:1-bookworm sh -lc \
  'set -e; export PATH=/usr/local/cargo/bin:$PATH; \
   export CARGO_HOME=/work/target/ae_agents/round5_text_animator_blur/cargo-home; \
   export CARGO_TARGET_DIR=/work/target/ae_agents/round5_text_animator_blur/cargo-target; \
   cargo test -p render-core text_animator -- --nocapture'
```

Result: `3 passed; 0 failed`.

Conformance:

```bash
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/round5_text_animator_blur \
  --case TXT_030 --case TXT_040
```

Result: `conformance-pack.done ok=true cases=2`.

The first conformance attempt in a clean `rust:1-bookworm` container failed before
running because current `render-cli` depends on `media-gst` and the container did
not have `gstreamer-1.0` development headers. The successful run installed
`pkg-config`, `libgstreamer1.0-dev`, and `libgstreamer-plugins-base1.0-dev`.

## Files Changed

- `crates/render-core/src/layer_eval.rs`
- `docs/phase_reports/AGENT_ROUND5_TEXT_ANIMATOR_BLUR.md`
- `target/ae_agents/round5_text_animator_blur/**`

## Next Blocker

`TXT_030` no longer has the blur radius clamp/order blocker. The remaining gap is
the blur kernel/composite approximation: RGB improved and top spread is close to
AE, but background-alpha-normalized RMSE rose slightly because the current splat
filter creates a broader alpha footprint than AE.

`TXT_040` remains blocked on render-core expression selector/application
semantics at frame 0. This pass intentionally did not change selector semantics.

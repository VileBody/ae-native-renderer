# Agent Round 2 Effects

Date: 2026-05-03

Scope: Box Blur, Drop Shadow, Glow, and Minimax diagnosis. No formula tuning was
landed in this pass.

## Commands

Focused baseline conformance:

```text
docker run --rm --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm -lc '
  export PATH=/usr/local/cargo/bin:$PATH
  apt-get update
  apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev libfreetype6-dev libharfbuzz-dev fontconfig
  export CARGO_TARGET_DIR=/tmp/ae_native_renderer_cargo_target
  cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round2_effects --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_050 --case EFF_070
'
```

Result:

```text
conformance-pack.done ok=true cases=5 report=target/ae_agents/round2_effects/report.json
```

Sidecar rerun after wiring the hook was attempted with the same cases and output
directory, using `render_frame_with_footage_traced`:

```text
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/round2_effects \
  --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_050 --case EFF_070
```

It is currently blocked by pre-existing incomplete render-core text/collapse
telemetry helpers:

```text
cannot find function `record_rasterized_precomp_trace`
cannot find function `record_collapsed_precomp_trace`
cannot find function `record_collapsed_text_raster_trace`
cannot find function `text_expression_weight_detail`
cannot find function `animator_contribution_json`
```

Targeted effects tests:

```text
cargo test -p effects box_blur
cargo test -p effects glow
cargo test -p effects drop_shadow
cargo test -p effects minimax
cargo test -p effects
```

Result: all passed. Full `effects` suite: 30 passed.

## Metrics

Aggregate Round 2 metrics from
`target/ae_agents/round2_effects/report.json`:

| Case | Frames | RGB max | RGB mean | RGB RMSE | RGB changed | BG alpha norm max | BG alpha norm mean | BG alpha norm RMSE | BG alpha norm changed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| EFF_010 | 1 | 116 | 0.1635 | 3.3191 | 2.2800% | 255 | 1.2643 | 16.2588 | 2.3739% |
| EFF_020 | 1 | 137 | 12.6578 | 32.1739 | 27.3201% | 254 | 15.3025 | 45.0600 | 28.1830% |
| EFF_030 | 1 | 0 | 0.0000 | 0.0000 | 0.0000% | 0 | 0.0000 | 0.0000 | 0.0000% |
| EFF_050 | 1 | 250 | 3.5333 | 29.3622 | 1.4782% | 250 | 2.6674 | 25.4462 | 1.4782% |
| EFF_070 | 6 | 126 | 1.8913 | 10.2863 | 8.1004% | 254 | 2.8430 | 19.5030 | 8.2296% |

Selected frame metrics:

| Case | Frame | Time | RGB max | RGB mean | BG alpha norm max | BG alpha norm mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| EFF_010 | 0 | 0.0000 | 116 | 0.1635 | 255 | 1.2643 |
| EFF_020 | 0 | 0.0000 | 137 | 12.6578 | 254 | 15.3025 |
| EFF_030 | 0 | 0.0000 | 0 | 0.0000 | 0 | 0.0000 |
| EFF_050 | 0 | 0.0000 | 250 | 3.5333 | 250 | 2.6674 |
| EFF_070 | 30 | 1.0000 | 126 | 1.9079 | 253 | 2.8888 |

## Sidecars

The hook is implemented narrowly:

- `FrameRenderTrace.effect_debug` stores one record per traced effect.
- `render-core` calls the existing debug helpers for Box Blur, Drop Shadow,
  Glow, and Minimax.
- `render-cli conformance-pack` writes records as deterministic JSON under
  `effects_debug/<case>/<frame>/<layer>_<effect-index>_<effect>.json`.
- Frame reports include the emitted sidecar paths in `effects_debug`.

No sidecar files were produced in this run because the traced conformance rerun
is blocked by the unrelated render-core missing-helper errors above.

## Diagnosis

- `EFF_010` Drop Shadow: RGB error is low but nonzero. The next isolated
  evidence should compare native sidecar `dx/dy`, raw offset shadow, and blurred
  shadow. Direction/offset remains the safest tiny formula target, but it is not
  proven yet.
- `EFF_020` Glow: RGB and background-alpha-normalized error remain high. The
  likely first divergent primitive is threshold source or alpha participation.
  Do not broad-tune radius/intensity from final pixels.
- `EFF_030` Box Blur: RGB and background-alpha-normalized are exact for this
  fixture. No formula patch is indicated from this case.
- `EFF_050` Minimax: Small changed area with high max/RMSE. Operation/channel
  enum and neighborhood semantics remain suspect, but sidecar evidence is still
  needed before mapping changes.
- `EFF_070` animated BoxBlur2/Glow: per-frame RGB evolves, so time-aware params
  remain active. Frame 30 should be the first sidecar comparison point for the
  animated Glow branch.

## Next Target

No effect is ready for formula tuning yet because first divergent intermediates
were not captured. Once the render-core helper blocker is cleared and sidecars
are generated, start with Drop Shadow direction/offset if `raw_offset_shadow_rgba`
is the first mismatch; otherwise Minimax enum/channel mapping is the next tiny
candidate. Glow should wait for threshold/mask sidecar evidence.

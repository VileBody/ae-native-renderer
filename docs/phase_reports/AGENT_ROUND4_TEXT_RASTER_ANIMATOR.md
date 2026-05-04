# Round 4 Text Raster / Animator Investigation

Scope: M05-M09 glyph raster/coverage plus per-glyph animator order for `TXT_030` and `TXT_040`.

## Result

No text-engine code changes were made. The first actionable mismatch is outside the owned text-engine scope:

- `TXT_030` points to render-core per-glyph animator blur/application, not glyph layout metrics or text-engine selector units.
- `TXT_040` frame 0 still points to render-core expression-selector semantics/application. Text-engine selector semantics were not changed.

## Evidence

Round 3 telemetry source:

- `target/ae_agents/round3_final_integrated/TXT_030/text_telemetry.jsonl`
- `target/ae_agents/round3_final_integrated/TXT_040/text_telemetry.jsonl`
- `target/ae_agents/round3_final_integrated/TXT_030/metrics.json`
- `target/ae_agents/round3_final_integrated/TXT_040/metrics.json`

`TXT_030` layout is stable and matches the accepted Round 3 placement assumptions. The `GLYPH MOTION` line is overfull and centered, with expected clipped edge glyphs:

- first glyph bbox starts at `x=-34.026`, baseline `256.0`
- last glyph extends past `x=512`
- frame 59 selector weights are all zero, so only base raster remains

Image bbox check against Round 3 output:

- `TXT_030` frame 0 native non-background bbox: `(0, 118, 512, 250)`
- `TXT_030` frame 0 AE non-background bbox: `(0, 106, 512, 204)`
- telemetry for frame 0 reports `blur_radius` values only `0` or `1`, despite the animator blur property being `[10.0, 10.0]`

That top-edge delta is approximately the missing 10px blur spread. The current runtime blur behavior is in `crates/render-core/src/layer_eval.rs`, where per-unit animator blur is converted to a small splat radius and clamped. `crates/text-engine/src/text_animator.rs::plan_blur_animator` keeps the full weighted blur values, but render-core is not using that plan for the final raster.

`TXT_040` frame 0 telemetry reports expression selector weights all `0.0`, while AE frame 0 is fully transparent/empty and later frames reveal the bounce. That is selector/application behavior in render-core, not basic glyph layout or raster coverage.

## Before Metrics

From `target/ae_agents/round3_final_integrated`:

- `TXT_030` summary `rgb.rmse_abs_diff`: `34.99816861713527`
- `TXT_030` summary `background_alpha_normalized.rmse_abs_diff`: `43.819631249213835`
- `TXT_040` summary `rgb.rmse_abs_diff`: `30.585968269602244`
- `TXT_040` summary `background_alpha_normalized.rmse_abs_diff`: `30.87372823366241`

## After Metrics

No after conformance run was produced because no evidence-backed text-engine code change was made. Re-running the same render-core path would reproduce the same blocker while adding noise.

## Validation

Ran text-engine tests through Docker because local macOS PATH has no `cargo`, while existing repo binaries are Linux ELF:

```bash
docker run --rm -e HOST_UID=$(id -u) -e HOST_GID=$(id -g) \
  -v "$PWD:/work" -w /work rust:1-bookworm sh -lc \
  'set -e; export PATH=/usr/local/cargo/bin:$PATH; \
   export CARGO_HOME=/work/target/ae_agents/round4_text_raster_animator/cargo-home; \
   export CARGO_TARGET_DIR=/work/target/ae_agents/round4_text_raster_animator/cargo-target; \
   cargo test -p text-engine -- --nocapture; \
   chown -R "$HOST_UID:$HOST_GID" /work/target/ae_agents/round4_text_raster_animator'
```

Result: `13 passed; 0 failed`.

## Next Blocker

The next fix should be owned by render-core:

- verify AE text animator blur is applied at full pixel radius and in the correct order relative to per-glyph transform/rasterization
- verify expression selector frame-0 and negative-weight application semantics for `TXT_040`

Do not change text-engine selector semantics unless render-core is changed to consume text-engine selector evaluation directly and the parity evidence is re-collected.

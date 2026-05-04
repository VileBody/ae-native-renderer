# Round 5 Collapse Deferred-Raster Contract

Scope: collapse/deferred-raster contract prep for `GPH_010`, without pixel changes.

## Decision

No render-path or pixel fix was made.

This pass adds a narrow graph/precomp-facing contract that separates deferred
vector/text primitive candidates from raster barriers. The contract is explicit
about the transform accumulation order and source-time rule the future renderer
must honor, while leaving actual text/vector rasterization in `layer_eval.rs`.

## Design

`PrecompGraph::deferred_raster_plan_for_precomp_layer` now returns
`PrecompDeferredRasterPlan` for a requested precomp layer.

The plan contains:

- `deferred_primitives`: solid/text candidates that can be rasterized after the
  final parent-space matrix is known.
- `raster_barriers`: layer-specific fallback blockers for footage, adjustment
  layers, effects, missing targets, cycles, and non-collapsed nested precomps.
- `transform_steps`: ordered matrix inputs:
  root precomp boundary, nested collapsed precomp boundaries, then source layer.
- `source_time_steps`: ordered precomp boundary timing rules:
  `max(parent_time - layer.start, 0.0)` for each crossed precomp boundary.
- `active_window` and `opacity`: source layer active-window and opacity
  evaluation expectation at final source time.

This is intentionally a contract/diagnostic layer. `can_defer_all_primitives`
only becomes true when collapse was requested, all candidates are vector/text,
and no raster barriers exist.

## Files Changed

- `crates/render-core/src/precomp.rs`
  - Added deferred-raster contract types.
  - Added `PrecompGraph::deferred_raster_plan_for_precomp_layer`.
  - Added barrier collection with layer-specific reasons.
  - Added targeted deferred contract tests.
- `crates/render-core/src/graph.rs`
  - Added `precomp_deferred_raster_plan` wrapper for scene callers.
- `crates/render-core/src/composition.rs`
  - Formatting-only change already present in this worktree.

No edits were made to `crates/render-core/src/layer_eval.rs`.

## Tests

Commands were run in Docker because local `cargo` is not in `PATH`.

```sh
docker run --rm -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/work/target/ae_agents/round5_collapse_contract/cargo-target \
  rust:1-bookworm cargo test -p render-core precomp -- --nocapture

docker run --rm -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/work/target/ae_agents/round5_collapse_contract/cargo-target-collapse \
  rust:1-bookworm cargo test -p render-core collapse -- --nocapture

docker run --rm -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/work/target/ae_agents/round5_collapse_contract/cargo-target-deferred \
  rust:1-bookworm cargo test -p render-core deferred_contract -- --nocapture

docker run --rm -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/work/target/ae_agents/round5_collapse_contract/cargo-target-cli \
  ae-native-renderer:round2-dev cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/round5_collapse_contract \
  --case GPH_010
```

Results:

- `cargo test -p render-core precomp -- --nocapture`: 17 passed.
- `cargo test -p render-core collapse -- --nocapture`: 7 passed.
- `cargo test -p render-core deferred_contract -- --nocapture`: 3 passed.
- `conformance-pack --case GPH_010`: ok, 1 case rendered, 0 failures.

`GPH_010` output:

- report: `target/ae_agents/round5_collapse_contract/report.json`
- metrics: `target/ae_agents/round5_collapse_contract/GPH_010/metrics.json`
- collapse telemetry: `target/ae_agents/round5_collapse_contract/GPH_010/collapse_telemetry.jsonl`
- text telemetry: `target/ae_agents/round5_collapse_contract/GPH_010/text_telemetry.jsonl`

Round 5 `GPH_010` metrics remained at the Round 4 diagnostic level:

| Metric | Mean | Changed pixels |
| --- | ---: | ---: |
| RGB | 15.707040 | 89852 |
| BG-alpha-normalized | 12.757180 | 89868 |

## What Remains For `layer_eval`

The render path still needs to consume this contract before any parity pixel
claim can be made:

1. Build renderable deferred primitives from the contract instead of
   rediscovering collapse eligibility locally.
2. Evaluate each transform step at its specified source time and multiply in the
   contract order.
3. Pass the final effective matrix and sampling scale explicitly into text/vector
   rasterization.
4. Keep text layout in source units unless the text engine has an explicit
   scale-aware layout mode that preserves paragraph metrics.
5. Emit telemetry per deferred primitive with source time, transform steps,
   effective matrix, raster scale, raster size, and fallback barrier reason.

Until those render-path pieces land, `GPH_010` remains a diagnostic collapse
case rather than a pixel-parity fix.

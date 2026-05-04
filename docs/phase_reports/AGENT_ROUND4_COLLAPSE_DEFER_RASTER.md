# Round 4 Collapse Deferred Raster Diagnostic

Scope: true collapse/deferred text-vector raster diagnostic for `GPH_010`.

## Decision

No graph/precomp-only pixel fix was made.

`crates/render-core/src/precomp.rs` and `crates/render-core/src/graph.rs` can already express the current collapse plan shape: a collapsed precomp boundary returns flattened `Solid`/`Text` layer references plus any nested collapsed precomp path. The remaining parity gap requires the pixel renderer contract in `layer_eval.rs` and, for final parity, text glyph coverage/layout behavior in `text-engine`. Per the round gate, this pass stops at the implementation contract instead of broad edits outside the owned block.

## Evidence

Round 3 final integrated telemetry:

- `target/ae_agents/round3_final_integrated/GPH_010/collapse_telemetry.jsonl`
- `target/ae_agents/round3_final_integrated/GPH_010/text_telemetry.jsonl`
- `target/ae_agents/round3_final_integrated/GPH_010/metrics.json`

The telemetry shows plausible matrix propagation:

- rasterized precomp matrix scale `1.8`, translation `[-310.8, -204.8]`, raster size `512x512`;
- collapsed precomp parent matrix scale `1.8`, translation `[-98.8, -204.8]`;
- collapsed text effective matrix scale `1.8`, effective raster scale `1.8000030517578125`;
- collapsed text raster size `922x922`;
- text telemetry records both `layer_text` at scale `1.0` and `collapsed_text` at scale `1.8000030517578125`.

Fresh Round 4 conformance output:

- output directory: `target/ae_agents/round4_collapse`
- report: `target/ae_agents/round4_collapse/report.json`
- metrics: `target/ae_agents/round4_collapse/GPH_010/metrics.json`
- collapse telemetry: `target/ae_agents/round4_collapse/GPH_010/collapse_telemetry.jsonl`
- text telemetry: `target/ae_agents/round4_collapse/GPH_010/text_telemetry.jsonl`

Round 4 `GPH_010` metrics:

| Frame | RGB mean | RGB changed | BG-alpha-normalized mean | BG-alpha-normalized changed |
| --- | ---: | ---: | ---: | ---: |
| 0 | 15.707040 | 22463 | 12.757180 | 22467 |
| 15 | 15.707040 | 22463 | 12.757180 | 22467 |
| 30 | 15.707040 | 22463 | 12.757180 | 22467 |
| 45 | 15.707040 | 22463 | 12.757180 | 22467 |

Sidecar counts:

- `collapse_telemetry.jsonl`: 12 records, 3 per frame.
- `text_telemetry.jsonl`: 8 records, 2 per frame.

## Current Representation

`PrecompGraph::collapse_plan_for_precomp_layer` returns a `PrecompCollapsePlan` from `collapse.rs`. The plan can say:

- whether collapse was requested;
- whether the target can flatten as supported vector/text layers;
- which source layer refs participate;
- which nested collapsed precomp boundaries are crossed;
- which issues require rasterization first.

That is enough for planning and validation, but it is not a full deferred raster render contract. The current runtime path in `layer_eval.rs` independently:

- detects collapsible precomps with `can_collapse_composition`;
- composes parent and child matrices;
- rasterizes text at a scale hint derived from the effective matrix;
- records collapsed text raster telemetry.

So graph/precomp can describe "flatten these layers"; only the renderer can make "rasterize this vector/text primitive after final matrix, using this output sampling/coverage contract" real.

## Required Deferred Raster Contract

A true implementation should introduce an explicit render-facing contract before changing pixels:

1. Collapse planning returns renderable deferred primitives, not only borrowed layer refs.
2. Each deferred primitive carries:
   - source composition id and layer id;
   - source time after precomp layer timing;
   - accumulated precomp boundary transforms;
   - evaluated child transform;
   - final effective matrix in parent/output space;
   - opacity and active-window status at source time;
   - effect/raster barriers that force fallback to rasterize-first.
3. Text/vector rasterization receives the final effective matrix and output sampling scale as an explicit input, not as an internal guess.
4. Text layout remains in layer/source units unless the text engine explicitly supports scale-aware layout without changing paragraph metrics; glyph coverage is rasterized at final scale.
5. Telemetry records one deferred primitive per collapsed child with parent matrix, child matrix, effective matrix, raster scale, raster size, source time, and fallback reason if any.
6. Tests assert both the plan contract and pixel behavior:
   - vector/text-only precomp produces deferred primitives;
   - nested collapsed precomp accumulates transforms in order;
   - non-collapsed nested precomp, footage, adjustment, and effects produce raster barriers;
   - text collapse rasterizes at final effective scale while preserving source-space layout metrics;
   - `GPH_010` sidecars contain the expected 3 collapse records and 2 text records per frame.

## Risks

- A graph/precomp-only change can improve diagnostics but cannot guarantee AE parity because text rasterization, glyph coverage, and final sampling live outside the graph model.
- The current `matrix_scale_hint(...).clamp(1.0, 4.0)` behavior is runtime policy, not a graph contract.
- Scale-aware text layout may accidentally alter paragraph wrapping/metrics if font size and text box are both scaled without a clear source-space vs raster-space split.
- Effects and adjustment layers need explicit raster barriers to avoid accidentally flattening through an offscreen dependency.

## Verification

Local host did not have `cargo` in `PATH`, so verification was run in Docker.

Commands:

```sh
docker run --rm -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/work/target/ae_agents/round4_collapse/cargo-target rust:1-bookworm cargo test -p render-core precomp -- --nocapture
docker run --rm -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/work/target/ae_agents/round4_collapse/cargo-target-collapse rust:1-bookworm cargo test -p render-core collapsed_precomp_flattens_solid_with_parent_transform -- --nocapture
docker run --rm -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/work/target/ae_agents/round4_collapse/cargo-target-collapse-all rust:1-bookworm cargo test -p render-core collapse -- --nocapture
docker run --rm -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/work/target/ae_agents/round4_collapse/cargo-target-cli ae-native-renderer:round2-dev cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round4_collapse --case GPH_010
```

Results:

- `cargo test -p render-core precomp -- --nocapture`: 14 passed.
- `cargo test -p render-core collapsed_precomp_flattens_solid_with_parent_transform -- --nocapture`: 1 passed.
- `cargo test -p render-core collapse -- --nocapture`: 7 passed.
- `conformance-pack --case GPH_010`: ok, 1 case rendered, 0 failures.

## Files Changed

- `docs/phase_reports/AGENT_ROUND4_COLLAPSE_DEFER_RASTER.md`

No source edits were made in this pass.

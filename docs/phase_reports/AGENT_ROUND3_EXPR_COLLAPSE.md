# Agent Round3 Expression Collapse

Status date: 2026-05-03.

## Scope

Round 3 focused on M17 expression evaluator traits and collapse-transform
diagnostics using the Round 2 integrated trace smoke outputs:

- `target/ae_agents/round2_integrated_trace_smoke/EXP_010/expression_telemetry.jsonl`
- `target/ae_agents/round2_integrated_trace_smoke/GPH_010/collapse_telemetry.jsonl`

The only runtime code changed by this block is
`crates/expression-engine/src/evaluator.rs`. I did not edit `text-engine` or
`render-core/src/layer_eval.rs`.

## External Semantics Guard

References used as guardrails:

- https://ae-expressions.docsforadobe.dev/objects/comp/
- https://ae-expressions.docsforadobe.dev/layer/general/
- https://ae-expressions.docsforadobe.dev/layer/sub-objects/
- https://ae-expressions.docsforadobe.dev/layer/layer-space-transforms/
- https://ae-expressions.docsforadobe.dev/objects/effect/
- https://helpx.adobe.com/after-effects/using/precomposing-nesting-pre-rendering.html

Applied constraints:

- `thisComp.width`, `thisComp.height`, `thisComp.duration`, and
  `thisComp.frameDuration` stay scalar context variables.
- `thisLayer.inPoint` and `thisLayer.outPoint` stay scalar layer timing values.
- `value + [x, y]` remains the supported Vec2 property-expression shape for
  EXP_010-style position expressions.
- `thisComp.layer(...)`, transform sub-objects, effect point controls, and
  layer-space conversion methods are intentionally not implemented without a
  template or telemetry case proving the needed subset.
- Effect control points are layer-space values if/when a supported expression
  samples them.

## Expression Evidence And Fix

The AE fixture source for `EXP_010` is a generated expression subset:

```js
intro = 0.25; outro = 0.25; amp = 34; freq = 2.0;
edge = Math.min(time - inPoint, outPoint - time);
env = Math.max(0, Math.min(1, edge / intro));
value + [Math.sin(time * freq * 2 * Math.PI) * amp * env, 0];
```

Round 2 telemetry already records the native named `edge_wobble` samples, but
the generic evaluator could not evaluate the assignment-statement form of the
same supported pattern. The evaluator already had the necessary scalar/Vec2
math, `Math.min/max/sin`, `time`, `inPoint`, `outPoint`, and `value` coercion.

Implemented low-risk fix:

- added parser-local variables;
- added semicolon-separated statement evaluation;
- added simple `name = expression` assignment statements;
- added a unit test for the exact EXP_010-style expression subset.

This is not arbitrary JavaScript support.

## Collapse Evidence

Round 2 `GPH_010` collapse telemetry shows:

- rasterized precomp matrix: scale 1.8, translation `[-310.8, -204.8]`,
  raster size `512x512`;
- collapsed precomp parent matrix: scale 1.8, translation `[-98.8, -204.8]`;
- child text matrix: identity;
- effective matrix: same as parent matrix;
- effective raster scale: `1.800003`;
- collapsed text raster size: `922x922`;
- sharpness probe: `alpha_edge_max=255`, `alpha_edge_mean=0.417653`,
  `alpha_coverage_ratio=0.003835`.

Diagnosis:

- Matrix propagation is not the current blocker for `GPH_010`.
- Rasterization scale is not the current blocker; the scale-aware approximation
  is active and reports the expected 1.8x raster.
- Remaining blocker is true collapse/defer-rasterization parity for text/vector
  rendering, plus text raster/layout parity. Adobe's collapse documentation says
  nested and containing transformations are combined and performed together
  instead of flattening/cropping; the current native path still rasterizes text
  into an intermediate bitmap in `layer_eval.rs`.

Because the required work would touch `layer_eval.rs` and/or `text-engine`, this
block stops at diagnostics rather than making broad edits outside scope.

## Metrics

Metrics below use `metrics.rgb` and `metrics.background_alpha_normalized`, not
raw RGBA. Round 3 was run against the current dirty worktree, where other agents
may have changed non-owned files.

| Case | Metric | Round 2 mean | Round 2 changed | Round 3 mean | Round 3 changed |
| --- | --- | ---: | ---: | ---: | ---: |
| `EXP_010` | RGB | 1.304101 | 12838 | 1.304101 | 12838 |
| `EXP_010` | BG-alpha-normalized | 1.188759 | 12838 | 1.188759 | 12838 |
| `GPH_010` | RGB | 18.658145 | 107504 | 15.707040 | 89852 |
| `GPH_010` | BG-alpha-normalized | 16.043449 | 107524 | 12.757180 | 89868 |

Round 3 outputs:

- `target/ae_agents/round3_expr_collapse/report.json`
- `target/ae_agents/round3_expr_collapse/EXP_010/metrics.json`
- `target/ae_agents/round3_expr_collapse/GPH_010/metrics.json`
- `target/ae_agents/round3_expr_collapse/EXP_010/expression_telemetry.jsonl`
- `target/ae_agents/round3_expr_collapse/GPH_010/collapse_telemetry.jsonl`

Sidecar counts:

| Case | Expression records | Collapse records | Text records |
| --- | ---: | ---: | ---: |
| `EXP_010` | 8 | 0 | 0 |
| `GPH_010` | 0 | 12 | 8 |

`GPH_010` improved in the current Round 3 worktree, but this block did not edit
the text or collapse runtime path. Treat the improvement as current-worktree
evidence, not as a collapse implementation change from this block.

## Commands

Local `cargo` was not in PATH, so validation used Docker with the official Rust
image and an explicit Cargo PATH.

```sh
docker run --rm -v "$PWD:/work" -w /work rust:1-bookworm sh -lc '
set -eu
export PATH=/usr/local/cargo/bin:$PATH
export CARGO_HOME=/work/target/ae_agents/cargo-home
export CARGO_TARGET_DIR=/work/target/ae_agents/round3_expr_collapse/cargo-target
cargo test -p expression-engine -- --nocapture
'
```

Result: 7 passed.

```sh
docker run --rm -v "$PWD:/work" -w /work rust:1-bookworm sh -lc '
set -eu
export PATH=/usr/local/cargo/bin:$PATH
export CARGO_HOME=/work/target/ae_agents/cargo-home
export CARGO_TARGET_DIR=/work/target/ae_agents/round3_expr_collapse/cargo-target
cargo test -p render-core collapsed_precomp_flattens_solid_with_parent_transform -- --nocapture
cargo test -p render-core precomp -- --nocapture
'
```

Results:

- `collapsed_precomp_flattens_solid_with_parent_transform`: 1 passed.
- `precomp`: 14 passed.

```sh
docker run --rm -v "$PWD:/work" -w /work rust:1-bookworm sh -lc '
set -eu
export PATH=/usr/local/cargo/bin:$PATH
export CARGO_HOME=/work/target/ae_agents/cargo-home
export CARGO_TARGET_DIR=/work/target/ae_agents/round3_expr_collapse/cargo-target
apt-get update >/dev/null
apt-get install -y --no-install-recommends fontconfig pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev >/dev/null
cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round3_expr_collapse --case EXP_010 --case GPH_010
'
```

Result: `conformance-pack.done ok=true cases=2`.

## Files Changed

- `crates/expression-engine/src/evaluator.rs`
- `docs/phase_reports/AGENT_ROUND3_EXPR_COLLAPSE.md`
- `target/ae_agents/round3_expr_collapse/`

Inspected but not intentionally changed by this block:

- `crates/render-core/src/precomp.rs`
- `crates/render-core/src/graph.rs`
- `crates/render-core/src/composition.rs`
- `crates/render-core/src/layer_eval.rs`

## Next Blocker

For expressions, the next safe step is wiring the exact supported EXP_010
assignment subset into property-expression telemetry/runtime where appropriate,
without expanding to general ExtendScript.

For collapse, the next blocker is a true deferred text/vector raster path or a
clear renderer-level contract for when collapsed text remains vector-like until
the final parent matrix is known. That work likely crosses into
`layer_eval.rs` and `text-engine`, so it should be owned separately from this
diagnostic pass.

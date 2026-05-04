# OS Round 5 Critic Board

Status date: 2026-05-04.

Goal: unblock remaining AE parity work by generating probe packs where final
goldens are underdetermined, while separately attacking render-core blockers
that already have enough evidence.

## Active Agents

| Agent | Block | Output |
| --- | --- | --- |
| Pauli | Glow / Drop Shadow AE probe pack | `fixtures/ae_probe_pack/glow_shadow/`, `AGENT_ROUND5_GLOW_SHADOW_PROBE_PACK.md` |
| Arendt | Minimax AE enum/channel probe pack | `fixtures/ae_probe_pack/minimax/`, `AGENT_ROUND5_MINIMAX_PROBE_PACK.md` |
| Meitner | Turbulent coordinate-field probe pack | `fixtures/ae_probe_pack/turbulent_field/`, `AGENT_ROUND5_TURBULENT_FIELD_PROBE_PACK.md` |
| Averroes | Text animator blur/application in render-core | `AGENT_ROUND5_TEXT_ANIMATOR_BLUR.md` |
| Russell | Collapse deferred-raster contract | `AGENT_ROUND5_COLLAPSE_CONTRACT.md` |

## Baseline

Round 4 established that these cases are blocked without new evidence:

| Case | RGB mean | Blocker |
| --- | ---: | --- |
| `EFF_020` | 12.6578 | Glow threshold/mask/composite intermediate |
| `EFF_050` | 3.5333 | Minimax operation/channel enum mapping |
| `EFF_060` | 2.8989 | Turbulent field/noise basis |
| `TXT_030` | 8.1287 | render-core per-glyph blur/application |
| `TXT_040` | 4.2506 | render-core expression selector/application |
| `GPH_010` | 15.7070 | deferred text/vector raster contract |
| `STK_030` | 3.6281 | composed regression target after primitives |

## Acceptance Rules

Probe packs are accepted if they are:

- self-contained under `fixtures/ae_probe_pack/<name>/`;
- documented with render/export instructions;
- explicit about inputs, AE properties, frames, and output measurements;
- not falsely wired into native conformance before AE goldens exist.

Render-core patches are accepted if they have:

- focused tests;
- before/after metrics for the relevant native-vs-AE case;
- telemetry explaining the changed behavior;
- no broad rewrites outside the assigned scope.

## No-Go Rules

- Do not tune Glow/Minimax/Turbulent formulas until AE probes exist.
- Do not modify existing clean conformance goldens or `ae_conformance_pack`
  unless the task explicitly owns that migration.
- Do not solve collapse with a graph-only pixel hack.
- Do not move text-engine placement/raster logic unless render-core evidence
  proves it is necessary.

## Expected Outputs

Round 5 should produce:

- AE probe packs ready for the user to render in After Effects;
- at least one render-core attempt for text blur/application;
- a collapse contract/test that narrows the eventual render-path work;
- final focused conformance run showing no regressions.

## Agent Results

### Pauli / Glow Shadow Probe Pack

Status: completed, probe pack accepted pending final validation.

Files:

```text
fixtures/ae_probe_pack/glow_shadow/README.md
fixtures/ae_probe_pack/glow_shadow/manifest.json
fixtures/ae_probe_pack/glow_shadow/jsx/build_glow_shadow_probe_project.jsx
docs/phase_reports/AGENT_ROUND5_GLOW_SHADOW_PROBE_PACK.md
```

Coverage: Glow source/mask/blurred/scaled/final, alpha/luma split variants, Drop
Shadow source/raw/no-softness/shadow-only/blurred/final, and manual raw-offset
hypotheses.

### Arendt / Minimax Probe Pack

Status: completed, probe pack accepted pending final validation.

Files:

```text
fixtures/ae_probe_pack/minimax/README.md
fixtures/ae_probe_pack/minimax/manifest.json
fixtures/ae_probe_pack/minimax/jsx/build_minimax_probe_project.jsx
fixtures/ae_probe_pack/minimax/scripts/measure_minimax_probe.py
docs/phase_reports/AGENT_ROUND5_MINIMAX_PROBE_PACK.md
```

Coverage: operation values `1/2`, channel values `1/2`, radius `0/12`, explicit
EFF_050 tuple, property metadata dump, bbox and row-sample measurement helper.

### Meitner / Turbulent Field Probe Pack

Status: completed, probe pack accepted pending final validation.

Files:

```text
fixtures/ae_probe_pack/turbulent_field/README.md
fixtures/ae_probe_pack/turbulent_field/manifest.json
fixtures/ae_probe_pack/turbulent_field/scripts/generate_assets.py
fixtures/ae_probe_pack/turbulent_field/jsx/build_turbulent_field_probe_project.jsx
fixtures/ae_probe_pack/turbulent_field/assets/primitives/
docs/phase_reports/AGENT_ROUND5_TURBULENT_FIELD_PROBE_PACK.md
```

Coverage: coordinate field, impulse grid, hard-edge/alpha ramp, unique
checkerboard; amount/size/complexity/evolution/displacement-type/seed/pinning/
resize/sampler probe suites.

### Averroes / Text Animator Blur

Status: completed, render-core patch accepted pending final integrated test.

Report:

```text
docs/phase_reports/AGENT_ROUND5_TEXT_ANIMATOR_BLUR.md
```

Patch: remove legacy text animator blur `/8` shrink and tiny clamp, and allow
blur splats to contribute when the pixel center is just off-canvas but the blur
radius overlaps the output.

Validation:

```text
cargo test -p render-core text_animator -- --nocapture
3 passed
conformance TXT_030/TXT_040: 2 cases, 0 failures
```

Metrics:

| Case | Metric | Before | After |
| --- | --- | ---: | ---: |
| `TXT_030` | RGB mean | 8.1287 | 7.7574 |
| `TXT_030` | RGB RMSE | 34.9982 | 31.6915 |
| `TXT_030` | BG-norm mean | 10.7500 | 11.4010 |
| `TXT_030` | BG-norm RMSE | 43.8196 | 44.6295 |
| `TXT_040` | RGB/BG-norm | unchanged | unchanged |

Decision: accept if final integrated test stays clean. This removes a known
artificial clamp and moves `TXT_030` frame-0 bbox toward AE. The next blocker is
blur kernel/composite quality, not blur radius delivery.

### Russell / Collapse Contract

Status: completed, contract patch accepted pending final integrated test.

Report:

```text
docs/phase_reports/AGENT_ROUND5_COLLAPSE_CONTRACT.md
```

Patch: `PrecompDeferredRasterPlan` with deferred primitives, raster barriers,
transform steps, source-time steps, active-window and opacity contract; scene
wrapper in `graph.rs`.

Validation:

```text
cargo test -p render-core precomp -- --nocapture            17 passed
cargo test -p render-core collapse -- --nocapture            7 passed
cargo test -p render-core deferred_contract -- --nocapture   3 passed
conformance GPH_010: 1 case, 0 failures
```

Decision: accept as contract prep, not pixel parity. Render path still needs to
consume the contract before `GPH_010` can move.

## Final Integrated Verification

Probe pack validation:

```text
json.tool: glow_shadow/minimax/turbulent manifests valid
node --check: all three JSX builders parse
py_compile: minimax measurement helper and turbulent asset generator parse
turbulent primitives: 512x512 8-bit RGBA PNGs
```

Render-core validation:

```text
cargo fmt -p render-core -- --check
cargo test -p render-core text_animator -- --nocapture      3 passed
cargo test -p render-core precomp -- --nocapture           17 passed
cargo test -p render-core collapse -- --nocapture           7 passed
cargo test -p render-core deferred_contract -- --nocapture  3 passed
render-cli conformance-pack --case TXT_030 --case TXT_040 --case GPH_010 --case STK_030
```

Conformance output:

```text
target/ae_agents/round5_final_integrated/report.json
```

Result:

```text
conformance: 4 cases, 0 failures
git diff --check: clean
```

Round 4 -> Round 5 focused metrics:

| Case | R4 RGB mean | R5 RGB mean | Delta | R4 RGB RMSE | R5 RGB RMSE | R4 BG mean | R5 BG mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `TXT_030` | 8.1287 | 7.7574 | -0.3713 | 34.9982 | 31.6915 | 10.7500 | 11.4010 |
| `TXT_040` | 4.2506 | 4.2506 | +0.0000 | 30.5860 | 30.5860 | 4.2329 | 4.2329 |
| `GPH_010` | 15.7070 | 15.7070 | +0.0000 | 59.2228 | 59.2228 | 12.7572 | 12.7572 |
| `STK_030` | 3.6281 | 3.6281 | +0.0000 | 21.8641 | 21.8641 | 17.4869 | 17.4869 |

Final Round 5 decision:

- Accept all three AE probe packs as ready for manual AE rendering.
- Accept text animator blur delivery patch, with note that the next text blocker
  is blur kernel/composite semantics.
- Accept collapse deferred-raster contract patch, with no pixel parity claim.
- No regressions observed in `TXT_040`, `GPH_010`, or `STK_030`.
- Next best move is to render the AE probe packs and ingest their outputs.

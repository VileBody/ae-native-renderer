# OS Round 4 Critic Board

Status date: 2026-05-04.

Goal: continue AE math parity work in parallel, but keep Round 3's rule:
primitive evidence first, composed stack second.

## Active Agents

| Agent | Block | Primary Cases | Output |
| --- | --- | --- | --- |
| Avicenna | Glow / Drop Shadow | `EFF_010`, `EFF_020`, `EFF_070` | `docs/phase_reports/AGENT_ROUND4_GLOW_SHADOW.md` |
| Dewey | Minimax | `EFF_050` | `docs/phase_reports/AGENT_ROUND4_MINIMAX.md` |
| Helmholtz | Turbulent field probes | `EFF_060` | `docs/phase_reports/AGENT_ROUND4_TURBULENT_FIELD_PROBES.md` |
| Herschel | Text raster / animator order | `TXT_030`, `TXT_040` | `docs/phase_reports/AGENT_ROUND4_TEXT_RASTER_ANIMATOR.md` |
| Volta | Collapse defer-raster | `GPH_010` | `docs/phase_reports/AGENT_ROUND4_COLLAPSE_DEFER_RASTER.md` |

## Round 3 Baseline

Final integrated baseline:

```text
target/ae_agents/round3_final_integrated/report.json
```

Key metrics:

| Case | RGB mean | Background-normalized mean | Status |
| --- | ---: | ---: | --- |
| `STK_030` | 3.6281 | 17.4869 | Stack regression target, not direct tuning target |
| `EFF_010` | 0.1635 | 1.2643 | Drop Shadow small residual |
| `EFF_020` | 12.6578 | 15.3025 | Glow first effects blocker |
| `EFF_030` | 0.0000 | 0.0000 | Box Blur guard |
| `EFF_050` | 3.5333 | 2.6674 | Minimax enum/channel/neighborhood blocker |
| `EFF_070` | 1.8913 | 2.8430 | Animated Glow/Blur regression |
| `EFF_040` | 0.0676 | 0.1453 | Geometry2 mostly solved; edge residual |
| `EFF_060` | 2.8989 | 2.8006 | Turbulent field blocker |
| `TXT_030` | 8.1287 | 10.7500 | Glyph coverage / animator order blocker |
| `TXT_040` | 4.2506 | 4.2329 | Placement improved; selector/expression edge case |
| `EXP_010` | 1.3041 | 1.1888 | Supported assignment subset implemented |
| `GPH_010` | 15.7070 | 12.7572 | Collapse still diagnostic |

## Acceptance Rules

Every accepted Round 4 change needs:

- a primitive or diagnostic case target;
- before/after split metrics;
- telemetry, sidecar, or fixture evidence for the first divergent module;
- focused tests for touched behavior;
- a report with changed files and remaining blocker.

Accepted non-code outputs:

- a precise AE probe/fixture spec when formula tuning is blocked;
- a clean design contract when true implementation crosses multiple modules.

## No-Go Rules

- No final-pixel-only tuning for Glow, Minimax, or Turbulent.
- No direct `STK_030` tuning before `EFF_020`, `EFF_050`, and `EFF_060` are
  better explained.
- No full ExtendScript work.
- No full collapse rewrite without a deferred-raster contract and tests.
- No text selector semantic changes inside `text-engine` if evidence points to
  `render-core`.

## Expected Outcomes

Best case:

- Glow or Minimax gets a small evidence-backed formula fix.
- Text raster/coverage gets a measurable improvement.
- Collapse gets a concrete implementation contract.
- Turbulent gets an AE field-probe plan good enough to generate next goldens.

Acceptable case:

- Agents stop without formula patches but produce exact probe specs that tell us
  what AE fixtures to generate next.

Failure case:

- Any patch that improves a composed diff while making the primitive evidence
  less explainable.

## Agent Results

### Herschel / Text Raster Animator

Status: completed, no text-engine patch accepted.

Report:

```text
docs/phase_reports/AGENT_ROUND4_TEXT_RASTER_ANIMATOR.md
```

Result:

```text
cargo test -p text-engine -- --nocapture
13 passed
```

Decision: correct stop. Evidence points to `render-core` application/order, not
basic glyph layout or text-engine raster coverage:

- `TXT_030`: per-glyph animator blur/order, with telemetry blur radius only
  `0/1` despite animator blur `[10,10]`.
- `TXT_040`: frame-0 expression selector/application semantics.

Next text owner should be `render-core/src/layer_eval.rs`, not `text-engine`.

### Avicenna / Glow Shadow

Status: completed, no formula patch accepted.

Report:

```text
docs/phase_reports/AGENT_ROUND4_GLOW_SHADOW.md
```

Result:

```text
cargo test -p effects glow -- --nocapture       3 passed
cargo test -p effects drop_shadow -- --nocapture 2 passed
conformance EFF_010/EFF_020/EFF_070: 3 cases, 0 failures
```

Metrics unchanged from Round 3:

| Case | RGB mean | Background-normalized mean | Decision |
| --- | ---: | ---: | --- |
| `EFF_010` | 0.1635 | 1.2643 | Needs AE raw/blurred shadow probe |
| `EFF_020` | 12.6578 | 15.3025 | Needs AE Glow threshold/mask probe |
| `EFF_070` | 1.8913 | 2.8430 | Blocked by same Glow branch |

Decision: correct stop. Next action is adding AE-side intermediate probes:
Glow `threshold_source/blurred/scaled/final`, Drop Shadow
`raw_offset/blurred/final`, plus no-softness and shadow-only variants.

### Volta / Collapse Defer Raster

Status: completed, no graph/precomp pixel patch accepted.

Report:

```text
docs/phase_reports/AGENT_ROUND4_COLLAPSE_DEFER_RASTER.md
```

Result:

```text
cargo test -p render-core precomp -- --nocapture                              14 passed
cargo test -p render-core collapsed_precomp_flattens_solid_with_parent_transform -- --nocapture 1 passed
cargo test -p render-core collapse -- --nocapture                               7 passed
conformance GPH_010: 1 case, 0 failures
```

Decision: correct stop. `graph/precomp` can express the current flatten plan,
but true deferred text/vector rasterization belongs to the render path. Next
implementation needs an explicit render-facing deferred primitive contract
before touching pixels.

### Helmholtz / Turbulent Field Probes

Status: completed, instrumentation tests accepted; no formula patch accepted.

Report:

```text
docs/phase_reports/AGENT_ROUND4_TURBULENT_FIELD_PROBES.md
```

Result:

```text
cargo test -p effects turbulent -- --nocapture 10 passed
conformance EFF_060: 1 case, 0 failures
```

Metrics unchanged:

| Case | RGB mean | Background-normalized mean | Decision |
| --- | ---: | ---: | --- |
| `EFF_060` | 2.8989 | 2.8006 | Needs AE field probe |

Decision: accept the extra field-model tests and probe design. M14 remains
`implemented approximate`; the next action is generating an AE coordinate-field
micro-pack to identify noise basis/evolution/sampler/pinning.

### Dewey / Minimax

Status: completed, debug/evidence test accepted; no formula patch accepted.

Report:

```text
docs/phase_reports/AGENT_ROUND4_MINIMAX.md
```

Result:

```text
cargo test -p effects minimax -- --nocapture 5 passed
conformance EFF_050: 1 case, 0 failures
```

Metrics unchanged:

| Case | RGB mean | Background-normalized mean | Decision |
| --- | ---: | ---: | --- |
| `EFF_050` | 3.5333 | 2.6674 | Needs AE enum/channel probe |

Decision: correct stop. Native erodes the visible square while AE keeps the
original bbox, so the first blocker is likely operation/channel enum mapping,
not radius rounding or neighborhood shape. Next action is an AE Minimax enum
matrix probe with pre/post effect bboxes and row samples.

## Final Integrated Verification

Output:

```text
target/ae_agents/round4_final_integrated/report.json
```

Verification:

```text
cargo fmt -p effects -- --check
cargo test -p effects minimax -- --nocapture
cargo test -p effects turbulent -- --nocapture
cargo test -p effects glow -- --nocapture
cargo test -p effects drop_shadow -- --nocapture
render-cli conformance-pack --case EFF_010 --case EFF_020 --case EFF_050 --case EFF_060 --case EFF_070 --case TXT_030 --case TXT_040 --case GPH_010 --case STK_030
```

Result:

```text
minimax tests: 5 passed
turbulent tests: 10 passed
glow tests: 3 passed
drop_shadow tests: 2 passed
conformance: 9 cases, 0 failures
git diff --check: clean
```

Round 3 -> Round 4 focused metrics:

| Case | Round3 RGB mean | Round4 RGB mean | Delta | Round3 BG mean | Round4 BG mean |
| --- | ---: | ---: | ---: | ---: | ---: |
| `EFF_010` | 0.1635 | 0.1635 | +0.0000 | 1.2643 | 1.2643 |
| `EFF_020` | 12.6578 | 12.6578 | +0.0000 | 15.3025 | 15.3025 |
| `EFF_050` | 3.5333 | 3.5333 | +0.0000 | 2.6674 | 2.6674 |
| `EFF_060` | 2.8989 | 2.8989 | +0.0000 | 2.8006 | 2.8006 |
| `EFF_070` | 1.8913 | 1.8913 | +0.0000 | 2.8430 | 2.8430 |
| `TXT_030` | 8.1287 | 8.1287 | +0.0000 | 10.7500 | 10.7500 |
| `TXT_040` | 4.2506 | 4.2506 | +0.0000 | 4.2329 | 4.2329 |
| `GPH_010` | 15.7070 | 15.7070 | +0.0000 | 12.7572 | 12.7572 |
| `STK_030` | 3.6281 | 3.6281 | +0.0000 | 17.4869 | 17.4869 |

Final Round 4 decision:

- Accept Minimax debug/evidence test.
- Accept Turbulent field-boundary tests and probe spec.
- Accept Glow/Drop Shadow, Text, and Collapse reports as correct stops.
- No formula parity metric improved in Round 4; that is expected because the
  remaining work needs new AE intermediate/field probes.
- Next engineering move is AE probe pack generation, not another blind formula
  tuning pass.

## Docs Policy

Adobe/SDK docs may be used for:

- match names;
- parameter names and broad semantics;
- coordinate spaces;
- expression object access;
- collapse/precomp conceptual behavior.

They are not proof for:

- exact glow masks/blends;
- minimax neighborhood/channel quirks;
- turbulent noise basis;
- glyph raster coverage;
- deferred vector/text rasterization.

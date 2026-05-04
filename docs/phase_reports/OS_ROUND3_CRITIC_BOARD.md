# OS Round 3 Critic Board

Status date: 2026-05-03.

Goal: run parallel formula investigation without letting composed template diffs
turn into guesswork.

## Active Agents

| Agent | Block | Primary Cases | Output |
| --- | --- | --- | --- |
| Kierkegaard | `M12` Geometry2 | `EFF_040` | `docs/phase_reports/AGENT_ROUND3_GEOMETRY2.md` |
| Aristotle | `M10/M11/M13` Effects | `EFF_010`, `EFF_020`, `EFF_030`, `EFF_050`, `EFF_070` | `docs/phase_reports/AGENT_ROUND3_EFFECTS.md` |
| Hypatia | `M14` Turbulent Displace | `EFF_060` | `docs/phase_reports/AGENT_ROUND3_TURBULENT.md` |
| Newton | `M05-M09` Text glyphs | `TXT_010`, `TXT_020`, `TXT_030`, `TXT_040` | `docs/phase_reports/AGENT_ROUND3_TEXT_GLYPHS.md` |
| Kant | `M17` Expressions + Collapse | `EXP_010`, `GPH_010` | `docs/phase_reports/AGENT_ROUND3_EXPR_COLLAPSE.md` |

## Agent Results

### Aristotle / Effects

Status: completed, no formula patch accepted.

Report:

```text
docs/phase_reports/AGENT_ROUND3_EFFECTS.md
```

Verification:

```text
cargo test -p effects
render-cli conformance-pack --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_050 --case EFF_070
```

Result:

```text
effects tests: 30 passed
conformance: 5 cases, 0 failures
```

Metrics:

| Case | RGB mean | Background-normalized mean | Decision |
| --- | ---: | ---: | --- |
| `EFF_010` | 0.1635 | 1.2643 | Needs AE raw/blurred shadow intermediate |
| `EFF_020` | 12.6578 | 15.3025 | First effects tuning candidate, but needs AE Glow threshold/mask intermediate |
| `EFF_030` | 0.0000 | 0.0000 | Exact under split metrics; keep as guard |
| `EFF_050` | 3.5333 | 2.6674 | Needs AE Minimax enum/channel/neighborhood evidence |
| `EFF_070` | 1.8913 | 2.8430 | Blocked by same Glow branch uncertainty |

Critic decision: correct stop. The native sidecars expose resolved params and
native hashes, but not AE intermediates. Formula changes here would be
prose-driven. Next action for effects is to extend the AE conformance pack with
intermediate/matte cases or exportable step-wise fixtures.

### Hypatia / Turbulent Displace

Status: completed, refactor/regression patch accepted pending final integrated
test.

Report:

```text
docs/phase_reports/AGENT_ROUND3_TURBULENT.md
```

Verification:

```text
cargo test -p effects turbulent -- --nocapture
render-cli conformance-pack --case EFF_060
```

Result:

```text
turbulent tests: 8 passed
conformance: 1 case, 0 failures
```

Metrics:

| Case | RGB mean | RGB RMSE | RGB max | Background-normalized mean |
| --- | ---: | ---: | ---: | ---: |
| `EFF_060` | 2.8989 | 16.4709 | 200 | 2.8006 |

Sidecars are byte-identical with the Round 2 integrated trace output, as
expected for a no-math-change refactor.

Critic decision: accepted as instrumentation hardening, not formula tuning. It
separates parameter mapping, noise/field vector, sampler, and edge policy and
adds field-level regression coverage. The next real tuning blocker is still
AE-equivalent field telemetry for noise basis/evolution/displacement variants.

### Kant / Expressions And Collapse

Status: completed, narrow expression subset patch accepted pending final
integrated test. Collapse remains diagnostic.

Report:

```text
docs/phase_reports/AGENT_ROUND3_EXPR_COLLAPSE.md
```

Verification:

```text
cargo test -p expression-engine -- --nocapture
cargo test -p render-core collapsed_precomp_flattens_solid_with_parent_transform -- --nocapture
cargo test -p render-core precomp -- --nocapture
render-cli conformance-pack --case EXP_010 --case GPH_010
```

Result:

```text
expression tests: 7 passed
collapse focused test: 1 passed
precomp tests: 14 passed
conformance: 2 cases, 0 failures
```

Metrics:

| Case | RGB mean | Background-normalized mean | Decision |
| --- | ---: | ---: | --- |
| `EXP_010` | 1.3041 | 1.1888 | Assignment subset implemented; conformance unchanged because runtime already used named pattern |
| `GPH_010` | 15.7070 | 12.7572 | Improved in current worktree, but not attributed to this block |

Critic decision: accept the evaluator subset because it is constrained to
EXP_010-style local assignment statements and final scalar/Vec2 expression.
Do not mark collapse as implemented. Telemetry says matrix propagation and
raster scale are not the current blocker; true deferred text/vector raster
remains separate work.

### Newton / Text Glyphs

Status: completed, text placement patch accepted pending final integrated test.

Report:

```text
docs/phase_reports/AGENT_ROUND3_TEXT_GLYPHS.md
```

Verification:

```text
cargo fmt -p text-engine
cargo test -p text-engine -- --nocapture
render-cli conformance-pack --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040
```

Result:

```text
text-engine tests: 13 passed
conformance: 4 cases, 0 failures
```

Metrics:

| Case | Before RGB mean | After RGB mean | Before BG-norm mean | After BG-norm mean |
| --- | ---: | ---: | ---: | ---: |
| `TXT_010` | 9.0858 | 8.1849 | 8.7203 | 7.7265 |
| `TXT_020` | 8.6609 | 7.2454 | 8.3677 | 6.8646 |
| `TXT_030` | 10.8155 | 8.1287 | 13.5477 | 10.7500 |
| `TXT_040` | 6.8230 | 4.2506 | 6.6565 | 4.2329 |

Critic decision: accept. Evidence points to text-box placement rather than font
fallback. The patch improves all four text cases and makes `TXT_040` frame 59
bbox exact. Next text blocker is glyph raster/coverage plus per-glyph animator
order, not basic box centering.

### Kierkegaard / Geometry2

Status: completed, formula patch accepted pending final integrated test.

Report:

```text
docs/phase_reports/AGENT_ROUND3_GEOMETRY2.md
```

Verification:

```text
cargo test -p effects geometry -- --nocapture
render-cli conformance-pack --case EFF_040
```

Result:

```text
geometry tests: 8 passed
conformance: 1 case, 0 failures
```

Metrics:

| Case | Before RGB mean | After RGB mean | Before BG-norm mean | After BG-norm mean |
| --- | ---: | ---: | ---: | ---: |
| `EFF_040` | 8.9347 | 0.0676 | 7.0474 | 0.1453 |

Critic decision: accept. The patch is evidence-backed by sidecar/image probes:
`0004` is treated as uniform scale for the fixture, `0008` is not scaleY, and
the Transform effect is applied in layer-space origin. Remaining error is edge
and partial-alpha behavior; do not change pixel-center/sampler without a
separate edge fixture.

## Acceptance Rules

Every accepted change needs:

- a primitive case target, not only `STK_030` or full-template evidence;
- before/after split metrics: `rgb`, `alpha`, and
  `background_alpha_normalized`;
- sidecar or telemetry evidence explaining the first divergent module;
- a focused test where the behavior can be isolated;
- a report with changed files and remaining blocker.

## No-Go Rules

- Do not tune `STK_030` directly until `EFF_040`, `EFF_050`, and `EFF_060`
  are separately explained.
- Do not tune `GPH_010` as final-pixel collapse parity until text raster scale
  and matrix propagation are separately diagnosed.
- Do not introduce arbitrary ExtendScript support. Only supported expression
  traits that appear in fixtures/templates are in scope.
- Do not use final-pixel Turbulent Displace diffs as formula proof without
  field-level evidence.

## Current Priority

1. `EFF_040` Geometry2 matrix/anchor/pixel-center/sampler.
2. Isolated effects: `EFF_030` -> `EFF_010` -> `EFF_020` -> `EFF_050` ->
   `EFF_070`.
3. Text glyph metrics and text-box placement on exact Point-Light cases.
4. `EXP_010` supported expression trait values.
5. `GPH_010` collapse diagnostics after text/expression blockers move.
6. `STK_030` regression after primitive operators improve.

## Final Integrated Verification

Output:

```text
target/ae_agents/round3_final_integrated/report.json
```

Verification:

```text
cargo fmt -p effects -p text-engine -p expression-engine -- --check
cargo test -p effects geometry -- --nocapture
cargo test -p effects turbulent -- --nocapture
cargo test -p text-engine -- --nocapture
cargo test -p expression-engine -- --nocapture
render-cli conformance-pack --case TMP_020 --case STK_030 --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_050 --case EFF_070 --case EFF_040 --case EFF_060 --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case EXP_010 --case GPH_010
```

Result:

```text
geometry tests: 8 passed
turbulent tests: 8 passed
text-engine tests: 13 passed
expression-engine tests: 7 passed
conformance: 15 cases, 0 failures
git diff --check: clean
```

Round 2 -> Round 3 metrics:

| Case | Round2 RGB mean | Round3 RGB mean | Delta | Round2 BG mean | Round3 BG mean |
| --- | ---: | ---: | ---: | ---: | ---: |
| `TMP_020` | 0.0000 | 0.0000 | +0.0000 | 0.0000 | 0.0000 |
| `STK_030` | 20.2231 | 3.6281 | -16.5951 | 52.2771 | 17.4869 |
| `EFF_010` | 0.1635 | 0.1635 | +0.0000 | 1.2643 | 1.2643 |
| `EFF_020` | 12.6578 | 12.6578 | +0.0000 | 15.3025 | 15.3025 |
| `EFF_030` | 0.0000 | 0.0000 | +0.0000 | 0.0000 | 0.0000 |
| `EFF_050` | 3.5333 | 3.5333 | +0.0000 | 2.6674 | 2.6674 |
| `EFF_070` | 1.8913 | 1.8913 | +0.0000 | 2.8430 | 2.8430 |
| `EFF_040` | 8.9347 | 0.0676 | -8.8671 | 7.0474 | 0.1453 |
| `EFF_060` | 2.8989 | 2.8989 | +0.0000 | 2.8006 | 2.8006 |
| `TXT_010` | 9.0858 | 8.1849 | -0.9009 | 8.7203 | 7.7265 |
| `TXT_020` | 8.6609 | 7.2454 | -1.4155 | 8.3677 | 6.8646 |
| `TXT_030` | 10.8155 | 8.1287 | -2.6868 | 13.5477 | 10.7500 |
| `TXT_040` | 6.8230 | 4.2506 | -2.5724 | 6.6565 | 4.2329 |
| `EXP_010` | 1.3041 | 1.3041 | +0.0000 | 1.1888 | 1.1888 |
| `GPH_010` | 18.6581 | 15.7070 | -2.9511 | 16.0434 | 12.7572 |

Trace sidecars exist in the final integrated output:

```text
STK_030/adjustment_effects.jsonl       36
TXT_010/text_telemetry.jsonl           14
TXT_020/text_telemetry.jsonl           28
TXT_030/text_telemetry.jsonl           14
TXT_040/text_telemetry.jsonl           16
EXP_010/expression_telemetry.jsonl      8
GPH_010/text_telemetry.jsonl            8
GPH_010/collapse_telemetry.jsonl       12
```

Final Round 3 decision:

- Accept Geometry2 formula patch.
- Accept text-box placement patch.
- Accept expression assignment subset patch.
- Accept Turbulent field instrumentation/regression patch.
- Accept Effects block decision to stop without formula changes.
- Keep Glow/Minimax/Turbulent noise/glyph raster/collapse defer-raster as the
  next real blockers.

## Reverse Engineering Risk

Estimated need for binary reverse engineering/Ghidra:

```text
current template-focused parity: low to medium, roughly 10-20%
full broad AE parity: medium to high, roughly 40-60%
```

Reasoning:

- Most current blockers are observable through black-box fixtures:
  transforms, kernels, channel rules, glyph metrics, time buckets, matrix
  propagation, and sidecar hashes.
- The expensive unknowns are AE-specific procedural/effect internals such as
  Turbulent Displace noise fields, Glow blend details, Minimax channel rules,
  and edge/premultiplication conventions.
- Binary reverse engineering should stay a last resort. It creates legal,
  maintenance, and clean-room concerns and may still be slower than generating
  sharper fixtures.

Preferred escalation path:

```text
primitive fixture
  -> telemetry/sidecar
  -> parameter sweep
  -> public docs / known formulas
  -> clean-room black-box inference
  -> only then consider whether reverse engineering is worth discussing
```

## Adobe Docs Use

Public docs are useful as semantic constraints, not as parity proof.

Use them for:

- match names and supported effect identity;
- parameter meaning and UI-level behavior;
- coordinate spaces;
- expression object/property access;
- precomp/collapse conceptual behavior.

Do not use prose docs alone for:

- blur kernel exact weights;
- Glow blend/threshold internals;
- Minimax channel/neighborhood quirks;
- Turbulent Displace noise basis;
- text rasterization and glyph metrics.

References sent to workers:

```text
https://developer.adobe.com/apis/creativecloud/aftereffects.html
https://helpx.adobe.com/in/after-effects/using/distort-effects.html
https://helpx.adobe.com/after-effects/using/blur-sharpen-effects.html
https://helpx.adobe.com/after-effects/using/perspective-effects.html
https://helpx.adobe.com/after-effects/using/stylize-effects.html
https://helpx.adobe.com/after-effects/using/animating-text.html
https://helpx.adobe.com/after-effects/using/precomposing-nesting-pre-rendering.html
https://helpx.adobe.com/after-effects/using/expression-language-reference.html
https://ae-expressions.docsforadobe.dev/objects/effect/
https://ae-expressions.docsforadobe.dev/objects/comp/
https://ae-expressions.docsforadobe.dev/objects/layer/
https://ae-plugins.docsforadobe.dev/effect-basics/parameters/
https://ae-scripting.docsforadobe.dev/matchnames/effects/firstparty/
```

`docsforadobe.dev` is treated as a practical public reference/mirror, while
final parity decisions still require clean AE goldens and repo telemetry.

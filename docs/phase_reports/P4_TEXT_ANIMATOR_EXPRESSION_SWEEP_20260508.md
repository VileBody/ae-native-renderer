# P4 Text Animator / Expression Sweep

Status date: 2026-05-08.

## Scope

This pass stayed lateral in `M06`-`M09`/`M17` and did not touch CoolType raster
internals or `rasterize.rs`.

Owned edits:

- `crates/text-engine/src/text_animator.rs`
- `crates/expression-engine/src/evaluator.rs`
- this report

The worktree already contained unrelated edits outside this scope, including
`crates/effects/src/glow.rs`, `crates/effects/src/turbulent_displace.rs`, and
other phase reports. Those were left alone.

## What Got Broader

Text animator helper coverage now includes an explicit expression-selector
bounce planning surface:

- `BounceExpressionSelector`
- `TextExpressionSelectorWeight`
- `TextComposedSelectorWeight`
- `evaluate_bounce_expression_selector(...)`
- `compose_range_and_bounce_selector_weights(...)`

The helper records text index, text total, local time after per-character
delay, raw Amount percent, clamped Amount percent, expression weight, range
weight, and final composed weight. The default pre-delay Amount stays `0` to
match current runtime telemetry, but the helper can now model the generated JSX
`else { value }` branch through `pre_delay_amount_percent`.

The named expression evaluator now also honors that same generated bounce
selector branch when `ctx.value` is present. Before this pass, the fingerprint
shortcut always returned `0` before the delay elapsed. It now returns the
numeric selector `value` when supplied, and preserves the old `0` fallback when
no value is available.

No render-core selector, glyph transform-center, blur kernel, opacity
composition, collapse, or raster formula changed in this pass.

## Verification

Baseline before edits:

```text
cargo test -p text-engine text_animator -- --nocapture
8 passed

cargo test -p expression-engine -- --nocapture
12 passed
```

After edits:

```text
cargo test -p text-engine text_animator -- --nocapture
10 passed

cargo test -p expression-engine -- --nocapture
14 passed

cargo fmt -p text-engine -p expression-engine -- --check
passed
```

Focused conformance:

```text
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/p4_text_animator_expression_sweep_20260508 \
  --case TXT_030 --case TXT_040 --case EXP_010 --case GPH_010
```

Result:

```text
conformance-pack.done ok=true cases=4
```

The build emitted one pre-existing warning in `crates/effects/src/glow.rs`
about unnecessary index parentheses; it is outside this sweep.

## Metrics

Artifact root:

```text
target/ae_agents/p4_text_animator_expression_sweep_20260508
```

| Case | Frames | Primary visible mean | Primary visible RMSE | Background-alpha mean | Text passport |
| --- | ---: | ---: | ---: | ---: | --- |
| `TXT_030` | 7 | `7.073935917445591` | `30.278972153321728` | `16.13277462550572` | ok, 7 compared, 0 missing |
| `TXT_040` | 8 | `3.671418031056722` | `28.089647001255486` | `4.201260924339294` | ok, 8 compared, 0 missing |
| `EXP_010` | 8 | `1.2887310981750488` | `17.716706437861436` | `1.3197853565216064` | no text refs, 8 missing |
| `GPH_010` | 4 | `4.002211252848308` | `27.338614868445905` | `4.577075958251953` | no text refs, 4 missing |

Key sidecars:

```text
target/ae_agents/p4_text_animator_expression_sweep_20260508/report.json
target/ae_agents/p4_text_animator_expression_sweep_20260508/TXT_030/text_telemetry.jsonl
target/ae_agents/p4_text_animator_expression_sweep_20260508/TXT_040/text_telemetry.jsonl
target/ae_agents/p4_text_animator_expression_sweep_20260508/EXP_010/expression_telemetry.jsonl
target/ae_agents/p4_text_animator_expression_sweep_20260508/GPH_010/collapse_telemetry.jsonl
```

## Notes

This is a helper/evaluator broadening pass, not a visual formula retune. The
focused conformance metrics are therefore a no-regression check for the current
runtime surface. The next runtime integration point remains the existing
`render-core` selector path: it still computes bounce Amount directly and does
not yet consume the new text-engine composition helper.

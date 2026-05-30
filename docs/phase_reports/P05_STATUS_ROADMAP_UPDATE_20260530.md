# P05 Status Roadmap Update

Date: 2026-05-30

## Decision

```text
p05_staged_candidate_green_pr_opened_pending_review_merge
```

## Current State

P05 is no longer blocked on the old P6/current-text AD68 production-boundary
question.

The current candidate is packaged on branch:

```text
codex-p6-opt-in-ad68-text-route
```

Latest candidate commit:

```text
77cc0b6 Stage P05 current text renderer candidate
```

Draft PR:

```text
https://github.com/VileBody/ae-native-renderer/pull/1
```

The PR currently contains the existing P6 branch history plus the final P05
candidate commit. The top commit to review for the final packaging step is
`77cc0b6`.

## What Is In The Candidate

- P6 current-text AD68 pixel path and native row-event materializer updates.
- BEE / M17 / M19 collapsed text carrier handoff.
- M07 text animator, layout, and source mapping updates.
- Expression selector schema/bridge updates for parsed position property and
  pre-delay selector behavior.
- Text telemetry/golden refreshes and static/runtime helper tooling updates.

## Production Default Reading

P6 current-text AD68 pixels are production-default true for the bounded current
text route included in this branch.

The explicit disable escape hatch remains:

```text
AE_NATIVE_RENDERER_P6_AD68_TEXT_PIXELS_OPT_IN=0
```

This update does not claim broad arbitrary-AE text parity. It claims that the
current P05 candidate has the P6 current-text route integrated, validated, and
packaged for review.

## Validation

Validation was run against a temporary clean worktree containing only
`HEAD + staged patch`, so the known unstaged dirty context was not part of the
test result.

Passed:

```text
git diff --cached --check
git diff --check
cargo fmt --check -p text-engine
cargo fmt --check -p render-core
cargo fmt --check -p render-cli
cargo fmt --check -p ae-bridge
cargo fmt --check -p render-ir
python3 -m py_compile scripts/ae_trace_cooltype_text.py scripts/analyze_are_sampler_trace.py scripts/run_master_conformance_gate.py
env CARGO_BUILD_JOBS=2 cargo test --offline -p text-engine -- --test-threads=1
env CARGO_BUILD_JOBS=2 cargo test --offline -p render-core -- --test-threads=1
env CARGO_BUILD_JOBS=2 cargo test --offline -p render-cli -- --test-threads=1
env CARGO_BUILD_JOBS=2 cargo test --offline -p ae-bridge -- --test-threads=1
env CARGO_BUILD_JOBS=2 cargo test --offline -p render-ir -- --test-threads=1
```

Package results:

- `text-engine`: full package green, including corpus/materializer/native bucket tests.
- `render-core`: 62 tests passed in staged-only snapshot.
- `render-cli`: 20 `p2_text_journal` binary tests and 23 main binary tests passed.
- `ae-bridge`: compile/test pass, 0 unit tests.
- `render-ir`: compile/test pass, 0 unit tests.

Known validation note:

- `render-core` staged-only snapshot emitted one non-blocking unused import
  warning in tests. It was not patched because the warning disappears only by
  mixing in an unstaged P2 journal carveout, which is intentionally outside the
  P05 candidate.

## Remaining Carveouts

These are not part of P05:

- `crates/render-core/src/layer_eval.rs` unstaged M17 collapsed-text alpha/scale
  metric-fit experiment.
- `crates/render-core/src/layer_eval.rs` unstaged P2 frame-trace text journal
  test.
- Untracked static/TDD/probe helper scripts and fixtures.
- `.env.iac`.
- `repomix-output.xml`.

## Next Step

The next process action is PR review/merge decision for PR #1.

After merge, P05 can move from:

```text
staged_candidate_green_pr_opened_pending_review_merge
```

to:

```text
p05_green_merged
```

only if remote checks/review do not introduce a new blocker.

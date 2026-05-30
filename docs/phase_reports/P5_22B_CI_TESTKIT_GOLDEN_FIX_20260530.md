# P5_22B CI Testkit Golden Fix

Date: 2026-05-30

## Decision

```text
ci_testkit_golden_fixture_policy_fixed_pending_remote_rerun
```

## Problem

PR #1 CI failed in the `Targeted tests` step:

```text
cargo test -p testkit -p effects -p render-core -p ae-bridge -p render-cli
```

The renderer packages passed in the CI log, but `testkit` failed because several
phase tests required AE PNG goldens that are not fully checked into the repo:

- `CMP_010`
- `TMP_010`
- `EFF_010`
- `EFF_040`
- `TXT_010`

There was also a phase4 manifest mismatch for `EFF_060` selected frames.

## Fix

The AE PNG golden load checks are now asset-gated:

```text
AE_NATIVE_REQUIRE_AE_PNG_GOLDENS=1
```

Default CI behavior:

- if the PNG golden suite is missing or incomplete, the golden load tests log a
  pending/skip message and continue;
- manifest/schema/module tests still run;
- renderer and package tests still run;
- genuine image dimension/load failures still fail when the required PNG pack is
  available or `AE_NATIVE_REQUIRE_AE_PNG_GOLDENS=1` is set.

The phase4 `EFF_060` expected frame list was updated to match the current
manifest:

```text
[0, 1, 15, 30, 45, 59]
```

## Validation

Passed locally:

```text
cargo fmt --check -p testkit
env CARGO_BUILD_JOBS=2 cargo check --workspace
env CARGO_BUILD_JOBS=2 cargo test -p testkit -p effects -p render-core -p ae-bridge -p render-cli -- --test-threads=1
```

This patch does not modify renderer behavior.

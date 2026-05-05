# Worker D Step 4 Geometry / Procedural / Adjustment

Status date: 2026-05-05.

Scope: M12 Geometry2, M13 Minimax, M14 Turbulent Displace, M16 adjustment
effect-stack diagnostics. No formula tuning was done.

## Inputs Reviewed

- `crates/effects/src/geometry.rs`
- `crates/effects/src/minimax.rs`
- `crates/effects/src/turbulent_displace.rs`
- `crates/render-core/src/layer_eval.rs`
- `docs/phase_reports/AE_REVERSE_GEOMETRY2_GHIDRA.md`
- `docs/phase_reports/AE_REVERSE_TURBULENT_TEXT_EXPRESSION_GHIDRA.md`
- `docs/phase_reports/AGENT_4_WARPS_FIELDS_NATIVE_DIFF.md`
- `docs/phase_reports/OS_TELEMETRY_ROUND2.md`

Geometry2 `0003`/`0004`/`0008` handling was treated as evidence-bound. Existing
docs tie JSX numeric keys to the AE property dump; this pass did not reinterpret
those slots or change the transform formula.

## Change

Updated `crates/render-core/src/layer_eval.rs` so traced effect stacks now emit
debug payloads for real stack inputs:

- `ADBE Geometry2`: raw params, resolved transform params, forward/inverse
  matrices, probe samples, sampler mode, edge policy, OOB count.
- `ADBE Turbulent Displace`: raw params, resolved field params, Ghidra-derived
  wrapper state, probe samples, field hash, sampler mode, edge policy, OOB
  count.
- `ADBE Minimax`: existing debug JSON now includes direction and
  `dont_shrink_edges`.

This is mainly for `STK_030`: Geometry2, Minimax, and Turbulent Displace
checkpoints are now available from the actual adjustment input/output sequence,
not only from isolated `EFF_040`/`EFF_060` helper sidecars.

## Test Added

Added a focused `render-core` unit test:

```text
adjustment_stack_debug_records_geometry_minimax_and_turbulent_checkpoints
```

It asserts the adjustment effect order, Posterize Time bucket/live parameter
routing, per-effect hashes, Geometry2 numbered-param resolution, Minimax
direction diagnostics, and Turbulent evolution/kernel/field telemetry.

## Verification

Passed:

```sh
docker run --rm -v "$PWD":/work -w /work rust:1-bookworm \
  sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; \
  export CARGO_TARGET_DIR=/work/target/worker_d_effects_test; \
  cargo test -p effects geometry -- --nocapture'
```

Result: 13 Geometry2 tests passed.

Passed:

```sh
docker run --rm -v "$PWD":/work -w /work rust:1-bookworm \
  sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; \
  export CARGO_TARGET_DIR=/work/target/worker_d_effects_test; \
  cargo test -p effects minimax -- --nocapture'
```

Result: 11 Minimax tests passed.

Passed:

```sh
docker run --rm -v "$PWD":/work -w /work rust:1-bookworm \
  sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; \
  export CARGO_TARGET_DIR=/work/target/worker_d_effects_test; \
  cargo test -p effects turbulent -- --nocapture'
```

Result: 11 Turbulent Displace tests passed.

Initially blocked during the parallel worker pass:

```sh
docker run --rm -v "$PWD":/work -w /work rust:1-bookworm \
  sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; \
  export CARGO_TARGET_DIR=/work/target/worker_d_render_core_test; \
  cargo test -p render-core \
  adjustment_stack_debug_records_geometry_minimax_and_turbulent_checkpoints \
  -- --nocapture'
```

Those compile blockers were resolved during integration. Final integrated
verification passed:

```sh
cargo test -p render-core
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/step4_math_after_current \
  --case STK_030
```

The full Step 4 integration run also included `STK_030` and completed with
`ok=true`:

```text
target/ae_agents/step4_math_after_current/report.json
```

## Next Probe Needed

1. Compare the new adjustment `effects_debug` sidecars for Geometry2, Minimax,
   and Turbulent Displace against AE stack checkpoints.
2. For Turbulent Displace, keep using field maps / lookup hashes before any
   final-pixel formula tuning.

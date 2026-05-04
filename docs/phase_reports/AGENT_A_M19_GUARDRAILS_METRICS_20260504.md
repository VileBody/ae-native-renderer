# Agent A M19 Guardrails / Metrics Pass

Status date: 2026-05-04

## Scope

Agent A scope for this pass:

- `M19` color, alpha, sampling, gamma assumptions.
- Shared conformance metric contract.
- Guardrail documentation for other agents.

Out of scope and not edited: effect formulas, geometry, text, expression, and
temporal formulas.

## Code Map

Diff and metric code:

- `crates/testkit/src/image_diff.rs`
  - raw `rgba`, `rgb`, `alpha`;
  - `background_alpha_normalized`;
  - `foreground_rgb`;
  - `rgb_over_*_background`;
  - new explicit `RGB_ALPHA_METRIC_POLICY`;
  - new `diff_rgb8_under_alpha_policy`.
- `crates/render-cli/src/conformance_pack.rs`
  - writes native/AE/diff PNGs;
  - writes per-case `metrics.json`;
  - now emits `metric_contract` and summary/frame
    `metrics.rgb_under_alpha_policy`.
- `crates/render-cli/src/main.rs`
  - generic `compare` path still reports raw RGBA only. It was mapped but not
    changed in this scoped pass.

Contact sheets:

- No automated contact-sheet generator was found in code.
- `docs/AE_REMOTE_GOLDEN_LOOP.md` documents manual contact-sheet output and
  expected `contact_sheet.png` artifact naming.

## Implemented

- Added serializable M19 metric flags:
  - raw RGB ignores alpha;
  - alpha is reported separately;
  - RGB-under-alpha uses source-over projection;
  - AE/reference background RGB is the projection target;
  - no unpremultiply is applied;
  - premult contract is not locked;
  - the metric policy is diagnostic only.
- Added `diff_rgb8_under_alpha_policy`, an explicit alias for the existing
  over-background projection metric.
- Added `rgb_under_alpha_policy` to conformance-pack frame and summary metrics.
- Added `metric_contract.schema = "m19.rgb_alpha_metric_policy.v1"` to
  conformance-pack `metrics.json`.
- Added docs:
  - `docs/reverse_engineering/MATH_CONTRACTS_GUARDRAILS.md`
  - `docs/CONFORMANCE.md` metric contract update
  - `docs/reverse_engineering/README.md` cross-link

## Proven

- Raw RGBA, raw RGB, and alpha-only metrics are separate and unit-tested.
- Background alpha normalization ignores only background RGB matches; it does not
  hide foreground alpha/RGB differences.
- RGB-under-alpha projection ignores invisible RGB at alpha zero and catches
  premult-looking RGB at partial alpha.
- `GF::Composite`, `GF::Unpremultiply`, `GF::AlphaGain`,
  `GF::PackedAlphaGain`, `GF::BlendUnpackedAlpha`, `BoxBlurOptions` alpha fields,
  and `ImageRenderer` pixel-format/worker selection remain the relevant M19
  reverse targets.

## BLOCKER

`M19` is instrumented/testable, not parity locked.

Remaining global blockers:

- Exact AE renderer-boundary alpha contract: straight, premult, packed alpha, or
  format-tagged conversion.
- Exact meaning of `GF::Composite` bool flags and full `IR_BlendMode` mapping.
- Gamma/color managed path for non-default working spaces and non-PNG output.
- 16/32 bpc rounding, float range, and output conversion.
- Effect-specific edge/OOB policy beyond current transparent-black native
  approximation.
- AE CPU/GPU path selection and possible divergence.
- Time semantics remain module-owned; final pixel diffs must not be tuned against
  M19 while frame/effect sample time is unresolved.

No formula was invented for these unknown global entities.

## Verification

Commands run:

```sh
rustfmt crates/testkit/src/image_diff.rs crates/render-cli/src/conformance_pack.rs
cargo test -p testkit image_diff
docker run --rm --entrypoint /bin/sh -v "$PWD:/work" -w /work ae-native-renderer:round2-dev -c 'export CARGO_TARGET_DIR=/tmp/ae_native_renderer_cargo_target; cargo test -p testkit image_diff'
docker run --rm --entrypoint /bin/sh -v "$PWD:/work" -w /work ae-native-renderer:round2-dev -c 'export CARGO_TARGET_DIR=/tmp/ae_native_renderer_cargo_target; cargo test -p render-cli conformance_pack'
git diff --check
```

Results:

- Local `rustfmt`: unavailable (`command not found`).
- Local `cargo`: unavailable (`command not found`).
- Docker `cargo test -p testkit image_diff`: passed, 9 tests.
- Docker `cargo test -p render-cli conformance_pack`: passed, 3 tests.
- `git diff --check`: passed.
- Existing non-blocking warning observed during `render-cli` test:
  `render-core/src/motion_blur.rs` has an unused
  `DEFAULT_MOTION_BLUR_SAMPLES` import.

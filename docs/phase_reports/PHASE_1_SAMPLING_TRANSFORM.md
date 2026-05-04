# Phase 1: Sampling/Composite + Transform/Ease

Date: 2026-05-03

Worker scope: M19 color/alpha/sampling/composite plus M03/M04 transform,
keyframes, and ease. I only changed the Phase 1 testkit hook and this report.

## Decomposition

1. Primitive: prove the source PNGs and AE golden PNGs are loadable as RGBA8
   with the pack composition dimensions.
2. Interpolation: cover linear/hold/ease timing via `INT_010` and `INT_020`
   selected frames from the AE pack.
3. Operator static: cover static transform/sampler behavior via `EFF_040`
   Geometry2 coordinate-field golden.
4. Composition: cover alpha/composite/sampling audit via `CMP_010`.
5. Repeat test: keep a small automated Phase 1 audit that reloads the exact
   selected manifest frames and fails on missing cases, missing frames, or
   wrong PNG dimensions.

## AE Golden Consumption

Added `testkit::phase1` with:

- `PHASE1_CASE_IDS = ["CMP_010", "INT_010", "INT_020", "EFF_040"]`
- AE conformance pack v1 manifest structs
- `AeConformancePack::load`
- `AeConformancePack::golden_png_path`
- `AeConformancePack::load_golden_png`
- `AeConformancePack::audit_phase1_goldens`

The audit consumed all selected frames from `frames_to_compare` for the four
Phase 1 cases: 15 PNGs total, all 512x512 RGBA.

## Focused Checks

- `CMP_010`: declared in manifest with module `M19`; selected golden frame 0
  loads at expected dimensions.
- `INT_010`: declared with module `M04`; selected frames 0, 10, 20, 30, 45,
  and 59 load at expected dimensions.
- `INT_020`: declared with module `M04`; selected frames 0, 5, 10, 15, 30, 45,
  and 59 load at expected dimensions.
- `EFF_040`: declared with module `M12`; selected golden frame 0 loads at
  expected dimensions.

These checks are intentionally not native-vs-AE parity assertions yet. The AE
pack does not currently expose matching native scene recipes or a case runner
that maps these case ids to render-core inputs.

## Commands Run

- `cargo fmt -p testkit`
  - Failed locally: `cargo` is not installed in PATH.
- `cargo test -p testkit phase1 -- --nocapture`
  - Failed locally: `cargo` is not installed in PATH.
- `docker run --rm -v "$PWD":/app -w /app rust:1-bookworm cargo test -p testkit phase1 -- --nocapture`
  - Pass: 2 passed, 0 failed. Warnings only from neighboring phase re-exports.
- `docker run --rm -v "$PWD":/app -w /app rust:1-bookworm cargo check -p testkit`
  - Pass. Warnings are from neighboring `phase3`/`phase4` code, not Phase 1.
- `docker run --rm -v "$PWD":/app -w /app rust:1-bookworm cargo fmt -p testkit -- --check`
  - Blocked: `cargo-fmt` is not installed in that container toolchain.
- `docker run --rm -v "$PWD":/app -w /app rust:1-bookworm sh -lc 'rustup component add rustfmt >/tmp/rustfmt-install.log && cargo fmt -p testkit -- --check'`
  - Blocked: `rustup` is not installed in that container.

## Changed Files

- `crates/testkit/src/phase1.rs`: new Phase 1 AE pack loader/audit helpers and
  focused tests.
- `crates/testkit/src/lib.rs`: exported `phase1`.
- `docs/phase_reports/PHASE_1_SAMPLING_TRANSFORM.md`: this report.

## Blockers

- No native scene/recipe mapping for AE pack case ids `CMP_010`, `INT_010`,
  `INT_020`, and `EFF_040`.
- No automated runner that renders those native cases and compares against the
  AE pack PNG layout.
- `MATH_PARITY_STATUS.md` still marks M03/M04/M12/M19 below AE-golden parity
  lock; current renderer code still documents straight RGBA8, approximate
  transform/ease, and sampler/composite assumptions.
- Local host lacks `cargo`; Docker can run checks, but the available Rust image
  lacks rustfmt/rustup.

## Gate Status

Phase 1 cannot pass the conformance gate now. The pack PNGs are consumable and
the selected Phase 1 golden frames are verified, but there is not yet a native
render path that can generate comparable outputs for these AE cases.

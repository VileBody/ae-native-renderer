# Agent Round 8 Effects

Date: 2026-05-04

Scope: Agent 3 / Effects formulas. I stayed inside the assigned effects files
and this report. The worktree was already dirty on entry, including Box Blur,
Glow, Drop Shadow, Minimax, and `docs/EFFECTS.md`; this pass only added a small
Glow evidence test and did not revert or retune existing changes.

## Current Inspection

- `ADBE Box Blur2`: current dirty state already applies time-aware radius and
  iterations, reports `iterations_applied`, first-iteration hashes, and an
  explicit clipped edge policy. No constants were changed in this pass.
- `ADBE Drop Shadow`: current dirty state already exposes source alpha, raw
  offset shadow, blurred shadow, and final hashes. No offset or softness tuning
  was touched here.
- `ADBE Glo2`: current dirty state parses `based_on` / `0001`, has threshold
  source branching, and exposes `threshold_source_rgba`, blurred, scaled, and
  final hashes. Existing tests covered color and alpha source behavior
  separately, but did not pin the debug trace intermediate across all branches.
- `ADBE Minimax`: current dirty state has operation/channel parsing plus a
  render hash trace. Neighborhood shape and AE enum semantics still need
  AE-side probes before more formula work.

## Patch

- Added `glow::tests::debug_trace_reports_distinct_threshold_sources_for_based_on_modes`.
- The test uses one two-pixel input to isolate Glow threshold-source behavior:
  a dark high-alpha pixel and a bright low-alpha pixel at threshold `120`.
- The test verifies `glow_debug_trace(...).hashes.threshold_source_rgba` matches
  the expected threshold source canvas for `combined`, `color_channels`, and
  `alpha_channel`, and asserts the representative source pixels for each branch.
- No Glow constants, radius mapping, intensity scaling, composite behavior, or
  docs parameter contract were changed in this pass.

## Manual Inspection

Representative fixture:

```text
input pixels:
  x0 [32, 32, 32, 255]   dark RGB, alpha above threshold
  x1 [240, 240, 240, 64] bright RGB, alpha below threshold
threshold: 120, radius: 0, intensity: 1.0
```

What the native intermediate shows:

| based_on | threshold source pixels | source hash |
| --- | --- | --- |
| `combined` | `[[32,32,32,255], [240,240,240,64]]` | `0x2b1db0a3793ebbd7` |
| `color_channels` | `[[0,0,0,0], [240,240,240,64]]` | `0x948551ecfbbc32a6` |
| `alpha_channel` | `[[32,32,32,255], [0,0,0,0]]` | `0xbbf2b9ddbdb4cb07` |

Human read: the branch split is visible at the mask/source level, not only in
the final composite. `color_channels` correctly ignores the opaque dark pixel;
`alpha_channel` correctly ignores the bright translucent pixel; `combined`
admits both. The suspicious part is unchanged: this is native-side evidence
only. Without AE intermediate exports for the same two-pixel source, we still
should not tune threshold math, radius/kernel mapping, intensity, or blending.

## Verification

Docker commands used a separate target directory:
`target/ae_agents/round8_effects`.

```text
docker cargo test -p effects
result: passed, 45 tests

docker cargo fmt -p effects -- --check
result: passed after installing rustfmt in the ephemeral container

git diff --check
result: passed
```

Note: the first Docker `cargo test` attempt failed because `cargo` was not in
PATH for the arbitrary container UID. Re-running with
`/usr/local/cargo/bin` in PATH passed. The first fmt attempt found that
`cargo-fmt` was not installed for the container toolchain; installing the
`rustfmt` component inside the ephemeral container fixed that.

## Next AE Tuning Steps

1. Add an AE Glow intermediate probe matching the two-pixel source above for
   `0001` absent/default, `0001=1`, and `0001=2`.
2. Compare AE and native `threshold_source_rgba` first. Only if that matches,
   move to `blurred_glow_rgba`, then `intensity_scaled_glow_rgba`, then final
   composite.
3. Keep Box Blur iteration `1/2/3` impulse and edge probes as the next blur
   kernel gate, because Glow and Drop Shadow both depend on blur behavior.
4. Keep Minimax enum/neighborhood probes separate from Glow tuning; do not use
   composed stack metrics to retune these primitives.

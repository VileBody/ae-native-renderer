# Phase 3 Effects: Blur / Shadow / Glow / Minimax

Date: 2026-05-03

Scope: M10 Drop Shadow / Box Blur, M11 Glow, M13 Minimax, plus stack order probes against AE goldens.

## Files Changed

- `crates/testkit/src/phase3.rs` - added focused Phase 3 conformance checks for manifest selection, AE golden existence/dimensions, operator-passport decomposition, and stack coverage.
- `crates/testkit/src/lib.rs` - exported `phase3` testkit module.
- `docs/phase_reports/PHASE_3_EFFECTS_BLUR_SHADOW_GLOW_MINIMAX.md` - this report.

No effect formula files were changed.

## AE Goldens

Manifest: `fixtures/ae_conformance_pack/manifest.json`

Selected Phase 3 frames:

| Case | Ladder | Modules | Selected frames | Golden checked |
| --- | --- | --- | --- | --- |
| `EFF_010` Drop Shadow on alpha square | `operator_static` | `M10` | `0` | `ae_goldens/png/EFF_010/EFF_010_00000.png` |
| `EFF_020` Glow on luma ramp | `operator_static` | `M11` | `0` | `ae_goldens/png/EFF_020/EFF_020_00000.png` |
| `EFF_030` Box Blur on impulse | `operator_static` | `M10` | `0` | `ae_goldens/png/EFF_030/EFF_030_00000.png` |
| `EFF_050` Minimax alpha-square morphology | `operator_static` | `M13` | `0` | `ae_goldens/png/EFF_050/EFF_050_00000.png` |
| `STK_010` Drop Shadow x2 stack | `stack` | `M10`, `M19` | `0` | `ae_goldens/png/STK_010/STK_010_00000.png` |
| `STK_020` Blur then Minimax non-commutative stack | `stack` | `M10`, `M13`, `M19` | `0` | `ae_goldens/png/STK_020/STK_020_00000.png` |

All selected files exist locally and were verified with `file` as `512 x 512`, 8-bit/color RGBA, non-interlaced PNGs.

## Decomposition

| Block | Primitive | Interpolation | Static operator | Animated operator | Stack / retest |
| --- | --- | --- | --- | --- | --- |
| Box Blur / `ADBE Box Blur2` | `impulse_center` in `EFF_030`; `alpha_square`/`hard_edge` in `STK_020` | Params accept wrapped/direct scalar values; current effect parser is static unless render-core pre-evaluates animation | `crates/effects/src/box_blur.rs` uses separable box blur, integer rounded radius, one pass; `iterations` is parsed but not applied | Covered by manifest only through broader `EFF_070`; no Phase 3 pixel assertion added | `STK_020` establishes Blur then Minimax non-commutative order |
| Drop Shadow / `ADBE Drop Shadow` | `alpha_square` in `EFF_010` and `STK_010` | Params accept wrapped/direct values; current effect parser is static unless render-core pre-evaluates animation | `drop_shadow.rs` builds alpha-derived colored shadow, integer rounded offset, box blur softness/2, then normal composite | Covered by manifest only through broader `EFF_070`; no Phase 3 pixel assertion added | `STK_010` covers repeated Drop Shadow order and compositing |
| Glow / `ADBE Glo2` | `luma_ramp` in `EFF_020` | Params accept wrapped/direct values; current effect parser is static unless render-core pre-evaluates animation | `glow.rs` thresholds by luma or alpha, box-blurs radius/2, scales intensity, normal-composites original | Covered by manifest only through broader `EFF_070`; no Phase 3 pixel assertion added | No dedicated Glow stack in Phase 3 manifest |
| Minimax / `ADBE Minimax` | `alpha_square` in `EFF_050`; `hard_edge` in `STK_020` | `radius` uses `param_f32_at_any`, including simple linear keyframes; operation/channels are static enum params | `minimax.rs` applies square-neighborhood min/max over alpha or RGBA with clamped integer radius | Animated radius parsing has unit coverage; AE animated golden comparison is not present in Phase 3 selected cases | `STK_020` covers Blur then Minimax order |

## Native vs AE Assumptions

Current native code remains approximate. The most important assumptions that still need AE telemetry before formula tuning:

- Box Blur: AE kernel shape, edge policy, radius-to-kernel mapping, and iteration semantics are not proven. Native parses `iterations` but does not run multiple blur passes.
- Drop Shadow: AE direction coordinate convention, subpixel offset behavior, softness mapping, premult/straight alpha treatment, and shadow compositing are not proven.
- Glow: AE threshold units/range, alpha-vs-luma participation, radius mapping, glow blending mode, and intensity scaling are not proven.
- Minimax: AE operation enum mapping, channel modes, neighborhood shape, edge behavior, and radius rounding are not proven.
- Stacks: AE goldens exist for non-commutative stacks, but native-vs-AE generated output diffs are not wired into an automated Phase 3 gate yet.

## Checks Added

`crates/testkit/src/phase3.rs` adds:

- `phase3_manifest_selects_static_and_stack_frame_zero`
- `phase3_selected_ae_goldens_exist_and_are_512_rgba`
- `phase3_effect_passports_decompose_required_operator_risks`
- `phase3_stack_cases_cover_non_commutative_ordering`

These checks intentionally stop before formula tuning. They validate that the AE references and operator risk model are present, without overfitting unverified pixel formulas.

## Commands Run

Passed:

```sh
for id in EFF_010 EFF_020 EFF_030 EFF_050 STK_010 STK_020; do test -f "fixtures/ae_conformance_pack/ae_goldens/png/$id/${id}_00000.png" && file "fixtures/ae_conformance_pack/ae_goldens/png/$id/${id}_00000.png"; done
```

Blocked:

```sh
cargo fmt -p testkit
cargo test -p testkit phase3
```

Both Rust commands failed before running because `cargo` is not available in this environment PATH:

```text
zsh:1: command not found: cargo
```

## Pass / Fail

- AE golden presence for selected Phase 3 frame 0: pass.
- Manifest selected-frame decomposition: added as Rust test, not executable here due missing `cargo`.
- Operator passport risk decomposition: added as Rust test, not executable here due missing `cargo`.
- Native-vs-AE pixel diff against `EFF_010`, `EFF_020`, `EFF_030`, `EFF_050`, `STK_010`, `STK_020`: not run; no native render/diff harness was wired in this phase.

## Blockers

- `cargo` is missing from PATH, so targeted Rust tests and formatting could not be executed in this shell.
- Phase 3 still lacks automated native render output generation and image diffs against the checked-in AE goldens.
- Current effect formulas are documented approximations; there is insufficient telemetry to tune kernel, threshold, premult, edge, enum, and stack semantics safely.

## Gate Decision

Phase 3 should not pass the AE parity gate yet.

It can pass a preparatory "goldens and checks are present" gate once the added testkit checks run in a Rust-enabled environment. It cannot pass the full parity gate until native frames are rendered for the six selected cases, compared with `image_diff`, and formula/stack divergences are either within thresholds or explicitly accepted.

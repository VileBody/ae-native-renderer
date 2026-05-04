# Agent Round 3 Effects

Date: 2026-05-03

Scope: M10/M11/M13 isolated effect formula tuning for `EFF_030`,
`EFF_010`, `EFF_020`, `EFF_050`, and animated `EFF_070`.

Owned source files inspected:

- `crates/effects/src/box_blur.rs`
- `crates/effects/src/drop_shadow.rs`
- `crates/effects/src/glow.rs`
- `crates/effects/src/minimax.rs`
- `crates/effects/src/registry.rs`

No Round 3 formula patch was made. Existing source modifications in those files
pre-dated this pass; this agent only wrote this report and generated
`target/ae_agents/round3_effects`.

## Official Docs Check

Sources used as naming and semantic guards only:

- `https://ae-scripting.docsforadobe.dev/matchnames/effects/firstparty/`
- `https://helpx.adobe.com/after-effects/using/blur-sharpen-effects.html`
- `https://helpx.adobe.com/after-effects/using/stylize-effects.html`
- `https://helpx.adobe.com/after-effects/using/perspective-effects.html`

Findings:

- Official first-party match names confirm `ADBE Box Blur2`,
  `ADBE Drop Shadow`, `ADBE Glo2`, and `ADBE Minimax`. I did not add
  `ADBE Glow`; the scripting match-name table lists Glow as `ADBE Glo2`.
- Adobe prose confirms only broad semantics: Fast Box Blur has repeat-edge
  style edge behavior; Drop Shadow is alpha-shaped with opacity, softness,
  Shadow Only, and layer-bound expansion behavior; Glow can be based on color
  or alpha cues with threshold/radius/intensity/composite controls; Minimax is
  a channel effect. These docs do not prove exact kernels or enum ordinals.

## Commands

```text
docker run --rm --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm -lc '
  set -e
  export PATH=/usr/local/cargo/bin:$PATH
  export DEBIAN_FRONTEND=noninteractive
  apt-get update >/tmp/apt-update.log
  apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev libfreetype6-dev libharfbuzz-dev fontconfig >/tmp/apt-install.log
  export CARGO_TARGET_DIR=/work/target/ae_agents/round3_effects/cargo-target
  cargo test -p effects
  cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round3_effects --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_050 --case EFF_070
'
```

Results:

- `cargo test -p effects`: pass, 30 passed.
- `conformance-pack`: pass, `ok=true cases=5`,
  `target/ae_agents/round3_effects/report.json`.

## Before/After Metrics

Before source: `target/ae_agents/round2_integrated/*/metrics.json`.
After source: `target/ae_agents/round3_effects/*/metrics.json`.

| Case | Before RGB max | Before RGB mean | Before BG-alpha-norm mean | After RGB max | After RGB mean | After BG-alpha-norm mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `EFF_010` | 116 | 0.1635 | 1.2643 | 116 | 0.1635 | 1.2643 |
| `EFF_020` | 137 | 12.6578 | 15.3025 | 137 | 12.6578 | 15.3025 |
| `EFF_030` | 0 | 0.0000 | 0.0000 | 0 | 0.0000 | 0.0000 |
| `EFF_050` | 250 | 3.5333 | 2.6674 | 250 | 3.5333 | 2.6674 |
| `EFF_070` | 126 | 1.8913 | 2.8430 | 126 | 1.8913 | 2.8430 |

The after metrics match Round 2 exactly for these cases. That rules out a
Round 3 regression but does not prove any new formula correction.

## Sidecar Evidence

Generated sidecars:

- `target/ae_agents/round3_effects/effects_debug/EFF_010/0/EFF_010_drop_shadow_0_ADBE_Drop_Shadow.json`
- `target/ae_agents/round3_effects/effects_debug/EFF_020/0/EFF_020_glow_0_ADBE_Glo2.json`
- `target/ae_agents/round3_effects/effects_debug/EFF_030/0/EFF_030_box_blur_0_ADBE_Box_Blur2.json`
- `target/ae_agents/round3_effects/effects_debug/EFF_050/0/EFF_050_minimax_0_ADBE_Minimax.json`
- `target/ae_agents/round3_effects/effects_debug/EFF_070/{0,10,20,30,45,59}/...`

Important resolved values:

| Case | Operator | Native sidecar resolved values |
| --- | --- | --- |
| `EFF_010` | Drop Shadow | `direction=135`, `distance=28`, `dx=-20`, `dy=20`, `softness=18`, `blur_radius=9`, `opacity_normalized=0.705882` |
| `EFF_020` | Glow | `threshold=120`, `radius=35`, `kernel_radius=18`, `intensity=1.25`, `threshold_source_rgba == input_rgba` |
| `EFF_030` | Box Blur | `radius=18`, `iterations=3`, `kernel_radius=18`, RGB and BG-alpha-normalized exact |
| `EFF_050` | Minimax | `operation=minimum`, `channels=alpha`, `radius=12`, `kernel_radius=12` |
| `EFF_070` | Animated Box Blur | frame 0 `radius=1`, frame 30 `radius=14.5`, frame 59 `radius=27.55` |
| `EFF_070` | Animated Glow | frame 0 `radius=10`, frame 30 `radius=32.5`, frame 59 `radius=54.25`; threshold source remains full input |

## Failure Ranking

Ranked by split metrics plus earliest native intermediate that could explain
the visible mismatch. Current sidecars contain native intermediates only, so the
rank is diagnostic, not proof of an exact formula fix.

| Rank | Case | First unresolved operator/intermediate | Evidence | Decision |
| ---: | --- | --- | --- | --- |
| 1 | `EFF_020` Glow | `threshold_source_rgba` | Highest isolated RGB mean: 12.6578. Native threshold source equals input because the implementation admits pixels by alpha as well as luminance. Adobe docs allow color- or alpha-based glow, but the AE per-param mode and AE threshold mask are not captured. | No patch. Need AE threshold-source or resolved `Glow Based On` telemetry. |
| 2 | `EFF_010` Drop Shadow | `raw_offset_shadow_rgba` vs `blurred_shadow_rgba` | Low but real RGB mean: 0.1635. Native sidecar resolves `dx=-20`, `dy=20`, `blur_radius=9`. Adobe docs confirm alpha-shaped shadow and softness, but not direction convention or softness kernel. | No patch. Need AE raw offset shadow and blurred shadow hashes/images. |
| 3 | `EFF_050` Minimax | `output_rgba` | RGB mean 3.5333 over only 1.4782% changed pixels, with high max 250. Sidecar currently has only input/output, so enum/channel/neighborhood cannot be separated. | No patch. Need AE resolved operation/channel and neighborhood output sidecar. |
| 4 | `EFF_070` animated Glow branch | `threshold_source_rgba`, then blur/composite | Aggregate RGB mean 1.8913 and grows from frame 0 to 59. Animated Box Blur and Glow radius sampling is active, but the same Glow mask uncertainty from `EFF_020` remains. | No patch. Need branch-isolated AE sidecars for animated Glow. |
| 5 | `EFF_030` Box Blur | None visible after background alpha normalization | RGB and BG-alpha-normalized metrics are exact. Sidecar shows `iterations=3` is parsed, but this fixture does not prove a kernel or iteration mismatch. | No patch. Need a non-degenerate isolated AE blur intermediate before tuning iterations/edge policy. |

## Files Changed

- Added `docs/phase_reports/AGENT_ROUND3_EFFECTS.md`.
- Generated `target/ae_agents/round3_effects/`.
- No effect source formula changes were made in this pass.

## Remaining Blockers

The current sidecars are enough to know the native resolved params and native
operator hashes, but not enough to prove the first divergent formula against AE.
Needed telemetry:

- AE-exported or AE-derived intermediate hashes/images for Glow threshold
  source, blurred glow, intensity-scaled glow, and composite result.
- AE raw offset shadow and softened shadow for Drop Shadow before final
  composite.
- AE Minimax resolved operation/channel/radius plus post-neighborhood output.
- A Box Blur case whose RGB does not already match after background alpha
  normalization, or AE blur pass intermediates for the existing impulse case.

Until those are available, tuning radius, enum ordinals, opacity units, or
composite math would be prose-driven rather than sidecar-proven.

# Agent 3 Effects Native Diff

Date: 2026-05-03

Scope: M10 Drop Shadow / Box Blur, M11 Glow, M13 Minimax, and M19 alpha/composite dependency.

## Modules And Cases

| Module | Cases | Notes |
| --- | --- | --- |
| M10 Drop Shadow / Box Blur | EFF_010, EFF_030, EFF_070, STK_010, STK_020 | Box Blur is also a dependency of Drop Shadow softness and Glow blur. |
| M11 Glow | EFF_020, EFF_070 | Static luma-ramp case plus animated radius case. |
| M13 Minimax | EFF_050, STK_020 | Static alpha-square morphology plus non-commutative stack probe. |
| M19 Color / alpha / composite | STK_010, STK_020, all rendered frames as dependency | Current diffs are dominated by background/output alpha policy. |

Primary cases rendered:

```text
EFF_010, EFF_020, EFF_030, EFF_050, EFF_070, STK_010, STK_020
```

Output:

```text
target/ae_agents/agent3_effects_blur_shadow_glow/
```

## Commands

Host cargo was unavailable:

```sh
cargo --version
```

Result:

```text
zsh:1: command not found: cargo
```

Successful native conformance run:

```sh
docker run --rm --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc '
set -e
export DEBIAN_FRONTEND=noninteractive
export PATH=/usr/local/cargo/bin:$PATH
apt-get update
apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev fontconfig
cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/agent3_effects_blur_shadow_glow --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_050 --case EFF_070 --case STK_010 --case STK_020
'
```

Result:

```text
conformance-pack.done ok=true cases=7 report=target/ae_agents/agent3_effects_blur_shadow_glow/report.json
```

Note: `ok=true` means measured with no thresholds supplied; it is not an AE parity pass.

## Top Metrics

From each case `metrics.json` summary:

| Case | Frames | Max abs | Mean abs | RMSE abs | Changed pixel ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| EFF_050 | 1 | 255 | 64.4476 | 128.0682 | 0.9839 |
| EFF_030 | 1 | 255 | 63.7500 | 127.5000 | 1.0000 |
| STK_010 | 1 | 255 | 62.1006 | 125.6464 | 0.9780 |
| EFF_010 | 1 | 255 | 61.8626 | 125.3641 | 0.9743 |
| EFF_070 | 6 | 255 | 60.0135 | 122.2775 | 0.9791 |
| EFF_020 | 1 | 255 | 55.9842 | 111.3102 | 0.9209 |
| STK_020 | 1 | 255 | 53.2829 | 113.7625 | 0.8665 |

EFF_070 per-frame mean abs stays almost flat despite animated effect params:

| Frame | Mean abs | RMSE abs | Changed pixels |
| ---: | ---: | ---: | ---: |
| 0 | 60.0627 | 122.6758 | 256187 |
| 10 | 60.0364 | 122.5301 | 256199 |
| 20 | 60.0183 | 122.3824 | 256479 |
| 30 | 60.0047 | 122.2382 | 256619 |
| 45 | 59.9864 | 122.0195 | 257065 |
| 59 | 59.9724 | 121.8171 | 257485 |

## First Divergent Primitive

The first visible divergence is M19 output alpha/background policy, before effect formulas can be judged from full RGBA metrics.

Observed from generated PNGs:

| Case | Native alpha | AE alpha | Alpha diagnosis |
| --- | --- | --- | --- |
| EFF_010 | alpha bbox full frame, alpha mean 255 | alpha bbox `(214,214)-(316,316)`, alpha mean 8.04 | Native keeps opaque comp background. |
| EFF_020 | alpha bbox full frame, alpha mean 255 | alpha bbox `(95,95)-(417,417)`, alpha mean 69.04 | Glow alpha is swamped by opaque background alpha. |
| EFF_030 | alpha bbox full frame, alpha mean 255 | no nonzero alpha | Diff is entirely background alpha; RGB is identical. |
| EFF_050 | alpha bbox full frame, alpha mean 255 | alpha bbox `(211,211)-(301,301)`, alpha mean 7.81 | Minimax morphology cannot be judged by RGBA summary yet. |
| STK_020 | alpha bbox full frame, alpha mean 255 | alpha bbox `(60,166)-(452,346)`, alpha mean 62.48 | Stack diff includes M19 alpha plus effect math. |

Native comp background comes from `ae_background_rgba8() = [5, 5, 6, 255]`, while the AE goldens preserve the same dark RGB background with transparent alpha outside content. This makes every frame hit max diff 255 and changes most pixels.

## Effect Diagnoses After M19

These are based on RGB-only checks and content bounding boxes, so they are diagnostic rather than formula-tuning proof.

| Area | Evidence | Suspected first effect primitive |
| --- | --- | --- |
| Drop Shadow | EFF_010 native RGB content bbox `(192,214)-(298,321)` while AE alpha/content is around `(214,214)-(316,316)`. STK_010 similarly differs in vertical/horizontal spread. | Shadow offset convention is likely wrong before softness tuning: native uses `dx=cos(direction)*distance`, `dy=sin(direction)*distance`, integer-rounded, then box blur `softness/2`. Need shadow mask/offset telemetry. |
| Box Blur | EFF_030 RGB is identical after the impulse is blurred away, so the RGBA failure is M19-only for this fixture. Code still parses `iterations` but does not apply repeated passes. | Current impulse case is too weak after alpha fix unless it captures kernel before quantization. Need blur kernel/intermediate telemetry and an impulse/ramp that survives rounding. |
| Glow | EFF_020 RGB mean diff is 12.658 with native content bbox `(117,112)-(400,400)` and AE content reaching roughly `(95,95)-(417,417)`. Native threshold condition accepts pixels when `luminance >= threshold || alpha >= threshold`, so the fully opaque luma ramp makes the whole ramp eligible. | Glow threshold/mask semantics likely diverge before blend tuning. Need threshold mask, blurred glow buffer, and final blend telemetry. |
| Minimax | EFF_050 native RGB content bbox `(223,223)-(289,289)` while AE content bbox is `(211,211)-(301,301)`, close to the original scaled square. Native maps operation `0001=2` to Minimum and channel `0003=1` to Alpha, then erodes a square neighborhood. | Minimax operation/channel enum mapping or channel semantics diverge before radius tuning. Need operation/channel logs and neighborhood mask telemetry. |
| Animated params | EFF_070 native RGB bbox remains `(170,181)-(418,331)` for all frames while AE bbox changes over time. BoxBlur2 and Glow use `param_f32_any`, which does not evaluate `{ "keyframes": [...] }`; Minimax already uses `param_f32_at_any`. | Tiny next patch should make BoxBlur2 radius/iterations and Glow threshold/radius/intensity time-aware with `param_f32_at_any`, then rerun EFF_070. |
| Stack order | `render-core` applies effects in scene order with `for spec in effects`. STK_020 scene has left `Box Blur2 -> Minimax` and right `Minimax -> Box Blur2` as intended. | No stack-order bug proven yet; STK_010/STK_020 failures currently inherit M19 plus primitive effect differences. Need per-effect intermediate PNGs for stack layers. |

EFF_070 manifest lists M14, but the generated `scene.json` from current `conformance_pack.rs` contains only `ADBE Box Blur2` and `ADBE Glo2` layers for this case. I did not assign EFF_070 findings to Turbulent Displace in this Agent 3 report.

## Next Telemetry Or Formula Patch Needed

Do these before formula tuning:

1. Add or expose an M19 diagnostic mode for conformance diffs: compare straight RGBA with AE-compatible transparent background alpha, or emit a second metric with background alpha normalized. Without this, max/mean diffs are dominated by alpha 255 vs 0.
2. Emit effect intermediates under each case directory:
   - Box Blur: input alpha, kernel radius, iteration count, horizontal pass, final blur.
   - Drop Shadow: source alpha mask, raw offset mask, blurred shadow mask, shadow-only composite.
   - Glow: threshold mask, blurred glow buffer, intensity-scaled buffer, blended result.
   - Minimax: operation enum, channel enum, radius, neighborhood shape, post-morph alpha/RGB.
3. Apply the tiny animated-param diagnostic patch for BoxBlur2 and Glow by switching numeric params that can be animated from `param_f32_any` to `param_f32_at_any`; rerun EFF_070 before any AE formula tuning.
4. After M19 normalization and intermediates, retest isolated EFF_010/EFF_020/EFF_030/EFF_050 before using STK_010/STK_020 as stack gate evidence.

## Gate Decision

Agent 3 native conformance is now measured for all requested cases, but M10/M11/M13 cannot move into formula tuning from the current full RGBA metrics. The first blocker is M19 alpha/background parity. The next useful Agent 3 work is telemetry plus the small time-varying parameter parser fix for BoxBlur2/Glow, not shadow/glow/minimax formula tuning yet.

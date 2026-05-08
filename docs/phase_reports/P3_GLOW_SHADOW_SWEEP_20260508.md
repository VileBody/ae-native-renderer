# P3 Glow / Shadow Sweep

Date: 2026-05-08.

## Scope

This pass audited the current Glow and Drop Shadow implementations against the
existing ImageRenderer/GPUFoundation evidence and reran the focused
`EFF_010` / `EFF_020` / `EFF_070` / `STK_010` / `STK_020` conformance slice.

Read inputs:

- `docs/phase_reports/M11_IMAGERENDERER_RECURSIVE_GAUSSIAN_20260507.md`
- `docs/phase_reports/BOX_BLUR_GLOW_FAST_PATH_20260505.md`
- `docs/phase_reports/DROP_SHADOW_SOFTNESS_20260505.md`
- `docs/phase_reports/DROP_SHADOW_COMPOSITE_20260505.md`
- `docs/phase_reports/AGENT_ROUND8_EFFECTS.md`
- `docs/EFFECTS.md`
- current `crates/effects/src/glow.rs`
- current `crates/effects/src/drop_shadow.rs`

## Evidence Checks

- Glow radius routing remains the Frida-confirmed
  `ir_gaussian_radius = glow_radius * 0.4`.
- Glow recursive Gaussian still uses the recovered causal / anti-causal
  coefficient path and zero padding outside the active line.
- Drop Shadow softness already matches the dynamic CPU trace:
  `ceil((softness * 0.5) / 2.71)`, one alpha-only blur iteration.
- Drop Shadow opacity/color semantics already match the composite probe:
  opacity is raw `0..255`, source alpha multiplies it, and the color property's
  alpha byte is ignored.
- `IR_CompositeWithBlendMode` remains only partially recovered for Glow. This
  pass did not retune blend/premult policy without a direct worker probe.

## Patch

- `crates/effects/src/glow.rs`
  - kept the ImageRenderer recursive Gaussian horizontal and vertical passes in
    float storage;
  - moved RGBA8 quantization to the final Glow blur boundary instead of after
    the horizontal pass;
  - added `recursive_gaussian_quantizes_once_after_both_axes` to guard against
    reintroducing the intermediate `u8` round trip.

No Drop Shadow formula changes were made. No changes were made to
`crates/effects/src/turbulent_displace.rs`.

## Verification

Commands:

```text
CARGO_TARGET_DIR=target/p3_glow_shadow_sweep_cargo cargo test -p effects
CARGO_TARGET_DIR=target/p3_glow_shadow_sweep_cargo cargo run -p render-cli -- conformance-pack --case EFF_010 --case EFF_020 --case EFF_070 --case STK_010 --case STK_020 --out target/ae_agents/p3_glow_shadow_sweep_float_blur_20260508
python3 scripts/build_visual_review_pack.py --before target/ae_agents/p3_glow_shadow_sweep_baseline_20260508 --after target/ae_agents/p3_glow_shadow_sweep_float_blur_20260508 --out target/visual_review/p3_glow_shadow_sweep_20260508 --case EFF_010 --case EFF_020 --case EFF_070 --case STK_010 --case STK_020
```

Unit result:

```text
effects: 84 passed
```

Focused conformance primary visible metric
`rgb_straight_source_over_ae_background`:

| Case | Baseline | Float-staged blur | Delta |
| --- | ---: | ---: | ---: |
| `EFF_010` | 0.115046 | 0.115046 | +0.000000 |
| `EFF_020` | 1.628605 | 1.629317 | +0.000712 |
| `EFF_070` | 0.352847 | 0.353011 | +0.000164 |
| `STK_010` | 0.069822 | 0.069822 | +0.000000 |
| `STK_020` | 4.836876 | 4.836876 | +0.000000 |

Alpha mean:

| Case | Baseline | Float-staged blur | Delta |
| --- | ---: | ---: | ---: |
| `EFF_010` | 0.910919 | 0.910919 | +0.000000 |
| `EFF_020` | 3.148506 | 3.156036 | +0.007530 |
| `EFF_070` | 1.302526 | 1.302537 | +0.000011 |
| `STK_010` | 0.906399 | 0.906399 | +0.000000 |
| `STK_020` | 9.030304 | 9.030304 | +0.000000 |

Artifact dirs:

```text
target/ae_agents/p3_glow_shadow_sweep_baseline_20260508
target/ae_agents/p3_glow_shadow_sweep_float_blur_20260508
target/visual_review/p3_glow_shadow_sweep_20260508
target/ae_agents/p3_glow_shadow_sweep_float_blur_gate_20260508
```

The focused dashboard classification was generated from a five-case report only,
so it records missing cases for the rest of the master policy. The relevant
rendered cases stayed regression-free under policy thresholds.

## Remaining P3 Blockers

- Glow `IR_CompositeWithBlendMode` worker details: exact premult/straight
  staging, opacity, and blend alpha handling for Add/Screen/Normal need direct
  static or dynamic evidence before changing composite math.
- Glow color-loop controls (`0007` onward) and unsupported operation modes are
  still not implemented.
- Glow residual is not solved by this quantization boundary correction; the
  isolated `EFF_020` visible mean remains around `1.63`.
- Drop Shadow fractional direction/distance and nonzero-softness edge/kernel
  shape remain narrower follow-up probes, but current opacity, color, source
  alpha, and softness-radius semantics are already evidence-backed.
- `STK_020` remains high, but it is Box Blur + Minimax and is not owned by this
  Glow/Shadow sweep.

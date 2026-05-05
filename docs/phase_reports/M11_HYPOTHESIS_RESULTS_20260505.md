# M11 Glow Hypothesis Results - 2026-05-05

Agent B scope: M11 Glow only. Local Rust toolchain was used; Docker was not used.
No edits were made to `crates/effects/src/glow.rs`, M19, shared alpha, or other
agents' modules.

## Context Read

- `docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md`, section
  `M11 Glow`
- `docs/reverse_engineering/effect_math_blur_glow_shadow.md`
- `crates/effects/src/glow.rs`

Current native Glow already has the required first-pass telemetry:
`GlowBasedOn`, threshold source hash, blurred glow hash, intensity-scaled glow
hash, final hash, alpha stats, and an M19 alpha/composite diagnostic policy.
No additional local candidate switch was required for this pass.

## First-Pass Hypotheses

H1. `Glow Based On` enum/source:

- Native mapping: absent `0001` -> `combined`; `0001=1` -> `color_channels`;
  `0001=2` -> `alpha_channel`.
- Native source predicates:
  - `combined`: luma >= threshold OR alpha >= threshold
  - `color_channels`: luma >= threshold
  - `alpha_channel`: alpha >= threshold
- Status: `accepted` for native parser/instrumentation; `needs_new_probe` for
  AE's exact absent-`0001` default because current evidence is final-output-only.

H2. Threshold source:

- Current native thresholds before blur in straight RGBA8 space and copies the
  original pixel into `threshold_source` when the predicate passes.
- `EFF_020` and `EFF_070` both resolve `threshold_source_rgba == input_rgba`
  because the source primitives are fully opaque and the absent-`0001` branch is
  `combined`.
- Status: `needs_new_probe`. This is the first discriminator before tuning blur,
  intensity, or blend.

H3. Blur route/radius mapping:

- Current native uses shared separable box average via `blur_canvas`.
- Glow maps radius with `kernel_radius = ceil(radius / 2)`, clamped by shared
  `blur_radius`.
- Evidence in this run: `EFF_020 radius=35 -> kernel_radius=18`; `EFF_070`
  samples animated radius and maps `10 -> 5`, `17.5 -> 9`, `25 -> 13`,
  `32.5 -> 17`, `43.75 -> 22`, `54.25 -> 28`.
- Status: `instrumented`, not accepted against AE Glow internals until H2 is
  isolated.

H4. Intensity scaling:

- Current native scales RGB by `intensity` and alpha by
  `clamp(intensity, 0, 8)`, then rounds/clamps to RGBA8.
- Evidence in this run: `EFF_020 intensity=1.25`, `EFF_070 intensity=0.5`;
  sidecars record `intensity_scaled_glow_rgba`.
- Status: `instrumented`, not accepted; should not be tuned before threshold
  source is proven.

H5. Final composite:

- Current native composites as normal source-over: input over scaled glow.
- Status: `needs_new_probe` / `ORCHESTRATOR_BLOCKER`. Reports surface the M19
  alpha/composite guardrail: raw RGBA is compatibility-only, preferred tuning
  metrics are `rgb_straight_source_over_ae_background`,
  `background_alpha_normalized`, and `alpha`. Glow final composite may depend on
  ImageRenderer/M19 behavior, so this pass does not mask it with a local formula.

## Commands Run

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M11 \
  --candidate glow_firstpass_current_eff020 \
  --status instrumented \
  --gate isolated \
  --question "Does the current first-pass Glow formula explain isolated EFF_020?" \
  --hypothesis "Absent 0001 uses combined luma-or-alpha threshold, threshold happens pre-blur in straight RGBA, radius maps through ceil(radius/2), intensity scales RGB and alpha, and final composite is source-over input over scaled glow." \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md \
  --evidence docs/reverse_engineering/effect_math_blur_glow_shadow.md \
  --case EFF_020 \
  --out target/ae_agents/m11_glow_firstpass_eff020
```

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M11 \
  --candidate glow_firstpass_current_eff070 \
  --status instrumented \
  --gate isolated \
  --question "Does the current first-pass Glow formula hold when EFF_070 animates blur and glow radius?" \
  --hypothesis "Animated Glow samples threshold/radius/intensity at frame time, then applies the same combined threshold source, ceil(radius/2) blur route, RGB+alpha intensity scaling, and source-over final composite." \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md \
  --evidence docs/reverse_engineering/effect_math_blur_glow_shadow.md \
  --case EFF_070 \
  --out target/ae_agents/m11_glow_firstpass_eff070
```

## Metrics

Verification commands:

```bash
cargo fmt --all
cargo test -p effects glow
```

Results: both passed. `cargo test -p render-cli conformance_pack` was not run
because no shared code was changed.

| Case | Frames | Visible RGB mean | Visible RGB RMSE | Background alpha-normalized mean | Alpha mean | Max diff | Changed pixels |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `EFF_020` | 1 | 11.1764 | 31.2360 | 9.2992 | 1.7450 | 137 | 81916 |
| `EFF_070` | 6 | 1.6587 | 9.6173 | 1.5212 | 0.7011 | 123 | 148127 |

`EFF_020` sidecar:

- `based_on=combined`, `based_on_param_source=default_absent`
- `threshold=120`, `radius=35`, `kernel_radius=18`, `intensity=1.25`
- `input_rgba=0x8b604016e3289165`
- `threshold_source_rgba=0x8b604016e3289165`
- alpha nonzero pixels: input `65536`, threshold source `65536`, blurred glow
  `85220`, intensity-scaled glow `85220`, final `85220`

`EFF_070` animated Glow sidecar summary:

| Frame | Radius | Kernel radius | Intensity | Threshold-source nonzero | Blurred nonzero | Final nonzero |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 10.0 | 5 | 0.5 | 19600 | 22500 | 22500 |
| 10 | 17.5 | 9 | 0.5 | 19600 | 24960 | 24960 |
| 20 | 25.0 | 13 | 0.5 | 19600 | 27544 | 27544 |
| 30 | 32.5 | 17 | 0.5 | 19600 | 30244 | 30244 |
| 45 | 43.75 | 22 | 0.5 | 19600 | 33780 | 33780 |
| 59 | 54.25 | 28 | 0.5 | 19600 | 38264 | 38264 |

## Evidence Interpretation

- Round 5 AE outputs prove that `0001` is a real branch point:
  `GLO_090`, `GLO_100`, and `GLO_101` differ materially.
- Native sidecars prove current parser/telemetry can separate absent `0001`,
  raw `0001=1`, and raw `0001=2`.
- The current first-pass candidate remains too broad for `EFF_020`: the absent
  combined branch admits the whole opaque luma ramp as threshold source, while
  final visible RGB miss remains high.
- `EFF_070` confirms animated Glow parameter sampling and radius mapping are
  active, but it is not an isolated Glow-only proof because the case also
  includes animated Box Blur and other module ownership (`M04`, `M10`, `M14`).
- M19/global alpha policy surfaced in report diagnostics. This pass treats it as
  `ORCHESTRATOR_BLOCKER` for final composite interpretation and does not hide it
  with a local Glow formula.

## Result

- Accepted:
  - Native `GlowBasedOn` parser/instrumentation for absent, `0001=1`, and
    `0001=2`.
  - Native animated Glow parameter sampling in `EFF_070`.
  - Native first-pass radius mapping is instrumented and visible in sidecars.

- Rejected:
  - Treating current final-pixel mismatch as evidence to tune radius, intensity,
    or blend immediately. H2 is not isolated yet.
  - Using raw RGBA as the M11 tuning metric. M19 reports mark raw RGBA as
    compatibility-only.

- Needs new probe:
  - AE threshold-source intermediate for absent `0001`, `0001=1`, and `0001=2`.
  - AE source-mask pack with dark/high-alpha and bright/low-alpha pixels,
    threshold `120`, radius `0`, intensity `1`, then radius sweep.
  - After threshold source is locked, rerun `EFF_020` before testing blur
    radius, intensity scaling, or final composite variants.

## Next Discriminator

Build/run the tiny AE Glow source-mask probe described in
`effect_math_blur_glow_shadow.md`:

- source pixels: dark/high-alpha `[32,32,32,255]` and bright/low-alpha
  `[240,240,240,64]`
- params: threshold `120`, radius `0`, intensity `1`
- variants: absent `0001`, `0001=1`, `0001=2`

This directly discriminates luma-only, alpha-only, and combined threshold source
without blur/intensity/composite confounds.

## Artifacts

- `target/ae_agents/m11_glow_firstpass_eff020/report.json`
- `target/ae_agents/m11_glow_firstpass_eff020/hypothesis_report.json`
- `target/ae_agents/m11_glow_firstpass_eff020/effects_debug/EFF_020/0/EFF_020_glow_0_ADBE_Glo2.json`
- `target/ae_agents/m11_glow_firstpass_eff070/report.json`
- `target/ae_agents/m11_glow_firstpass_eff070/hypothesis_report.json`
- `target/ae_agents/m11_glow_firstpass_eff070/effects_debug/EFF_070/{0,10,20,30,45,59}/EFF_070_animated_glow_0_ADBE_Glo2.json`

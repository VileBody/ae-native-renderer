# M10 Hypothesis Results: Drop Shadow / Box Blur

Date: 2026-05-05

Agent: Agent A

Scope:

- Owned code reviewed: `crates/effects/src/drop_shadow.rs`,
  `crates/effects/src/box_blur.rs`.
- Source docs reviewed:
  `docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md` M10 and
  `docs/reverse_engineering/effect_math_blur_glow_shadow.md`.
- No source-code candidate switch was added. Existing M10 sidecars already expose
  the first-pass parameters, hashes, and alpha stats needed for these gates.

## First-Pass Hypotheses

| ID | Hypothesis | Evidence | Decision | Next exact discriminator |
| --- | --- | --- | --- | --- |
| H1 | Drop Shadow mask source is source alpha only, scaled by opacity and color alpha. | Current code reads `input.pixel(x,y)[3]`; `EFF_010` sidecar reports `source_alpha=input_alpha_channel`, `shadow_mask=offset_alpha_scaled_by_opacity_and_color_alpha`; raw offset coverage equals input coverage: `7056` nonzero px. | needs_new_probe | Colored semi-transparent source where RGB/luma and alpha disagree; run shadow-only no-softness and compare raw offset mask. |
| H2 | Direction convention is `dx=trunc(-cos(deg)*distance)`, `dy=trunc(sin(deg)*distance)`. | Round 5 notes resolve AE `direction=135`, `distance=28` to down/right shadow. `EFF_010` sidecar resolves `dx=19`, `dy=19`. | accepted for `135/28`; needs_new_probe for full angle table | No-softness shadow-only sweep at `0/45/90/135/180/225/270/315` with fractional projected components. |
| H3 | Softness maps to alpha-only blur radius `ceil(softness/2)+1` when softness is positive. | Round 5 softened bbox expands about 10 px for softness `18`; `EFF_010` sidecar resolves `blur_radius=10`; blurred shadow coverage grows from `7056` to `10784` px. | accepted for coarse bbox; needs_new_probe for exact weights | Shadow-only softness sweep `0/1/2/8/18/32`, with AE alpha bbox and alpha histogram. |
| H4 | Box Blur uses ceil radius, rounded iteration count, separable box passes, integer floor average, and clipped layer bounds. | `EFF_030` candidate is exact on all reported metrics: raw RGBA/RGB/alpha/background mean `0`, max `0`, changed pixels `0`. Sidecar resolves `kernel_radius=18`, `iterations_applied=3`, `edge_policy=clip_to_layer_bounds`. | accepted for centered integer-radius impulse `EFF_030`; needs_new_probe for edge and fractional radius | Edge/corner impulse pack with radius `0/0.49/0.5/1/2/10/18`, iterations `1/2/3`, transparent and opaque edge pixels. |
| H5 | Final Drop Shadow composite is normal source-over under-composite: blurred colored shadow under original source. | Current code composites `input` over `shadow`; `EFF_010` sidecar reports `composite=straight_rgba8_normal_source_over`; preferred RGB metric is close but not exact: mean `0.1131159`, changed `4649` px, max `123`; alpha mean `0.8911972`, max `104`. | needs_new_probe | Colored translucent shadow and partial-alpha source with overlap, plus `shadow_only` pair to separate mask/kernel from final composite. |

## Hypothesis-Pack Runs

### `h1_drop_shadow_current`

Command:

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M10 \
  --candidate h1_drop_shadow_current \
  --status instrumented \
  --gate isolated \
  --question "Does current Drop Shadow use source alpha mask, AE 135-degree down-right offset, softness radius ceil(s/2)+1, and source-over-shadow final composite?" \
  --hypothesis "Current Drop Shadow formula matches first-pass M10 geometry/composite hypothesis for EFF_010." \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md#M10 \
  --evidence docs/reverse_engineering/effect_math_blur_glow_shadow.md \
  --candidate-config target/ae_agents/m10_h1_drop_shadow_current/candidate_config.json \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/m10_h1_drop_shadow_current \
  --case EFF_010
```

Artifacts:

- `target/ae_agents/m10_h1_drop_shadow_current/candidate_config.json`
- `target/ae_agents/m10_h1_drop_shadow_current/hypothesis_report.json`
- `target/ae_agents/m10_h1_drop_shadow_current/report.json`
- `target/ae_agents/m10_h1_drop_shadow_current/EFF_010/metrics.json`
- `target/ae_agents/m10_h1_drop_shadow_current/effects_debug/EFF_010/0/EFF_010_drop_shadow_0_ADBE_Drop_Shadow.json`

Metrics summary:

| Metric | mean_abs_diff | rmse_abs_diff | max_abs_diff | changed_pixels |
| --- | ---: | ---: | ---: | ---: |
| rgba | `0.3427582` | `4.4525412` | `104` | `5589` |
| rgb | `0.1599452` | `2.3807448` | `100` | `5504` |
| alpha | `0.8911972` | `7.8928229` | `104` | `5015` |
| background_alpha_normalized | `0.3424797` | `4.4523660` | `104` | `5504` |
| rgb_straight_source_over_ae_background | `0.1131159` | `2.5968612` | `123` | `4649` |

Sidecar facts:

- `dx=19`, `dy=19`.
- `opacity_normalized=0.70588237`.
- `blur_radius=10`.
- raw offset shadow: `7056` nonzero alpha px, alpha sum `1249168`.
- blurred shadow: `10784` nonzero alpha px, alpha sum `1245538`.
- final output: `12228` nonzero alpha px, alpha sum `2272149`.

### `h4_box_blur_current`

Command:

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M10 \
  --candidate h4_box_blur_current \
  --status instrumented \
  --gate isolated \
  --question "Does current Box Blur use ceil radius, rounded repeated separable passes, clipped edges, and integer floor averaging?" \
  --hypothesis "Current Box Blur formula matches first-pass M10 blur-kernel hypothesis for EFF_030." \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md#M10 \
  --evidence docs/reverse_engineering/effect_math_blur_glow_shadow.md \
  --candidate-config target/ae_agents/m10_h4_box_blur_current/candidate_config.json \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/m10_h4_box_blur_current \
  --case EFF_030
```

Artifacts:

- `target/ae_agents/m10_h4_box_blur_current/candidate_config.json`
- `target/ae_agents/m10_h4_box_blur_current/hypothesis_report.json`
- `target/ae_agents/m10_h4_box_blur_current/report.json`
- `target/ae_agents/m10_h4_box_blur_current/EFF_030/metrics.json`
- `target/ae_agents/m10_h4_box_blur_current/effects_debug/EFF_030/0/EFF_030_box_blur_0_ADBE_Box_Blur2.json`

Metrics summary:

| Metric | mean_abs_diff | rmse_abs_diff | max_abs_diff | changed_pixels |
| --- | ---: | ---: | ---: | ---: |
| rgba | `0.0` | `0.0` | `0` | `0` |
| rgb | `0.0` | `0.0` | `0` | `0` |
| alpha | `0.0` | `0.0` | `0` | `0` |
| background_alpha_normalized | `0.0` | `0.0` | `0` | `0` |
| rgb_straight_source_over_ae_background | `0.0` | `0.0` | `0` | `0` |

Sidecar facts:

- `radius=18.0`, `kernel_radius=18`.
- `iterations=3.0`, `iterations_applied=3`.
- `edge_policy=clip_to_layer_bounds`.
- horizontal pass has `37` nonzero alpha px and alpha max `6`.
- first iteration and final output are fully transparent after integer floor
  averaging; this explains why `EFF_030` is exact but weak as a kernel proof.

## Accepted / Rejected / Needs New Probe

Accepted:

- H2 direction convention for the observed `direction=135`, `distance=28`
  tuple.
- H4 Box Blur current integer-radius centered impulse behavior for `EFF_030`.

Rejected:

- None in this pass.

Needs new probe:

- H1 alpha-only mask source needs an RGB/luma-vs-alpha discriminator.
- H2 direction convention needs a full angle/fractional rounding table.
- H3 softness mapping needs shadow-only alpha histograms across softness values.
- H4 blur kernel needs edge and fractional-radius probes.
- H5 final under-composite needs colored translucent overlap probes.

## Blockers

No `ORCHESTRATOR_BLOCKER` was found in this pass. M19 alpha policy appears as an
expected metric guardrail in conformance output, but the docs explicitly scope
effect input/output premultiply wrappers to M10/M11/M13 rather than a global M19
blocker.

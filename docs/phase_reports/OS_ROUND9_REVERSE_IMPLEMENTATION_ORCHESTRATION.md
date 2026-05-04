# OS Round 9 Reverse Implementation Orchestration

Date: 2026-05-04

## Purpose

Round 9 moves from pure analysis into guarded implementation. The rule for this
round is:

```text
rewrite what the predecoded code already proves;
probe only branch/substrate/hidden-kernel questions;
stop on guardrail contradictions.
```

This is not a broad formula-tuning round. A patch is acceptable only when the
agent can point to recovered code, an existing test, or a narrow new primitive
test.

## Active Agents

| Agent | Runtime id | Scope | Write lane | Status |
| --- | --- | --- | --- | --- |
| A / Substrate | `019df4b4-1d21-7890-8d23-28328b37d3ae` | `M19`, metrics, alpha/color guardrails | metrics/testkit/docs | completed; accepted metric gate |
| B / Geometry | `019df4b4-5c75-7a00-9d1d-73dae64bce96` | `M03`, `M04`, `M12`, `M17`, `M18` | transform/geometry/collapse/motion/docs | completed; accepted narrow patch |
| C / Temporal | `019df4b4-8a99-71f0-a36b-19dcccf1da8b` | `M09`, `M15`, `M16`, expression boundary | temporal/adjustment/expression/docs | completed; accepted expression boundary |
| D / Text | `019df4b4-b60d-77c2-87cb-d2ac4993fc3e` | `M05`, `M06`, `M07`, `M08` | text/glyph/animator/docs | completed; accepted telemetry/font patch |
| E / Blur/Glow | `019df4b4-e096-7323-b710-d7041713841f` | `M10`, `M11`, shared blur | blur/drop-shadow/glow/docs | completed; accepted ASM-derived patch |
| F / Fields | `019df4b5-0a49-75b0-80e9-d9c3506865c2` | `M13`, `M14` | minimax/turbulent/docs | completed; accepted Minimax patch |

## Acceptance Gates

| Gate | Meaning | Required evidence |
| --- | --- | --- |
| `direct_from_reverse` | Formula/behavior is visible enough in predecoded code. | Function/address, pseudocode, native delta, primitive test or existing conformance evidence. |
| `needs_probe` | Assembly shows wrapper/dispatch but not the branch or hidden kernel result. | Exact missing question, smallest JSX/probe shape, blocked downstream objects. |
| `blocked_by_guardrail` | Shared assumption is unknown or contradicted. | `BLOCKER` payload with affected modules and stop condition. |
| `manual_review_required` | Metrics can pass while image looks wrong. | Contact sheet or artifact path plus human-readable expected/actual note. |

## Cross-Agent Guardrails

- `M19` alpha/premult/export policy can block final tuning for text blur,
  Drop Shadow, Glow, Minimax alpha, collapse composite, and motion blur
  accumulation.
- `M15/M16` time routing owns Posterize/adjustment semantics. Effects with
  animated params should report their param-time assumptions instead of
  changing temporal routing locally.
- `M12` Geometry2 parameter mapping must preserve the recovered distinction:
  payload `"0008"` can be Rotation while matchName `ADBE Geometry2-0008` is
  Opacity.
- `M05` glyph parity is blocked by CoolType unless the agent adds telemetry or a
  proven native shaping substitute. Fontdue visual matching is not parity.
- `M14` Turbulent final vector math is blocked if the path stays inside hidden
  GPU kernels; field probes or kernel extraction are acceptable next evidence.

## Orchestrator TODO

1. Poll all agents and review changed files before accepting any code patch.
2. Merge only non-conflicting patches; ask agents to narrow if write scopes
   overlap.
3. Run focused tests by lane, then one integration smoke if patches land.
4. Update this board with accepted contracts and rejected assumptions.
5. Convert accepted `needs_probe` items into the next AE probe pack.

## Verification Matrix

| Lane | First checks | Wider checks |
| --- | --- | --- |
| A / `M19` | `cargo test -p testkit image_diff` | native conformance report schema and RGB/alpha metric output |
| B / Geometry | `cargo test -p effects geometry` | selected `EFF_040`, `GPH_010`, motion blur smoke |
| C / Temporal | `cargo test -p effects posterize`; `cargo test -p render-core motion_blur` | selected `TMP_020`, `TMP_030`, `STK_030` |
| D / Text | `cargo test -p text-engine` | selected `TXT_010`-`TXT_040`, `GPH_010` sidecars |
| E / Blur/Glow | `cargo test -p effects box_blur drop_shadow glow` | selected `EFF_010`, `EFF_020`, `EFF_030`, `EFF_070`, `STK_020` |
| F / Fields | `cargo test -p effects minimax turbulent` | selected `EFF_050`, `EFF_060`, `STK_030` |

Final integration should include a manual contact-sheet review for any lane
whose metrics improve but whose artifact could still be visually wrong.

Baseline focused checks in Docker:

```text
cargo test -p testkit image_diff                         9 passed
cargo test -p effects geometry                           13 passed
cargo test -p effects box_blur                           9 passed
cargo test -p effects drop_shadow                        5 passed
cargo test -p effects glow                               8 passed
cargo test -p effects minimax                            11 passed
cargo test -p effects turbulent                          11 passed
cargo test -p text-engine                                17 passed
cargo test -p render-core motion_blur                    6 passed
cargo test -p render-core posterize                      5 passed
```

Final combined checks after all agent patches:

```text
git diff --check                                           passed
cargo test -p testkit image_diff                         9 passed
cargo test -p effects                                    63 passed
cargo test -p text-engine                                17 passed
cargo test -p expression-engine                          12 passed
cargo test -p render-core motion_blur                    6 passed
cargo test -p render-core posterize                      5 passed
cargo test -p render-core adjustment_effect_sidecar      1 passed
cargo test -p render-core selector_weight_supports...    1 passed
cargo test -p testkit phase5_font_assumptions...         1 passed
cargo test -p render-cli conformance_pack                3 passed
```

## Agent B Review

Status: accepted as `needs_probe` with one `direct_from_reverse` implementation.

Accepted:

- `render_ir::DEFAULT_MOTION_BLUR_SAMPLES = 17`, supported by the recovered
  Transform/Geometry2 caller constant `0x11`.
- `render-core` motion blur test that verifies the default enabled ladder uses
  the 17-sample count.
- Geometry2 matrix order, inverse sampling, and payload/matchName mapping stay
  as current working contracts.

Not accepted for formula tuning yet:

- Geometry2 opacity, motion blur accumulation, and collapsed final composite
  remain blocked on `M19`.
- True collapse transformations are still not parity-locked because current
  runtime does not carry final deferred text/vector primitives through the
  renderer.

## Agent A Review

Status: accepted as `instrumented/testable`; not `parity_locked`.

Accepted:

- `RGB_ALPHA_METRIC_POLICY` explicitly records that raw RGB ignores alpha,
  alpha is split, no unpremultiply is applied, and premult is not locked.
- `diff_rgb8_under_alpha_policy` compares visible RGB after straight source-over
  projection onto the AE/reference background RGB.
- `render-cli conformance-pack` now emits `metric_contract` and
  `rgb_under_alpha_policy` metrics.
- `docs/reverse_engineering/MATH_CONTRACTS_GUARDRAILS.md` is the current shared
  guardrail document for agents.

Still blocked:

- Exact straight/premult/packed-alpha renderer-boundary contract.
- `GF::Composite` bool flags and full blend-mode mapping.
- Gamma/color-managed path, 16/32 bpc rounding, effect-specific OOB, and AE
  CPU/GPU divergence.

Decision:

- Effects/text/collapse/motion can use the new metric gate for diagnosis.
- Final alpha-sensitive formula tuning still requires module-local probes or a
  locked M19 contract.

## Agent D Review

Status: accepted as text telemetry hardening; glyph parity remains blocked.

Accepted:

- Local fixture font resolution before fontconfig fallback for `Point-Light` /
  `Point` and `Montserrat-*`, preventing silent DejaVu fallback in Docker when
  fixture fonts are mounted.
- `FontResolutionSource::FixtureAsset` telemetry.
- `TextLayoutTelemetry.line_boxes` with line char ranges, baselines,
  line/glyph boxes, and normalized boxes.
- Selector telemetry now carries selector rank, selector-center percent, and
  pre-wiggly base weight.

Still blocked:

- `Basic_Text.aex` delegates glyph metrics and construction through CoolType/TXT.
  Native fontdue/simple layout is useful for telemetry and approximate rendering
  but not AE parity truth.
- Text blur still depends on M19 and effect/kernel alpha policy.

## Agent C Review

Status: accepted as Expression Engine v2 boundary, not arbitrary JS parity.

Accepted:

- `PropertyExpressionEvaluator` / `BoundaryPropertyExpressionEvaluator`.
- `PropertyExpressionHost` boundary for host-owned property access.
- Scalar/Vec2/Vec3 coercion at the property boundary.
- Seeded context for `time`, `value`, `thisComp.*`, and selected
  `thisLayer.*` numeric fields.
- `valueAtTime(...)` requires a host sampler and returns an explicit `BLOCKER`
  if none exists.
- Named/fingerprint fallback remains available for scoped shortcuts such as
  `edge_wobble`.

Still blocked:

- Generic AE Expression Engine v2 needs BEE/Scripting time/property host
  recovery or probes. No arbitrary JS execution is accepted.

## Agent E Review

Status: accepted for direct blur/shadow/glow implementation from reverse.

Accepted:

- Shared blur radius uses ceil quantization, matching the recovered
  `GF::FastBoxBlur` / `BoxBlur_1DImgOpInfo` rounding mode.
- Box Blur and Glow have render-time animated-param checks.
- Drop Shadow offset uses truncation and the AE-probed sign convention.
- Drop Shadow softness now blurs alpha only and reapplies shadow RGB after the
  blur, matching `SetBlurAlphaChannelOnly`.
- Glow uses `radius / 2` before shared ceil quantization.

Still blocked:

- Drop Shadow final `CompositeShadowMask` premult/composite behavior.
- Glow `IR_CompositeWithBlendMode`, blend operation mapping, and final
  premult/straight handling.
- Final pixel tuning for these effects waits on M19/module probes.

## Agent F Review

Status: accepted for `M13`; `M14` remains blocked.

Accepted:

- Minimax operation enum surface: minimum, maximum, minimum-then-maximum,
  maximum-then-minimum.
- Minimax channel enum surface: color, alpha-and-color, red, green, blue,
  alpha.
- Minimax direction enum surface: horizontal-and-vertical, horizontal, vertical.
- Native Minimax now uses one-dimensional extrema passes composed by Direction
  instead of only the previous square-neighborhood approximation.
- `minimax_debug_trace` reports direction and `dont_shrink_edges` so edge
  semantics can be probed without guessing.

Still blocked:

- Exact fractional-radius quantization and `Don't Shrink Edges` edge/sentinel
  behavior need AE probes.
- GPU/CPU path parity is not locked.
- Turbulent Displace final vector math stays blocked by hidden
  `TurbulentDisplaceFracAllKernel` / `TurbulentDisplaceFrac1DKernel`; do not
  tune from final PNGs.

Verification observed:

- Baseline Docker lane passed `cargo test -p effects minimax`: 11 tests.
- Baseline Docker lane passed `cargo test -p effects turbulent`: 11 tests.

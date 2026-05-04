# Math Object Agent TZ

Status date: 2026-05-04

Predecoded evidence root:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

This iteration is an analysis sprint, not a formula-tuning sprint. Agents should
extract AE-shaped formulas, hidden dependencies, parameter mapping, time/space
semantics, and stop conditions. Code changes should wait until the orchestrator
accepts the object passport.

## Shared Output Contract

Each agent writes one phase report:

```text
docs/phase_reports/AGENT_<AREA>_MATH_OBJECTS_20260504.md
```

The report must include:

1. Scope and assigned math objects.
2. Evidence table: file/function/address/source, confidence, affected modules.
3. Object passport for each math object:
   - parameter mapping;
   - coordinate/time/color space;
   - sampling rule;
   - alpha/premult policy;
   - formula or pseudocode;
   - native implementation delta;
   - test/probe needed next.
4. Guardrail findings: any unknown entity that can affect other agents.
5. Final recommendation: `ready_for_formula_tuning`, `needs_probe`,
   `blocked_by_guardrail`, or `out_of_scope`.

## Guardrails

Guardrails are shared assumptions that, if wrong, poison multiple modules. The
alpha/premult mismatch was the canonical example: agents could tune Drop
Shadow, Glow, Minimax, text blur, and composite independently, but all their
results would be biased by a bad global alpha model.

An agent must stop and escalate to the orchestrator when it finds one of these:

| Guardrail | Stop condition | Likely affected objects |
| --- | --- | --- |
| Alpha/premult/straight policy | Evidence contradicts straight RGBA8 or normal source-over assumptions. | `M01`, `M10`, `M11`, `M13`, `M17`, `M19` |
| Color/gamma/channel range | Evidence indicates non-sRGB, linear-light, 16/32 bpc, premultiplied export, or hidden channel conversion. | all visual effects, text, composite |
| Sampling/OOB policy | Evidence shows clamp/repeat/transparent/edge extend differs by sampler/effect. | `M02`, `M03`, `M10`, `M11`, `M12`, `M14`, `M17` |
| Coordinate origin/handedness | Evidence changes sign/origin/order for matrix, offset, UV, or text coordinates. | `M03`, `M07`, `M10`, `M12`, `M14`, `M17`, `M18` |
| Time semantics | Evidence shows effect params, source frames, adjustment layers, or motion blur use different sample times. | `M02`, `M04`, `M09`, `M15`, `M16`, `M18` |
| AE hidden pipeline call | A plugin delegates to a shared DLL/kernel not assigned to the agent. | effects using `GPUFoundation.dll`, `ImageRenderer.dll`, `BEE.dll` |
| Parameter index remap | Numeric AE params map to different fields than current native code assumes. | object-specific plus templates using it |
| Ghidra missing/decompiler failure | A key target is data-only, missing, or decompiler-failed and disassembly/xrefs are insufficient. | assigned object and dependencies |
| Cross-object dependency | Formula depends on a module owned by another agent. | both owners |

Escalation payload:

```text
BLOCKER:
  title:
  affected objects:
  evidence paths:
  exact address/function:
  what assumption broke:
  hypotheses:
  smallest probe/test needed:
  can continue on unrelated work: yes/no
```

The agent should not tune formulas or propose code patches on the blocked
object until the orchestrator resolves or narrows the guardrail.

## Agent Allocation

### Agent A: Substrate / Global Guardrails

Assigned objects: `M01`, `M02`, `M19`

Evidence:

```text
core_alpha_gpufoundation/
blur_gpufoundation_kernels/
imagerenderer_gaussian_composite/
docs/MATH_PARITY_STATUS.md
```

TZ:

- Recover the global pixel model: straight vs premult, alpha normalization,
  channel range, byte/float conversion, and source-over/composite flags.
- Map `GF::Composite`, `GF::AlphaGain`, `GF::PackedAlphaGain`,
  `GF::Unpremultiply`, `GF::BlendUnpackedAlpha`.
- Read `GF::FastBoxBlur`/`GF::GaussianBlur` only for alpha-option policy shared
  by effects.
- Produce a global `M19` passport that other agents must reference.
- Explicitly list which modules are safe to analyze before `M19` is locked and
  which are not.

Done for this iteration:

- We know whether native's straight RGBA8 assumption is acceptable for tuning,
  or we have a concrete blocker/probe.
- We know the likely alpha-option enum/flags used by shared blur/composite
  paths.
- Other agents have a one-page substrate contract.

### Agent B: Transform / Geometry / Collapse / Motion

Assigned objects: `M03`, `M04`, `M12`, `M17`, `M18`

Evidence:

```text
geometry_transform_wrapper/
geometry_transform_gpufoundation/
posterize_time/
docs/phase_reports/AE_REVERSE_GEOMETRY2_GHIDRA.md
docs/MOTION_BLUR.md
```

TZ:

- Confirm matrix order, anchor/position/scale/rotation convention, and inverse
  sampling direction.
- For Geometry2, answer AE param mapping for `0003`, `0004`, `0008` and any
  sampler/edge mode hints.
- For collapse, identify what can be deferred-rasterized and what must be
  rasterized at precomp boundaries.
- For motion blur, recover sample count/time/weight clues from
  `GF::TransformWithMotionBlur` and `GF::Motion`.
- For Bezier/ease, only define required probe/test shape unless Ghidra gives
  direct evidence.

Done for this iteration:

- Geometry2 has a parameter/matrix/sampling passport.
- Collapse has a decision table: text/vector/footage/precomp/adjustment.
- Motion blur has an AE-shaped sampling hypothesis and explicit unknowns.

### Agent C: Temporal / Adjustment / Property Expressions

Assigned objects: `M09`, `M15`, `M16`

Evidence:

```text
posterize_time/
time_displace_temporal/
scripting_expression_host/
docs/reverse_engineering/effect_math_temporal_noise_distort.md
```

TZ:

- Confirm Posterize Time bucket formula, boundary epsilon, and frame-rate
  checkout behavior from `Posterize_Time.aex`.
- Separate layer/source time, effect param time, adjustment lower-stack time,
  and downstream effect time.
- Use `Scripting.aex` data targets for BEE/time bridge pointers:
  `GetTimePalSuite`, `TDB_SecondsToTime`, `BEE_FpLongToTime`,
  `BEE_CompToLayerTime`.
- For generated `edge_wobble`, identify what belongs to expression evaluator
  vs our named fingerprint shortcut.
- Define required telemetry checkpoints: source frame index, effect param time,
  adjustment input hash, post-effect hashes.

Done for this iteration:

- A temporal contract that other agents can use when an effect has animated
  params.
- A clear answer for whether `STK_030` over-posterize risk is resolved or still
  blocked.
- A minimal expression v2 scope: what can be evaluated now vs later.

### Agent D: Text / Glyph / Animator / Selector

Assigned objects: `M05`, `M06`, `M07`, `M08`

Evidence:

```text
basic_text_aex/
scripting_expression_host/
docs/reverse_engineering/effect_math_text_expression.md
fixtures/ae_conformance_pack/
```

TZ:

- Recover text/glyph pipeline hints from `Basic_Text.aex` and existing
  CoolType/Text notes.
- Define glyph metrics needed for AE-like layout: advance, bbox, baseline,
  kerning/shaping, line composer, glyph rasterization scale.
- Define selector passport: characters/words/lines unit boundaries, start/end,
  shape, smoothness, randomize order, wiggly selector.
- Define animator passport: per-glyph position/scale/rotation/opacity/blur and
  transform origin.
- For expression selector bounce, decide whether current named shortcut is
  enough for template parity or must route through expression evaluator v2.

Done for this iteration:

- Text modules have a concrete ladder from current approximation to testable AE
  parity.
- We know which glyph metrics are global blockers vs per-template tuning.
- Text animator blur has a probe plan that does not depend on final-frame
  eyeballing only.

### Agent E: Blur / Drop Shadow / Glow

Assigned objects: `M10`, `M11`, shared blur kernel subset of `M19`

Evidence:

```text
box_blur_aex/
drop_shadow_aex/
glow_aex/
blur_gpufoundation_kernels/
imagerenderer_gaussian_composite/
docs/reverse_engineering/effect_math_blur_glow_shadow.md
```

TZ:

- For Box Blur: recover radius quantization, iterations, edge handling,
  fractional radius policy, alpha options, and whether AE calls
  `GF::FastBoxBlur`.
- For Drop Shadow: recover direction/distance sign convention, opacity units,
  softness-to-kernel mapping, shadow-only branch, and composite order.
- For Glow: recover based-on enum, threshold source, radius mapping, intensity,
  blend/composite route, and `IR_GaussianBlur` dependency.
- Produce formulas only after referencing Agent A's substrate contract.

Done for this iteration:

- Each effect has a passport with exact knowns/unknowns and next probes.
- Any dependency on `ImageRenderer.dll` or `GPUFoundation.dll` is explicit.
- We know which mismatch is kernel math vs alpha/composite substrate.

### Agent F: Minimax / Turbulent Displace

Assigned objects: `M13`, `M14`

Evidence:

```text
minimax_aex/
turbulent_displace_aex/
docs/reverse_engineering/effect_math_temporal_noise_distort.md
docs/phase_reports/AE_REVERSE_TURBULENT_TEXT_EXPRESSION_GHIDRA.md
```

TZ:

- For Minimax: recover operation enum, channel enum, direction enum, radius
  rounding, neighborhood shape, edge policy, and 8/16/32 bpc differences.
- For Turbulent Displace: recover param indices `1..14`, internal displacement
  modes, `FracAll` vs `Frac1D` dispatch, complexity/octave split, lookup table
  construction, evolution/seed/cycle semantics, pinning/resize/AA policy.
- Define field-level comparison checkpoints rather than final-pixel-only
  comparisons.
- Explicitly flag if alpha/premult or temporal sampling is needed before
  formulas can be tuned.

Done for this iteration:

- Minimax has enum/neighborhood/channel passport and a probe matrix.
- Turbulent has an AE-shaped state model ready to implement or a precise list
  of unresolved constants.
- Both modules identify whether they are blocked by `M19` or `M15`.

## Per-Object TZ Matrix

| Object | Owner | Iteration output |
| --- | --- | --- |
| `M01` Timeline/z-order/composite | A | Composite order and alpha contract; blockers for final-frame diffs. |
| `M02` Footage source-time sampling | A/C | Source-time contract and frame-index telemetry requirements. |
| `M03` 2D transforms | B | Matrix/order/sampler passport. |
| `M04` Keyframes/ease | B | Probe plan and Bezier parity questions; no tuning yet unless evidence is direct. |
| `M05` Text raster/glyph layout | D | Glyph metrics passport and global layout blockers. |
| `M06` Text range selector | D | Unit/shape/smoothness/randomize/wiggly passport. |
| `M07` Text animator transform/blur | D | Per-glyph transform/blur passport and probe plan. |
| `M08` Expression selector bounce | D/C | Bounce formula responsibility split: selector shortcut vs expression evaluator. |
| `M09` Property expression subset | C | Edge-wobble/evaluator scope and time access contract. |
| `M10` Drop Shadow | E | Offset/softness/opacity/composite passport. |
| `M11` Glow | E | Threshold/radius/intensity/blend passport. |
| `M12` Geometry2 | B | Param mapping, matrix, sampler/edge passport. |
| `M13` Minimax | F | Enum/channel/neighborhood/radius passport. |
| `M14` Turbulent Displace | F | State model, dispatch, lookup/evolution/seed passport. |
| `M15` Posterize Time | C | Bucket/time-routing contract and boundary probes. |
| `M16` Adjustment stack | C | Lower-stack/effect-time ordering contract. |
| `M17` Collapse transformations | B | Deferred rasterization decision table. |
| `M18` Motion blur | B/C | Sample times/weights/accumulation hypothesis. |
| `M19` Color/alpha/sampling/gamma | A | Global substrate contract; must be referenced by E/F/B where relevant. |
| `M20` Masks/mattes/blend modes | Orchestrator | Out of current scope unless an agent finds hidden usage. |
| `M21` 3D/camera/spatial/ExtendScript | Orchestrator | Out of current scope unless a current template unexpectedly depends on it. |

## Orchestrator Acceptance Criteria

Before code tuning starts, the orchestrator should accept or reject each object
passport using:

- Evidence has exact paths and addresses.
- Assumptions are labeled `confirmed`, `inferred`, or `unknown`.
- Cross-object dependencies are explicit.
- Guardrail blockers are either resolved or isolated.
- There is a smallest next probe/test for every unresolved formula term.

Only accepted passports move to implementation/formula tuning.

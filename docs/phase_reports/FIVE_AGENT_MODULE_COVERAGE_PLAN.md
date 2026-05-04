# Five-Agent Module Coverage Plan

Status date: 2026-05-03.

This plan is the next OS-level work split after `render-cli conformance-pack`.
The goal is to cover the renderer math modules from `docs/MATH_PARITY_STATUS.md`
with five focused agents, using the checked-in AE conformance pack as the common
measurement surface.

The shared loop for every agent is:

```text
decomposition
  -> isolated native/AE result
  -> composition or stack result
  -> repeat native/AE test
  -> OS gate decision
```

No agent should tune formulas from a full-template PNG diff alone. Formula
changes need a smaller case that explains the failure.

## Common Contract

Each agent writes a short report under `docs/phase_reports/`, and produces
native outputs under `target/ae_agents/<agent>/`.

Base command shape:

```sh
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/<agent> \
  --case <CASE_ID>
```

Use repeated `--case` flags for a focused batch.

Each report should include:

- module ids covered;
- case ids rendered;
- top native-vs-AE metric offenders from `metrics.json`;
- first suspected divergent primitive;
- required telemetry or formula patch before the next gate.

The OS/critic owns the final promotion:

```text
implemented approximate
  -> instrumented/testable
  -> AE golden exists
  -> formula tuning
  -> parity locked
```

## Agent 1: Core Pixel Substrate

Scope:

- `M01` timeline activity, z-order, opacity compositing;
- `M03` transform matrix, anchor/position/scale/rotation sampling;
- `M04` hold/linear/Bezier keyframe sampling;
- `M19` color, alpha, sampling, gamma assumptions.

Primary cases:

```text
PRI_010, CMP_010, INT_010, INT_020, EFF_040
```

Decomposition:

- split final pixel error into layer order, opacity, premult/straight alpha,
  transform matrix, interpolation value, and sampler behavior;
- treat `EFF_040` only as a coordinate/sampler probe for `M03`, not as the
  owner of Geometry2 formula tuning.

Isolated result:

- render `PRI_010`, `CMP_010`, `INT_010`, `INT_020`;
- record per-frame mean/max/RMSE and identify frame boundaries where ease or
  alpha differs.

Composition/retest:

- rerun with `EFF_040` to see whether matrix/sampler assumptions still explain
  coordinate-field failures before Agent 4 touches Geometry2.

Gate:

- later agents can tune effects only after Agent 1 names the active assumptions
  for alpha, pixel centers, bilinear/nearest behavior, and keyframe sampling.

## Agent 2: Temporal Graph And Motion

Scope:

- `M02` footage source-time sampling and media frame selection;
- `M15` Posterize Time temporal behavior;
- `M16` adjustment-layer timing/order owner for temporal resampling;
- `M17` precomp/collapse graph ownership, with text details handed to Agent 5;
- `M18` motion blur.

Primary cases:

```text
TMP_010, TMP_020, TMP_030, STK_030, GPH_010
```

Decomposition:

- split comp time, layer time, source time, effect time, posterized bucket time,
  adjustment lower-stack resampling, precomp time, and motion-blur sample time;
- keep `M20`/`M21` as detection/reporting only if payload inventory surfaces
  masks, mattes, 3D, cameras, spatial paths, roving keyframes, or arbitrary
  ExtendScript.

Isolated result:

- render `TMP_010`, `TMP_020`, `TMP_030`;
- compare numbered-frame identity and bucket boundaries before any stack tuning.

Composition/retest:

- rerun `STK_030` after isolated temporal behavior is diagnosed;
- rerun `GPH_010` only for graph/collapse ordering, leaving text sharpness to
  Agent 5.

Gate:

- no Posterize Time or motion-blur formula tuning until the report lists sample
  times, weights/buckets, and source-frame ids for the failing frames.

## Agent 3: Blur, Shadow, Glow, Morphology

Scope:

- `M10` Drop Shadow and Box Blur dependency;
- `M11` Glow;
- `M13` Minimax;
- `M19` effect-composite dependency from Agent 1.

Primary cases:

```text
EFF_010, EFF_020, EFF_030, EFF_050, EFF_070, STK_010, STK_020
```

Decomposition:

- split blur kernel, edge policy, alpha mask, shadow offset, shadow composite,
  glow threshold/mask, glow blur, glow blend, Minimax neighborhood, and channel
  mode.

Isolated result:

- render `EFF_010`, `EFF_020`, `EFF_030`, `EFF_050`;
- report whether failures are kernel/edge/alpha/enum/composite.

Composition/retest:

- rerun `STK_010` and `STK_020`;
- use `EFF_070` only after the static operators are diagnosed, because it mixes
  animated ease with several effects.

Gate:

- formula tuning is allowed only when the agent can show the failing
  intermediate: blur mask, shadow mask, glow mask, or Minimax neighborhood.

## Agent 4: Coordinate Warps And Procedural Fields

Scope:

- `M12` Geometry2;
- `M14` Turbulent Displace;
- `M16` stack interaction as a consumer of Agent 2's timing decision;
- `M19` sampler/color dependency from Agent 1.

Primary cases:

```text
EFF_040, EFF_060, STK_030
```

Decomposition:

- split Geometry2 parameter mapping, matrix, inverse matrix, source UV,
  sampler/edge mode, Turbulent noise value, dx/dy field, displaced UV, and final
  sampled pixels.

Isolated result:

- render `EFF_040`;
- render `EFF_060` at all selected frames;
- classify failures as matrix, field generation, evolution, edge, or sampling.

Composition/retest:

- rerun `STK_030` only after `EFF_040` and `EFF_060` have named blockers;
- coordinate with Agent 2 on Posterize Time and Agent 3 on Minimax before any
  stack-level conclusion.

Gate:

- Turbulent Displace cannot be tuned from final pixels until a displacement
  field or UV telemetry artifact exists for the same frames.

## Agent 5: Text, Glyphs, Expressions

Scope:

- `M05` text rasterization and glyph layout;
- `M06` Range Selector reveal;
- `M07` glyph animator position/scale/rotation/opacity/blur;
- `M08` expression selector bounce;
- `M09` generated property expression subset;
- `M17` collapse text sharpness consumer.

Primary cases:

```text
TXT_010, TXT_020, TXT_030, TXT_040, EXP_010, GPH_010
```

Decomposition:

- split font resolution, glyph id/advance/bbox, line breaking, word/character
  units, selector weights, per-glyph transform, blur radius, expression sampled
  value, and collapsed raster scale.

Isolated result:

- render `TXT_010`, `TXT_020`, `TXT_030`, `TXT_040`, `EXP_010`;
- confirm Montserrat from bundled assets and Point-Light from the host font
  environment before treating text diffs as formula failures.

Composition/retest:

- rerun `GPH_010` after glyph layout and collapse graph assumptions are named;
- use template text slices only after isolated text cases identify whether the
  blocker is font, layout, selector, expression, blur, or collapse.

Gate:

- no glyph/evaluator tuning without telemetry for glyph layout, selector weight,
  expression value, and final per-glyph transform at the failing frame.

## Coverage Summary

| Agent | Modules |
| --- | --- |
| 1 Core Pixel Substrate | `M01`, `M03`, `M04`, `M19` |
| 2 Temporal Graph And Motion | `M02`, `M15`, `M16`, `M17`, `M18`; `M20`/`M21` detection only |
| 3 Blur, Shadow, Glow, Morphology | `M10`, `M11`, `M13` |
| 4 Coordinate Warps And Procedural Fields | `M12`, `M14` |
| 5 Text, Glyphs, Expressions | `M05`, `M06`, `M07`, `M08`, `M09`; text-side `M17` |

`M20` masks/mattes/blend modes and `M21` 3D/camera/spatial paths/roving
keyframes/full ExtendScript are not required by the current three-template
target or current conformance pack. They stay as explicit inventory/fallback
items until payload analysis proves they block a real template.

## OS Next Step

Run all five batches once with the current native runner, then rank modules by
the worst isolated-case metrics before assigning formula work. The first
accepted formula patches should come from isolated cases, not from `STK_*` or
full-template frames.

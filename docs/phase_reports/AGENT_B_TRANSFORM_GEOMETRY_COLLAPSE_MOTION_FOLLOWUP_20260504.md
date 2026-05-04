# Agent B Transform / Geometry2 / Collapse / Motion Follow-up

Status date: 2026-05-04

Predecoded evidence root:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

## Scope

Assigned objects: `M03`, `M04`, `M12`, `M17`, `M18`.

This pass audited current source against the round2 Ghidra bundle and made only
one low-risk source change: the native default motion-blur sample count now
uses the AE Transform/Geometry2 observed 17-sample ladder. Existing alpha
accumulation math was not changed because Agent A has not locked `M19`.

Pre-existing dirty worktree changes outside this area were not reverted or
edited.

## Evidence Table

| Object | Evidence | Address / function | Finding | Confidence |
| --- | --- | --- | --- | --- |
| `M03`, `M12` | `geometry_transform_gpufoundation/04_GF_TransformToMatrix_180074eb0/decompile.c` | `180074eb0`, `GF::TransformToMatrix` | Builds transform from anchor, pixel aspect, scale, rotation, optional skew/skew-axis, inverse pixel aspect, then position. | high |
| `M03`, `M12`, `M18` | `geometry_transform_gpufoundation/08_GF_TransformWithMotionBlur_180076ed0/decompile.c` | `180076ed0`, `GF::TransformWithMotionBlur` | Uses `TransformOperation::NumMatrices`, uploads `InvertedMatrices` as `9` floats per matrix, passes `GetOpacityMultiplier` into kernels. | high |
| `M12`, `M18` | `geometry_transform_wrapper/01_Geometry2_render_wrapper_180009a20/decompile.c` | `180009a20` | Geometry2 wrapper receives a vector of transformations, calls `GF::TransformsToMatrices`, constructs `TransformOperation`, then calls `GF::TransformWithMotionBlur`. | high |
| `M18` | `geometry_transform_wrapper/02_Geometry2_mb_candidate_33a0_1800033a0/decompile.c` | `1800033a0` | Motion sample builder uses shutter/phase fixed-point ratios and adds a step inside the sample loop. Exact first/last timestamp remains ambiguous. | medium |
| `M18` | `geometry_transform_wrapper/03_Geometry2_mb_candidate_4440_180004440/data_refs.tsv` | `18000458e -> 180023268` | Caller reads `0x11`, the observed 17-sample Transform/Geometry2 motion ladder. | high |
| `M17` | current `crates/render-core/src/precomp.rs` and `crates/render-core/src/layer_eval.rs` | native source audit | Native has collapse planning, nested collapsed precomp traversal, solid flattening, and scale-aware collapsed text raster approximation. It does not yet consume a true deferred text/vector primitive in the renderer. | high |

## Geometry2 / Transform Audit

Implemented in current source:

- Matrix order in `crates/effects/src/geometry.rs` matches the recovered GF
  order:

```text
T(position)
* S(1 / pixel_aspect, 1)
* R(-skew_axis)
* Hx(-tan(skew))
* R(skew_axis)
* R(rotation)
* S(scale_x, scale_y)
* S(pixel_aspect, 1)
* T(-anchor)
```

- Runtime sampling is inverse-matrix sampling: destination pixel coordinates
  are mapped through `inverse_matrix` before bilinear source sampling.
- Geometry2 numbered payload mapping is correct for the high-risk fields:
  - payload `"0003"` = Uniform Scale fallback / checkbox namespace;
  - payload `"0004"` = Scale Height;
  - payload `"0005"` = Scale Width;
  - payload `"0008"` = Rotation;
  - matchName `ADBE Geometry2-0008` = Opacity, not payload `"0008"`.
- Tests already cover skew order, pixel aspect basis, numbered scale mapping,
  and `"0008"` as rotation fallback.

Exact missing Geometry2 patch, intentionally not done in this pass:

1. Add parsed fields for payload `"0009"` / matchName
   `ADBE Geometry2-0008` opacity, `"0010"` / use comp shutter, `"0011"` /
   shutter angle, and `"0012"` / sampling mode.
2. Do not apply opacity until `M19` straight/premult and alpha-gain policy is
   locked. Ghidra proves an opacity multiplier exists, but not the project-wide
   alpha boundary.
3. Do not switch Bicubic/Lanczos/Area sampling until the sampler/OOB probe
   resolves kernel edge behavior and UI-to-quality mapping.

## Collapse Plan

Already present:

- `PrecompGraph::deferred_raster_plan_for_precomp_layer` can describe deferred
  `SolidVector` and `TextVector` primitives, transform steps, source-time steps,
  active windows, and opacity contracts.
- Collapse planning rejects raster barriers: footage, adjustment layers,
  layer effects, non-collapsed nested precomps, missing targets, and cycles.
- Runtime collapse path can flatten solids with the parent matrix.
- Runtime text collapse rasterizes text with a final-matrix scale hint, records
  parent/child/effective matrices, raster scale, raster size, and a sharpness
  probe.
- Nested collapsed precomp traversal composes parent and child matrices in
  order.

True parity plan:

1. Make `DeferredRasterPrimitive` the render contract, not only a planning
   artifact. The renderer should carry text/vector primitives until the final
   effective matrix is known.
2. For each collapsed primitive, evaluate source time through every crossed
   precomp boundary, then evaluate the source layer transform at that final
   source time.
3. Propagate matrices as:

```text
effective_matrix =
  root_precomp_boundary_matrix
  * nested_collapsed_boundary_matrix...
  * source_layer_matrix
```

4. Text layout stays in source/layer coordinates. Glyph/vector rasterization
   happens once at the final effective matrix/scale. The current scale-hint
   raster path remains an approximation.
5. Sampling policy:
   - text/vector primitives: defer raster, no intermediate sampler;
   - solids: may flatten as transformable vector-like rectangles;
   - footage and non-collapsed precomp: raster boundary, then normal inverse
     sampling at the parent transform;
   - adjustment/effects/mattes/masks: raster barrier until proven defer-safe.
6. Telemetry should emit one deferred primitive record with source layer,
   source time, transform steps, final matrix, raster backend, sampling policy,
   and fallback reason.

## Motion Blur Minimal Implementation

Implemented now:

- Default `MotionBlurSettings.samples` is `17`, matching the recovered
  Transform/Geometry2 caller constant `0x11`.
- Explicit scene settings still override the default.
- Current sample schedule remains midpoint placement inside the shutter
  interval:

```text
exposure = frame_duration * shutter_angle / 360
open     = frame_time + frame_duration * shutter_phase / 360
close    = open + exposure
sample_i = open + exposure * ((i + 0.5) / sample_count)
```

- Sample count is clamped to `1..64`.
- Each active sample evaluates layer transform/effects/source time through the
  existing render path.
- Accumulation policy was deliberately left unchanged:

```text
sample_weight = sample_opacity / (100 * effective_sample_count)
accum_rgba += sample_rgba8 * sample_weight
```

This is an implementation approximation, not final AE parity.

Exact missing motion-blur patch, intentionally not done in this pass:

1. Probe endpoint placement. `FUN_1800033a0` adds the step before evaluation,
   but the decompiler shape does not prove the effective open/close endpoints.
2. Probe `ShouldInterpolateEndMatrices` and `MatrixRadii` behavior for quality
   modes and low-pass radius buffers.
3. Probe Geometry2 effect-specific shutter controls:
   `Use Composition's Shutter` and `Shutter Angle`.
4. Wait for `M19` before changing opacity/premult/alpha accumulation.

## BLOCKER: M19 Alpha/Accumulation Contract

affected objects:

`M17`, `M18`, `M19`, and any final-pixel comparison involving collapsed text,
motion blur, or Geometry2 opacity.

evidence paths:

- `docs/phase_reports/AGENT_A_SUBSTRATE_MATH_OBJECTS_20260504.md`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/geometry_transform_gpufoundation/08_GF_TransformWithMotionBlur_180076ed0/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/core_alpha_gpufoundation/05_GF_AlphaGain_180022570/decompile.c`
- `target/reverse/predecoded/20260504_222748_full_predecode_round2/core_alpha_gpufoundation/06_GF_PackedAlphaGain_180022cc0/decompile.c`

exact address/function:

- `180076ed0`, `GF::TransformWithMotionBlur`
- `180022570`, `GF::AlphaGain`
- `180022cc0`, `GF::PackedAlphaGain`

what assumption broke:

Native straight `RGBA8` accumulation is not a locked AE contract. Ghidra shows
opacity multiplier and alpha-gain paths, but Agent A has not resolved the
straight/premult boundary or final alpha formula.

hypotheses:

- Uniform sample weights are probably acceptable for a first matrix schedule
  probe.
- Opacity may be applied through a GF alpha-gain path rather than by directly
  scaling straight RGBA bytes.

smallest probe/test needed:

Render an opaque moving bar and a partially transparent colored moving bar with
17 samples, shutter angle `180`, phase `-90/0/90`, and opacity `50`. Compare
support width, centroid, RGB, and alpha separately against AE.

can continue on unrelated work: yes. Matrix schedule, transform mapping, and
collapse topology can continue. Do not tune alpha-sensitive output pixels.

## BLOCKER: True Vector/Text Deferred Raster Backend

affected objects:

`M17`, text/glyph objects (`M05`/`M07`), and `M19` for final composite.

evidence paths:

- `crates/render-ir/src/schema.rs`
- `crates/render-core/src/precomp.rs`
- `crates/render-core/src/layer_eval.rs`
- `docs/phase_reports/AGENT_ROUND4_COLLAPSE_DEFER_RASTER.md`

exact address/function:

No AE binary address was recovered for collapse. This is a host/render-graph
contract blocker in native source.

what assumption broke:

The IR has `Solid`, `Footage`, `Text`, `Precomp`, and `Adjustment` layer
variants, but no shape/vector/Illustrator primitive representation. Current
collapsed text is rasterized through a scale-hint approximation rather than a
final deferred raster primitive.

hypotheses:

- Solid rectangles can remain vector-like.
- Text can reach parity only after the text engine accepts final matrix/scale
  without changing source layout metrics.
- Shape/vector parity requires a new IR primitive or a proven payload source.

smallest probe/test needed:

Add a collapsed/pre-rasterized AE pair with one text layer, one solid rectangle,
and one shape/vector layer. Require telemetry to show deferred primitive kind,
effective matrix, source time, raster backend, and barrier/fallback reason.

can continue on unrelated work: yes. Collapse graph topology and barrier
classification can continue. Do not mark `GPH_010` as true collapse parity.

## Recommendation

`needs_probe`

Carry forward:

- Keep current Geometry2 matrix order and inverse sampling.
- Keep payload `"0008"` as Rotation and matchName `ADBE Geometry2-0008` as
  Opacity.
- Use 17 samples as the default AE Transform/Geometry2 motion-blur ladder.
- Keep midpoint placement and straight RGBA8 accumulation labeled approximate
  until endpoint/weight and `M19` probes resolve.
- Treat current collapse runtime as an approximation: topology and matrix
  propagation are useful, but true text/vector deferred raster parity is not
  implemented yet.

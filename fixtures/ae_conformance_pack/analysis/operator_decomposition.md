# AE Conformance Pack Operator Decomposition

Status source: `docs/MATH_PARITY_STATUS.md` dated 2026-05-03, `docs/EFFECTS.md`, and current imported scenes under `target/native_template_runs/*/scene.json`.

This pack should make AE parity debuggable by testing the renderer as a ladder of increasingly composed behavior:

```text
primitive -> property/interpolation -> operator static -> animated operator -> stack -> template slice -> full template
```

Green native tests currently mean stable approximations and testability, not AE parity. The first useful conformance goal is therefore to produce AE references and telemetry that can identify the first divergent module, not only a final PNG diff.

## Ladder

| Level | What It Proves | Typical Fixture | Expected Checkpoints |
| --- | --- | --- | --- |
| primitive | Deterministic source pixels, alpha/color representation, coordinates, glyph segmentation, source frame identity. | impulse, ramp, alpha ramp, checkerboard, coordinate field, numbered frames, short text strings. | asset hash, source pixel hash, UV readback, frame index, glyph/word/line units. |
| property/interpolation | Scalar/Vec2 sampling, hold/linear/ease boundaries, expression sample values. | single animated property with sparse keyframes and boundary frames. | input time, sampled time, keyframe span, interpolation mode, sampled value. |
| operator static | Pixel or graph behavior with constant params. | isolated effect/layer on simple probe image. | kernel, mask, matrix, UV, selector weights, premult/straight state. |
| animated operator | Same operator with animated params and boundary frames. | static source plus animated params or moving source plus static params. | param sample time/value, operator intermediate hashes, source frame/time. |
| stack | Ordered non-commuting operators and adjustment/precomp boundaries. | effect pairs, adjustment layers, collapsed vs non-collapsed precomp. | graph order, per-effect input/output hash, layer/precomp time mapping. |
| template slice | A small scene slice from `template_4th`, `impulse_2nd`, or `scenes_3rd`. | one or a few real payload layers with real fonts/assets/effects. | layer IDs, feature counters, per-module telemetry, output diff. |
| full template | End-to-end confidence over the imported target snapshot. | current complete scene JSON and assets. | manifest, render log, final frames, blocking module list. |

## Current Template Slices

| Template | Observed Slice | Required Modules | Risk Focus |
| --- | --- | --- | --- |
| `template_4th` | 12 footage layers, 10 text layers, 10 word-based text animators, 20 Drop Shadows, 10 Glows. | `M01`, `M02`, `M03`, `M04`, `M05`, `M06`, `M10`, `M11`, `M17`, `M19` | Text layout/reveal, Drop Shadow, Glow, color/composite. |
| `impulse_2nd` | 18 footage layers, 27 text layers, 27 character animators with bounce selector, 81 Drop Shadows. | `M01`, `M02`, `M03`, `M04`, `M05`, `M07`, `M08`, `M10`, `M17`, `M19` | Glyph animator order, expression selector bounce, blur animator, Drop Shadow, ease. |
| `scenes_3rd` | 8 footage layers, 15 text layers, 15 adjustment layers; each adjustment uses Geometry2, Posterize Time, Minimax, Turbulent Displace. | `M01`, `M02`, `M03`, `M04`, `M05`, `M06`, `M09`, `M12`, `M13`, `M14`, `M15`, `M16`, `M19` | Adjustment/effect order, Posterize Time boundaries, coordinate warps, expression motion. |

## Block Map

### Core Temporal And Geometry

`transforms_interpolation_ease` covers `M03` and `M04`. It should start with coordinate-field and grid primitives, then compare anchor/position/scale/rotation/opacity at static, linear, hold, and eased keyframes. Important AE cases are identity, half-pixel translation, negative scale, rotation around non-zero anchor, opacity boundaries, and cubic ease near keyframe boundaries. Compare frames at the first frame, exact keyframes, one frame before/after keyframes, middle eased spans, and final frame.

`source_layer_precomp_time` covers `M01`, `M02`, and `M17`. It should prove layer activity, z-order, source start, nested comp time, reversed or offset source sampling, and collapsed text/solid precomp behavior. Use numbered-frame sources and nested comps with visible frame IDs. Compare activity-window boundaries, source-start boundaries, nested comp loop/offset boundaries, and final template slice frames.

`posterize_time` covers `M15` and depends on `M02`, `M04`, and `M16`. It must test temporal quantization above layer/source/effect time, including adjustment-layer lower-stack resampling. Use numbered frames, animated transforms/effect params, and a non-commuting adjustment stack. Compare frames around every posterized bucket boundary.

`motion_blur` covers `M18`. It should begin with a moving solid/coordinate field and then add text glyph movement and precomp graph cases. Compare shutter angle 0, 90, 180, 360; shutter phase offsets; sample count changes; static-layer skip; and Posterize Time interaction. Required telemetry is sample times, weights, per-sample matrices, bboxes, and accumulation hashes.

### Pixel Effects

`drop_shadow` covers `M10`. Use impulse, alpha ramp, edge square, text alpha, and real template text slices. AE cases should vary color, opacity, direction, distance, softness, and shadow-only. It needs source-alpha, shadow-mask, blurred-shadow, offset-shadow, and final-composite telemetry.

`glow` covers `M11`. Use luma ramps, alpha ramps, colored impulses, text, and footage slices. AE cases should vary threshold, radius, intensity, color/alpha edge cases, and animated params. Telemetry should include luma, threshold mask, blurred glow, glow contribution, and composite mode state.

`box_blur` is listed in `docs/EFFECTS.md` and `crates/testkit/src/operator.rs`, but has no dedicated `Mxx` ID in `docs/MATH_PARITY_STATUS.md`. It should still be in the pack because Drop Shadow and Glow blur tuning depend on the same sampling and alpha decisions. Use impulse, checkerboard, alpha ramp, edge square, radius 0/1/fractional/large, and iterations 1/2/3.

`geometry2` covers `M12`. Use coordinate fields, checkerboards, alpha ramps, and adjustment-layer canvases. AE cases should include anchor/position/scale width/height/uniform scale, rotation, half-pixel offsets, edge sampling, and animated scalar params.

`minimax` covers `M13`. Use impulse, alpha ramp, checkerboard, near-edge square, and colored channel ramps. AE cases should cover operation enum, radius 0/1/large, alpha-only vs RGBA channels, and edge behavior.

`turbulent_displace` covers `M14`. Use coordinate fields and grids first, then footage/text slices. AE cases should vary amount, size, complexity, evolution, seed-like stable inputs, animated evolution, and edge sampling. The critical checkpoint is a displacement field diff before sampling the source image.

### Text And Expressions

`text_reveal` covers `M05` and `M06`. It should use simple Latin, Cyrillic, mixed punctuation, multi-line text, word/character/line BasedOn modes, and boundary start/end/offset values. Telemetry must include glyph layout, word segments, line segments, selector weights, glyph opacity, and glyph matrices.

`glyph_animator_point_light` covers `M05`, `M07`, and `M19`. Treat this as the focused glyph-level animator block for position/scale/rotation/blur/opacity under a point-light-like visual probe. Use glyph rectangles and text alpha to prove per-glyph transforms before full text styling.

`montserrat_reveal` covers `M05`, `M06`, `M10`, `M11`, and `M19`. This is a template-quality text reveal slice using the Montserrat font path or fallback inventory used by the renderer. It should compare Cyrillic and Latin real strings from `template_4th`/`impulse_2nd`, with Drop Shadow/Glow variants isolated and then stacked.

`expression_selector_bounce` covers `M08` with dependencies on `M04`, `M05`, and `M07`. It should record expression amount per glyph over time and compare delay/frequency/decay semantics at early, peak, decay, and settle frames.

`edge_wobble_expression` covers `M09`. It should use a numbered-frame or coordinate-field source with generated `edge_wobble` position expression and compare property samples, source frame IDs, and resulting matrices over time.

### Graph, Stack, And Rendering Assumptions

`adjustment_stacks` covers `M16` and depends on `M12`, `M13`, `M14`, `M15`, and `M19`. It should prove that adjustment layers operate on accumulated lower canvas content and that effect ordering is stable. Use non-commuting stacks such as Geometry2 before/after Minimax and Posterize Time before/after animated warps.

`collapse_transformations` covers `M17`. It should compare collapsed vs non-collapsed nested text/solid precomps, parent matrix composition, deferred text rasterization, scale-aware text quality, and graph cycle rejection behavior where applicable.

`color_composite_sampling` covers `M01`, `M03`, and `M19`. It is the base block that all effects depend on: straight vs premult alpha, RGBA8 quantization, bilinear sampling, edge sampling, normal alpha composite, layer opacity, and gamma/color-space assumptions.

## Promotion Guidance

1. Generate primitive and operator goldens before full-template goldens.
2. For every `implemented approximate` module, add telemetry before formula tuning.
3. For every `instrumented/testable` module, export AE references and thresholds before changing formulas.
4. Template goldens should name the weakest blocking module from `docs/MATH_PARITY_STATUS.md`.
5. Keep `block_cards.json` as the machine-readable source for generator planning, and this document as the human map.

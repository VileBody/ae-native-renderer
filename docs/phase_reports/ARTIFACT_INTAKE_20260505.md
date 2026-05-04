# Reverse Artifact Intake

Generated: `2026-05-05T00:21:46`

This document is the current intake map for AE parity work: what evidence we need, what is already local, and what still blocks formula tuning.

## Summary

| Block | Status | Evidence | Missing / Risk |
| --- | --- | --- | --- |
| `M19_core_alpha_composite` | `ready` | binaries 6/6, predecode 1/1, AE artifacts 3/3 | none |
| `M10_M11_M13_glow_shadow_blur` | `ready` | binaries 5/5, predecode 5/5, AE artifacts 3/3 | none |
| `M12_minimax` | `ready` | binaries 1/1, predecode 1/1, AE artifacts 4/4 | none |
| `M14_turbulent_displace` | `ready` | binaries 2/2, predecode 1/1, AE artifacts 3/3 | none |
| `M05_M07_text_glyph_cooltype` | `partial` | binaries 3/3, predecode 1/1, AE artifacts 2/2 | CoolType binary is present; exact CoolType function targets are not selected yet. |
| `M08_M09_expression_host` | `ready` | binaries 3/3, predecode 2/2, AE artifacts 2/2 | none |
| `M15_M17_geometry_collapse_motion` | `ready` | binaries 2/2, predecode 2/2, AE artifacts 2/2 | none |
| `M16_temporal_motion_scheduler` | `ready` | binaries 2/2, predecode 3/3, AE artifacts 1/1 | none |

## Blocks

### M19_core_alpha_composite

Lock straight/premult alpha, hidden RGB, normal blend, opacity, and source-over metric normalization.

Questions:
- What do GF::Composite bool flags mean?
- What alpha type enters/exits Transfer/Composite/AlphaGain?
- When does AE preserve hidden RGB under alpha zero?

Binaries:
- `target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll`: present, bytes=2263560, sha256=36ea8222bc1e
- `target/reverse/ae_2026/core_composite_alpha/RendererCPU.dll`: present, bytes=444424, sha256=0fdcc6d09a06
- `target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Blend.aex`: present, bytes=43528, sha256=383e45e13ecd
- `target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/SolidComposite.aex`: present, bytes=47624, sha256=c81b6c635bbe
- `target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Unmult.aex`: present, bytes=106504, sha256=8f6151614e75
- `target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Unmultiply.aex`: present, bytes=47112, sha256=150dcd06d54d

Predecode:
- `core_alpha_gpufoundation`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/core_alpha_gpufoundation`, decompile=8, disasm=8

AE artifacts:
- `fixtures/ae_conformance_pack/manifest.json`: present (bytes=7183, cases=23)
- `fixtures/ae_conformance_pack/ae_goldens/metadata/tiff_png_summary.json`: present (bytes=14778)
- `fixtures/ae_conformance_pack/ae_goldens/downloads/ae_conformance_goldens_png_tiff_ae_conformance_clean_cases_85_20260503_170505_85.zip`: present (zip=1)

### M10_M11_M13_glow_shadow_blur

Lock Drop Shadow alpha mask/composite, Glow mask source/composite, and shared blur alpha options.

Questions:
- Does Drop Shadow blur alpha only and where does color enter?
- Does Glow threshold use RGB, alpha, luma, or premult source?
- Which ImageRenderer/GPUFoundation blur/composite path is used?

Binaries:
- `target/reverse/ae_2026/effects_blur_glow_shadow/Box_Blur.aex`: present, bytes=106504, sha256=aa8f2eb1489e
- `target/reverse/ae_2026/effects_blur_glow_shadow/Drop_Shadow.aex`: present, bytes=113160, sha256=f5ef57b00fa3
- `target/reverse/ae_2026/effects_blur_glow_shadow/Glow.aex`: present, bytes=116232, sha256=bb8f60143cbb
- `target/reverse/ae_2026/core_composite_alpha/ImageRenderer.dll`: present, bytes=2325000, sha256=daaf1ec09961
- `target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll`: present, bytes=2263560, sha256=36ea8222bc1e

Predecode:
- `blur_gpufoundation_kernels`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/blur_gpufoundation_kernels`, decompile=8, disasm=8
- `box_blur_aex`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/box_blur_aex`, decompile=8, disasm=8
- `drop_shadow_aex`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/drop_shadow_aex`, decompile=8, disasm=8
- `glow_aex`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/glow_aex`, decompile=7, disasm=7
- `imagerenderer_gaussian_composite`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/imagerenderer_gaussian_composite`, decompile=6, disasm=6

AE artifacts:
- `fixtures/ae_probe_pack/glow_shadow/manifest.json`: present (bytes=10093, cases=18)
- `fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/glow_shadow/ae_probe_outputs/png`: present (png=18)
- `fixtures/ae_probe_pack/downloads/ae_probe_round5_85_20260504_011829_tiff_png_outputs_85.zip`: present (zip=1)

### M12_minimax

Lock operation/channel/direction enum mapping, radius rounding, edge policy, and bpc path.

Questions:
- Are radius values floored, ceiled, rounded, or fractional?
- How does don't-shrink-edges alter sampling outside source bounds?
- Are 8/16/32 bpc callbacks equivalent after normalization?

Binaries:
- `target/reverse/ae_2026/effects_temporal_noise_distort/Minimax.aex`: present, bytes=135688, sha256=95afa7a3b4a5

Predecode:
- `minimax_aex`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/minimax_aex`, decompile=7, disasm=7

AE artifacts:
- `fixtures/ae_probe_pack/minimax/manifest.json`: present (bytes=3437, cases=9)
- `fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/minimax/ae_goldens/metadata/minimax_measurements.json`: present (bytes=117809, cases=9)
- `fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/minimax/ae_goldens/metadata/minimax_property_dump.json`: present (bytes=16095)
- `fixtures/ae_probe_pack/downloads/ae_probe_round5_85_20260504_011829_tiff_png_outputs_85.zip`: present (zip=1)

### M14_turbulent_displace

Recover the hidden displacement field kernel, seed/evolution semantics, pinning, and field units.

Questions:
- What do FracAllKernel and Frac1DKernel compute exactly?
- How do amount/size/complexity/evolution/random seed feed lookup tables?
- Which pinning and out-of-bounds rule is applied?

Binaries:
- `target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentDisplace.aex`: present, bytes=124424, sha256=f3eedc80b4eb
- `target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentNoise.aex`: present, bytes=100360, sha256=7acc6f0d84b4

Predecode:
- `turbulent_displace_aex`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/turbulent_displace_aex`, decompile=9, disasm=9

AE artifacts:
- `fixtures/ae_probe_pack/turbulent_field/manifest.json`: present (bytes=6312, probe_suites=11)
- `fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field/ae_goldens/metadata/turbulent_vector_measurements.json`: present (bytes=144726, cases=43)
- `fixtures/ae_probe_pack/downloads/ae_probe_round5_85_20260504_011829_tiff_png_outputs_85.zip`: present (zip=1)

### M05_M07_text_glyph_cooltype

Lock glyph metrics, baseline/line boxes, text blur source space, and collapsed text sharpness substrate.

Questions:
- Which CoolType/TXT path supplies glyph ids, advances, bboxes, and baselines?
- What is AE text animator blur kernel and coordinate space?
- When does collapsed precomp defer text rasterization?

Binaries:
- `target/reverse/ae_2026/text_expression/Basic_Text.aex`: present, bytes=677896, sha256=e309414b6812
- `target/reverse/ae_2026/text_expression/CoolType.dll`: present, bytes=4549128, sha256=00b91d7d9672
- `target/reverse/ae_2026/text_expression/AfterFXLib.dll`: present, bytes=52127240, sha256=26adb4973934

Predecode:
- `basic_text_aex`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/basic_text_aex`, decompile=10, disasm=10

AE artifacts:
- `fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf`: present (bytes=195404)
- `fixtures/ae_conformance_pack/ae_goldens/metadata/effect_property_dump.json`: present (bytes=29552)

Note: CoolType binary is present; exact CoolType function targets are not selected yet.

### M08_M09_expression_host

Lock property expression host context, layer-local time, vector/scalar coercion, and expression selector variables.

Questions:
- How do BEE and Scripting convert comp time to layer/expression time?
- Where are value/textIndex/textTotal/inPoint/outPoint injected?
- Which property value coercions are AE host behavior versus ExtendScript engine behavior?

Binaries:
- `target/reverse/ae_2026/text_expression/Scripting.aex`: present, bytes=23887368, sha256=3a7064f8dbad
- `target/reverse/ae_2026/text_expression/extendscript.dll`: present, bytes=770056, sha256=2acda4e1feb5
- `target/reverse/ae_2026/core_composite_alpha/BEE.dll`: present, bytes=24164872, sha256=e78df04d63dd

Predecode:
- `scripting_expression_host`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host`, decompile=14, disasm=14
- `bee_temporal_scheduler`: `target/reverse/predecoded/20260505_002125_bee_temporal_scheduler_intake/bee_temporal_scheduler`, decompile=6, disasm=6

AE artifacts:
- `fixtures/ae_conformance_pack/manifest.json`: present (bytes=7183, cases=23)
- `fixtures/ae_conformance_pack/ae_goldens/metadata/effect_property_dump.json`: present (bytes=29552)

### M15_M17_geometry_collapse_motion

Lock Geometry2 matrix path, transform sampling, motion-blur matrix samples, and collapse/deferred-raster boundary.

Questions:
- Which Geometry2 params map to GF::Transformation fields?
- How does TransformWithMotionBlur choose matrix samples?
- Where is the raster barrier for collapsed text/vector layers?

Binaries:
- `target/reverse/ae_2026/Transform.aex`: present, bytes=159752, sha256=21661b33ef1b
- `target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll`: present, bytes=2263560, sha256=36ea8222bc1e

Predecode:
- `geometry_transform_gpufoundation`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/geometry_transform_gpufoundation`, decompile=8, disasm=8
- `geometry_transform_wrapper`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/geometry_transform_wrapper`, decompile=4, disasm=4

AE artifacts:
- `fixtures/ae_conformance_pack/manifest.json`: present (bytes=7183, cases=23)
- `fixtures/ae_conformance_pack/ae_goldens/metadata/tiff_png_summary.json`: present (bytes=14778)

### M16_temporal_motion_scheduler

Lock posterize source-time blocking and motion-blur shutter/sample scheduling.

Questions:
- Which time is bucketed by Posterize Time?
- How are shutter angle/phase converted into sample times?
- Does expression/effect/sample time use comp or layer local units?

Binaries:
- `target/reverse/ae_2026/effects_temporal_noise_distort/Posterize_Time.aex`: present, bytes=65544, sha256=9e91bcee7857
- `target/reverse/ae_2026/core_composite_alpha/BEE.dll`: present, bytes=24164872, sha256=e78df04d63dd

Predecode:
- `posterize_time`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/posterize_time`, decompile=4, disasm=4
- `time_displace_temporal`: `target/reverse/predecoded/20260504_222748_full_predecode_round2/time_displace_temporal`, decompile=6, disasm=6
- `bee_temporal_scheduler`: `target/reverse/predecoded/20260505_002125_bee_temporal_scheduler_intake/bee_temporal_scheduler`, decompile=6, disasm=6

AE artifacts:
- `fixtures/ae_conformance_pack/manifest.json`: present (bytes=7183, cases=23)

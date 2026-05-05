# Ghidra Extraction: Blocker Modules Round 2

Status date: 2026-05-05

Purpose: prepare concrete Ghidra extraction bundles for the modules blocked
after hypothesis round 1, so follow-up work can analyze decompiled targets
directly instead of reopening Ghidra projects or guessing from final PNGs.

## Direct Answers

### What We Did Not Know Before Round 1

Round 1 proved that the missing information was not mostly "which obvious big
parameter is wrong". The unknowns were lower-level discriminators:

- Geometry2: pixel-center convention, sampler/OOB footprint, and the exact
  meaning of sampler quality enum `0012`.
- Glow: threshold-source mask before blur/intensity/composite, especially for
  absent `0001`, `0001=1`, and `0001=2`.
- Drop Shadow / Blur: softness radius rounding, fractional edge behavior, and
  effect-local shadow composite policy.
- Minimax: full operation/channel/direction enums beyond the narrow values
  already present in the pack.
- Turbulent Displace: AE vector field/kernel behavior before coordinate-field
  displacement can be tuned.

The important correction: final PNG conformance can confirm a formula, but it is
not enough to choose among several hidden pipeline conventions when their error
appears only on edges or after a stacked adjustment layer.

### What Was Missing

We lacked two kinds of evidence:

- isolated AE probe cases that expose one hidden convention at a time;
- Ghidra bundles for the exact functions that own those conventions.

This extraction closes the second gap for the current blocker modules. The first
gap still needs probe packs/goldens for cases where the decompiled code points
to more than one plausible runtime path.

### Why Geometry2 Was Not Finished

Geometry2's large pieces are already accepted:

- property mapping for `0003/0004/0005/0008/0009`;
- recovered GF matrix order;
- layer/effect-space inverse sampling;
- adjustment-stack participation.

The remaining Geometry2 error is small in mean diff and concentrated around
sampling edges. Running 36 native variants against the same current AE final
image would rank candidates, but it would not prove whether the winner is
pixel-center, OOB threshold, footprint coverage, or quality enum behavior. The
right fix is either:

- generate the edge/OOB/`0012` AE discriminator pack, then sweep candidates; or
- read the relevant GF/Transform sampling path from the extracted bundles and
  validate it against a smaller probe.

### Why Probes Were Not Made Earlier

That was an orchestration miss. We let "finite hypotheses over existing
fixtures" run before adding a strict "probe availability gate". For modules
whose unknowns are pipeline conventions rather than exposed params, the correct
sequence is:

1. name the finite hypothesis set;
2. check whether existing fixtures isolate each hypothesis;
3. generate missing probes before assigning formula tuning;
4. only then implement/rank candidates.

Round 1 should have stopped at step 2 for Geometry2/Glow/Minimax/Turbulent and
produced the probe/extraction queue immediately.

## Extraction Run

Run root:

```text
target/reverse/predecoded/20260505_153013_blocker_modules_round2
```

Command:

```sh
python3 scripts/prepare_ghidra_bundles.py \
  --task geometry_transform_gpufoundation \
  --task geometry_transform_wrapper \
  --task blur_gpufoundation_kernels \
  --task box_blur_aex \
  --task drop_shadow_aex \
  --task glow_aex \
  --task imagerenderer_gaussian_composite \
  --task minimax_aex \
  --task turbulent_displace_aex \
  --task basic_text_aex \
  --task cooltype_glyph_metrics \
  --task cooltype_glyph_metrics_core \
  --skip-missing \
  --run-name blocker_modules_round2
```

Result: `12/12` tasks completed with return code `0`.

The run produced `126` target folders and `630` extracted files across
metadata, decompile output, disassembly, callees, callers, and references.

## Task Coverage

| Task | Targets | Status | Main use |
| --- | ---: | --- | --- |
| `geometry_transform_gpufoundation` | 8 | ok | Matrix, quality, sampler, bounds, motion blur path. |
| `geometry_transform_wrapper` | 4 | ok | Geometry2 wrapper, param bridge, motion wrapper candidates. |
| `blur_gpufoundation_kernels` | 8 | ok | Shared blur kernels and alpha flags. |
| `box_blur_aex` | 8 | ok | Box Blur wrapper and delayed GF blur calls. |
| `drop_shadow_aex` | 8 | ok | Offset, softness, shadow alpha/composite candidates. |
| `glow_aex` | 7 | ok | Glow mask/render wrapper and routing. |
| `imagerenderer_gaussian_composite` | 6 | ok | Gaussian implementation and composite dispatcher. |
| `minimax_aex` | 7 | ok | 8/16/32 bpc callbacks, GPU path, comparators. |
| `turbulent_displace_aex` | 9 | ok | Param setup, lookup tables, kernel dispatch. |
| `basic_text_aex` | 10 | ok | Basic Text / CoolType entry candidates. |
| `cooltype_glyph_metrics` | 31 | ok | Glyph ids, widths, bboxes, baselines, feature processing. |
| `cooltype_glyph_metrics_core` | 20 | ok | Core glyph metric callees and fixed-point scale. |

## Binary Inputs

| SHA-256 | Path |
| --- | --- |
| `36ea8222bc1e3abae148771cbea03eb60cd4bbcc708e145af5dd9451782d2466` | `target/reverse/ae_2026/GPUFoundation.dll` |
| `21661b33ef1b6aefdc9a686316c0be5d29642683eb94848cdc58d6ad38ef17ba` | `target/reverse/ae_2026/Transform.aex` |
| `aa8f2eb1489ebb492ee2d37873bfec394cfeb7dcfbcf6fa8f29af0e4a7d46b98` | `target/reverse/ae_2026/effects_blur_glow_shadow/Box_Blur.aex` |
| `f5ef57b00fa3607125c5ede95766af6d84b5612fdadfb9de3b6d6c1495d0f9fe` | `target/reverse/ae_2026/effects_blur_glow_shadow/Drop_Shadow.aex` |
| `bb8f60143cbb342fdb3c074a129647b65c5e593772c305593707e0dee9a9b2d4` | `target/reverse/ae_2026/effects_blur_glow_shadow/Glow.aex` |
| `daaf1ec0996126fec10159490ea5eb93323e399e8ee4030bfde3259586887883` | `target/reverse/ae_2026/core_composite_alpha/ImageRenderer.dll` |
| `95afa7a3b4a539e8389c60901149f285e8cd3e809d9ad11a08f80d979fa19b32` | `target/reverse/ae_2026/effects_temporal_noise_distort/Minimax.aex` |
| `f3eedc80b4ebe033a497f6dc50f7822059c3770113016e7a68231b25d578180b` | `target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentDisplace.aex` |
| `e309414b6812daa65eedf7fa99ed087260dd70f8ef5b8314eeab2174799512cc` | `target/reverse/ae_2026/text_expression/Basic_Text.aex` |
| `00b91d7d96723ed995f3d57f99db2aacaca86df4c9ab70b0200ac734cb696d21` | `target/reverse/ae_2026/text_expression/CoolType.dll` |

## Follow-Up Handoff

Use the extracted folders as the source of truth for the next analysis pass:

- Geometry2: inspect `geometry_transform_gpufoundation` and
  `geometry_transform_wrapper` for pixel center, sampler quality enum, and OOB
  behavior before changing `crates/effects/src/geometry.rs`.
- Glow: inspect `glow_aex` plus `imagerenderer_gaussian_composite` for mask
  creation, threshold basis, Gaussian radius mapping, and composite dispatch.
- Shadow/Blur: inspect `drop_shadow_aex`, `box_blur_aex`, and
  `blur_gpufoundation_kernels` for direction/distance conversion, softness
  rounding, alpha-only blur flags, and source-over policy.
- Minimax: inspect `minimax_aex` for operation/channel/direction enum tables,
  radius rounding, and "Don't Shrink Edges".
- Turbulent: inspect `turbulent_displace_aex` for lookup table construction,
  evolution/time inputs, kernel dispatch, and field coordinate scaling.
- Text: inspect `basic_text_aex`, `cooltype_glyph_metrics`, and
  `cooltype_glyph_metrics_core` for glyph id, width, bbox, baseline, and
  feature processing contracts.

Any finding that introduces a shared hidden policy, like the earlier M19 alpha
case, should be escalated to orchestration before local formula changes.

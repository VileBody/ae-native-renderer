# Box Blur / Glow Fast Path Trace - 2026-05-05

## Scope

Fast dynamic pass for the blur-adjacent effects cluster:

- Box Blur radius / iterations / channel flags.
- Glow radius mapping into AE's CPU `IR_GaussianBlur` path.
- First alpha-channel flag contrast against Drop Shadow.

## Probe Execution

All AE renders ran on the reserved `85.239.48.31` node through the S3 pack path.
Glow probes were split into small JSX entrypoints after the full builder proved too
large and crash-prone for repeated dynamic tracing.

Chunked Glow entrypoints:

- `jsx/build_glow_radius_probe_project.jsx`
- `jsx/build_glow_mask_probe_project.jsx`
- `jsx/build_glow_intensity_probe_project.jsx`
- `jsx/build_glow_composite_probe_project.jsx`

## Box Blur Findings

Trace logs:

- `target/dynamic_tools_85/box_blur_radius_alpha_20260505/SHBL_BB_S08_R11P2_I1.jsonl`
- `target/dynamic_tools_85/box_blur_radius_alpha_20260505/SHBL_BB_S18_R07_I3_DIV271.jsonl`

Observed constructor:

`Box_Blur.aex -> GPUFoundation.DLL::GF::BoxBlur_1DImgOpInfo`

| Case | AE radius | GF radius float | GF rounded radius | Iterations | Flags |
| --- | ---: | ---: | ---: | ---: | ---: |
| `SHBL_BB_S08_R11P2_I1` | 11.2 | 11.1999969 | 12 | 1 | `0x7f` |
| `SHBL_BB_S18_R07_I3_DIV271` | 7 | 7 | 7 | 3 | `0x7f` |

Conclusion:

- Box Blur uses the AE radius value directly.
- Radius quantization is `ceil(radius)`.
- Iterations map directly to the AE iteration parameter.
- Flags `0x7f` indicate all-channel/RGBA blur in this path.
- This contrasts with Drop Shadow softness, where the confirmed path sets alpha-only blur (`0x61`).

## Glow Radius Findings

Trace logs:

- `target/dynamic_tools_85/glow_radius_alpha_retry_20260505/GMD_RADIUS_R5.jsonl`
- `target/dynamic_tools_85/glow_radius_alpha_split_20260505/GMD_RADIUS_R0P5.jsonl`
- `target/dynamic_tools_85/glow_radius_alpha_split_20260505/GMD_RADIUS_R1.jsonl`
- `target/dynamic_tools_85/glow_radius_alpha_split_20260505/GMD_RADIUS_R2.jsonl`
- `target/dynamic_tools_85/glow_radius_alpha_split_20260505/GMD_RADIUS_R10.jsonl`

Observed call:

`Glow.aex+0x746d -> ImageRenderer.dll::IR_GaussianBlur`

| Case | AE Glow Radius | `IR_GaussianBlur` radius arg |
| --- | ---: | ---: |
| `GMD_RADIUS_R0P5` | 0.5 | 0.200000003 |
| `GMD_RADIUS_R1` | 1 | 0.400000006 |
| `GMD_RADIUS_R2` | 2 | 0.800000012 |
| `GMD_RADIUS_R5` | 5 | 2 |
| `GMD_RADIUS_R10` | 10 | 4 |

Conclusion:

```text
ae_ir_gaussian_radius = glow_radius * 0.4
```

Native was previously using `glow_radius / 2.0`. It is now switched to the traced
AE mapping before the existing native blur approximation.

## Remaining Work

- Decode `IR_GaussianBlur` kernel details beyond the radius argument.
- Trace `Glow.aex+0x86b0 -> IR_CompositeWithBlendMode` with a composite-specific chunk.
- Run mask/intensity/composite chunks to lock Glow threshold and alpha/RGB policy.
- Replace the native box approximation with an AE-like Gaussian kernel once the
  kernel/edge policy is recovered.

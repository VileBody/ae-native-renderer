# M19 Reverse Lock

Status date: 2026-05-05

M19 is now locked for the native renderer's RGBA8 normal-composite substrate.
The lock is intentionally narrow: it covers layer/source-over compositing,
transparent background RGB, layer opacity as source alpha gain, and PNG/TIFF
golden comparison semantics. It does not claim full AE internals for arbitrary
blend modes, 16/32 bpc, color-managed output, CPU/GPU divergence, or
effect-local premultiply wrappers.

## Locked Answers

| Question | Locked answer | Evidence / implementation |
| --- | --- | --- |
| AE internal canvas: straight or premult RGBA? | There is no single global answer in AE. The reversed GPUFoundation path is format-tagged and can cross straight/premult boundaries. Native M19 locks straight RGBA8 at render-core/effect boundaries and uses premult math only inside normal source-over. | `Composite.cl`, `AEFX_Matte.cl`, `GF::Unpremultiply`, `GF::AlphaGain`; `raster_cpu::composite_normal_pixel`. |
| Source-over formula | `alphaA = src.a * opacity`; `outA = alphaA + dst.a * (1 - alphaA)`; RGB is computed in premult form and unpremultiplied before RGBA8 quantization. | Decoded `Composite.cl` / `AEFX_Matte.cl`; `crates/raster-cpu/src/composite.rs` unit tests. |
| Float/u8/rounding | Native release contract computes the equation in `f32` and rounds to nearest RGBA8 at the output boundary. This matches the current PNG/TIFF conformance target. | `composite_normal_pixel`; `partial_alpha_matches_reversed_source_over_formula`. |
| RGB when alpha is zero | A transparent source contribution is a no-op and preserves destination hidden RGB. AE goldens preserve transparent comp background RGB such as `[5, 5, 6, 0]`. | `alpha_composite_background_findings.md`; `transparent_source_preserves_destination_rgb_under_zero_alpha`. |
| Premultiply/unpremultiply timing | For normal layer composite, premultiply is internal to the source-over equation and output returns to straight RGBA8. Other AE conversions are module/path-specific and must not be inferred globally. | Decoded composite kernels; M19 scope limits in conformance report. |
| PNG/TIFF transparent RGB | Golden comparison keeps exported transparent RGB instead of normalizing to transparent black. Raw RGBA is compatibility-only; visible tuning uses RGB-over-background plus alpha/background metrics. | `RGB_ALPHA_METRIC_POLICY`; `diff_rgb8_under_alpha_policy`; AE golden background samples. |
| Layer opacity | Normal layer opacity is treated as source alpha gain before source-over. It does not pre-scale straight RGB separately. Near-zero opacity follows the `GF::Composite` no-op fast path threshold. | `GF::Composite` epsilon evidence; `layer_opacity_scales_source_alpha_before_source_over`; `epsilon_layer_opacity_preserves_destination`. |
| Effect input/output | Native effect boundary is straight RGBA8. AE effect wrappers may premultiply/unpremultiply internally; those policies are owned by effect modules (`M10`, `M11`, `M13`, etc.) and are no longer a global M19 blocker. | Effect sidecars keep module-local alpha telemetry; M19 gate scope limits. |

## Guardrails

- Do not tune from raw `rgba` alone. Use
  `rgb_straight_source_over_ae_background`, `background_alpha_normalized`, and
  `alpha`.
- If an effect needs premultiplied blur, matte, or packed-alpha behavior, add the
  formula and evidence to that effect module instead of reopening M19.
- If a future target switches to 16/32 bpc, color-managed output, non-normal
  blend modes, or a different AE renderer path, create a new contract version
  rather than changing `m19.rgb_alpha_metric_policy.v1` silently.

## Current Code Contract

```text
Canvas storage:              straight RGBA8
Normal composite math:       f32 source-over, premult internal, straight output
Opacity <= epsilon:          destination no-op
Transparent source alpha:    destination hidden RGB preserved
Transparent export RGB:      preserved for conformance metrics
Raw RGBA metric:             compatibility-only
Primary visible metric:      rgb_straight_source_over_ae_background
```

This moves M19 from `instrumented/testable` to
`reverse implemented (RGBA8 normal composite)`.

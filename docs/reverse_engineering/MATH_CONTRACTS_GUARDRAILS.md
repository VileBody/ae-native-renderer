# Math Contracts / Guardrails

Status date: 2026-05-05

This is the shared M19 contract for native conformance work. It records the
locked RGBA8 normal-composite substrate, what remains compatibility-only, and
what belongs to module-local effect contracts.

Evidence roots:

- `target/reverse/predecoded/20260504_222748_full_predecode_round2`
- `docs/reverse_engineering/alpha_composite_background_findings.md`
- `docs/reverse_engineering/M19_REVERSE_LOCK.md`
- `docs/phase_reports/AGENT_A_SUBSTRATE_MATH_OBJECTS_20260504.md`
- `docs/CONFORMANCE.md`

## Metric Contract

The conformance runner writes raw compatibility metrics and an M19 split metric
set. The metric policy is locked for the current RGBA8 normal-composite output
contract; it is not a claim that every AE internal surface uses the same storage
format.

`testkit::RGB_ALPHA_METRIC_POLICY` is the JSON contract emitted by
`render-cli conformance-pack`:

- `rgba`: absolute RGBA8 diff. This is the legacy compatibility field and can be
  dominated by invisible alpha/background policy.
- `rgb`: raw RGB diff with alpha ignored.
- `alpha`: raw alpha-channel diff.
- `background_alpha_normalized`: RGBA diff where alpha is ignored only for pixels
  whose RGB matches the detected native and AE background-corner RGB.
- `foreground_rgb`: raw RGB diff over the union foreground mask, useful when a
  background alpha mismatch hides the actual module signal.
- `rgb_under_alpha_policy`: RGB diff after straight RGBA8 source-over projection
  onto the AE/reference background RGB. This normalizes invisible RGB under alpha
  zero and exposes premult-looking RGB at partial alpha.
- `rgb_straight_source_over_ae_background`: explicit alias for
  `rgb_under_alpha_policy`; added so effect/alpha reports name the current
  straight-RGBA output contract directly.
- `rgb_over_native_background` and `rgb_over_ae_background`: explicit projection
  variants for background-sensitivity checks.

Current flags:

- `raw_rgb_ignores_alpha=true`
- `alpha_reported_separately=true`
- `rgb_under_alpha_policy_uses_source_over=true`
- `rgb_under_alpha_policy_uses_reference_background=true`
- `premult_unpremultiply_applied=false`
- `premult_contract_locked=true`
- `diagnostic_only=false`

Guardrail: do not tune an effect formula from `rgba` alone when `alpha` or
`background_corner.rgb_matches_alpha_differs` is high. Use `rgb`,
`rgb_under_alpha_policy`, and effect-specific telemetry to decide whether the
visible math changed.

## Alpha / Premult

Confirmed:

- AE normal source-over in the decoded composite shaders follows the standard
  premultiply, source-over, unpremultiply shape.
- Native `raster_cpu::composite_normal_pixel` is the locked straight RGBA8
  normal source-over primitive. It computes in `f32`, premultiplies internally,
  unpremultiplies before RGBA8 quantization, applies layer opacity as source
  alpha gain, and preserves destination hidden RGB for zero-alpha source pixels.
- AE conformance PNGs preserve background RGB under alpha zero, for example
  `[5, 5, 6, 0]`.
- `GPUFoundation.dll` exposes `GF::Unpremultiply`, `GF::AlphaGain`,
  `GF::PackedAlphaGain`, and `GF::BlendUnpackedAlpha`.
- `GF::Composite` takes a pixel format, an `IR_BlendMode`, opacity, and two
  bool-like flags forwarded into the kernel arguments.

Out of M19 scope:

- Exact meaning of the `GF::Composite` bool flags and the full normal-blend enum
  mapping for every AE layer mode.
- Whether blur/effect kernels premultiply before spreading RGB under partial
  alpha.
- 16/32 bpc, color-managed output, CPU/GPU divergence, and arbitrary blend modes.

Policy: keep native effect boundaries as straight RGBA8 unless the effect module
locks a local premultiply/unpremultiply wrapper. Any alpha-sensitive formula
tuning must record the M19 metrics above plus module-local pre/post-alpha
telemetry.

Step 4 effects/alpha update: Box Blur, Drop Shadow, and Glow sidecars now report
diagnostic-only `alpha_policy` and per-intermediate alpha stats. These are probes,
not proof of AE internals; do not use them to change global alpha behavior without
an orchestrator decision.

## Gamma / Color

Confirmed:

- Current native canvas and PNG diff path operate on 8-bit RGBA bytes.
- `ImageRenderer` has a pixel-format map that normalizes several format codes.
- The collected core bundle includes color and output modules such as
  `ColorSpaceConverter.dll`, `OCIOWrapper.dll`, `LUTEngine.dll`, and
  `dvamediatypes.dll`.

Unknown / BLOCKER:

- sRGB vs linear-light compositing for AE projects with non-default working space.
- Hidden channel conversion for 16/32 bpc, color-managed output, and non-PNG
  export paths.

Policy: do not claim gamma/color parity from PNG byte diffs alone. For color
tuning, record project working space, bpc, output module, and whether AE goldens
were exported as straight PNG/TIFF.

## Edge Sampling

Confirmed:

- Native `BilinearSampler` currently samples in pixel space, rounds/interpolates
  RGBA channels independently, and returns transparent black outside image
  bounds.
- Shared AE blur paths contain explicit copy/fill/kernel branches and can fill
  transparent black around padded/intermediate buffers.

Unknown / BLOCKER:

- Effect-specific OOB policy for every kernel: transparent, clamp, repeat, edge
  extend, or alpha-specialized behavior.
- Pixel-center convention for every transform/effect path.

Policy: geometry and warp tuning must compare UV/sample telemetry before final
pixels. Do not infer an edge formula from final RGB alone.

## Time Semantics

Confirmed:

- Conformance pack frame time is `frame / fps`.
- Current native runner records frame index and time in `metrics.json`.

Unknown / BLOCKER:

- Exact AE boundary semantics for source frame selection, Posterize Time buckets,
  effect param sampling, adjustment-layer lower-stack resampling, and motion blur
  subframe sampling are module-specific and not resolved by M19.

Policy: final diffs must not be used to tune color/alpha if the same case has an
unresolved temporal owner. Record source frame index and effect param time before
touching substrate math.

## Quality / BPC

Confirmed:

- Native conformance metrics currently compare RGBA8 output.
- `GF::Unpremultiply` stride logic recognizes component-depth-like cases for 8,
  10, 16, 24, and 32-bit layouts.
- `BoxBlurOptions` has separate source and destination alpha type fields.

Unknown / BLOCKER:

- AE 16/32 bpc rounding, float range, sub-byte quantization, and output conversion
  for the current template set.

Policy: a module is not `parity locked` for quality/bpc until it passes an AE
golden at the claimed bpc or records that the release gate is RGBA8-only.

## CPU / GPU Path

Confirmed:

- Current native renderer is CPU-side.
- AE evidence crosses `GPUFoundation.dll` and `ImageRenderer.dll`.
- `ImageRenderer` selects one of eight CPU composite workers from three bool
  fields at offsets `+8`, `+9`, and `+10`.

Unknown / BLOCKER:

- Whether AE goldens were produced by CPU or GPU path for each case, and whether
  CPU/GPU paths differ for alpha, blur, or color management.

Policy: every AE golden run should record AE version, renderer/path when known,
output module, and bpc. If CPU/GPU divergence is observed, lock formulas only
against the explicitly selected path.

## Parameter Mapping

Confirmed:

- Numeric AE params must remain traceable as raw `000n` fields until a matchName
  mapping is proven.
- `BoxBlurOptions::GetSrcAlphaType` reads offset `+4`; `GetDestAlphaType` reads
  offset `+8`; `SetBlurAlphaChannelOnly` clears option bits `0xe` and sets bit
  `0x1`.

Unknown / BLOCKER:

- Any global enum or flag whose values are not mapped by direct evidence,
  including `IR_BlendMode`, alpha type enum names, pixel-format bits, quality
  switches, and preserve-alpha/matte flags.

Policy: if an unknown global entity controls multiple modules, write `BLOCKER`
in the report and do not invent a formula. Keep formulas local only when their
inputs are mapped or explicitly labeled `approximation`.

## Status Gate

- `implemented`: native code executes the feature.
- `instrumented`: metrics or telemetry expose enough intermediate state to
  localize divergence.
- `AE golden exists`: AE output is present with documented export settings.
- `formula tuning`: code changes are based on a known divergent primitive, not
  only final-frame appearance.
- `parity locked`: split metrics, alpha policy, bpc/quality, CPU/GPU path, and
  effect-specific telemetry agree within declared thresholds.

M19 is `reverse implemented (RGBA8 normal composite)`. Full-template parity still
depends on module-local effect formulas, sampling, color-management, and bpc
contracts.

# Ghidra Analysis M11 Glow Mask Composite - 2026-05-05

## Status

`PARTIAL / PROBE_REQUIRED`.

The fresh extraction confirms Glow render routing, the ImageRenderer Gaussian blur
entrypoints, and the ImageRenderer composite dispatcher shape. It does not expose
the bodies of the Glow-local helpers that most likely build the threshold source
and apply the final effect-local composite, so absent `0001`, `0001=1`, and
`0001=2` remain probe-gated.

No implementation code was read or changed. M19 was not reopened. I see no direct
contradiction with the existing M19 guardrail; the composite uncertainty remains
M11 effect-local unless a probe proves otherwise.

## Inputs

- `target/reverse/predecoded/20260505_153013_blocker_modules_round2/glow_aex`
- `target/reverse/predecoded/20260505_153013_blocker_modules_round2/imagerenderer_gaussian_composite`
- Context only: `docs/phase_reports/M11_HYPOTHESIS_RESULTS_20260505.md` and
  `docs/phase_reports/HYPOTHESIS_ROUND1_ORCHESTRATOR_SUMMARY_20260505.md`

## Findings

1. Threshold-source / mask

- `glow_aex/03_Glow_render_candidate_5f50_180005f50/decompile.c` is parameter
  setup, not the render body. It registers params 1..0x12 plus 0x18/0x19 and
  writes `param_2 + 0x30 = 0xf` as the effect parameter count hint.
- The numeric defaults strongly identify the core controls:
  - param 2: threshold-like slider, max `0xff0000`, default `0x990000`
    (153/255, AE 60 percent style).
  - param 3: radius-like slider, max `0x3e80000`, default `0x0a0000` (10).
  - param 4: intensity-like slider, max `0xff0000`, default `0x010000` (1).
- `Glow_mask_candidate_5970` is not the pixel threshold mask. It opens a file
  dialog for curve/map files (`ACV;AMP`) and writes a `FILE_Spec`; do not use it
  as evidence for threshold behavior.
- Runtime render dispatch is in `EffectMainExtra`:
  - checked-out world depth `8` calls `FUN_180001810`.
  - depth `0x10` calls `FUN_1800013e0`.
  - depth `0x20` calls `FUN_180001c40`.
  - smart/render callback also reaches the same 16/32 bpc functions.
- The extracted 16 bpc body `FUN_1800013e0` reads param IDs `0xe`, `1`, and `7`
  before the main build path, acquires `PF World Suite`, calls
  `FUN_180004db0` for two floating expansion values, allocates an expanded world,
  then calls `FUN_180003690` and `FUN_1800071c0`.
- The threshold-source formula itself is therefore unknown from this extraction.
  The likely location is `FUN_180003690` and/or `FUN_1800071c0`, but those bodies
  are not in the prepared bundle.
- No Ghidra evidence here proves absent `0001`, `0001=1`, or `0001=2`. Keep the
  current native parser mapping as implementation-side instrumentation only:
  absent -> combined, `0001=1` -> color channels, `0001=2` -> alpha channel.

2. Radius / intensity mapping and GaussianBlur

- `FUN_1800013e0` calls `FUN_180004db0(param_1, &local_148, &local_140)` and
  uses those two values as x/y expansion: destination dimensions are source plus
  `2 * expansion`, and origin offsets are source origin minus expansion. This is
  the strongest Glow.aex evidence for blur-radius derived pad/bounds.
- The actual blur call is not visible inside the Glow extraction. It is probably
  behind `FUN_1800071c0`, `FUN_180002610`, or `FUN_180008470`.
- ImageRenderer confirms the Gaussian implementation path:
  - `IR_GaussianBlur` at `1800a0e40` is a wrapper that calls `FUN_18009fca0`.
  - `FUN_18009fca0` takes two double radius/sigma-like values (`param_11`,
    `param_12`) and a direction/mode field (`param_16`), validates x/y extents,
    builds recursive Gaussian coefficients with `cos`, `sin`, and `exp`, then
    runs separable horizontal and vertical task objects.
  - Horizontal pass is `_anon_5B6C0454::BlurHorizontal::vftable`; vertical pass
    is `_anon_5B6C0454::BlurVertical::vftable`.
  - Small spans call direct helpers (`FUN_18009f4d0`, `FUN_18009f000`);
    larger spans call `FUN_1800a1650`, which executes a task on all processors.
- `FUN_18009fca0` can allocate a transient pixel buffer when source/destination
  format/alpha flags differ, then frees it at the end.
- No evidence in this bundle accepts the current native `ceil(radius / 2)` box
  blur mapping for AE Glow. It should remain rejected as an AE-internal claim
  until the Glow helper or a radius probe confirms it.

3. Final composite / blend / pixel format / alpha hints

- ImageRenderer composite path is confirmed but not formula-resolved:
  - `IR_CompositeWithBlendMode` calls the special jump table target
    `18005cee0`, then `FUN_1800a1650`, then worker select `FUN_1800769b0`.
  - `FUN_1800769b0` selects one of eight workers from three boolean flags at
    offsets `+8`, `+9`, `+0xa`.
  - The selected workers are `FUN_1800757c0`, `FUN_1800754b0`, `FUN_180075190`,
    `FUN_180074d30`, `FUN_180074a00`, `FUN_180074590`, `FUN_180074110`, and
    `FUN_180073b50`.
- `IR_pixel_format_map` maps/falls back among format codes `4`, `0x104`,
  `0x805`, `0x905`, `0xa05`, `0xb05`, `0x1004`, and `0x1104`, choosing SIMD or
  scalar converter pointers depending on global CPU feature flags.
- `IR_GaussianBlur_impl` chooses intermediate format codes `0x108`, `0x909`,
  `0xb09`, or `0x10a` based on source flags and `param_14`, and requires source
  and destination alpha class bits `(flags & 0x1c0)` to match unless it allocates
  a transient buffer.
- Alpha/composite hints exist, but final Glow ordering is not accepted:
  `IR_FillWithTransparentBlack`, `IR_Convert`, transient buffer allocation, and
  `IR_CompositeWithBlendMode` all appear in the ImageRenderer path. The Glow.aex
  extraction does not prove whether final is source-over, add/screen-like blend,
  original-on-top, or another effect-local wrapper.

4. Confirming functions / addresses

- Glow entry metadata: `Glow.aex`, effect match name `ADBE Glo2`,
  registration helper `FUN_180009000` at `180009000`.
- Param setup: `FUN_180005f50` at `180005f50`.
- File dialog helper misnamed as mask candidate: `FUN_180005970` at `180005970`.
- Render dispatcher: `EffectMainExtra` at `180007f60`.
- 16 bpc render body in bundle: `FUN_1800013e0` at `1800013e0`.
- Other render bodies referenced but not extracted here: `FUN_180001810` and
  `FUN_180001c40`.
- Likely threshold/blur/composite helpers referenced by 16 bpc render:
  `FUN_180003690`, `FUN_1800071c0`, `FUN_180002610`, `FUN_180008470`,
  `FUN_180004db0`.
- ImageRenderer Gaussian wrapper: `IR_GaussianBlur` at `1800a0e40`.
- ImageRenderer Gaussian implementation: `FUN_18009fca0` at `18009fca0`.
- ImageRenderer task executor: `FUN_1800a1650` at `1800a1650`.
- ImageRenderer composite worker selector: `FUN_1800769b0` at `1800769b0`.
- ImageRenderer special jump table target: `FUN_18005cee0` at `18005cee0`.
- ImageRenderer pixel format map: `FUN_180079800` at `180079800`.

## Accepted/Rejected/Unknown

Accepted:

- Glow render path dispatches by output depth through `EffectMainExtra` to
  separate 8/16/32 bpc render bodies.
- `Glow_mask_candidate_5970` is not the pixel mask; it is a file picker/helper.
- ImageRenderer Gaussian is separable horizontal/vertical recursive Gaussian,
  not the current native shared box blur.
- ImageRenderer final composite has a blend-mode dispatcher with multiple worker
  paths and format/alpha flag handling.

Rejected:

- Do not accept `ceil(radius / 2)` box blur as AE Glow internals from this
  Ghidra pass.
- Do not tune intensity or final blend from current final pixels before the
  threshold source is isolated.
- Do not reopen M19 from this evidence. No contradiction was established.

Unknown:

- Exact threshold predicate for absent `0001`, `0001=1`, and `0001=2`.
- Whether the threshold-source copies full RGBA, RGB with alpha-derived mask,
  premultiplied RGB, or a single-channel alpha/luma mask before blur.
- Exact radius value passed from Glow.aex into `IR_GaussianBlur`.
- Exact intensity scaling range and whether alpha is scaled with RGB.
- Final Glow composite order and blend mode for Composite Original / Glow
  Operation variants.

## Missing Probes

1. Source-mask discriminator:
   threshold `120`, radius `0`, intensity `1`; pixels
   dark/high-alpha `[32,32,32,255]`, bright/low-alpha `[240,240,240,64]`,
   and dark/low-alpha control; variants absent `0001`, `0001=1`, `0001=2`.

2. Radius/Gaussian discriminator:
   after mask is locked, use a single bright impulse with threshold pass,
   intensity `1`, no original composite if available; sweep radius
   `0`, `0.5`, `1`, `2`, `5`, `10`, `35`. Compare against recursive Gaussian,
   box `ceil(radius/2)`, and direct-radius Gaussian candidates.

3. Intensity discriminator:
   locked mask and radius `0` or smallest nonzero radius; sweep intensity
   `0`, `0.5`, `1`, `1.25`, `2`, `8`, `10` and measure RGB/alpha separately.

4. Composite discriminator:
   locked blurred glow over transparent, opaque black, and semi-transparent
   colored backgrounds; sweep Composite Original and Glow Operation params.
   Mark `ORCHESTRATOR_BLOCKER` only if this contradicts the global M19 contract,
   and cite the exact ImageRenderer worker reached.

5. Ghidra follow-up extraction:
   add `FUN_180003690`, `FUN_1800071c0`, `FUN_180002610`, `FUN_180008470`,
   `FUN_180004db0`, `FUN_180001810`, and `FUN_180001c40` to the next Glow.aex
   bundle. Add ImageRenderer workers selected by `FUN_1800769b0` if composite
   probes split across multiple runtime paths.

## Next Implementation Candidate

Do not implement a formula change yet. The next implementation-adjacent step is
to add the source-mask probe pack and sidecar comparison for absent `0001`,
`0001=1`, and `0001=2`. If that probe proves the native parser mapping, the
first real candidate should replace Glow's current box blur with an
ImageRenderer-style separable recursive Gaussian candidate behind a local
feature flag, then compare radius sweeps before touching intensity or final
composite.

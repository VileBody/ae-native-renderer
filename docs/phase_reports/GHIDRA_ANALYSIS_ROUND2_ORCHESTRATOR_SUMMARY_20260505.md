# Ghidra Analysis Round 2 Orchestrator Summary

Status date: 2026-05-05

Scope: review the five agent reports produced from
`target/reverse/predecoded/20260505_153013_blocker_modules_round2`.

Methodology update: this round separates three outcomes:

- `implementation-candidate`: Ghidra plus existing probes are strong enough to
  write a local candidate, still gated by isolated conformance.
- `probe-gated`: the formula space is finite, but existing fixtures do not
  isolate the remaining choice.
- `deeper-extraction-gated`: Ghidra found the next relevant helpers/kernels, so
  more extraction must happen before formula work.

## Agent Reports

| Module | Report | Outcome |
| --- | --- | --- |
| `M10` Shadow / Blur | `docs/phase_reports/GHIDRA_ANALYSIS_M10_SHADOW_BLUR_20260505.md` | `probe-gated`, close to implementation candidate |
| `M11` Glow | `docs/phase_reports/GHIDRA_ANALYSIS_M11_GLOW_MASK_COMPOSITE_20260505.md` | `deeper-extraction-gated` plus probe-gated |
| `M12` Geometry2 | `docs/phase_reports/GHIDRA_ANALYSIS_M12_GEOMETRY2_SAMPLER_20260505.md` | `probe-gated`, with shared sampler blocker |
| `M13` Minimax | `docs/phase_reports/GHIDRA_ANALYSIS_M13_MINIMAX_ENUM_RADIUS_20260505.md` | partial `implementation-candidate`, fractional/edge probe-gated |
| `M14` Turbulent Displace | `docs/phase_reports/GHIDRA_ANALYSIS_M14_TURBULENT_KERNEL_20260505.md` | `deeper-extraction-gated` or AE vector-field-gated |

## Module Verdicts

### `M10` Shadow / Blur

Ready facts:

- offset integer conversion uses truncation toward zero at the Drop Shadow GPU
  consumer boundary;
- working sign remains `dx = trunc(-cos(deg) * distance)`,
  `dy = trunc(sin(deg) * distance)`;
- softness routes through `GF::FastBoxBlur`;
- radius quantization in the blur kernel is ceil-like;
- Drop Shadow explicitly sets alpha-channel-only blur;
- shadow color/composite is local to `CompositeShadowMask`.

Not locked:

- upstream direction producer was not in this extraction;
- active softness factor/count branch (`1.4 x 1` vs `1.0 x 3`) needs a sweep;
- exact local `CompositeShadowMask` alpha/color math needs a colored translucent
  probe.

Next action:

Build the M10 probe pack first, then implement the candidate if the probes match
the Ghidra path. Do not change renderer-wide alpha/composite from M10 evidence.

### `M11` Glow

Ready facts:

- Glow dispatches by output depth through separate 8/16/32 bpc render paths;
- extracted `Glow_mask_candidate_5970` is a file picker/helper, not the pixel
  threshold mask;
- ImageRenderer Gaussian is separable recursive Gaussian, not the current native
  shared box blur;
- ImageRenderer composite has multiple worker paths and format/alpha flag
  handling.

Not locked:

- threshold predicate and threshold source for absent `0001`, `0001=1`,
  `0001=2`;
- exact Glow radius into `IR_GaussianBlur`;
- intensity scaling;
- final Glow operation/composite ordering.

Next action:

Add the missing Glow helper targets to Ghidra extraction:
`FUN_180003690`, `FUN_1800071c0`, `FUN_180002610`, `FUN_180008470`,
`FUN_180004db0`, `FUN_180001810`, `FUN_180001c40`, plus ImageRenderer composite
workers selected by `FUN_1800769b0`. In parallel, generate the Glow source-mask
probe. Do not implement Glow math yet.

### `M12` Geometry2

Ready facts:

- destination is prefilled with transparent black before transform;
- Geometry2 sampling goes through shared GPUFoundation transform kernels;
- GF host code has visible nearest, bilinear, bicubic Lanczos, and bicubic
  area-sample branches;
- area/no-area selection depends on host-computed matrix radii.

Not locked:

- pixel center convention is not derivable from current extraction;
- source OOB/clamp/filter footprint is hidden in shared kernels;
- AE UI `0012` to GF quality dword mapping is not proven.

Orchestrator blocker:

This is a shared sampler policy. Do not tune Geometry2 by changing global
sampling from final `EFF_040` pixels. We need a Geometry2 edge/OOB/quality probe
or deeper shared GF kernel extraction.

Next action:

Generate `GEO2_EDGE_010`: impulse/checkerboard/alpha-border, edge UV targets,
subpixel translate, `0012` Bilinear/Bicubic sweep, and sidecar telemetry for
quality bytes if possible.

### `M13` Minimax

Ready facts:

- visible operation labels: `1=Minimum`, `2=Maximum`,
  `3=Minimum Then Maximum`, `4=Maximum Then Minimum`;
- visible channel labels: `1=Color`, `2=Alpha and Color`, `3=Red`, `4=Green`,
  `5=Blue`, `6=Alpha`;
- visible direction labels: `1=Horizontal & Vertical`, `2=Just Horizontal`,
  `3=Just Vertical`;
- active callback radius window is integer `2r + 1`;
- CPU callbacks share the same separable monotonic queue shape across 8/16/32
  bpc.

Not locked:

- stage order for operations `3/4` still needs an impulse probe;
- `0004=2/3` runtime orientation still needs an impulse probe;
- fractional radius quantization is not visible in callback body;
- `0005` boolean polarity and exact edge semantics need boundary probes;
- CPU/GPU edge parity remains unknown.

Next action:

Implement only the label-order enum mapping and integer-radius morphology if the
current native code lacks it; keep fractional radius and `Don't Shrink Edges`
behind probes. Do not assign stack residuals to M13.

### `M14` Turbulent Displace

Ready facts:

- setup builds a `64 x 64` table from an LCG-like seed path;
- complexity has integer octave and fractional remainder;
- evolution/cycle state affects table period/seed path;
- internal modes `9..11` use `Frac1D`; other modes use `FracAll`;
- 1D path has horizontal/vertical axis lookup resources and phase constants.

Not locked:

- exact `FracAll` vector basis;
- exact sign convention from displacement vector to source coordinate;
- GPU sampler/address/filter policy;
- pinning/resize-layer interaction at edges.

Next action:

Do not replace the current field formula from this extraction alone. Either
extract GPU kernel bodies/resources for `TurbulentDisplaceFracAllKernel` and
`TurbulentDisplaceFrac1DKernel`, or run AE vector-field probes and fit the
field/sampler from those outputs.

## Prioritized Work Queue

1. `M10`: build shadow/softness/composite probe pack, then implement the
   Ghidra-backed candidate if probes confirm.
2. `M13`: implement/verify enum label mapping and integer-radius morphology;
   generate fractional radius and edge probes for the rest.
3. `M12`: build `GEO2_EDGE_010` before touching shared sampling.
4. `M11`: run Glow helper extraction and source-mask probe before changing
   Glow blur/composite.
5. `M14`: choose between deeper GPU kernel extraction and AE vector-field
   probes before field formula implementation.

## Guardrails For Next Agents

- Do not call a candidate "improved" unless its probe isolates the term being
  changed.
- Do not tune against `STK_*` final pixels until isolated module probes pass.
- Any finding that changes shared alpha, sampler, color, or composite policy
  must be escalated as `ORCHESTRATOR_BLOCKER`.
- A finite hypothesis list is incomplete unless every branch has either an
  existing fixture, a planned probe, or an explicit deeper-extraction target.

# Required Information Matrix

Status date: 2026-05-05

Purpose: exhaustive inventory of the information required before the current
AE parity blockers can move from "hypothesis/testing" into
`reverse_implemented` and then `parity_locked`.

This document follows the updated methodology:

```text
finite question
  -> existing evidence inventory
  -> missing Ghidra/probe/telemetry item
  -> isolated candidate
  -> composition regression
  -> lock
```

## Evidence Classes We Need For Every Module

Every math/effect module needs all of these before it can be called
`reverse_implemented`:

1. **AE environment contract**
   - AE version/build and source node.
   - BPC path: 8/16/32, and whether the module uses CPU, GPU, or mixed path.
   - Color management and working-space settings.
   - Render output format and alpha handling for PNG/TIFF/goldens.
   - Source asset dimensions, row origin, pixel aspect, and hidden RGB policy.
   - Time/frame rate/shutter settings when the module samples time.

2. **Parameter contract**
   - AE property id/matchName/key to native field mapping.
   - UI label order and enum numeric values.
   - Units: pixels, degrees, percent, fixed `16.16`, normalized `0..1`, time.
   - Default value, min/max, clamp, and bypass behavior.
   - Animated/time-varying parameter sampling rule.

3. **Pipeline contract**
   - Input image/world that the effect reads.
   - Intermediate buffers and whether they are transparent-prefilled, copied,
     clamped, alpha-only, RGB-only, premultiplied, or straight.
   - Kernel/dispatch path and pass order.
   - Output composite policy local to the effect.
   - Whether the behavior is module-local or shared renderer policy.

4. **Formula contract**
   - Exact arithmetic, constants, signs, matrix/order, branches, and rounding.
   - Edge/OOB behavior.
   - Pixel-center convention.
   - Per-BPC differences.
   - CPU/GPU parity or explicit path split.

5. **Observability contract**
   - AE isolated probe/golden that exposes exactly one unknown.
   - Native sidecar telemetry for params/intermediates.
   - Native render/diff output with mean/max/RGB-alpha split.
   - Regression/composition case after the isolated case passes.
   - Durable phase report linking formula, fixture, goldens, native output, and
     remaining uncertainty.

## Information Collected So Far

### Ghidra Predecode

Primary round-two extraction:

```text
target/reverse/predecoded/20260505_153013_blocker_modules_round2
```

Coverage:

- Geometry2 / GPUFoundation transform host.
- Drop Shadow / Box Blur / shared blur kernels.
- Glow / ImageRenderer Gaussian/composite dispatcher.
- Minimax.
- Turbulent Displace.
- Basic Text / CoolType glyph metrics.

Follow-up extraction collected in this pass:

```text
target/reverse/predecoded/20260505_155036_followup_m10_m11_helpers
```

Coverage:

- `drop_shadow_followup`: `4/4` targets, all functions present.
- `glow_aex_followup`: `8/8` targets, all functions present.
- `imagerenderer_composite_workers`: `9/9` targets, all functions present.

The follow-up targets were added to
`docs/reverse_engineering/GHIDRA_PREDECODE_TASKS.json` so they are repeatable.

### Existing AE Probe Packs

Existing packs:

- `fixtures/ae_probe_pack/glow_shadow/manifest.json`
  - Glow cases: `GLO_010..GLO_101`.
  - Drop Shadow cases: `DSH_010..DSH_061`.
- `fixtures/ae_probe_pack/minimax/manifest.json`
  - Minimax cases: `MINIMAX_PRE`, `MINIMAX_OP1_CH1_R0`,
    `MINIMAX_OP1_CH1_R12`, `MINIMAX_OP1_CH2_R0`,
    `MINIMAX_OP1_CH2_R12`, `MINIMAX_OP2_CH1_R0`,
    `MINIMAX_OP2_CH1_R12_EFF050`, `MINIMAX_OP2_CH2_R0`,
    `MINIMAX_OP2_CH2_R12`.
- `fixtures/ae_probe_pack/turbulent_field/manifest.json`
  - Suites: `TD_AMOUNT_SWEEP`, `TD_SIZE_SWEEP`, `TD_COMPLEXITY_SWEEP`,
    `TD_EVOLUTION_STATIC`, `TD_EVOLUTION_ANIM`, `TD_DISPLACEMENT_TYPE`,
    `TD_SEED_SWEEP`, `TD_PINNING_EDGE`, `TD_RESIZE_LAYER`,
    `TD_SAMPLER_CHECK`, `TD_IMPULSE_GRID`.
- `fixtures/ae_conformance_pack/manifest.json`
  - Core effect cases: `EFF_010..EFF_070`, `STK_010..STK_030`,
    plus text/expression/graph/composite cases.

These packs are useful, but they are not exhaustive under the new gate because
several remaining unknowns require discriminator cases that do not yet exist.

## Global Shared Policies

These are cross-module. If one of these changes, it can invalidate multiple
agents' work.

### `M19` Alpha / Composite

Status: locked for the narrow RGBA8 normal source-over contract.

Still required:

- Keep effect-local composite decisions separate from global M19.
- If an effect contradicts M19, cite the exact Ghidra worker/probe before
  reopening the global contract.

### Shared Sampler / Pixel Center

Status: not locked.

Required information:

- Destination pixel center convention: integer center vs half-pixel center.
- Source OOB policy: transparent, clamp, edge extension, partial footprint.
- Bilinear/bicubic footprint contribution at source edge.
- UI quality enum to internal sampler enum mapping.
- Area/no-area threshold for matrix-filtered transforms.

Current owner: `M12 Geometry2`, but this may become a renderer-wide sampler
guardrail if probes show shared behavior.

### BPC / CPU / GPU Path Split

Status: partially known per module.

Required information:

- Which probes/render jobs hit 8, 16, or 32 bpc paths.
- Whether AE uses CPU callback or GPU kernel for each module in our test setup.
- Whether CPU/GPU produce identical output for edge/fractional cases.

### Color Management

Status: assumed stable for current packs, not independently locked.

Required information:

- Working space / color-management settings in generated AE projects.
- Whether effects operate in linearized or display-space values.
- Whether PNG/TIFF export converts before comparison.

## `M10` Drop Shadow / Box Blur

### Required Information

- Direction/distance:
  - exact UI direction convention;
  - sign for all quadrants;
  - float-to-int rounding for positive and negative offsets.
- Softness/radius:
  - active branch for `1.4 x 1` vs `1.0 x 3`;
  - divisor `2.71` usage;
  - per-axis radius quantization;
  - blur pass count.
- Blur kernel:
  - alpha-only input behavior;
  - src/dest alpha type flags;
  - kernel weights and edge behavior;
  - fractional softness behavior.
- Composite:
  - `CompositeShadowMask` color and opacity math;
  - `shadow_only` behavior;
  - overlap of shadow with source;
  - partial-alpha source behavior.

### Current Evidence

- Ghidra round two confirms truncation at consumer boundary, alpha-only blur,
  ceil-like radius quantization, and local `CompositeShadowMask`.
- Follow-up Ghidra extraction now includes
  `DropShadow_direction_distance_producer_73a0`.
- Existing AE probe pack has `DSH_010..DSH_061`.

### Still Missing

- Direction sweep across all quadrants with fractional projections.
- Softness sweep that identifies active branch.
- Alpha-only source with RGB noise under varying alpha.
- Colored translucent composite probe with `shadow_only=0/1`.
- Analysis of the new follow-up Ghidra bundle.

### Next Collection Step

Analyze:

```text
target/reverse/predecoded/20260505_155036_followup_m10_m11_helpers/drop_shadow_followup
```

Then generate `M10_SHADOW_BLUR_DISCRIMINATOR_010` if the new extraction still
leaves branch/composite ambiguity.

## `M11` Glow

### Required Information

- Threshold source:
  - absent `0001` behavior;
  - `0001=1` color-channel behavior;
  - `0001=2` alpha-channel behavior;
  - whether mask stores alpha only, RGB, luma, premultiplied values, or full
    RGBA.
- Gaussian/radius:
  - exact radius value passed to ImageRenderer;
  - recursive Gaussian coefficients and sigma/radius relationship;
  - padding/bounds expansion.
- Intensity:
  - scale domain and clamp behavior;
  - whether alpha scales with RGB.
- Composite:
  - Glow Operation;
  - Composite Original;
  - blend worker chosen by ImageRenderer flags;
  - format/alpha class conversions.
- BPC:
  - 8/16/32 path differences.

### Current Evidence

- Glow dispatch by BPC is known.
- ImageRenderer Gaussian is separable recursive Gaussian.
- Composite dispatcher and worker selector are known.
- Follow-up extraction now includes the previously missing Glow helper bodies
  and ImageRenderer composite workers.
- Existing AE probe pack has `GLO_010..GLO_101`.

### Still Missing

- Analysis of the new Glow helper/composite worker bundles.
- Source-mask discriminator for absent/`0001=1`/`0001=2`.
- Radius impulse sweep after mask is locked.
- Intensity sweep after mask/radius are locked.
- Composite discriminator after threshold/radius/intensity are locked.

### Next Collection Step

Analyze:

```text
target/reverse/predecoded/20260505_155036_followup_m10_m11_helpers/glow_aex_followup
target/reverse/predecoded/20260505_155036_followup_m10_m11_helpers/imagerenderer_composite_workers
```

Then generate `GLOW_MASK_010`.

## `M12` Geometry2 / Shared Sampler

### Required Information

- Pixel center:
  - output pixel center used by AE transform kernels;
  - source UV computation after inverse matrix.
- OOB/edge:
  - source bounds threshold;
  - transparent vs clamp vs edge extension;
  - partial bilinear/bicubic footprint contribution;
  - hidden RGB behavior at transparent edge pixels.
- Sampling quality:
  - AE UI `0012` to GF quality dword mapping;
  - nearest/bilinear/bicubic Lanczos/bicubic area branches;
  - sharp vs smooth second dword.
- Area/no-area:
  - matrix radii threshold;
  - footprint size under scale/rotation.

### Current Evidence

- Destination transparent prefill is proven.
- Shared GF transform kernel families are identified.
- Current isolated `EFF_040` is very close but edge-sensitive.

### Still Missing

- AE edge/impulse probe for pixel center and OOB.
- `0012` UI sweep with sidecar/state dump if possible.
- GF kernel/body extraction or equivalent GPU kernel resource analysis if the
  probe is insufficient.

### Next Collection Step

Generate `GEO2_EDGE_010`:

- impulse/checkerboard/alpha-border sources;
- UV targets around `-0.5`, `0`, `0.5`, `width - 1`, `width - 0.5`, `width`;
- identity, subpixel translate, scale, small rotation;
- Bilinear/Bicubic `0012` sweep;
- native sidecar with UV, selected neighbors, OOB, sampled RGBA.

## `M13` Minimax

### Required Information

- Operation:
  - `1=min`, `2=max`;
  - stage order for `3=Minimum Then Maximum`;
  - stage order for `4=Maximum Then Minimum`.
- Channel:
  - `1=Color`, `2=Alpha and Color`, `3=R`, `4=G`, `5=B`, `6=A`;
  - alpha preservation or mutation per mode.
- Direction:
  - `1=Horizontal & Vertical`, `2=Just Horizontal`, `3=Just Vertical`;
  - exact pass order for HV.
- Radius:
  - integer window `2r + 1`;
  - upstream fractional quantization;
  - bypass behavior at radius `0`;
  - CPU/GPU parity.
- Edge:
  - `Don't Shrink Edges` boolean polarity;
  - boundary queue seeding/clipping semantics.
- BPC:
  - 8/16/32 component comparison and rounding.

### Current Evidence

- Label-order enum skeleton is known.
- Integer active-kernel radius window is known.
- Existing probe pack covers only a narrow subset: op `1/2`, channel `1/2`,
  radius `0/12`.

### Still Missing

- Operation `3/4` impulse probe.
- Channel `3..6` lane probe.
- Direction `2/3` orientation probe.
- Fractional radius sweep.
- Edge `0005=0/1` boundary probe.
- 16/32 bpc focused probes.

### Next Collection Step

Generate `MINIMAX_ENUM_RADIUS_EDGE_010`, then implement only confirmed enum and
integer-radius behavior.

## `M14` Turbulent Displace

### Required Information

- Table/noise:
  - exact `64 x 64` table construction;
  - seed and cycle/evolution period;
  - complexity integer/fractional octave composition.
- Displacement:
  - `FracAll` vector basis per displacement type;
  - sign from vector to source coordinate;
  - amount/size coordinate scale;
  - offset phase.
- 1D modes:
  - internal `9/10/11` mapping;
  - horizontal/vertical lookup coordinate and phase constants.
- Sampler/OOB:
  - pinning behavior;
  - resize-layer behavior;
  - source sampling at borders;
  - alpha/RGB handling.
- BPC/GPU:
  - kernel path and pixel format mapping.

### Current Evidence

- Table-driven setup is confirmed.
- LCG-like seed path and complexity/evolution flow are partially known.
- `FracAll` vs `Frac1D` dispatch split is known.
- Existing `turbulent_field` pack has broad suites, but vector-field goldens
  still need decoding/measurement for formula fitting.

### Still Missing

- AE vector-field measurements for amount/size/complexity/evolution/type sweeps.
- Internal-mode probes for horizontal/vertical/cross.
- Pinning/OOB probes over coordinate-field and checkerboard sources.
- GPU kernel extraction/resource analysis if vector probes are insufficient.

### Next Collection Step

Run measurement/decoding over existing `turbulent_field` AE outputs if present;
otherwise produce new AE vector-field goldens.

## Text / CoolType / Expression Follow-Through

These are not the current five blocker modules, but they are required for the
template-level math finish line.

### Text / Glyph Metrics

Required:

- font selection, fallback, and exact font files;
- glyph id mapping;
- advances, bboxes, baselines;
- per-line layout, tracking, leading, paragraph alignment;
- feature processing/ligatures;
- raster coverage if moving from layout parity to pixel text parity.

Current evidence:

- Point Light and Montserrat files are present in the pack assets.
- CoolType Ghidra extraction covers glyph ids, widths, bboxes, baselines, and
  feature processing targets.

Missing:

- deeper CoolType raster coverage/glyph-id probe;
- native telemetry matched against AE `sourceRectAtTime` and glyph runs.

### Expression Evaluator

Required:

- exact property expression host access for `thisLayer`, `thisComp`, time,
  value, indices, and layer transforms;
- scalar/vector coercion;
- time sampling and delayed expression policy;
- whitelisted expression patterns from the templates.

Current evidence:

- Scripting expression host task exists.

Missing:

- focused analysis of `scripting_expression_host`;
- template-expression census tied to native evaluator cases.

## Required Output Artifacts Before Locking A Module

Each module lock needs these files or equivalent:

- `docs/phase_reports/AE_REVERSE_<MODULE>_FORMULA_<DATE>.md`
- AE probe pack manifest entry with case ids.
- AE goldens/measurements for those case ids.
- Native `hypothesis-pack` report for isolated cases.
- Native composition regression report for the relevant `STK_*` or template
  case.
- Code tests for parameter mapping, formula edge cases, and at least one native
  image comparison.
- Update to `docs/MATH_PARITY_STATUS.md` and module-specific effect docs.

## Immediate Collection Queue

1. Analyze the new follow-up Ghidra bundles for M10/M11.
2. Generate missing probe packs:
   - `M10_SHADOW_BLUR_DISCRIMINATOR_010`
   - `GLOW_MASK_010`
   - `GEO2_EDGE_010`
   - `MINIMAX_ENUM_RADIUS_EDGE_010`
3. Decode or regenerate Turbulent vector-field goldens.
4. Add native telemetry fields needed by those probes.
5. Only then start formula implementation candidates in this order:
   `M10`, `M13`, `M12`, `M11`, `M14`.

# AE Reverse Temporal / Posterize / Motion Ghidra

Status date: 2026-05-04

This is an intermediate Agent C checkpoint, not the final temporal report. Ghidra is the primary evidence source in this round. Existing probes, decoded shaders, and conformance outputs are validation/corroboration only.

## Inputs

| Binary | SHA-256 | Status |
| --- | --- | --- |
| `target/reverse/ae_2026/effects_temporal_noise_distort/Posterize_Time.aex` | `9e91bcee785766f54cdb2c0ab90a7cee6bcee5b34fdee660ce608ad0a9484d60` | Imported/decompiled in isolated Ghidra project. |
| `target/reverse/ae_2026/effects_temporal_noise_distort/Time_Displace.aex` | `710abb9bd2f3d8442fa26cef69a08f69420f6eda8341e8b3949cefa4b301dceb` | Imported/decompiled; first source-time findings added below. |
| `target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll` | `36ea8222bc1e3abae148771cbea03eb60cd4bbcc708e145af5dd9451782d2466` | Motion-blur transform backend mapped from targeted Ghidra logs. |
| `target/reverse/ae_2026/core_composite_alpha/AfterFXLib.dll` | `26adb49739349d41b0a6a8f2f711ea0782844410e9c5a71d0ccc65edd13cab31` | Core scheduling/export target map captured; full scheduler decompile pending. |
| `target/reverse/ae_2026/core_composite_alpha/BEE.dll` | `e78df04d63ddbb45f3ba2dc3278c162941a4738b65b97a88ebc051dd9a731752` | Newly collected from node `85.239.48.31`; primary owner of BEE render options, shutter, layer checkout, and effect-stack scheduling exports. |

Ghidra project:

```text
target/reverse/ghidra_projects/agent_temporal_motion
```

Raw outputs:

```text
target/reverse/agent_temporal_motion/20260504_211603_posterize_time_timeout30/
target/reverse/agent_temporal_motion/20260504_211757_posterize_targeted/
target/reverse/agent_temporal_motion/20260504_211904_time_displace_timeout30/
target/reverse/agent_temporal_motion/20260504_212140_time_displace_targeted/
target/reverse/agent_temporal_motion/posterize_180001350_disasm.txt
target/reverse/agent_temporal_motion/posterize_180001990_case17_disasm.txt
target/reverse/agent_temporal_motion/gpufoundation_motion_blur_evidence_index.txt
target/reverse/agent_temporal_motion/afterfx_temporal_scheduler_strings.txt
target/reverse/agent_temporal_motion/afterfx_temporal_scheduler_objdump.txt
target/reverse/agent_temporal_motion/bee_collection_manifest.json
target/reverse/agent_temporal_motion/bee_temporal_scheduler_exports.txt
target/reverse/agent_temporal_motion/bee_temporal_scheduler_strings.txt
```

Scripts:

```text
target/reverse/ghidra_scripts/DumpTemporalMotionHints.java
target/reverse/ghidra_scripts/DumpTemporalTargeted.java
```

## Posterize Time Findings

### Parameter Mapping

Confirmed:

- Effect string/match identifiers in `Posterize_Time.aex`:
  - `ADBE_Posterize_Time`
  - `ADBE Posterize Time`
  - `Posterize Time`
- UI parameter string:
  - `$$$/AE/Posterize_Time/LStr/0001=Frame Rate`
- Parameter checkout helper:
  - `Posterize_Time.aex` function `FUN_180006220`, entry `0x180006220`
  - called with parameter index `1`, which corresponds to `Frame Rate`.

Likely/inferred from parameter setup in `EffectMainExtra` case `4`:

- Frame Rate value is an AE fixed-point value, not a raw float.
- `DAT_180008430` at VA `0x180008430` is `1.52587890625e-05`, exactly `1 / 65536`.
- Candidate fixed-point parameter constants:
  - `0x10000` -> `1.0`
  - `0x3e70000` -> `999.0`
  - `0x400000` -> `64.0`
  - `0x180000` -> `24.0`
- Native implication: payload values may already be JSON floats, but AE plugin internals convert `PF_Fixed` via `value / 65536.0`. Any direct AE param dump using integer fixed values must normalize this way.

Confidence: confirmed for fixed conversion constant and parameter index; inferred for min/max/default roles until the PF parameter struct layout is labeled.

### Time Semantics

Confirmed in Ghidra for `Posterize_Time.aex`:

- `FUN_180001350` and `EffectMainExtra` case `0x17` both:
  - check out Frame Rate at the current input time;
  - convert fixed frame rate to FPS via `fixed * (1 / 65536.0)`;
  - compute a bucket-sized time step using AE input time scale;
  - re-checkout/render input at the quantized time.

Formula recovered from `FUN_180001350`:

```text
fps = frame_rate_fixed / 65536.0
bucket_size_in_ae_ticks = time_scale / fps
bucket_time_ticks = trunc(current_time_ticks / bucket_size_in_ae_ticks) * bucket_size_in_ae_ticks
render_or_checkout_input_at(bucket_time_ticks)
```

The negative/pre-roll branch is now confirmed from disassembly: both inspected render paths use `vcvttsd2si`, the x86 truncating scalar-double-to-int conversion. No `floor`, `ceil`, or negative-time correction branch is visible around the bucket conversion. Therefore AE plugin bucket selection truncates toward zero for negative `current_time_ticks / bucket_size_in_ae_ticks`.

Native implication:

- For ordinary positive times, current native `floor(time * fps + epsilon) / fps` matches the recovered plugin behavior.
- Negative pre-roll/layer-local times should use trunc-toward-zero, not mathematical floor. A negative fixture is still needed to lock host scheduling around layer start/precomp, but the plugin-local bucket formula itself is confirmed.
- The plugin evidence proves source/input checkout is posterized, but does not yet prove the full AE adjustment-layer downstream scheduling policy.

Confidence: confirmed for bucket formula and negative-time cast inside the plugin; unknown for global adjustment-stack scheduling in `AfterFXLib`.

### Sampling Rules

Confirmed for Posterize Time:

- Posterize Time itself does not reveal pixel sampler math. It time-shifts the input checkout/render; pixel sampling belongs to the source/effect/render backend selected after the time substitution.

Implication:

- For `TMP_010` / `TMP_020`, validation should compare selected source frame identity and bucket boundaries, not sampler kernels.
- For `STK_030`, sampler/effect diffs after the bucket must be attributed to Geometry2/Minimax/Turbulent unless temporal telemetry shows the wrong bucket.

Confidence: confirmed for no local pixel sampler in the plugin path inspected.

### Alpha / Premult Policy

Confirmed for Posterize Time:

- No alpha/premult math was found in the Posterize plugin path. The plugin re-checks out/renders an input at a quantized time and returns that result.

Native implication:

- Alpha/premult differences in `STK_030` should not be tuned inside Posterize Time.
- Preserve hidden RGB/background behavior remains a core composite/render target policy, not a Posterize Time formula.

Confidence: confirmed for inspected plugin functions; alpha behavior downstream remains owned by core composite/effects.

### Color / Numeric Policy

Confirmed:

- Frame Rate uses fixed 16.16 conversion: `1 / 65536.0`.
- Bucket math is done in double precision in the decompiled plugin path, then cast back to an integer AE tick value.
- Integer cast of `current_time / bucket_size` truncates toward zero.

Unknown:

- Exact clamping/validation for invalid FPS values (`<= 0`, NaN, extreme values) is not fully labeled yet. Param setup suggests UI-level range limits, but runtime guard behavior needs one targeted pass around `FUN_180006220` and AE probes if malformed values are possible in payloads.

Confidence: confirmed for normal numeric path; unknown for invalid/unbounded payload behavior.

## Evidence

Posterize raw Ghidra evidence:

```text
target/reverse/agent_temporal_motion/20260504_211603_posterize_time_timeout30/stdout.txt
target/reverse/agent_temporal_motion/20260504_211757_posterize_targeted/stdout.txt
target/reverse/agent_temporal_motion/posterize_180001350_disasm.txt
target/reverse/agent_temporal_motion/posterize_180001990_case17_disasm.txt
```

Key functions:

| Function | Entry | Evidence | Confidence |
| --- | ---: | --- | --- |
| `EffectMainExtra` | `0x180001690` | PF command dispatch; setup uses Frame Rate param; smart/render command case `0x17` computes bucket time and invokes downstream checkout/render with quantized time. | confirmed |
| `FUN_180001350` | `0x180001350` | Helper used by `EffectMainExtra` case `0x0b`; same bucket formula, then checkout/render input at quantized AE ticks. Disassembly at `0x180001440` and `0x180001450` uses `vcvttsd2si`, confirming trunc-toward-zero bucket conversion. | confirmed |
| `EffectMainExtra` case `0x17` bucket block | around `0x180001a90` | Smart/render path repeats `vdivsd`, `vcvttsd2si`, multiply by bucket size, then second `vcvttsd2si`; no negative-time branch visible. | confirmed |
| `FUN_180006220` | `0x180006220` | Checks out parameter index `1` at current time and returns fixed-point Frame Rate value. | confirmed |
| `DAT_180008430` | `0x180008430` | Constant bytes decode to double `1.52587890625e-05 == 1/65536`. | confirmed |

Strings:

```text
ADBE_Posterize_Time
ADBE Posterize Time
$$$/AE/Posterize_Time/LStr/0001=Frame Rate
Plug-Ins/Effects/Posterize_Time
```

## Blockers / Not Found Yet

- Actual AE adjustment-layer downstream stack policy is not proven from `Posterize_Time.aex`; it now points to `BEE.dll` exports rather than only `AfterFXLib.dll` strings. We need Ghidra targets around effect-stack evaluation and adjustment layer render callbacks in BEE.
- Motion blur sample time generation is not in the GPUFoundation transform backend inspected so far. GPUFoundation consumes a prebuilt vector of matrices; shutter/sample scheduling appears upstream in `BEE_RenderOptions` inside `BEE.dll`.
- PF command IDs are still unlabeled. Current report names cases by numeric switch values (`0x0b`, `0x17`) until SDK enum mapping or decompiler labels are added.

## Time Displacement Findings

This is corroborating temporal-domain evidence, not a native implementation plan yet.

### Parameter Mapping

Confirmed strings from `Time_Displace.aex`:

| UI index | String |
| --- | --- |
| `0001` | `Stretch Map to Fit` |
| `0002` | `Time Displacement Layer` |
| `0003` | `Max Displacement Time [sec]` |
| `0004` | `Time Resolution [fps]` |
| `0005` | `If Layer Sizes Differ` |

Effect identifiers:

```text
ADBE_Time_Displacement
ADBE Time Displacement
```

Parameter setup constants recovered from `FUN_180002920`:

| Param | Candidate role | Fixed/raw constants | Notes |
| --- | --- | --- | --- |
| `0003` Max Displacement Time [sec] | fixed16.16 slider/value | `0xf1f00000`, `0xfffc0000`, `0xe100000`, `0x40000`, `0x10000` | Decode as approximately `-3600`, `-4`, `3600`, `4`, `1`. Struct field roles still need PF layout labels. |
| `0004` Time Resolution [fps] | fixed16.16 fps value | `0x28f`, `0x10000`, `0xe100000`, `0x7f0000`, `0x3c0000` | Decode as approximately `0.0099945`, `1`, `3600`, `127`, `60`. Likely default `60fps`, but exact UI min/max/default roles need labels. |

### Time / Source Selection Semantics

Confirmed from targeted decompile:

- `EffectMain` at `0x180002f70` dispatches render command `0x0b` to one of two render variants:
  - `FUN_180001ba0`
  - `FUN_180002100`
- The two variants are depth/format variants:
  - `FUN_180002100` uses `0x100` lookup entries and byte map samples;
  - `FUN_180001ba0` uses `0x8001` lookup entries and 16-bit-ish map samples.
- Both variants build a displacement lookup table first, then group identical source times before checking out/rendering source frames. This means source checkout count can be much lower than pixel count.
- Source checkout calls use a per-pixel/per-bucket time offset:

```text
source_time_ticks = current_time_ticks + displacement_ticks
checkout_source_at(source_time_ticks)
```

Observed map-to-time shape:

```text
map_luma_8  = (R8  * 0x2646 + G8  * 0x4b23 + B8  * 0x0e97 + 0x4000) >> 15
map_luma_16 = (R16 * 0x2646 + G16 * 0x4b23 + B16 * 0x0e97 + 0x4000) >> 15

high_precision_map_domain = 0x8001 entries, centered at 16384.0, scaled by 1/16384
byte_map_domain           = 0x100 entries, centered at 127.0,   scaled by 1/128
```

The luma weights sum to `0x8000`, matching AE-style weighted RGB luma with fixed-point rounding.

Observed LUT time quantization shape:

```text
current_seconds = current_time_ticks / time_scale
max_displacement_seconds = max_displacement_fixed * (1 / 65536.0)
time_resolution_step_seconds = 65536.0 / time_resolution_fixed

raw_time_seconds = current_seconds + normalized_map_value * max_displacement_seconds
clamped_time_seconds = clamp(raw_time_seconds, 0, (source_duration_ticks - 1) / time_scale)
quantized_time_seconds = clamped_time_seconds - suite_remainder(clamped_time_seconds, time_resolution_step_seconds)
displacement_bucket_ticks = int(quantized_time_seconds * 65536.0 + (quantized_time_seconds < 0 ? -0.5 : 0.5))
```

The `suite_remainder` name is not locked yet; Ghidra shows a suite/vtable call at `*(param_1 + 0xb0) + 0x108`, and the surrounding formula is `time - callback(time, step)`. For non-negative clamped time this is consistent with floor-to-grid at the selected `Time Resolution [fps]`.

Observed source checkout shape:

```text
current_seconds = current_time_ticks / time_scale
rounded_current_fixed = int(current_seconds * 65536.0 + (current_seconds < 0 ? -0.5 : 0.5))
checkout_source(displacement_bucket_ticks, rounded_current_fixed)
```

The decompile shows sign-aware `+0.5` / `-0.5` bias before conversion to int. That is round-to-nearest away from zero for seconds-to-fixed conversion, distinct from Posterize's trunc bucket conversion. Negative displaced source times are clamped to zero before the `Time_Displace` LUT quantization.

Frame interpolation policy:

- No explicit temporal interpolation between two checked-out source frames is visible inside the inspected `Time_Displace.aex` render variants.
- The plugin groups pixels by quantized source time, checks out the host/source frame for each group, then scatters/copies pixels from the returned world.
- Any footage frame blending or source frame interpolation appears delegated to the host checkout path, not performed locally by `Time_Displace.aex`.

Observed constant policy in `Time_Displace.aex`:

| VA | Value | Meaning |
| ---: | ---: | --- |
| `0x180007540` | `1.52587890625e-05` | `1 / 65536`, fixed16.16 conversion |
| `0x180007558` | `65536.0` | seconds/time value back to AE fixed/ticks |
| `0x180007550` | `1.0` | clamp/max helper constant |
| `0x180007548` | `0.5f` | rounding/bias helper candidate |
| `0x180007560` | `-0.5f` | rounding/bias helper candidate |
| `0x180007534` | `1 / 16384` as `f32` | high precision map normalization candidate |
| `0x180007538` | `1 / 128` as `f32` | 8-bit map normalization candidate |

Native implication:

- Time Displacement should not be modeled as one global time shift. It is a per-pixel temporal source sampler.
- The effect likely needs a source checkout/cache keyed by quantized source time, plus a map-level grouping step, before pixel sampling is tuned.
- `Time Resolution [fps]` is now formula-shaped: the LUT builder uses `65536.0 / time_resolution_fixed` as a seconds step and applies `time - suite_remainder(time, step)` after clamp. The exact suite callback name must still be labeled, but implementation can be tested as floor-to-grid for non-negative clamped source time.

Confidence: confirmed for string mapping, lookup sizes, fixed constants, grouped source checkout shape, clamp range, and step derivation; likely for depth/format interpretation and floor-to-grid Time Resolution semantics pending suite callback naming.

### Sampling Rules

Confirmed:

- Pixel sampling is a grouped temporal source checkout followed by direct pixel copy/scatter from the checked-out source world.
- Out-of-bounds source coordinates clear the target pixel in the inspected paths.
- No local bilinear/bicubic temporal blending was found in the effect path inspected.

Confidence: confirmed for inspected render variants; host checkout may still apply source-level frame blending outside the plugin.

### Alpha / Premult Policy

Confirmed:

- `Time_Displace.aex` uses the displacement map RGB channels for luma/time lookup. Alpha is not part of the recovered map luma formula.
- The source pixel copy preserves/copies the checked-out source pixel format for the chosen source time. The plugin path inspected does not expose a new alpha compositing formula.

Native implication:

- Alpha/premult tuning for Time Displace should happen in the source world copy/composite policy, not in map-time conversion.

Confidence: confirmed for map luma formula; alpha copy details still depend on exact world format branch.

## Motion Blur / GPUFoundation Findings

This checkpoint only covers the GPU transform backend, not the upstream AE shutter scheduler.

### Parameter Mapping

AfterFXLib/BEE strings confirm user-facing shutter controls:

```text
shutterAngleNumEdit
shutterPhaseNumEdit
Motion Blur
Samples Per Frame
Adaptive Sample Limit
```

GPUFoundation itself does not expose comp-level shutter angle/phase parameters in the inspected transform backend. It receives a `GF::TransformOperation` containing a vector of matrices. The comp-level shutter methods are exported from `BEE.dll`.

### Time Semantics / Sample Generation

Confirmed from existing targeted Ghidra logs:

- `GF::TransformWithMotionBlur` at `0x180076ed0` checks `TransformOperation::IsMotionBlur`.
- When motion blur is active, it uses `TransformOperation::NumMatrices()`.
- `TransformOperation::NumMatrices()` computes the vector length from matrix storage; the matrix stride is `0x48` bytes, matching a 3x3 double matrix.
- `TransformWithMotionBlur` copies `TransformOperation::InvertedMatrices()` into a float matrix buffer. The allocation shape is `sample_count * 0x24` bytes, matching 9 floats per sample.

Native implication:

- GPUFoundation is the renderer muscle here, not the scheduler brain. It consumes already-sampled transform matrices.
- Shutter angle, shutter phase, adaptive sample count, and exact sample time generation must be recovered upstream from `BEE.dll` / `BEE_RenderOptions`.
- Native motion blur should eventually split into two pieces: AE-time sample scheduler and backend matrix/pixel accumulation.

Confidence: confirmed for GPUFoundation sample-count source and matrix ingestion; unknown for shutter sample time formula.

### Sampling Rules

Confirmed strings referenced from `GF::TransformWithMotionBlur`:

```text
XFormMotionBlur_kSamplingMethod_BicubicAreaSample_kSamplingSharpness_Sharp_kAreaSampling_NoArea
XFormMotionBlur_kSamplingMethod_BicubicAreaSample_kSamplingSharpness_Sharp_kAreaSampling_Area
XFormMotionBlur_kSamplingMethod_BicubicAreaSample_kSamplingSharpness_Smooth_kAreaSampling_NoArea
XFormMotionBlur_kSamplingMethod_BicubicAreaSample_kSamplingSharpness_Smooth_kAreaSampling_Area
XFormMotionBlur_kSamplingMethod_Bilinear_kSamplingSharpness_NA_kAreaSampling_NoArea
XFormMotionBlur_kSamplingMethod_Bilinear_kSamplingSharpness_NA_kAreaSampling_Area
XFormMotionBlur_kSamplingMethod_NearestNeighbor_kSamplingSharpness_NA_kAreaSampling_NoArea
XFormMotionBlur_kSamplingMethod_BicubicLanczos_kSamplingSharpness_Sharp_kAreaSampling_NoArea
XFormMotionBlur_kSamplingMethod_BicubicLanczos_kSamplingSharpness_Smooth_kAreaSampling_NoArea
GPU.MotionBlur.UsingLanczosLowPass
```

Confidence: confirmed for backend sampler option strings and xrefs; branch mapping to AE quality settings is not fully labeled yet.

### Alpha / Premult Policy

No standalone alpha/premult formula was recovered from this temporal pass. GPUFoundation transform/composite alpha policy belongs to the core composite/effects reports; this temporal report only confirms matrix sample consumption.

## Adjustment Stack / Scheduler Target Map

AfterFXLib first exposed the scheduler names, but its Ghidra import log showed those symbols resolving through `BEE.DLL`. `BEE.dll` was then collected from the Windows AE node and its exports confirm exact RVAs for the temporal/scheduler targets.

Relevant strings/symbols captured in raw outputs:

```text
target/reverse/agent_temporal_motion/afterfx_temporal_scheduler_strings.txt
target/reverse/agent_temporal_motion/afterfx_temporal_scheduler_objdump.txt
target/reverse/agent_temporal_motion/bee_collection_manifest.json
target/reverse/agent_temporal_motion/bee_temporal_scheduler_exports.txt
target/reverse/agent_temporal_motion/bee_temporal_scheduler_strings.txt
```

Priority Ghidra targets:

| Symbol | BEE RVA | Why it matters |
| --- | ---: | --- |
| `BEE_RenderOptions::GetShutterSampleInfo` | `0x4ba230` | Main candidate for motion-blur sample time generation. |
| `BEE_RenderOptions::GetShutterStartTime` | `0x4ba2f0` | Candidate for shutter phase/start offset. |
| `BEE_RenderOptions::GetShutterDuration` | `0x4ba1a0` | Candidate for shutter angle to time-duration conversion. |
| `BEE_RenderOptions::GetShutterAngle` | `0x4ba190` | Ratio source for shutter duration. |
| `BEE_RenderOptions::GetShutterPhase` | `0x4ba220` | Ratio source for shutter start. |
| `BEE_RenderOptions::GetFrameShutterRange` | `0x4b96d0` | Candidate for conservative frame time window. |
| `BEE_RenderOptions::GetTime` / `GetTimeStep` | `0x4bac20` / `0x4bac40` | Base render time and frame duration source. |
| `BEE_GetLayerROAndTimeFromCompRO` | `0x454d00` | Candidate for converting comp render time to layer render time. |
| `BEE_CheckoutLayerFrame` / `BEE_WorkQueue_CheckoutLayerFrame` | `0x453e00` / `0x7d72c0` | Main checkout scheduling path for layer frames/effects. |
| `BEE_MultiCheckoutLayerFrame::CheckoutLayerFrame_SINGLE/MULTI` | `0x7ad240` / `0x7ac130` | Candidate for grouped source-time checkout and concurrent stack scheduling. |
| `BEE_LayerRenderOptions::BasicSetupForLayer` | `0x4b8d10` | Candidate for layer-local time/start/precomp render option initialization. |
| `BEE_LayerRenderOptions::IdenticalExceptForTime` | `0x4bb390` | Candidate for cache equivalence and temporal cache keys. |
| `BEE_LayerRenderOptions::SetEffectsToRender` / `GetEffectsToRender` | `0x4bc070` / `0x4b95b0` | Candidate for partial-stack/effect-stack scheduling, relevant to adjustment layers and Posterize downstream behavior. |

Adjustment-layer strings were also found:

```text
SwitchAdjustment
SwitchAdjustment_Xs
Adjustment Layer
```

Current state: target map confirmed with owning binary and RVAs. Formula-level shutter/adjustment policy is still pending BEE Ghidra decompile.

## Next Targeted Symbol / Function

Immediate next targets:

```text
BEE.dll: BEE_RenderOptions::GetShutterSampleInfo at 0x4ba230
BEE.dll: BEE_RenderOptions::GetShutterStartTime / GetShutterDuration
BEE.dll: BEE_GetLayerROAndTimeFromCompRO at 0x454d00
BEE.dll: BEE_CheckoutLayerFrame / BEE_WorkQueue_CheckoutLayerFrame
BEE.dll: BEE_LayerRenderOptions::SetEffectsToRender / GetEffectsToRender
Time_Displace.aex: FUN_180001000 / FUN_1800013c0 LUT quantization and Time Resolution rounding
```

## Validation Plan

Cases to run after this checkpoint:

| Case | Purpose |
| --- | --- |
| `TMP_010` | Source/layer start time and source frame identity. |
| `TMP_020` | Positive-time Posterize bucket boundaries. |
| `TMP_NEG_010` | New needed fixture: negative/pre-roll layer-local time to test trunc-toward-zero vs floor. |
| `TMP_MULTI_010` | New needed fixture: multiple Posterize Time effects in one stack. |
| `STK_030` | Regression only after isolated temporal bucket telemetry says lower-stack bucket and downstream effect param time are correct. |

Validation metrics should report source frame id/bucket time separately from final RGBA diff. For `STK_030`, final pixel tuning remains blocked until Geometry2/Minimax/Turbulent and adjustment scheduling are isolated.

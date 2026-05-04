# AE Reverse: Core Alpha / Composite / Sampling

Status date: 2026-05-04

Agent: A, Core Composite / Alpha / Sampling Substrate.

## Scope

Ghidra-first reverse checkpoint for:

- normal composite and blend-mode dispatch;
- opacity / alpha gain;
- preserve-alpha and hidden RGB under alpha zero;
- straight/premult boundaries;
- sampler ownership / pixel-center convention where visible in core binaries.

Validation fixtures referenced for this module: `PRI_010`, `CMP_010`, `EFF_040`.

## Inputs

Primary binaries copied from AE 2026 node:

```text
36ea8222bc1e3abae148771cbea03eb60cd4bbcc708e145af5dd9451782d2466  target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll
0fdcc6d09a065c13880483736ccdb1b6b9efedc161004a6b3350dbf04913b323  target/reverse/ae_2026/core_composite_alpha/RendererCPU.dll
26adb49739349d41b0a6a8f2f711ea0782844410e9c5a71d0ccc65edd13cab31  target/reverse/ae_2026/core_composite_alpha/AfterFXLib.dll
383e45e13ecdcc017b100188db121454b4a74491f99f51a9bb95b4312e43a342  target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Blend.aex
c81b6c635bbe8958ace6b22356b86b30b2b28d382c1e6c6fab292865253bce67  target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/SolidComposite.aex
8f6151614e75a412064cd2b32549790f57c5e404a47ffa39d440fa17d5e1863c  target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Unmult.aex
150dcd06d54dd84fb28e3bc477ad323efbca7fc44b392381f3fb478e319fd6d4  target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Unmultiply.aex
```

Hash file: `target/reverse/agent_core_alpha/sha256.txt`.

## What Ran

Ghidra install:

```text
/Applications/ghidra_12.0_PUBLIC/support/analyzeHeadless
```

Raw helper script:

```text
target/reverse/agent_core_alpha/AgentCoreAlphaDump.java
target/reverse/agent_core_alpha/AgentCoreAlphaCompositeMap.java
target/reverse/agent_core_alpha/AgentCoreAlphaAexParamDump.java
target/reverse/agent_core_alpha/AgentCoreAlphaCompositeCallers.java
target/reverse/agent_core_alpha/AgentCoreAlphaInsnWindow.java
```

Runs:

```text
target/reverse/agent_core_alpha/20260504_211305/
  Broad GPUFoundation import; interrupted because full auto-analysis was too slow under concurrent agent load.

target/reverse/agent_core_alpha/20260504_211758_bounded/
  GPUFoundation.dll bounded pass succeeded.
  RendererCPU.dll import started, but checkpoint request arrived before useful post-script output.

target/reverse/agent_core_alpha/*_composite_map/
  Focused no-analysis rerun against the saved GPUFoundation project.
  Resolved Composite@GF selected decompile lines and kernel string VAs.

target/reverse/agent_core_alpha/*_aex_params/
  Long AEX wrapper pass for Blend/SolidComposite/Unmult/Unmultiply.
  Completed; current concrete output:
  target/reverse/agent_core_alpha/20260504_212758_aex_params/
```

Supplemental targeted triage, used only to orient the next Ghidra pass:

```text
target/reverse/agent_core_alpha/strings_triage.txt
target/reverse/agent_core_alpha/aex_strings_triage.txt
target/reverse/agent_core_alpha/objdump_gpu_exports_triage.txt
```

## Blockers

- Full `GPUFoundation.dll` auto-analysis is slow when several agents run Ghidra simultaneously. Bounded analysis plus no-analysis reruns against saved projects produced usable symbols/decompile.
- No PDB was found for `GPUFoundation.dll`, so structure fields are inferred from exported C++ symbols, RVAs, offsets, and decompiler output.
- AEX Ghidra imports for `Blend.aex`, `SolidComposite.aex`, `Unmult.aex`, and `Unmultiply.aex` completed. UI labels/match names and several render/setup command paths are xref-confirmed, but PF param defaults/units are still only partially recovered.
- The shader sources / AE probes / PNG conformance are treated as validation only in this report, not as primary proof.

## Progress Chunk: 2026-05-04 21:40

What ran:

```text
target/reverse/agent_core_alpha/AgentCoreAlphaCompositeMap.java
target/reverse/agent_core_alpha/AgentCoreAlphaAexParamDump.java

target/reverse/agent_core_alpha/20260504_212557_composite_map/composite_map.dump.txt
target/reverse/agent_core_alpha/20260504_212758_aex_params/
  Blend_aex.dump.txt
  SolidComposite_aex.dump.txt
  Unmult_aex.dump.txt
  Unmultiply_aex.dump.txt
```

Concrete findings in this chunk:

- `Composite@GF` at `0x18002af50` is now opened and its `IR_BlendMode` switch is resolved to kernel names.
- Normal blend mode is enum `0x12`; if normal opacity is approximately zero, `Composite@GF` copies the destination/lower image through `FastDeviceMemcpy2D` instead of dispatching the normal kernel.
- Otherwise opacity is not pre-applied outside the kernel; `param_11` is copied into the kernel arg block as `fStack_54`.
- The trailing bool-like args are copied as `param_13 -> uStack_50` and `param_14 -> uStack_4f`; these are real kernel flags, but exact semantic names still require caller/callee inspection.
- AEX wrappers confirm match names and render/setup command routing for `Blend`, `SolidComposite`, `Unmult`, and `Unmultiply`; `Unmult` has a GPU path through `GetGPUIVFFromPFEffectWorld`, `AEFX_Unmult`, and `UnmultKernel`.

Confidence:

```text
Composite enum/kernel table: high.
Opacity API order: high at the Composite@GF boundary, medium for exact in-kernel formula.
AEX UI label mapping: high for labels/match names, medium for parameter indices by LStr order.
Preserve-alpha / premult flag naming: low until Composite callers or kernel args are resolved.
```

## Progress Chunk: 2026-05-04 21:47

What ran:

```text
target/reverse/agent_core_alpha/AgentCoreAlphaCompositeCallers.java
target/reverse/agent_core_alpha/AgentCoreAlphaInsnWindow.java

target/reverse/agent_core_alpha/20260504_214123_composite_callers/composite_callers.dump.txt
target/reverse/agent_core_alpha/20260504_214212_insn_windows/insn_windows.dump.txt
```

New findings:

- `_DAT_18019551c` is `8.0e-06f`; this is the epsilon used in alpha/opacity comparisons.
- `_DAT_180194fe4` is `1.0f`; `AlphaGain` uses `gain + eps < 1.0f` to choose kernel vs memcpy.
- Direct code xrefs to `Composite@GF`, `AlphaGain@GF`, and `Unpremultiply@GF` inside `GPUFoundation.dll` mostly come from export/data tables; the only direct code caller found for `Composite@GF` and `AlphaGain@GF` is `Motion@GF` at `0x18003eac0`.
- In the `Motion@GF -> Composite@GF` instruction window, stack args are built as:

```text
Composite arg 11 / opacity      <- XMM15
Composite arg 12 / IR_BlendMode <- [RBP + 0x0c]
Composite arg 13 / bool flag 1  <- byte [RSP + 0x9b0]
Composite arg 14 / bool flag 2  <- constant false
```

- The same `Motion@GF` path has an optimization branch: when the separate composite path is not used and blend mode is normal (`0x12`), it calls `AlphaGain@GF`; otherwise it calls another blend/transfer helper at `0x180090440`.

Interpretation:

```text
Opacity order is now better bounded:
  - normal/no-composite optimized path: opacity can be applied as AlphaGain before output/copy;
  - composite path: opacity is passed into Composite kernel args.

Preserve-alpha naming is still not locked:
  - Composite flag 2 is false in the observed Motion path;
  - Composite flag 1 is forwarded from Motion's second bool parameter;
  - no AEX UI property in this scope directly names either flag.
```

## Concrete Findings

### 1. Core alpha/composite APIs are exported from `GPUFoundation.dll`

Evidence: Ghidra/export table, confirmed.

Important functions:

```text
0x18002af50  ?Composite@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXH1HPEAXHUPixelFormat@dvamediatypes@@HHMW4IR_BlendMode@@_N5@Z
0x180022570  ?AlphaGain@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXHPEAXHUPixelFormat@dvamediatypes@@HHM@Z
0x180022cc0  ?PackedAlphaGain@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXHPEAXHUPixelFormat@dvamediatypes@@HHM@Z
0x180022fc0  ?Unpremultiply@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXHPEAXHUPixelFormat@dvamediatypes@@HHAEBUfloat4@@@Z
0x18002c620  ?BlendUnpackedAlpha@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXPEAXUPixelFormat@dvamediatypes@@HHHH_N@Z
0x18001ed80  ??0TransferDescriptor@GF@@QEAA@W4TransferMode@1@W4MaskFlags@1@HM_NPEBULinearBlendingTables@1@@Z
0x18001e3e0  ?Transfer@GF@@YAHAEBV?$shared_ptr@VDevice@GF@@@std@@PEBXHHH1HHHPEAXHHHAEBV?$PointT@H@geom@dvacore@@3UPixelFormat@dvamediatypes@@AEBVTransferDescriptor@1@@Z
```

Implication: native composite/effect code should be aligned against these
core functions first, then against plugin wrappers.

### 2. `AlphaGain` has explicit zero/one fast paths

Evidence: Ghidra decompile of `AlphaGain@GF` and `PackedAlphaGain@GF`, confirmed for control flow.

Observed behavior:

- width/height <= 0 returns success/no-op;
- gain approximately zero routes to `FillWithTransparentBlack`;
- gain approximately one routes to `FastDeviceMemcpy2D`;
- intermediate gain loads and dispatches an alpha-gain kernel.

Confidence:

- Control-flow and fast paths: confirmed.
- Exact epsilon constants: confirmed as `8.0e-06f` and `1.0f`.
- Channel semantics: likely from function split: `AlphaGain` vs `PackedAlphaGain`; exact per-channel math still needs kernel call target inspection in Ghidra.

Native implication:

```text
AlphaGain(gain <= eps) should produce transparent black, not preserve hidden RGB.
AlphaGain(gain ~= 1) should memcpy source unchanged.
PackedAlphaGain and AlphaGain must stay separate; do not collapse them into one helper.
```

### 3. `Composite@GF` is a blend-mode dispatcher keyed by `IR_BlendMode`

Evidence: Ghidra decompile of `Composite@GF` at `0x18002af50`, confirmed for dispatch shape.

Observed behavior:

- early no-op when width/height <= 0;
- one special branch for `param_12 == 0x12`;
- otherwise a switch over `param_12`;
- each switch arm loads a different kernel through `LoadKernel`;
- global per-device kernel caches are indexed by device/backend state.

Resolved `IR_BlendMode` kernel mapping from focused Ghidra rerun:

```text
0x00 -> BlendMode_kBlendMode_Color_Kernel
0x01 -> BlendMode_kBlendMode_ColorBurn_Kernel
0x02 -> BlendMode_kBlendMode_ColorDodge_Kernel
0x03 -> BlendMode_kBlendMode_Darken_Kernel
0x04 -> BlendMode_kBlendMode_DarkerColor_Kernel
0x05 -> BlendMode_kBlendMode_Difference_Kernel
0x06 -> BlendMode_kBlendMode_Dissolve_Kernel
0x07 -> BlendMode_kBlendMode_Exclusion_Kernel
0x08 -> BlendMode_kBlendMode_HardLight_Kernel
0x09 -> BlendMode_kBlendMode_HardMix_Kernel
0x0a -> BlendMode_kBlendMode_Hue_Kernel
0x0b -> BlendMode_kBlendMode_Lighten_Kernel
0x0c -> BlendMode_kBlendMode_LighterColor_Kernel
0x0d -> BlendMode_kBlendMode_LinearBurn_Kernel
0x0e -> BlendMode_kBlendMode_LinearDodgeAdd_Kernel
0x0f -> BlendMode_kBlendMode_LinearLight_Kernel
0x10 -> BlendMode_kBlendMode_Luminosity_Kernel
0x11 -> BlendMode_kBlendMode_Multiply_Kernel
0x12 -> BlendMode_kBlendMode_Normal_Kernel
0x13 -> BlendMode_kBlendMode_Overlay_Kernel
0x14 -> BlendMode_kBlendMode_PinLight_Kernel
0x15 -> BlendMode_kBlendMode_Saturation_Kernel
0x16 -> BlendMode_kBlendMode_Screen_Kernel
0x17 -> BlendMode_kBlendMode_SoftLight_Kernel
0x18 -> BlendMode_kBlendMode_VividLight_Kernel
0x19 -> BlendMode_kBlendMode_Subtract_Kernel
0x1a -> BlendMode_kBlendMode_Divide_Kernel
0x1b -> BlendMode_kMaskBlendMode_Add_Kernel
0x1c -> BlendMode_kMaskBlendMode_Subtract_Kernel
0x1d -> BlendMode_kMaskBlendMode_Intersect_Kernel
```

The module/library string for these loads is `Composite`; each switch arm passes
that library plus the mode-specific kernel string into `LoadKernel`.

Important normal/opacity branch:

```text
if IR_BlendMode == 0x12 and opacity <= epsilon:
  FastDeviceMemcpy2D(src/dst arguments)
else:
  dispatch BlendMode_kBlendMode_Normal_Kernel
```

The decompile names the float as `param_11` and `IR_BlendMode` as `param_12`.
Two trailing bool-like parameters (`param_13`, `param_14`) are copied into the
kernel argument block immediately before dispatch; these are strong candidates
for linear/preserve-alpha/premult policy flags, but exact meanings are not yet
confirmed.

Current confidence:

- `param_12` is the blend-mode enum: confirmed by exported signature `W4IR_BlendMode`.
- enum-to-kernel mapping: confirmed.
- enum-to-AE UI label mapping: likely for standard blend labels, still needs wrapper/callsite confirmation.
- exact normal-composite formula: not yet recovered from CPU decompile; must inspect selected kernel target or CPU fallback path.

Next exact target:

```text
Function: 0x18002af50 ?Composite@GF...
Resolve meaning of param_13 and param_14.
Inspect higher-level caller naming for Motion bool parameters.
Inspect CPU fallback or kernel source equivalent for BlendMode_kBlendMode_Normal_Kernel.
```

### 4. `TransferDescriptor` contains opacity and blend-table state

Evidence: Ghidra decompile, confirmed.

Observed layout:

```text
TransferDescriptor +0x00  transfer mode
TransferDescriptor +0x04  mask flags
TransferDescriptor +0x08  unknown int / mode data
TransferDescriptor +0x0c  opacity float returned by GetOpacity()
TransferDescriptor +0x10  bool-like flag
TransferDescriptor +0x18  LinearBlendingTables* returned by GetBlendingTables()
```

The constructor writes these fields and calls `TransferDescriptor::Calculate`.

Confidence:

- offsets for opacity and blend table pointer: confirmed.
- meaning of all constructor args: likely but not locked.
- `TransferDescriptor::Calculate` internals: unknown, next target.

Native implication: opacity should be treated as part of transfer/composite descriptor state, not only as a layer-side scalar.

### 5. Plugin wrapper parameter labels and wrapper paths are Ghidra-mapped

Evidence: AEX strings, xrefs, and selected wrapper decompile. Labels/match names are confirmed; defaults/units remain partial.

Observed labels/match names:

```text
Blend.aex:
  matchName ADBE_Blend
  LStr params:
    0001 Blend With Layer
    0002 Mode
    0003 Crossfade | Color Only | Tint Only | Darken Only | Lighten Only
    0004 Blend With Original
    0005 If Layer Sizes Differ
    0006 Center | Stretch to Fit
  Render/setup evidence:
    FUN_180001960 reads param slots at +0x38 and uses PF suites for layer/world copy/composite.
    FUN_180001490 / FUN_180001580 xref PF Color16 Suite.

SolidComposite.aex:
  matchName ADBE_Solid_Composite
  LStr params:
    0001 Normal | Add | Multiply | Screen | Overlay | Soft Light | Hard Light | Color Dodge | Color Burn | Darken | Lighten | Difference | Exclusion | Hue | Saturation | Color | Luminosity
    0002 Source Opacity
    0003 Color
    0004 Opacity
    0005 Blending Mode
  Command routing:
    FilterMain case 4  -> FUN_180001240 param setup
    FilterMain case 0xb -> FUN_180001d20 render
    FilterMain case 0x18 -> FUN_180002500 GPU/smart path
  PF ColorParamSuite helpers: FUN_180004640 / FUN_1800046f0.

Unmult.aex:
  matchName ADBE_Unmult
  LStr params:
    0001 Background Color
    0002 Black | White
    0003 Black Level
    0004 White Level
    0005 Softness
    0006 Remove Color Matting
    0007 Clip HDR Results
  GPU evidence:
    EffectMainExtra case 4 -> FUN_180003fe0 param setup
    EffectMainExtra case 0xb -> render path
    EffectMainExtra case 0x18 -> FUN_1800056c0 GPU/smart path
    FUN_1800049e0 calls VF::GetGPUIVFFromPFEffectWorld for input/output worlds.
    AEFX_Unmult and UnmultKernel strings are xref-confirmed.

Unmultiply.aex:
  matchName ADBE_Remove_Color_Matting
  LStr params:
    0001 Background Color
    0002 Clipping
    0003 Clip HDR Results
  Command routing:
    EffectMainExtra case 4 -> FUN_1800012c0 param setup
    EffectMainExtra case 0xb -> PF world copy path
    EffectMainExtra case 0x18 -> PF ColorParamSuite + PF Fill Matte Suite path
```

Confidence:

- UI labels/match names: confirmed by binary strings.
- wrapper command routing: confirmed where listed.
- property indices: medium by `LStr` order; exact PF IDs/defaults/units still need deeper setup-function reconstruction.

## Required Questions

### Parameter Mapping

Current answer: partially locked.

- Core `Composite@GF` uses `IR_BlendMode` in `param_12`; enum values `0x00..0x1d` are mapped to kernel names above.
- Core `Composite@GF` takes opacity as `param_11`, copied into kernel arg `fStack_54`.
- Core `Composite@GF` copies bool flags as `param_13 -> uStack_50` and `param_14 -> uStack_4f`.
- In the observed `Motion@GF -> Composite@GF` call, `param_13` comes from Motion's second bool parameter and `param_14` is `false`.
- `TransferDescriptor` stores opacity at `+0x0c`.
- `SolidComposite.aex`, `Blend.aex`, `Unmult.aex`, `Unmultiply.aex` labels/match names and wrapper command routing are identified above.

Next exact step: decompile/retype AEX param setup functions to recover PF param IDs, defaults, slider ranges, and units rather than relying on `LStr` order.

### Time Semantics

Current answer: no time-dependent behavior found in this Agent A checkpoint.

- `Composite@GF`, `AlphaGain@GF`, `PackedAlphaGain@GF`, and `Unpremultiply@GF` signatures do not expose comp/layer/source time.
- Time semantics are likely owned above this substrate or in motion/transform paths.

Next exact step: inspect call sites from `AfterFXLib.dll` / plugin wrappers into `Composite@GF` to see whether animated opacity is pre-evaluated before the core call.

### Sampling Rules

Current answer: unknown / not locked.

Known from Ghidra symbols:

```text
0x1800725b0  TransformOperation(... USampleQuality ...)
0x180074c40  TransformOperation::Quality()
0x180076ed0  TransformWithMotionBlur(...)
```

Agent A did not yet recover pixel-center, OOB, or sampler ownership rules from Ghidra. `TransformOperation::Quality()` returns the first struct field, which confirms quality is stored on the operation, but not what each quality enum means.

Next exact step: focused pass on `TextureAffinePlusCompositePlusDecode` / `TransformOperation::Calculate` / `TransformWithMotionBlur`, then map `USampleQuality` enum and sampler kernels.

### Alpha / Premult Policy

Current answer: partial, but stronger than the checkpoint.

- `AlphaGain` zero branch fills transparent black: confirmed.
- `AlphaGain` one branch memcpy: confirmed.
- `AlphaGain` epsilon is `8.0e-06f`; one threshold is `1.0f`.
- `Motion@GF` uses an optimized normal path where normal blend can become `AlphaGain` rather than `Composite@GF`.
- `Composite@GF` receives opacity and the two bool flags as kernel args; it does not globally pre-apply opacity before kernel dispatch except for normal opacity-zero memcpy.
- `Unpremultiply@GF` is a dedicated exported operation taking a `float4` matte/color argument: confirmed.
- `Unmult.aex` confirms a separate GPU unmult path via `AEFX_Unmult` / `UnmultKernel`.
- Blur-related exported structs carry explicit `AlphaType` source/dest fields and alpha-only controls: confirmed but belongs partly to blur agent.
- Hidden RGB under alpha zero in final AE PNGs was previously validated by conformance, but core normal-composite kernel behavior is not yet Ghidra-confirmed.

Native policy implication for now:

```text
Keep transparent source as no-op in normal compositing unless a specific effect
like AlphaGain(gain=0) explicitly produces transparent black.
Do not globally erase RGB when alpha becomes zero; policy is operation-specific.
```

### Color / Numeric Policy

Current answer: partial, with constants recovered.

- Core functions pass `PixelFormat@dvamediatypes` explicitly.
- GPUFoundation contains many `BGRA_4444_32f` / `ARGB` / packed/unpacked conversion paths.
- `AlphaGain` and `Composite` use float opacity/gain parameters.
- Zero/one comparisons are epsilon based: epsilon is `8.0e-06f`, one is `1.0f`.

Next exact step: inspect the normal kernel/fallback math to lock straight-vs-premult arithmetic and rounding/clamp behavior.

## Validation Notes

No new conformance render was run in this checkpoint.

Existing relevant validation from previous Agent A/core-alpha work:

```text
EFF_040 after background/transparent-source alpha policy fix:
  rgba mean_abs_diff: 2.8359 -> 0.0603
  rgb mean_abs_diff:  3.7499 -> 0.0491
  background corner:  native == AE == [5,5,6,0]
```

Interpretation: background/hidden-RGB policy was a real mismatch. Remaining `EFF_040` diff is now small enough that Geometry2/effect formula tuning is meaningful.

Still needed:

```text
PRI_010: verify hidden RGB under alpha zero survives ordinary source-over.
CMP_010: isolate normal composite with zero-alpha source over nonzero hidden RGB destination.
EFF_040: keep as regression case after Geometry2/effect adjustments.
```

## Next Exact Inspections

1. `GPUFoundation.dll`:
   - inspect normal kernel or CPU fallback for `BlendMode_kBlendMode_Normal_Kernel`;
   - resolve `param_13` / `param_14` semantic names from higher-level callers or kernel arg reads.

2. `GPUFoundation.dll`:
   - inspect `Motion@GF` callers outside GPUFoundation, likely `AfterFXLib.dll` / render host;
   - name Motion's two bools from callsite context.

3. `GPUFoundation.dll`:
   - `0x18001e3e0 ?Transfer@GF...`
   - `0x18001ed80 TransferDescriptor ctor`
   - `TransferDescriptor::Calculate`
   - map transfer mode / mask flags / opacity / blend table.

4. AEX wrappers:
   - decompile/retype setup functions for PF param IDs/defaults/ranges/units:
     `Blend.aex`, `SolidComposite.aex`, `Unmult.aex`, `Unmultiply.aex`;
   - keep wrapper labels as mapping evidence, not formula proof.

5. Sampling substrate:
   - `0x1800725b0 TransformOperation ctor`;
   - `0x180074c40 Quality`;
   - `0x180076ed0 TransformWithMotionBlur`;
   - texture affine/composite/decode kernel dispatch table.

## Current Bottom Line

This checkpoint confirms the right Ghidra targets and several concrete policies:

- alpha/composite are centralized in `GPUFoundation.dll`;
- alpha gain has explicit transparent-black and memcpy fast paths;
- composite dispatch is driven by `IR_BlendMode`;
- opacity is stored in transfer descriptor state and passed directly into composite kernels;
- `Motion@GF` can optimize normal blend into `AlphaGain`, so opacity order depends on path;
- AEX labels/wrapper routing are confirmed for the four targeted plugins;
- sampling/pixel-center and exact normal-composite math still require focused target inspection.

Confidence overall: useful intermediate, not parity-locked.

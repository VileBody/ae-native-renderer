# AE Reverse: Turbulent / Text / Expression Ghidra

Status date: 2026-05-04.
Status: interim checkpoint, not final parity claim.

This round is Ghidra-first. Probes, decoded shaders, and conformance metrics
are only validation/corroboration after Ghidra identifies concrete behavior.

## Inputs

Ghidra:

- Headless: `/Applications/ghidra_12.0_PUBLIC/support/analyzeHeadless`
- Project: `target/reverse/ghidra_projects/agent_turbulent_text_expr`
- Raw output dir: `target/reverse/agent_turbulent_text_expr`
- Targeted script:
  `target/reverse/agent_turbulent_text_expr/scripts/DumpAgentTurbulentTextExpr.java`
- Heavy index script:
  `target/reverse/agent_turbulent_text_expr/scripts/DumpAgentHeavyIndex.java`
- Completed small run:
  `target/reverse/agent_turbulent_text_expr/20260504_211238/ghidra_small.stdout`
- Heavy run started:
  `target/reverse/agent_turbulent_text_expr/20260504_211910_heavy/ghidra_heavy.stdout`
- Heavy run checkpoint update: `CoolType.dll` finished analysis/indexing;
  `Scripting.aex` hit the 300s analysis timeout but still produced a useful
  index; `AfterFXLib.dll` import/link phase started after that, then the run
  was stopped to close this checkpoint. `AfterFXLib.dll` is not summarized here.

Input hashes:

| Binary | SHA-256 |
| --- | --- |
| `target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentDisplace.aex` | `f3eedc80b4ebe033a497f6dc50f7822059c3770113016e7a68231b25d578180b` |
| `target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentNoise.aex` | `7acc6f0d84b4d682fd2ad3b818ffecd219e2abb0dee83d8c2ccaed9b564b72ce` |
| `target/reverse/ae_2026/text_expression/AfterFXLib.dll` | `26adb49739349d41b0a6a8f2f711ea0782844410e9c5a71d0ccc65edd13cab31` |
| `target/reverse/ae_2026/text_expression/CoolType.dll` | `00b91d7d96723ed995f3d57f99db2aacaca86df4c9ab70b0200ac734cb696d21` |
| `target/reverse/ae_2026/text_expression/Basic_Text.aex` | `e309414b6812daa65eedf7fa99ed087260dd70f8ef5b8314eeab2174799512cc` |
| `target/reverse/ae_2026/text_expression/Scripting.aex` | `3a7064f8dbad79c572f213da34e2e7d155909556a7c1c055b94b2fdaa4d9cf3b` |
| `target/reverse/ae_2026/text_expression/extendscript.dll` | `2acda4e1feb5d9619b7b531337e66e2b77dba3e7a5dd1b51d924bf0dc9035a0d` |

## Turbulent Displace

Concrete Ghidra evidence:

| Evidence | Address / function | Confidence |
| --- | --- | --- |
| Plugin match/name string `ADBE_Turbulent_Displace` | `180015a70` | confirmed |
| UI/help match string `ADBE Turbulent Displace` | `1800161d8`, xref `FUN_18000f550` | confirmed |
| Source path `...AEFilterTurbulentDisplace\\Src\\TDispMain.cpp` | `180015d20`, xrefs `FUN_18000ad10` | confirmed |
| GPU kernel name `TurbulentDisplaceFrac1DKernel` | `180015d80`, xref `FUN_18000ad10` | confirmed |
| GPU kernel name `TurbulentDisplaceFracAllKernel` | `180015db8`, xref `FUN_18000ad10` | confirmed |
| Render/setup candidate | `FUN_18000ad10` | likely |
| Plugin entry | `FilterMain` at `180001f30` | confirmed |

Parameter mapping, Ghidra-confirmed labels:

| Resource ordinal | UI label / enum |
| --- | --- |
| `LStr/0001` | `Displacement` |
| `LStr/0002` | `Turbulent`, `Bulge`, `Twist`, `Turbulent Smoother`, `Bulge Smoother`, `Twist Smoother`, `Vertical Displacement`, `Horizontal Displacement`, `Cross Displacement` |
| `LStr/0003` | `Amount` |
| `LStr/0004` | `Size` |
| `LStr/0005` | `Offset (Turbulence)` |
| `LStr/0006` | `Complexity` |
| `LStr/0007` | `Evolution` |
| `LStr/0008` | `Evolution Options` |
| `LStr/0010` | `Cycle Evolution` |
| `LStr/0011` | `Cycle (in Revolutions)` |
| `LStr/0012` | `Random Seed` |
| `LStr/0013` | `Pinning` |
| `LStr/0014` | `None`, `Pin All`, `Pin Horizontal`, `Pin Vertical`, side pins, and locked variants |
| `LStr/0016` | `Resize Layer` |
| `LStr/0017` | `Antialiasing for Best Quality` |
| `LStr/0018` | `Low`, `High` |

Important: these `LStr` ordinals are confirmed UI/resource ordinals, not yet
confirmed AE property indices or payload keys. The existing native mapping in
`docs/EFFECTS.md` maps payload-style keys as `0001=displacement`,
`0002=amount`, `0003=size`, `0004=offset`, `0005=complexity`,
`0006=evolution`, `0010=random_seed`, `0012=pinning`, `0013=resize_layer`.
This may be correct for payload import, but it is not proven from Ghidra alone.
Next step must align all three namespaces: payload key, AE property index, and
resource/matchName label.

Time semantics:

- Confirmed params that depend on time/state: `Evolution`, `Cycle Evolution`,
  `Cycle (in Revolutions)`, `Random Seed`.
- Exact time source is not yet recovered. Current confidence: unknown.
- Target: decompile `FUN_18000ad10` around xrefs to `TurbulentDisplaceFrac*`
  and identify how the render setup writes evolution/seed into the GPU kernel
  argument block.

Sampling rules:

- Confirmed that the effect exposes `Antialiasing for Best Quality` with
  `Low|High`.
- Confirmed pinning modes include free, axis, side, and locked variants.
- Kernel names containing `Frac` strongly suggest fractional coordinate
  sampling, but sampler type, pixel-center convention, and edge/OOB behavior
  remain unconfirmed.

Alpha/premult policy:

- No Ghidra evidence yet from `TurbulentDisplace.aex` that answers straight vs
  premult or hidden-RGB behavior.
- Native implication: do not tune Turbulent final pixels until the Agent A
  alpha/composite policy is treated as substrate and the Turbulent field is
  compared through coordinate/UV telemetry.

Color/numeric policy:

- Not recovered yet. The likely math path is GPUFoundation/PPix-backed kernel
  setup from `FUN_18000ad10`, but bit depth, float range, and clamp policy are
  not proven.

Interim formula model:

```text
field = fractal_noise_2d_or_3d(
  position = dst_pixel_position / Size + Offset,
  octaves = Complexity,
  evolution = time_or_param_mapped_evolution,
  seed = Random Seed,
)

vector = displacement_mode_to_dxdy(field, mode)
src_uv = dst_pixel_position - Amount * vector
out = sample_source(src_uv, sampler_quality, pinning_or_resize_policy)
```

Confidence: inferred. Only the parameter/control surface and kernel entry
points are Ghidra-confirmed so far.

## Turbulent Displace Focused Decompile Update

Take-over run: 2026-05-04.

Raw outputs:

- `target/reverse/agent_turbulent_text_expr/20260504_215358_turbulent_focused_full_noanalysis/focused_decompile.txt`
- `target/reverse/agent_turbulent_text_expr/20260504_215436_turbulent_support_full_noanalysis/focused_decompile.txt`

Script/tooling notes:

- `DumpAgentFocusedDecompile.java` was widened from a 950-line cap to a
  2200-line cap, because `FUN_18000ad10` previously cut off before the final
  kernel dispatch branch.
- macOS does not provide `flock` here; focused reruns used an atomic
  `/tmp/ae-native-renderer-ghidra.lockdir` directory lock instead.
- `AgentEHello.java` was updated for Ghidra 12's `RefType` API so headless
  no longer logs a compile error while scanning scripts.

What is now confirmed from `FUN_18000ad10`:

| Area | Finding | Confidence |
| --- | --- | --- |
| Zero amount | PF param index `2` is read before render setup; after fixed-point conversion, zero amount copies source to output and exits. | confirmed |
| Parameter state | Render allocates an `0x8130` state block and a `0x405c` GPU parameter block. The first `0x4000` bytes are a packed 64-row noise/evolution table; scalar params live at `+0x4000..+0x4058`. | confirmed |
| Kernel split | Internal displacement modes `9`, `10`, `11` use `TurbulentDisplaceFrac1DKernel`; all other internal modes use `TurbulentDisplaceFracAllKernel`. | confirmed |
| 1D H/V buffers | Mode `9` builds/binds the horizontal lookup; mode `10` builds/binds the vertical lookup; mode `11` builds/binds both. | confirmed |
| Lookup textures | AE loads `TDHaxisHTexture` and `TDVaxisHTexture` from the Turbulent Displace source unit and binds them through GPUFoundation. | confirmed |
| Dispatch | `FracAll` uses a smaller GPU arg wrapper; `Frac1D` uses a larger wrapper with the extra H/V resources. Both end in a GPUFoundation command dispatch with grid/block dimensions selected by backend/device flags. | confirmed |
| Workgroup sizes | Backend/device flags select `1x1`, `64x1`, or `16x16` workgroups. This is performance plumbing, not renderer math. | confirmed |

What is now confirmed from `FUN_180003e70`:

| PF index | Observed role | Notes |
| --- | --- | --- |
| `1` | Displacement enum | This produces the internal displacement mode used by the kernel split. AE adjusts at least two enum values before storage, so UI enum and internal enum are not guaranteed to be identical. |
| `2` | Amount | Fixed-point scalar. Render exits early when resolved amount is zero. |
| `3` | Size | Fixed-point scalar; participates in coordinate scaling. |
| `4` | Offset/Turbulence point | X/Y fixed-point pair stored into state slots used by coordinate lookup. |
| `5` | Complexity | Split into integer octave count plus fractional remainder. Fractional complexity is later included in the packed table. |
| `6` | Evolution | Fixed-point evolution value; normalized by `90 * 65536`, then split into integer/fractional cycle state. |
| `8` | Cycle toggle / evolution option flag | Used as a boolean gate around cycle wrapping. Exact UI label alignment still needs property dump confirmation. |
| `9`, `10` | Cycle evolution controls | Used with constants `90 * 65536` and a larger default cycle range. Exact `Cycle Evolution` vs `Cycle (in Revolutions)` ordering needs one live AE property dump. |
| `12` | Pinning enum | Drives edge/pinning flags and supports many internal cases, including locked variants beyond the simple UI labels. |
| `14` | Antialias/best-quality control | A quality-like scalar is read and defaulted when render quality is not best. Exact label/index alignment needs confirmation. |

Numeric constants observed in setup:

- fixed-point scale: `1 / 65536`
- amount scalar: `0.01`
- smoother scale factor: approximately `0.35355339`
- evolution normalization: `90 * 65536`
- octave amplitude base/decay: `0.25` and `0.7`
- 1D H/V lookup phase offsets: approximately `7913.17` and `9711.73`

AE-style model to implement next:

```text
state.amount = fixed16(param[2])
if state.amount == 0:
  return copy_source()

state.displacement_mode = internalized_enum(param[1])
state.size = fixed16(param[3])
state.offset = fixed16_point(param[4])
state.complexity_int, state.complexity_frac = split(param[5])
state.evolution_cycle = split_evolution(param[6], cycle params)
state.noise_table = build_64_row_table(evolution_cycle, complexity_frac)

if mode in {9, 10, 11}:
  h_lookup = build_lookup(width + 2, offset.x, size, noise_table) when mode uses H
  v_lookup = build_lookup(height + 2, offset.y, size, noise_table) when mode uses V
  dispatch Frac1D
else:
  dispatch FracAll
```

Important uncertainty:

- The actual kernel body is still inside GPUFoundation-loaded kernel assets, not
  fully visible in the AEX wrapper. The wrapper now gives us the parameter
  contract and branch topology; exact noise basis and sampling math still need
  either kernel extraction or targeted AE field probes.
- UI resource ordinals, AE property indices, and generated payload keys are now
  mostly aligned, but `8/9/10/14` should be verified with a live property dump
  before changing public payload names.
- The existing native `crates/effects/src/turbulent_displace.rs` remains an
  approximate sine/noise model. It should not be formula-tuned against final
  pixels until it is replaced with this two-path AE-style field model or at
  least instrumented with equivalent state/lookup telemetry.

## Turbulent Noise

Concrete Ghidra evidence:

- Match/name string `ADBE_AIF_Perlin_Noise_3D` appears in `TurbulentNoise.aex`
  at `180011c88` / `180011e68`.
- Plugin strings call the effect `Turbulent Noise` and describe turbulent
  pattern generation.
- Parameter/resource strings include `Fractal Type`, `Noise Type`, `Overflow`,
  `Transform`, `Rotation`, `Scale`, `Scale Width`, `Scale Height`,
  `Offset Turbulence`, `Complexity`, `Sub Influence (%)`, `Sub Scaling`,
  `Sub Rotation`, `Sub Offset`, `Evolution`, `Cycle (in Revolutions)`,
  `Random Seed`, `Opacity`, `Blending Mode`, `Turbulence Factor`, and
  `Evolution Detail`.

Implication:

- The shared/noise-family clue is strong: AE exposes this as a Perlin-noise
  3D implementation. For Turbulent Displace, we should test whether its
  displacement field can be reproduced by the same Perlin/fractal basis with
  a different vector mapping.

Confidence: likely for shared basis, confirmed for strings/match name.

## Text / Glyph / Font Metrics

Concrete Ghidra evidence from `Basic_Text.aex`:

| Evidence | Address / function | Confidence |
| --- | --- | --- |
| Basic Text calls `TXT_GetFontServer()` | call observed near `180019?` decompile excerpt, import pointer `18006c2a0`, decorated string `1800985bc` | confirmed |
| Font preferences/font popup state | `basicTextFontPopup`, `mFontDataList`, `Standard Font Family`, `Standard Font Style`, `Font Preferences` | confirmed |
| CoolType text construction interface | `CTNewTextInterface`, `CTNewTextInterfaceV2` | confirmed |
| Text creation procs | `CTNewTextProcV3/V4`, `CTNewTextCPProcV2/V3`, `CTNewTextFontInstanceProcV3/V4`, `CTNewTextFontInstanceCPProcV2/V3` | confirmed |
| Text glyph access | `CTTextGetGlyphsV2`, `CTTextGetTextGlyphs`, `CTTextGetNumGlyphs`, `CTTextReleaseGlyphPointers` | confirmed |
| Font instance metrics | `CTFontInstanceGetGlyphIDProcV2`, `GetGlyphIDs`, `GetWidth`, `GetWidths`, `GetBBox`, `GetBBoxes`, `GetBaselineDeltas` | confirmed |
| Glyph access interface | `CTGlyphAccessInterface`, `CTGlyphAccessGetGlyphIDProc`, translator/features/language-system/access-path procs | confirmed |
| Baseline source | `CTFontInstanceGetBaselineDeltasProc`, `CTFontDictGetATCBaselineShiftProc` | confirmed |

Interpretation:

- AE text metrics are not just raw font file metrics. `Basic_Text.aex` goes
  through `TXT_FontServer` and CoolType interfaces for font resolution,
  glyph IDs, widths, bounding boxes, feature processing, and baseline deltas.
- Native `fontdue` metrics are therefore only an approximation. Parity should
  be tuned against a CoolType-like metric contract: glyph ID, advance/width,
  bbox/bboxes, baseline delta, feature-processed glyph run, and text glyph
  list.

Parameter mapping:

- Ghidra evidence here is interface-level, not AE Text Animator property
  index-level. Exact Range Selector / Animator property index mapping is still
  not recovered from `AfterFXLib.dll`/`Scripting.aex`.

Time semantics:

- No Ghidra evidence yet for text animator time sampling. Expected target is
  AfterFX text animator/evaluator code, not `Basic_Text.aex` font popup code.

Sampling rules:

- Text raster antialiasing is not recovered yet. However, the path is clearly
  CoolType/TXT, not direct native fontdue. The completed `CoolType.dll` index
  confirms GDI/DWrite-era text/font plumbing through imported pointers such as
  `GetTextFaceW`, `GetTextExtentPointI`, `ExtTextOutW`, `GetTextMetricsA`,
  `GetFontData`, `EnumFontFamiliesExW`, and `CreateFontIndirectW`, plus AGM
  text/path/raster interfaces such as `AGMNewPathNewTextPathV4`,
  `AGMSharedBezierPathReallocGlyphs`, `AGMPortSetTextGridSizeProc`,
  `AGMPortTextListClipProc`, and `AGMResourcePortHasTextProc`.

Alpha/premult policy:

- Not recovered in Agent E. Text alpha/raster output should be validated after
  Agent A's hidden-RGB/straight-alpha policy is treated as substrate.

Native implication:

- Before formula tuning `TXT_010..TXT_040`, add telemetry fields that mirror
  the CoolType contract: source codepoint, glyph ID, glyph-run order, advance,
  bbox, baseline delta, line metrics, selector unit index, selector ordered
  rank, selector weight, final glyph matrix, opacity, and blur radius.

## Expression Evaluator

Concrete Ghidra evidence from `extendscript.dll`:

| Evidence | Address / symbol | Confidence |
| --- | --- | --- |
| ExtendScript engine build | `@@@BUILDINFO@@@ ExtendScript 4.5.6 ...` | confirmed |
| Engine description | `Adobe ExtendScript 4.5.6`, `The ExtendScript scripting engine` | confirmed |
| Generic script property lookup | `ScScript::Dispatcher::findProperty`, `ScScript::Dispatcher::hasProperty` | confirmed |
| Runtime error plumbing | `ScScript::RuntimeError`, `ScScript::Engine::setError`, `ScScript::ScriptContainer::errorMessage` | confirmed |
| Generic time-ish property | string `@time` | confirmed, generic ExtendScript |
| Live property manager pointer | `LivePropertyManager` | confirmed |

Current interpretation:

- `extendscript.dll` is the generic scripting engine, not the AE expression
  host by itself.
- `Scripting.aex` is the next host layer: even after a 300s analysis timeout,
  its index shows delay-load ExtendScript integration symbols such as
  `dvascripting::extendscript::AcquireEngine`, `Initialize`, `Terminate`,
  `DisableScripting`, and `IsInitialized`.
- `Scripting.aex` also exposes AE host/time/text hooks via imported or cached
  symbols: `BEE_CompToLayerTime`, `BEE_FpLongToTime`, `BEE_SetExpressionsDebugger`,
  `BEE_AllowDelayedExpression`, `BEE_TextLayer::GetStreamDoc`,
  `BEE_TextLayer::GetTextGrid`, `TDB_SecondsToTime`, `GetTimePalSuite`,
  `BEE_TextDocumentStreamTraits::GetValue`, and `TimeRemap_Allowed`.
- AE expression builtins such as `thisLayer`, `thisComp`, `valueAtTime`,
  `seedRandom`, and `wiggle` were not confirmed yet in the completed outputs.
  They are now more likely to sit behind the `Scripting.aex` / `AfterFXLib.dll`
  host binding layer than in raw `extendscript.dll`.

Parameter/property access:

- Generic dispatcher/property lookup is confirmed in `extendscript.dll`.
- AE-specific property access is still unknown, but `Scripting.aex` confirms
  the correct next target family: BEE/TDB stream access and delayed-expression
  wrappers, not the generic ExtendScript dispatcher alone.

Numeric/vector coercion:

- Not recovered yet.

Time semantics:

- Generic `@time` exists, but expression-time semantics for AE property
  evaluation are not recovered.

Native implication:

- Do not claim Expression Engine v2 parity from `extendscript.dll`. Use it as
  proof that AE hosts a property-dispatching ExtendScript runtime, then reverse
  AE's host bindings in `Scripting.aex` / `AfterFXLib.dll`.

## Blockers

- No PDBs were available. Ghidra reports PDB lookup failures for imported PE
  files.
- External AE dependencies were not linked in the isolated project:
  `PF.DLL`, `TXT.DLL`, `GPUFOUNDATION.DLL`, `DVACORE.DLL`,
  `DVAMEDIATYPES.DLL`, `VIDEOFRAME.DLL`, `SCCORE.DLL`, and platform DLLs.
  This leaves many indirect calls unresolved.
- The first script matched too many generic imported symbols in `Basic_Text.aex`;
  raw output is useful but noisy. Next scripts must start from exact xrefs and
  RVAs.
- Heavy Ghidra import/index completed for `CoolType.dll`. `Scripting.aex`
  found/cached exports for local `AE_DVASCRIPTUI.DLL`, `AFTERFXLIB.DLL`,
  `DVASCRIPTING.DLL`, `DVASCRIPTINGES.DLL`, and `DVASCRIPTINGNAPI.DLL`, then
  hit the 300s analysis timeout but still produced a useful symbol/string
  index. `AfterFXLib.dll` import/linking began and showed dependency edges to
  `DVASCRIPTING.DLL`, `DVATEXTEDITOR.DLL`, `DVATYPEKIT.DLL`,
  `GPUFOUNDATION.DLL`, `TXT.DLL`, `TDB.DLL`, `BEE.DLL`, `PF.DLL`, and many
  renderer/media modules; the run was stopped before `AfterFXLib.dll` analysis
  completed, so formula-level conclusions are still pending.
- Current Ghidra evidence gives control surfaces, parameter packing, and
  Turbulent Displace kernel branch topology. It still does not expose the full
  GPU kernel body for exact Turbulent noise/sample formulas.

## Next Exact Targets

Turbulent Displace:

1. Add native telemetry that mirrors the AE wrapper state:
   internal displacement mode, kernel path, fixed-point amount/size/offset,
   complexity integer/fractional parts, evolution cycle split, H/V lookup
   lengths, and lookup hashes.
2. Replace the current single sine/noise path with an AE-shaped two-path model:
   `FracAll` for general modes and `Frac1D` for vertical/horizontal/cross
   displacement.
3. Verify property index alignment for `8/9/10/14` with a live AE property dump
   before renaming any public payload slots.
4. Validate against `EFF_060` and probe cases by comparing field telemetry
   (`dx`, `dy`, `src_uv`, OOB count, H/V lookup hashes), not final pixels first.
5. Extract or infer the actual kernel noise/sample body only after the wrapper
   state model is implemented.

Turbulent Noise:

1. Use `ADBE_AIF_Perlin_Noise_3D` as the next anchor.
2. Decompile the render/setup function that registers or dispatches the Perlin
   3D kernel.
3. Compare parameter names/order against Turbulent Displace to decide whether
   the same basis can be reused.

Text/Glyph:

1. Re-run a focused Ghidra script against `Basic_Text.aex` and `CoolType.dll`
   for only `CTFontInstanceInterfaceV2`, `CTTextGetGlyphsV2`,
   `CTGlyphAccessInterface`, `TXT_GetFontServer`, and baseline/bbox functions.
2. Decompile the xref functions:
   `FUN_1800192c0`, `FUN_180019520`, `FUN_180019bd0`,
   `FUN_18001e420`, `FUN_18001e630`, `FUN_180022280`,
   `FUN_180022650`, `FUN_180023110`.
3. Produce a glyph-metric contract for native telemetry and tests:
   glyph id, advance/width, bbox, baseline delta, glyph run order, and feature
   processing result.
4. Validate with `TXT_010`, `TXT_020`, `TXT_030`, `TXT_040`, `GPH_010`.

Text Animator / Selector:

1. Search `AfterFXLib.dll` and `Scripting.aex` for `ADBE Text Range`,
   `ADBE Text Selector`, `ADBE Text Expressible Selector`, `Randomize`,
   `Smoothness`, `Wiggly`, `Based On`.
2. Recover property indices/matchNames for range selector, animator properties,
   selector shape, smoothness, randomize order, and expression selector amount.
3. Defer formula tuning until property mapping is confirmed.

Expression:

1. Search heavy Ghidra output and rerun targeted string scans for
   `thisLayer`, `thisComp`, `valueAtTime`, `seedRandom`, `wiggle`,
   `posterizeTime`, `textIndex`, `textTotal`, and `selectorValue`.
2. If strings are absent, inspect AE host binding tables in `Scripting.aex` and
   `AfterFXLib.dll` via exported/demangled symbols and live property dispatch
   xrefs.
3. Recover numeric/vector coercion and time access order before implementing
   Expression Engine v2 beyond named/fingerprint shortcuts.

Validation:

- Turbulent: `EFF_060` plus existing turbulent field probe cases.
- Text: `TXT_010`, `TXT_020`, `TXT_030`, `TXT_040`, `GPH_010`.
- Expression: `EXP_010`, `TXT_040`.
- Metrics to use: RGB/alpha split, foreground-only, glyph rows, selector
  weights, expression samples, Turbulent field vectors, and manual AE/native/diff
  frame review.

# Ghidra-First Reverse Agent Brief

Status date: 2026-05-04

This round is Ghidra-first. AE probes, decoded shader snippets, and conformance
PNGs are validation and measurement tools, not a substitute for reverse
analysis. A useful agent result must answer concrete renderer questions, not
only say "formula recovered".

## Shared Rule

Every module report must answer these questions:

```text
1. Parameter mapping:
   Which payload keys, AE property indices, AE matchNames, units, defaults,
   enum values, and normalization rules feed the formula?

2. Time semantics:
   Which time is used: comp time, layer time, source time, precomp time,
   effect time, expression time, posterized bucket time, motion-blur sample
   time, or adjusted downstream stack time?

3. Sampling rules:
   Which pixel coordinate convention, source-UV mapping, sampler, edge policy,
   clamp/rounding behavior, kernel footprint, and quality enum are used?

4. Alpha/premult policy:
   Is the operation straight or premultiplied at each stage? Does it preserve
   hidden RGB under alpha zero? How are background, alpha gain, masks, mattes,
   preserve-alpha, and effect composites handled?

5. Color/numeric policy:
   Which bit depth/float range, linear/nonlinear branch, gamma/color-space
   branch, saturation/clamp, epsilon, NaN behavior, and integer rounding are
   used?

6. Evidence:
   Which binary/function/RVA/string/table proves the behavior, and what is the
   confidence: confirmed, likely, inferred, or unknown?

7. Validation:
   Which fixture/case demonstrates the behavior, what are before/after metrics,
   and what does manual visual review of AE/native/diff say?
```

Do not finish with a full-template diff. Finish with a small isolated finding
that explains the diff.

## Ghidra Discipline

Use isolated Ghidra projects unless you deliberately need a shared project.

```text
target/reverse/ghidra_projects/<topic>/
target/reverse/<topic>/
```

Take a lock for any shared project import or write:

```sh
flock /tmp/ae-native-renderer-ghidra.lock <ghidra-headless-command>
```

If multiple agents run at once, each agent should use a unique topic dir. Raw
logs stay under `target/reverse/<topic>/`; durable findings go to
`docs/phase_reports/AE_REVERSE_<MODULE>_GHIDRA.md`.

## Critical Hints

### Parameter Mapping: Geometry2 `0003` / `0004` / `0008`

There are two different numeric namespaces that can look the same:

```text
payload positional key: "0008"
AE effect matchName:    "ADBE Geometry2-0008"
```

For Geometry2, the current AE property dump says:

```text
property index 3  -> Uniform Scale      -> ADBE Geometry2-0011
property index 4  -> Scale Height       -> ADBE Geometry2-0003
property index 5  -> Scale Width        -> ADBE Geometry2-0004
property index 8  -> Rotation           -> ADBE Geometry2-0007
property index 9  -> Opacity            -> ADBE Geometry2-0008
```

So:

- payload key `"0003"` can mean property index 3, not necessarily matchName
  suffix `0003`;
- payload key `"0004"` can mean property index 4;
- payload key `"0008"` can mean property index 8 / Rotation;
- `ADBE Geometry2-0008` is Opacity.

Every agent touching effect params must report both:

```text
payload key
AE property index
AE matchName
UI label
default
unit conversion
```

### Time Semantics

Always split time into explicit domains:

```text
comp_time
layer_local_time = comp_time - layer.start
source_time = layer_local_time - source_start, then stretch/remap if present
precomp_time = parent_time translated through precomp layer
effect_time = time passed to effect parameter evaluation
expression_time = time observed by expression APIs
posterize_bucket_time = chosen/frozen time after Posterize Time
motion_sample_time = shutter sample time
adjustment_stack_time = time used for downstream layers under adjustment effects
```

Questions to answer in Ghidra:

- floor vs round vs ceil for frame/bucket selection;
- whether buckets are based on comp fps, source fps, effect fps, or custom fps;
- where Posterize Time sits relative to adjustment-layer lower-stack rendering;
- whether animated effect params are sampled at original time or posterized
  time;
- motion blur sample count, sample times, weights, shutter phase, and matrix
  interpolation path;
- whether source-frame selection preserves fractional UV sampling or snaps to
  frame identity first.

### Sampling Rules

Questions to answer:

- are pixel centers integer `(x, y)`, half-pixel `(x + 0.5, y + 0.5)`, or
  backend-dependent?
- is inverse mapping evaluated per destination pixel or forward splatted?
- bilinear, bicubic, Lanczos, nearest, area, or quality-dependent sampler?
- what happens at out-of-bounds: transparent, clamp, extend, mirror, wrap, or
  effect-specific edge behavior?
- how are kernel radii quantized: floor/ceil/round, odd/even footprint,
  separable passes?
- how are subpixel offsets rounded for telemetry/sample identity?
- does the sampler operate on straight or premultiplied RGB?

Known current fact: Geometry2 isolated conformance uses bilinear transparent
OOB in native. This is not parity-locked; verify Ghidra/AE behavior before
declaring done.

### Alpha / Premult Policy

Questions to answer:

- straight vs premult input/output for each effect stage;
- whether fully transparent source pixels are no-op and preserve destination
  hidden RGB;
- when effects premultiply before filtering and unpremultiply after filtering;
- whether glow/shadow/blur operate on alpha mask, RGB luma, or premultiplied
  RGB;
- alpha gain and opacity order;
- preserve-alpha behavior;
- matte/luma matte ordering;
- comp background alpha and hidden RGB policy;
- whether linear composite branch changes only RGB math or also alpha math.

Known current fact: AE conformance PNGs preserve background RGB under alpha
zero. Native `composite_normal` now keeps transparent source pixels as no-op.

## Agent Work Split

### Agent A: Core Composite / Alpha / Sampling Substrate

Primary binaries:

```text
target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll
target/reverse/ae_2026/core_composite_alpha/RendererCPU.dll
target/reverse/ae_2026/core_composite_alpha/AfterFXLib.dll
target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Blend.aex
target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/SolidComposite.aex
target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Unmult.aex
target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Unmultiply.aex
```

Answer:

- normal composite, opacity, preserve-alpha, alpha gain;
- hidden RGB under alpha zero;
- straight/premult boundaries;
- sampler ownership and pixel-center convention if visible in core code;
- blend mode dispatch and enum mapping.

Validation cases:

```text
PRI_010, CMP_010, EFF_040
```

Report:

```text
docs/phase_reports/AE_REVERSE_CORE_ALPHA_SAMPLING_GHIDRA.md
```

### Agent B: Geometry2 / Layer Transform / Collapse / Motion Matrix

Primary binaries:

```text
target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll
target/reverse/ae_2026/core_composite_alpha/Plug-ins/Effects/Transform.aex
target/reverse/ae_2026/Transform.aex
```

Answer:

- Geometry2 property mapping, especially `"0003"`, `"0004"`, `"0008"` vs
  `ADBE Geometry2-0003/0004/0008`;
- matrix order, pixel aspect, skew axis, rotation sign, anchor convention;
- sampler enum and quality branch;
- Geometry2 opacity/shutter/motion blur slots;
- collapse/deferred-raster matrix propagation if discoverable through core
  transform functions.

Validation cases:

```text
EFF_040, GPH_010, CMP_010
```

Report:

```text
docs/phase_reports/AE_REVERSE_GEOMETRY_COLLAPSE_GHIDRA_ROUND2.md
```

### Agent C: Temporal / Posterize Time / Motion Blur Time Domain

Primary binaries:

```text
target/reverse/ae_2026/effects_temporal_noise_distort/Posterize_Time.aex
target/reverse/ae_2026/effects_temporal_noise_distort/Time_Displace.aex
target/reverse/ae_2026/core_composite_alpha/GPUFoundation.dll
target/reverse/ae_2026/core_composite_alpha/AfterFXLib.dll
```

Answer:

- Posterize Time bucket formula and rounding;
- whether effect params and downstream adjustment stack sample posterized or
  original time;
- source frame selection and source fps interaction;
- motion blur sample time generation if visible in GPUFoundation/core;
- temporal behavior under precomp/adjustment layers.

Validation cases:

```text
TMP_010, TMP_020, TMP_030, STK_030
```

Report:

```text
docs/phase_reports/AE_REVERSE_TEMPORAL_POSTERIZE_MOTION_GHIDRA.md
```

### Agent D: Blur / Glow / Shadow / Minimax Effect Math

Primary binaries:

```text
target/reverse/ae_2026/effects_blur_glow_shadow/Box_Blur.aex
target/reverse/ae_2026/effects_blur_glow_shadow/Gaussian_Blur.aex
target/reverse/ae_2026/effects_blur_glow_shadow/Glow.aex
target/reverse/ae_2026/effects_blur_glow_shadow/Drop_Shadow.aex
target/reverse/ae_2026/effects_temporal_noise_distort/Minimax.aex
```

Answer:

- BoxBlur2/Gaussian kernel radius mapping and edge policy;
- whether blur is premultiplied before filtering;
- Drop Shadow offset/softness/opacity/composite order;
- Glow threshold, radius, intensity, blend, alpha/RGB mask source;
- Minimax neighborhood shape, channel mode, radius rounding, operation enum;
- time-aware parameter sampling for animated effect params.

Validation cases:

```text
EFF_010, EFF_020, EFF_030, EFF_050, EFF_070, STK_010, STK_020
```

Report:

```text
docs/phase_reports/AE_REVERSE_BLUR_GLOW_SHADOW_MINIMAX_GHIDRA.md
```

### Agent E: Turbulent / Text / Expression Evaluator

Primary binaries:

```text
target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentDisplace.aex
target/reverse/ae_2026/effects_temporal_noise_distort/TurbulentNoise.aex
target/reverse/ae_2026/text_expression/AfterFXLib.dll
target/reverse/ae_2026/text_expression/CoolType.dll
target/reverse/ae_2026/text_expression/Basic_Text.aex
target/reverse/ae_2026/text_expression/Scripting.aex
target/reverse/ae_2026/text_expression/extendscript.dll
```

Answer:

- Turbulent Displace noise basis, seed/evolution, displacement field units,
  vector direction, edge and sampler behavior;
- glyph metric source, text rasterization antialiasing, baseline/line metrics,
  Point/Montserrat font path if visible;
- text animator selector order, range shape, smoothness, randomize order,
  per-glyph transform composition;
- expression evaluator property access, numeric/vector coercion, time access,
  seeded/random/wiggle/bounce subset entrypoints.

Validation cases:

```text
EFF_060, TXT_010, TXT_020, TXT_030, TXT_040, EXP_010, GPH_010
```

Report:

```text
docs/phase_reports/AE_REVERSE_TURBULENT_TEXT_EXPRESSION_GHIDRA.md
```

## Minimum Final Report Shape

Each agent final message and Markdown report must include:

```text
1. Inputs: binaries, SHA-256, Ghidra project path, scripts/log paths.
2. Findings: exact parameter/time/sampling/alpha answers.
3. Formula: pseudocode or equation in our own words.
4. Evidence: symbol/function/RVA/string/table and confidence.
5. Native implication: files/functions that should change.
6. Validation: cases to run, metrics to compare, visual-review notes.
7. Remaining unknowns: concrete next probe/Ghidra question.
```

No raw decompiler dumps in docs. No Adobe binaries in tracked paths.

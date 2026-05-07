# M11 ImageRenderer Recursive Gaussian

Date: 2026-05-07.

## Scope

This pass replaces the native Glow finite Gaussian approximation with the
ImageRenderer recursive Gaussian subset recovered from Ghidra, and fixes the
Glow Operation numeric mapping used before `IR_CompositeWithBlendMode`.

## Evidence

- Frida/dynamic evidence from the earlier Glow radius pass already locked the
  route `IR_GaussianBlur(radius * 0.4)`.
- `target/reverse/predecoded/20260507_161757_imagerenderer_gaussian_helpers`
  exposes the scalar helper `FUN_18009eb40`.
- `target/reverse/predecoded/20260505_153013_blocker_modules_round2/imagerenderer_gaussian_composite/01_IR_GaussianBlur_impl_18009fca0/decompile.c`
  exposes the coefficient constants and setup.
- `target/reverse/predecoded/20260507_163031_glow_tables` plus Glow.aex strings
  expose `Glow Operation` values:
  `1=None`, `2=Normal`, `3=Add`, `6=Screen`.

## Recovered Blur Shape

The scalar ImageRenderer helper is a two-pass recursive filter over RGBA float
lines:

```text
forward[i] =
  c0 * x[i] + c1 * x[i - 1]
  - fb1 * forward[i - 1]
  - fb2 * forward[i - 2]

reverse[i] =
  r1 * x[i + 1] + r2 * x[i + 2]
  - fb1 * reverse[i + 1]
  - fb2 * reverse[i + 2]

out[i] = forward[i] + reverse[i]
```

The line runners zero-pad outside the active line. Native now applies this
filter horizontally and vertically for Glow, then returns to RGBA8 at the effect
boundary.

## Native Changes

- `crates/effects/src/glow.rs`
  - added ImageRenderer coefficient constants;
  - replaced finite separable Gaussian with recursive causal/anti-causal passes;
  - added `GlowOperation::Normal`;
  - fixed `0006` mapping to match Glow.aex operation table;
  - added unit coverage for the recovered operation values.

## Validation

Focused conformance:

```text
target/conformance_tmp/glow_shadow_ir_recursive_composite_table_20260507

EFF_020 primary visible: 1.657241 -> 1.628605
EFF_070 primary visible: 0.342495 -> 0.352847
EFF_010 primary visible: unchanged 0.115046
STK_010 primary visible: unchanged 0.069822
```

Manual sheets:

```text
target/visual_review/glow_shadow_ir_recursive_composite_table_20260507
```

Master gate:

```text
target/ae_agents/p2_glow_ir_recursive_composite_table_gate_20260507
case_count=19
accepted=2
approximate=17
regression=0
missing=0
```

## Remaining M11 Work

This closes the missing ImageRenderer Gaussian shape for the Add/default path.
Remaining work is narrower:

- effect-local blend/premult details inside `IR_CompositeWithBlendMode`;
- unsupported Glow Operation modes beyond none/normal/add/screen;
- Glow Colors / color loop controls if the target templates use them;
- possible final quantization differences if future isolated probes identify
  blur conversion as the owner.

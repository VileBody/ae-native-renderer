# Alpha / Composite / Background Findings

Date: 2026-05-04

## Scope

This note records the alpha/composite investigation from the AE 2026 node
`85.239.48.31`. It focuses on normal compositing, transparent comp background
behavior, and where to continue reverse engineering if alpha/matte parity still
blocks formula tuning.

## Inputs Collected From AE Node

Primary collection:

- `target/reverse/ae_2026/core_composite_alpha/`
- Manifest: `target/reverse/ae_2026/core_composite_alpha/core_composite_alpha_20260504.zip`
- Collection note: `docs/reverse_engineering/collected_core_composite_alpha_binaries.md`

Important files for alpha/composite work:

- `GPUFoundation.dll`
- `AfterFXLib.dll`
- `RendererCPU.dll`
- `RendererGPU.dll`
- `PTX/CL/Composite.clz`
- `PTX/CL/AEFX_Matte.clz`
- `PTX/CL/AlphaGain.clz`
- `PTX/CL/AlphaImageOperations.clz`
- `PTX/CL/CopyAlpha.clz`
- `PTX/CL/TrackMatte.clz`
- `PTX/CL/AEFX_Unmult.clz`
- `PTX/CL/FastBoxBlurPremultiply.clz`

The `.clz` files are compressed shader payloads. For this round, inspecting
the decoded shader kernels gave a faster and clearer path than decompiling the
DLLs in Ghidra.

Decoded local scratch:

- `target/reverse/ae_2026/core_composite_alpha/PTX/CL_decoded/Composite.cl`
- `target/reverse/ae_2026/core_composite_alpha/PTX/CL_decoded/AEFX_Matte.cl`
- `target/reverse/ae_2026/core_composite_alpha/PTX/CL_decoded/AlphaGain.cl`

Decode observation:

- `.clz` payloads are zip/deflate containers.
- The source member inside is XOR-obfuscated with byte `0x2B`.

## Normal Composite Formula

The decoded `Composite.cl` / `AEFX_Matte.cl` normal composite path matches the
standard source-over structure:

```text
upper_alpha = saturate(upper_alpha * alpha_gain)
lower_alpha = saturate(lower_alpha)

upper_premul_rgb = upper_rgb * upper_alpha
lower_contribution_alpha = lower_alpha * (1 - upper_alpha)
out_alpha = upper_alpha + lower_contribution_alpha
out_premul_rgb = upper_premul_rgb + lower_rgb * lower_contribution_alpha
out_rgb = unpremultiply(out_premul_rgb, out_alpha)
```

For the non-linear straight RGBA path, this agrees with our
`raster_cpu::composite_normal` math. The core source-over equation was not the
main mismatch.

## Actual Mismatch Found

AE conformance PNGs preserve comp background RGB under zero alpha. Example from
`EFF_040`:

```text
AE background corner: [5, 5, 6, 0]
old native corner:    [0, 0, 0, 0] or [5, 5, 6, 255] depending stage
fixed native corner:  [5, 5, 6, 0]
```

Two issues were involved:

1. The conformance recipe used AE-ish RGB but opaque alpha for the comp
   background.
2. Normal compositing let a fully transparent source pixel overwrite the
   destination's hidden RGB when both alphas were zero.

For AE-style conformance, a fully transparent source contribution should be a
no-op for the destination pixel, including RGB hidden under alpha zero.

## Code Changes

- `crates/render-cli/src/conformance_pack.rs`
  - `ae_background_rgba8()` now returns `[5, 5, 6, 0]`.
- `crates/raster-cpu/src/composite.rs`
  - `composite_normal()` now skips pixels whose effective source alpha is zero.
  - Added tests that preserve destination RGB under alpha zero and at zero layer
    opacity.

## Verification

Docker toolchain command:

```sh
docker run --rm --entrypoint /bin/sh -v "$PWD:/work" -w /work ae-native-renderer:round2-dev -c 'cargo test -p raster-cpu && cargo test -p render-cli conformance_pack && cargo run -p render-cli -- conformance-pack --out target/ae_conformance_alpha_composite_noop --case EFF_040'
```

Results:

- `cargo test -p raster-cpu`: passed, 3 tests.
- `cargo test -p render-cli conformance_pack`: passed, 3 tests.
- `EFF_040` native conformance: `ok=true`.

Metric improvement for `EFF_040` after the transparent-source no-op fix:

```text
rgba mean_abs_diff: 2.8358659744 -> 0.0602922440
rgb mean_abs_diff:  3.7499033610 -> 0.0491383870
background corner:  native now matches AE [5, 5, 6, 0]
```

Remaining visible diff is now real Geometry2/effect formula tuning, not a
background alpha accounting artifact.

## Next Reverse Targets

If alpha/matte/composite blocks future modules, inspect in this order:

1. `Composite.cl` / `AEFX_Matte.cl`
   - Blend modes, preserve-alpha behavior, linear composite branches.
2. `AlphaGain.cl`
   - Alpha gain, packed alpha gain, unpremultiply path.
3. `TrackMatte.cl`
   - Track matte/luma/alpha matte behavior.
4. `FastBoxBlurPremultiply.cl`
   - Premultiply handling around blur kernels.
5. `RendererCPU.dll` / `AfterFXLib.dll` in Ghidra
   - Use only if the shader kernels and AE probes disagree.

For parallel Ghidra sessions, use a lock around shared project writes. A simple
path is:

```text
target/reverse/ghidra/.ghidra-write.lock
```

Read-only imports can run in parallel if they use separate Ghidra project
directories.

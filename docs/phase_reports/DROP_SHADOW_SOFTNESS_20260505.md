# Drop Shadow Softness Pass

Date: 2026-05-05

## Decision

For the current 8bpc AE conformance render path, Drop Shadow softness is implemented as:

```text
flt_blur_input = softness / 2
radius = ceil(flt_blur_input / 2.71)
iterations = 1
blur target = alpha channel only
```

This replaces the older static-branch assumption `ceil(softness * 1.4 / 2.71)` with the active CPU path observed on the AE85 no-GPU node.

## Evidence

- Dynamic trace for `SHBL_SOFT_018` hit the active CPU chain:
  - `Drop_Shadow.aex+0x73a0` smart render wrapper
  - `Drop_Shadow.aex+0x1930` CPU branch
  - `Drop_Shadow.aex+0xc860` FLT Blur Suite thunk
  - `GPUFoundation.dll!GF::BoxBlur_1DImgOpInfo` constructor twice
- At `0xc860`, the stack carried `9.0` for AE softness `18`, so Drop Shadow halves softness before FLT blur.
- The two `GF::BoxBlur_1DImgOpInfo` constructor outputs both wrote:
  - `radius_float_20 = 3.321033239364624`
  - `rounded_radius_24 = 4`
  - `flags_or_alpha_28 = 0x61`
- `3.321033239364624 ~= (18 / 2) / 2.71`.
- Follow-up uncached CPU sweep confirmed the same mapping:

```text
case             flt_blur_input   radius_float          rounded
SHBL_SOFT_008    4.0              1.4760147333145142    2
SHBL_SOFT_018    9.0              3.321033239364624     4
SHBL_SOFT_032    16.0             5.904058933258057     6
```

## Rejected Branch For This Fixture

The older static branch:

```text
radius = ceil(softness * 1.4 / 2.71)
```

exists in `Drop_Shadow.aex+0x5bb0`, but it was not the active no-GPU CPU path for this fixture. It should stay documented as an alternate branch candidate, not as the accepted production formula.

## Dynamic Trace Note

S3-backed Frida runner:

```text
scripts/ae_trace_drop_shadow_softness.py
```

The runner can upload the tracer via S3, start it on the 85 node through WinRM, render selected AE cases, and pull JSONL logs back through S3. The successful pass used project reset/cache purge in JSX; without that, AE can serve cached frames and hide the effect math.

The successful uncached CPU trace is:

```text
target/dynamic_tools_85/drop_shadow_cpu_xmm_soft18_uncached_20260505/SHBL_SOFT_018.jsonl
target/dynamic_tools_85/drop_shadow_cpu_xmm_softness_sweep_20260505/SHBL_SOFT_008.jsonl
target/dynamic_tools_85/drop_shadow_cpu_xmm_softness_sweep_20260505/SHBL_SOFT_032.jsonl
```

Future 85-debug passes should include screenshots before/after render to catch modal/hung UI state.

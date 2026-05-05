# Ghidra Analysis M13 Minimax Enum Radius - 2026-05-05

## Status

Agent Minimax scope only. Analyzed fresh bundle
`target/reverse/predecoded/20260505_153013_blocker_modules_round2/minimax_aex`
and the local `Minimax.aex` strings. No implementation code was changed.

## Inputs

- Fresh extraction index: `minimax_aex/index.md`
- CPU callbacks:
  - `01_Minimax_callback_16bpc_180004ef0/decompile.c`
  - `02_Minimax_callback_8bpc_1800060b0/decompile.c`
  - `03_Minimax_callback_32bpc_180007270/decompile.c`
- GPU wrapper: `04_Minimax_gpu_path_18000c4b0/decompile.c`
- Comparators: `05/06/07_Minimax_cmp_*`
- Binary strings from `target/reverse/ae_2026/effects_temporal_noise_distort/Minimax.aex`
- Prior probe summary: `docs/phase_reports/M13_HYPOTHESIS_RESULTS_20260505.md`

## Findings

Operation enum `ADBE Minimax-0001`:

- String table exposes label order: `1=Minimum`, `2=Maximum`,
  `3=Minimum Then Maximum`, `4=Maximum Then Minimum`.
- Existing probes accepted `0001=1 minimum`, `0001=2 maximum`; inverted
  `0001=2 minimum` is rejected.
- Fresh callback evidence supports min/max polarity: default comparator stubs at
  `18000d6e0/6f0/700` are `a > b`; selected stubs at
  `18000d710/720/730` are `a < b`. The setup pass that maps UI operations
  `3/4` into one or two callback stages is not in this bundle.

Channel enum `ADBE Minimax-0003`:

- String table exposes label order: `1=Color`, `2=Alpha and Color`, `3=Red`,
  `4=Green`, `5=Blue`, `6=Alpha`.
- Existing probes accepted `0003=1` as RGB/color preserving alpha, and rejected
  `0003=1` as alpha-only for EFF_050.
- CPU callbacks receive an internal channel bitmask, not the UI enum directly.
  The callback allocates/updates per-lane monotonic queues only for enabled
  bits, so individual-channel modes should map upstream to single lane bits.

Direction enum `ADBE Minimax-0004`:

- String table exposes label order: `1=Horizontal & Vertical`,
  `2=Just Horizontal`, `3=Just Vertical`.
- CPU callbacks carry separate horizontal/vertical radius/pass state and an
  internal axis selector at `param_1[9]`.
- Fresh extraction confirms the two-axis machinery, but not the UI enum to pass
  plan write. Treat `0004=1/2/3` label order as visible, with `2/3` runtime
  orientation still needing an impulse probe.

Radius:

- CPU callbacks receive integer radius-like fields (`param_1[0]` /
  `param_1[1]` by axis). Queue capacity/window is `2 * radius + 1`.
- Inside the callback, radius `< 1` is coerced to `1`; therefore observed
  `radius=0` identity must be an upstream bypass/setup behavior, not this
  callback's active-kernel behavior.
- Integer radii in prior probes are accepted as exact (`6 -> 6`, `12 -> 12`).
- Fractional behavior is unknown from this bundle: setup quantization from AE
  float radius to callback integer radius is not visible.

`ADBE Minimax-0005` / Don't Shrink Edges:

- String table confirms first-class UI flag `Don't Shrink Edges`.
- CPU callback has an edge-policy byte at `*(char *)(param_1 + 10)`. When set,
  active queues are seeded/continued with sentinel samples at boundaries; when
  clear, the valid image window is clipped.
- The fresh bundle does not prove which UI boolean value maps to that byte, nor
  whether AE CPU/GPU edge behavior matches native clipped-window semantics.

8/16/32 bpc:

- Same separable monotonic-queue algorithm across depths.
- 8 bpc uses byte components, pixel stride 4, byte comparators.
- 16 bpc uses unsigned 16-bit components, pixel stride 8. Ghidra's decompile
  shows return-type noise for `FUN_18000d720`, but disassembly returns the
  boolean in `AL` via `SETB`/`SETA`.
- 32 bpc uses float components, pixel stride 16, scalar float compares.
- GPU path loads `AEFX_Minimax / MinimaxTraverseSTreeKernel`; kernel body is not
  in this extraction, so CPU/GPU parity at edges remains unproven.

## Accepted / Rejected / Unknown

Accepted:

- `0001=1 minimum`, `0001=2 maximum`.
- Visible operation labels: `0001=3 Minimum Then Maximum`,
  `0001=4 Maximum Then Minimum`.
- `0003=1 Color/RGB`, `0003=2 Alpha and Color`; visible labels for
  `0003=3 Red`, `4 Green`, `5 Blue`, `6 Alpha`.
- Visible direction labels: `0004=1 Horizontal & Vertical`,
  `0004=2 Just Horizontal`, `0004=3 Just Vertical`.
- Active callback radius window is integer and one-dimensional per pass:
  `2r + 1`.
- 8/16/32 CPU callbacks differ mainly by component type/stride/comparator type.

Rejected:

- `0001=2` as minimum for EFF_050.
- `0003=1` as alpha-only for EFF_050.
- Assigning STK residuals to Minimax from final PNGs alone. If residual is
  caused by upstream/downstream blur, geometry, alpha, sampler, or composite,
  keep it out of M13.

Unknown:

- AE-confirmed stage order for `0001=3/4` beyond visible label order.
- Runtime pass mapping for `0004=2/3` beyond visible label order.
- Fractional radius policy: floor, round, ceil, or fractional morphology.
- Exact `0005` boolean polarity and boundary sample semantics.
- CPU/GPU parity for edge cases and fractional/setup behavior.

## Missing Probes

- Operation sweep `0001=1..4` on an impulse/ramp source with intermediate
  samples.
- Channel sweep `0003=1..6` with distinct RGBA lane values.
- Direction sweep `0004=1..3` on separate horizontal and vertical impulses.
- Fractional radius sweep: `0.25`, `0.49`, `0.5`, `0.51`, `1.49`, `1.5`,
  `1.51`.
- Boundary impulse sweep with `0005=0/1`, comparing clipped, transparent-fill,
  repeated-edge, and sentinel/full-window possibilities.
- One 16 bpc and one 32 bpc probe with sub-byte/sub-float ramps.

## Next Implementation Candidate

Keep current implementation conservative:

- Operation mapping: `1=min`, `2=max`, `3=min_then_max`, `4=max_then_min`.
- Channel mapping by label order: `1=RGB`, `2=RGBA`, `3=R`, `4=G`, `5=B`,
  `6=A`.
- Direction mapping by label order: `1=HV`, `2=H`, `3=V`.
- Use integer `2r+1` separable morphology for integer radii.
- Do not change fractional radius or `Don't Shrink Edges` behavior until the
  missing probes isolate them.
- Do not tune M13 against STK residual unless Minimax-isolated probes reproduce
  the same error.

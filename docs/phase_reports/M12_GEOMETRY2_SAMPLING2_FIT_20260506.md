# M12 Geometry2 Sampling=2 Fit Pass

Date: 2026-05-06

## Goal

Do the Geometry2 bicubic pass in one fact loop:

- generate isolated `ADBE Geometry2` cases for `0012 = 1` and `0012 = 2`;
- render them on AE85 while Frida traces the CPU `Transform.aex` path;
- fit concrete sampler hypotheses from AE pixels;
- update native only after the traced parameter path and pixel fit agree.

## Artifacts

| Kind | Path |
| --- | --- |
| Probe pack | `fixtures/ae_probe_pack/geometry2_sampling2_fit` |
| AE render output | `target/ae_remote/ae_trace_batch_56_0f8fa7b7c2a7_20260506_122634` |
| AE PNG root | `target/ae_remote/ae_trace_batch_56_0f8fa7b7c2a7_20260506_122634/extracted/ae_trace_batch_56_0f8fa7b7c2a7_20260506_122634/geometry2_sampling2_fit/ae_goldens/png8` |
| Frida trace | `target/dynamic_tools_85/geometry2_sampling2_fit_trace_20260506/batch_56_0f8fa7b7c2a7.jsonl` |
| Frida summary | `target/dynamic_tools_85/geometry2_sampling2_fit_trace_20260506/summary.json` |
| S3 trace root | `ae_dynamic_traces/geometry2_sampling2_fit/20260506_122634` |
| Fit report | `target/ae_agents/geometry2_sampling2_fit_20260506/fit_report.json` |
| Candidate CSV | `target/ae_agents/geometry2_sampling2_fit_20260506/fit_candidates_refined_top16.csv` |
| Diff/native PNGs | `target/ae_agents/geometry2_sampling2_fit_20260506/winner_diffs` |
| Alpha AE render output | `target/ae_remote/ae_trace_batch_28_b84356833885_20260506_125433` |
| Alpha Frida trace | `target/dynamic_tools_85/geometry2_sampling2_alpha_trace_20260506/batch_28_b84356833885.jsonl` |
| Combined alpha fit report | `target/ae_agents/geometry2_sampling2_fit_combined_alpha_20260506/report/fit_report.json` |

## Frida Gate

The run was traced with:

```text
Transform.aex+0x5f30  geometry2_effect_proc_entry
Transform.aex+0x5b20  geometry2_wrapper_render_params_5b20
Stalker on Transform.aex PF_CMD_RENDER
```

Summary:

- `gpu_render_path_seen=false`
- `Transform.aex+0x5f30`: 1293 hits
- `Transform.aex+0x5b20`: 1293 hits
- Stalker summaries: `Transform.aex+0x5b20 x8`
- GPUFoundation matrix/bounds helpers: `TransformToMatrix`,
  `TransformsToMatrices`, `TransformedBounds`, `TransformedBoundsUnion`
- `PF_ParamDef[12]` remains `Sampling`, with values `1` and `2` in the same
  low-`s32` slot identified in the previous M12 trace.

This means the fit used the same CPU path we had already reverse-mapped; no GPU
quality branch was silently substituted.

## Fit Result

Candidate space:

- bilinear baseline;
- Keys cubic `a` sweep;
- Mitchell-Netravali `B/C` grid;
- transparent vs clamp edge;
- round vs floor output quantization.

Overall selected cases:

| Candidate | Mean Abs Channel Delta |
| --- | ---: |
| `keys_a_-0.700_transparent_round` | `31.106711` |
| `keys_a_-0.800_transparent_round` | `31.141839` |
| `keys_a_-0.700_transparent_floor` | `31.170452` |
| current native `keys_a_-0.500_transparent_round` | `31.854379` |

Opaque texture cases are the primary kernel-family signal because they avoid
alpha/premultiply ambiguity from sparse transparent impulse cases:

| Candidate | Mean Abs Channel Delta |
| --- | ---: |
| `keys_a_-0.700_transparent_round` | `2.730299` |
| `keys_a_-0.700_transparent_floor` | `2.820714` |
| `keys_a_-0.800_transparent_round` | `2.837407` |
| current native `keys_a_-0.500_transparent_round` | `4.035823` |

Q1 bilinear sanity on the opaque texture is `1.687966` mean channel delta. The
remaining Q1 error is concentrated at fractional/edge samples, so this pass
does not reopen the recovered matrix order.

## Alpha Sampler Follow-Up

The alpha-specific rerun rendered 28 `G2S_ALPHA_STEPS_*` cases under the same
Frida-gated CPU path:

- `gpu_render_path_seen=false`
- `Transform.aex+0x5f30` and `Transform.aex+0x5b20` were both hit 1237 times
- Stalker again confirmed `Transform.aex+0x5f30 -> 0x5b20`
- `PF_ParamDef[12]` still reports `Sampling`

The output module on AE85 produced RGB-only PNG/TIFF files for these cases:

```text
mode_counts = {"RGB:R,G,B": 28}
has_alpha_band_case_count = 0
```

So raw RGBA comparisons are diagnostic only: Pillow converts those RGB frames
to synthetic `A=255`, which makes direct alpha deltas meaningless.

The harness now records an explicit `rgb_over_black_projection` metric for
these cases. That metric compares AE's actual RGB output against each native
candidate after source-over projection on black:

| Branch | Winner | Mean Abs Channel Delta |
| --- | --- | ---: |
| `0012=1` bilinear | `bilinear_premult_unpremultiply_transparent_round` | `0.140631` |
| `0012=2` bicubic | `keys_a_-0.700_premult_unpremultiply_transparent_round` | `0.371739` |

For identity and most subpixel bilinear cases the projection is exact or within
1 code value. The bicubic alpha cases remain within low single-code deltas for
the reported per-case sample, with larger residuals isolated to scale/rotation
cases that still need a true alpha-preserving export before internal alpha can
be locked.

## Native Change

`Geometry2SamplerMode::Bicubic` now uses a Keys cubic kernel with:

```text
a = -0.7
edge = transparent partial footprint
alpha policy = premultiplied accumulation, then unpremultiply to straight RGBA
quantization = round to nearest after clamp
```

The sampler labels are now:

```text
bilinear_premult_unpremultiply_partial_footprint_transparent
bicubic_keys_a_-0.7_premult_unpremultiply_partial_footprint_transparent
```

## Verification

```text
cargo test -p effects geometry -- --nocapture
cargo test -p render-core adjustment_stack_debug_records_geometry_minimax_and_turbulent_checkpoints -- --nocapture
cargo run -p render-cli -- conformance-pack --case EFF_040
python3 -m py_compile fixtures/ae_probe_pack/geometry2_sampling2_fit/scripts/measure_geometry2_sampling2_fit.py scripts/ae_trace_drop_shadow_softness.py scripts/summarize_ae_frida_trace.py
```

After the alpha wrapper patch, `EFF_040` remains close on the M19-preferred
visible metric: `rgb_straight_source_over_ae_background=0.054426`,
`background_alpha_normalized=0.100533`, raw `rgba_mean=0.100533`, max `250`.
The max delta is raw/foreground RGB under partial alpha and is not used as the
primary formula-tuning gate.

## Next

Geometry2 is now past the parameter-mapping blocker and has a fitted bicubic
branch plus an alpha-aware sampler wrapper. The remaining work is narrow:

1. Get an alpha-preserving AE output-template/export path for the alpha steps
   pack so internal alpha can be compared directly instead of through RGB-only
   black projection.
2. Re-render a composed scene that actually uses `0012=2` and check the native
   delta drop.
3. If `0012=2` still has residuals, tune only edge handling/alpha wrapper,
   leaving matrix math frozen.

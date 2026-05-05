# Agent Process Log

This log records the concrete sequence of reads, commands, edits, tests, AE
runs, and decisions used while tuning the native renderer. It is intentionally
compact and append-only so the workflow can later be scripted.

## Format

| Time MSK | Type | Items | Outcome |
| --- | --- | --- | --- |

## 2026-05-06

| Time MSK | Type | Items | Outcome |
| --- | --- | --- | --- |
| 00:32 | command | `date`, `git status --short`, `ls docs`, `ls docs/phase_reports` | Confirmed clean worktree after `f3452ab`; oriented available docs/reports before selecting next block. |
| 00:33 | read | `docs/MATH_PARITY_STATUS.md`, `docs/EFFECTS.md`, `docs/phase_reports/M13_MINIMAX_DISCRIMINATOR_20260506.md`, `docs/phase_reports/PHASE_4_GEOMETRY_TURBULENT.md` | M13 is now instrumented/testable. Remaining highest-leverage block is M12 Geometry2 isolated conformance, then M14 Turbulent field replacement/tuning, with STK_030 as composed regression. |
| 00:40 | read | `crates/effects/src/geometry.rs`, `crates/render-cli/src/conformance_pack.rs`, `docs/phase_reports/M12_HYPOTHESIS_RESULTS_20260505.md`, `fixtures/ae_probe_pack/geometry2_edge/README.md` | Current M12 mapping/matrix is already accepted; remaining uncertainty is pixel-center/sampler/OOB. Existing `geometry2_edge` pack is the right next probe before touching formulas. |
| 00:42 | read/command | `render-cli conformance-pack --help`, `fixtures/ae_probe_pack/geometry2_edge/scripts/measure_geometry2_edge_probe.py`, `fixtures/ae_probe_pack/geometry2_edge/manifest.json` | Confirmed local conformance CLI options and that `geometry2_edge` measures pixel-center, OOB candidates, hidden RGB, and `0012` sampling branches. |
| 00:43 | command | `cargo run -p render-cli -- conformance-pack --case EFF_040 --out target/ae_agents/m12_eff040_20260506_baseline` | Fresh Geometry2 baseline matches previous M12 report: `rgba_mean=0.060292`, `rgb_visible_mean=0.045086`, `max=61`; sidecar still resolves anchor `[128,128]`, scale `[120,120]`, rotation `17`, sampler `bilinear_transparent_out_of_bounds`. |
| 00:44 | command/edit | `geometry2_edge` self-test, asset generation, `fixtures/ae_probe_pack/geometry2_edge/jsx/build_geometry2_edge_probe_project.jsx` | Measurement self-test passed (`pixel_center=integer`). Removed blocking AE `alert` and added project reset/cache purge for remote automation. |
| 00:50 | AE remote | `python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/geometry2_edge --entry-script jsx/build_geometry2_edge_probe_project.jsx --node http://85.239.48.31:8000 --job-id geometry2_edge_20260506` | AE85 rendered the isolated Geometry2 edge pack successfully. Output archive landed under `target/ae_remote/geometry2_edge_20260506`; 48 PNG cases extracted for measurement. |
| 00:53 | command | `python3 fixtures/ae_probe_pack/geometry2_edge/scripts/measure_geometry2_edge_probe.py ...` | Probe selected `pixel_center=integer`. OOB candidates favored `partial_footprint_transparent` over whole-sample transparent/clamp. Sampling `0012` produced a distinct branch, but hidden RGB marker cases did not expose transparent RGB. |
| 00:59 | edit/test | `crates/effects/src/geometry.rs`, `cargo test -p effects geometry -- --nocapture`, `cargo test -p render-core adjustment_stack_debug_records_geometry_minimax_and_turbulent_checkpoints -- --nocapture` | Implemented Geometry2 partial-footprint transparent bilinear sampling and updated adjustment-stack sidecar expectations. Focused Rust tests passed. |
| 01:01 | command | `cargo run -p render-cli -- conformance-pack --case EFF_040 --out target/ae_agents/m12_eff040_20260506_partial_footprint` | `EFF_040` final metrics stayed at `rgba_mean=0.060292`, `rgb_visible_mean=0.045086`, `max=61`; OOB count changed (`91194 -> 90181`). Residual is not owned by the full-canvas OOB policy. |
| 01:06 | Frida | `scripts/ae_trace_drop_shadow_softness.py` with Geometry2 broad/targeted hooks on AE85 | Confirmed no-GPU CPU path: `Transform.aex+0x5f30` EffectProc, `GPUFoundation.dll` `TransformsToMatrices`/`TransformToMatrix`/bounds helpers, `BEE`/`PIN` output exports, and no `TransformWithMotionBlur`/`TransformOperation::Quality` hits. Broad trace caught `Transform.aex+0x4440` and `+0x4ab0`. |
| 01:12 | command/edit | `scripts/summarize_ae_frida_trace.py`, `python3 scripts/summarize_ae_frida_trace.py target/dynamic_tools_85/geometry2_edge_trace_20260506` | Added a small trace summarizer so future Frida runs can be compared without manually opening huge JSONL files. |

# Phase 2 Temporal Core + Posterize Time

Date: 2026-05-03

## Scope

Phase 2 covers the temporal chain needed for M02 and M15:

1. comp time: render frame index maps to `frame / comp.fps`.
2. layer time: Posterize Time effects on the layer quantize the sampling time before layer transform, opacity, text reveal, footage, precomp, and downstream effect evaluation.
3. source time: footage uses `source_start + (layer_time - layer_start)`, clamped at zero; precomp uses `(layer_time - layer_start)`, clamped at zero.
4. effect time: effect stacks receive the posterized layer time, while the Posterize Time canvas effect itself stays a no-op because temporal sampling already happened upstream.
5. posterize buckets: current implementation floors to `floor(time * posterize_fps) / posterize_fps` with a small epsilon in `effects::posterize_time`.
6. adjustment lower-stack resampling: an adjustment layer with Posterize Time re-renders the already-lower stack at the quantized effect time before applying subsequent effects.

## Golden Assets

`fixtures/ae_conformance_pack/manifest.json` contains Phase 2 cases:

- `TMP_010`: source/layer start time and frame mapping, modules `M02`, `M15`.
- `TMP_020`: Posterize Time numbered-frame boundaries, module `M15`.
- `STK_030`: Geometry2 -> Posterize Time -> Minimax -> Turbulent Displace adjustment stack, modules `M12`, `M13`, `M14`, `M15`, `M16`, `M19`.

Usability check:

- `fixtures/ae_conformance_pack/ae_goldens/png/TMP_010`: 60 PNGs, 512x512 RGBA.
- `fixtures/ae_conformance_pack/ae_goldens/png/TMP_020`: 60 PNGs, 512x512 RGBA.
- `fixtures/ae_conformance_pack/ae_goldens/png/STK_030`: 60 PNGs, 512x512 RGBA.

## Implementation Notes

Existing behavior observed:

- `crates/effects/src/posterize_time.rs` parses named and AE-numbered frame-rate params and exposes `quantize_time`.
- `crates/render-core/src/layer_eval.rs` applies `posterized_time_for_effects` before layer/source/precomp/effect evaluation.
- render-core already has unit tests for layer sampling quantization and adjustment-layer lower-stack resampling.

Added Phase 2 testkit probes:

- 10 fps Posterize Time buckets over a 30 fps comp hold each numbered source frame for three comp frames.
- layer/source offsets are included in numbered-frame source expectations.
- manifest-listed compare frames for `TMP_010`, `TMP_020`, and `STK_030` exist and decode as 512x512 RGBA PNGs.

## Gate Status

Phase 2 is testable against available AE goldens, but not parity-locked yet. The current gate can validate temporal bucket/source expectations and golden availability; a full pass still needs native-vs-AE PNG diff wiring for these cases and threshold decisions.

## Commands Run

- `find fixtures/ae_conformance_pack/ae_goldens/png ...`: pass; found complete `TMP_010`, `TMP_020`, and `STK_030` PNG sequences.
- `file fixtures/ae_conformance_pack/ae_goldens/png/{TMP_010,TMP_020,STK_030}/*_00000.png`: pass; all sample PNGs are 512x512 RGBA.
- `cargo fmt --check`: blocked; `cargo` is not available in PATH.
- `cargo test -p testkit phase2`: blocked; `cargo` is not available in PATH.
- `cargo test -p effects posterize_time`: blocked; `cargo` is not available in PATH.
- `cargo test -p render-core posterize`: blocked; `cargo` is not available in PATH.

## Blockers

- Rust tooling is unavailable in the current shell, so targeted tests and formatting could not be executed here.
- Phase 2 cannot pass the full AE gate until native output is rendered and diffed against `TMP_010`, `TMP_020`, and `STK_030` thresholds.

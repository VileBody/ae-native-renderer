# Phase 5: Text, Expressions, Collapse

Status date: 2026-05-03

## Scope

Worker Phase 5 covers:

- M05 text rasterization and glyph layout
- M06 text range selector reveal by characters, words, and lines
- M07 glyph animator position, scale, rotation, opacity, and blur
- M08 expression selector bounce
- M09 generated property expressions, especially `edge_wobble`
- M17 collapse transformations for nested text sharpness

## AE Golden Coverage

The conformance pack manifest declares the Phase 5 cases and selected frames:

| Case | Modules | Frames |
| --- | --- | --- |
| `TXT_010` | M05, M06 | 0, 8, 16, 24, 32, 45, 59 |
| `TXT_020` | M05, M06 | 0, 8, 16, 24, 32, 45, 59 |
| `TXT_030` | M05, M07 | 0, 8, 16, 24, 32, 45, 59 |
| `TXT_040` | M08, M09 | 0, 5, 10, 15, 20, 30, 45, 59 |
| `EXP_010` | M09 | 0, 5, 10, 15, 20, 30, 45, 59 |
| `GPH_010` | M17, M05, M19 | 0, 15, 30, 45 |

Each of those six golden directories currently contains 60 PNG frames. Spot checks
with `file` show the selected first frames are 512x512 8-bit RGBA PNGs.

## Font Assumptions

- `Montserrat-BoldItalic` is required by `TXT_010` and `TXT_020`.
  `fixtures/ae_conformance_pack/assets/fonts/Montserrat-Italic[wght].ttf` is
  bundled and should be used as the deterministic Montserrat source.
- `Point-Light` is required by `TXT_030` and `TXT_040`. It is not bundled in the
  conformance pack; the manifest explicitly says to use the exact Point-Light
  font installed on the AE render machine.
- Local `fc-match` probing did not resolve either font in this shell environment
  (`fc-match ...` returned only separators), so Point-Light parity is blocked
  unless the exact font is installed or supplied.

## Current Native Decomposition

- Font resolution: `crates/text-engine/src/font_db.rs` resolves direct paths,
  then `fc-match`, then common fallback fonts. This is deterministic enough for
  testing, but not AE-equivalent when the exact font is absent.
- Glyph layout: `crates/text-engine/src/layout.rs` uses fontdue metrics, simple
  line splitting, centered text boxes, per-char advances, and glyph bboxes. It
  reports char, word, and line indices, but does not perform AE/CoreText shaping
  or kerning parity.
- Selectors: `crates/text-engine/src/text_animator.rs` has v2 helper primitives
  for characters, words, lines, smoothness, random order, wiggly, and blur
  planning. `render-core/src/layer_eval.rs` has its own runtime selector path,
  so selector telemetry must compare helper/runtime behavior before tuning.
- Glyph transforms and blur: runtime applies per-unit rectangular transforms
  and a bounded splat blur approximation over alpha bounds. This is useful for
  stable approximation, but not yet a per-glyph AE transfer/order model.
- Bounce expression selector: `expression-engine/src/evaluator.rs` recognizes a
  generated bounce fingerprint and evaluates delay/frequency/amplitude/decay.
  `render-core/src/layer_eval.rs` also has a native `PerCharacterBounce` path.
- `edge_wobble`: expression evaluator supports a small scalar/Vec2 subset. The
  render path references generated/named property expression behavior, but there
  is no Phase 5 telemetry yet for per-frame expression values.
- Collapse text sharpness: `render-core/src/precomp.rs` has collapse planning and
  text/solid-only flatten support; `layer_eval.rs` has scale-aware collapsed text
  rasterization. The missing piece for gate confidence is deferred-raster
  telemetry plus sharpness probes against `GPH_010`.

## Telemetry Plan Before Formula Tuning

Add deterministic telemetry records before changing formulas:

- M05: resolved font path/postscript, requested font family, font size, text box,
  glyph id, char index, word index, line index, advance, bbox, baseline.
- M06: unit extraction mode, selector index/order, start/end percentages,
  shape/smoothness/random seed/wiggly params, final selector weight per unit.
- M07: unit rectangle/glyph bbox, anchor, position delta, scale, rotation,
  opacity, blur radius, output matrix or sampled transform.
- M08: textIndex/textTotal, layer start, delay/frequency/amplitude/decay, raw
  expression selector amount before clamping.
- M09: expression source mode, input value, time, in/out points, context vars,
  raw evaluated Vec2/scalar, final property value for `edge_wobble`.
- M17: collapse mode, flattened layer list, parent and child matrices, effective
  text raster scale, rasterization size, final composite matrix, sharpness probe.

## Changes Made

- Added `crates/testkit/src/phase5.rs` with focused Phase 5 contract checks:
  manifest case/frame/module coverage, selected AE golden PNG validity, font
  assumption validation, and telemetry checkpoint coverage.
- Added `phase5` module export in `crates/testkit/src/lib.rs`.

No renderer math was changed. No shared `render-core` files were modified.

## Commands Run

- `rg --files fixtures/ae_conformance_pack crates/text-engine crates/expression-engine/src crates/render-core/src docs crates/testkit/src`
- `git status --short`
- `sed -n ...` over manifest, text engine, expression evaluator, render-core
  precomp/layer_eval, and docs
- `find fixtures/ae_conformance_pack/ae_goldens/png -maxdepth 2 -type f`
- `ls -l fixtures/ae_conformance_pack/assets/fonts ...`
- `file fixtures/ae_conformance_pack/ae_goldens/png/{TXT_010,TXT_020,TXT_030,TXT_040,EXP_010,GPH_010}/*_00000.png`
- `fc-match -f '%{family}|%{style}|%{file}\n' Montserrat-BoldItalic Point-Light`
- `for case in TXT_010 TXT_020 TXT_030 TXT_040 EXP_010 GPH_010; do find ... | wc -l; done`
- `cargo fmt -p testkit`
- `~/.cargo/bin/cargo fmt -p testkit`
- `rustc --version`
- `cargo test -p testkit phase5`

## Verification

Pass:

- Manifest decomposition for Phase 5 was verified manually.
- AE golden directories for the six Phase 5 cases exist and contain 60 PNG
  frames each.
- First-frame PNG spot checks are 512x512 8-bit RGBA.
- Bundled Montserrat variable italic font exists.
- New test file was statically reviewed after insertion.

Fail / not run:

- `cargo fmt -p testkit` failed because `cargo` is not in PATH.
- `~/.cargo/bin/cargo fmt -p testkit` failed because that path does not exist.
- `rustc --version` produced no output, indicating the Rust toolchain is not
  available in this shell environment.
- `cargo test -p testkit phase5` failed because `cargo` is not in PATH.

## Blockers

- Exact `Point-Light` is not bundled and was not resolved locally. `TXT_030` and
  `TXT_040` cannot pass tight AE parity without that font.
- No native-vs-AE image diff gate is wired for Phase 5 yet.
- Text animator runtime and text-engine v2 helper selector logic both exist;
  parity tuning needs telemetry to ensure the same unit/order/weight model.
- M05 glyph layout is fontdue/simple-layout based, not AE/CoreText/HarfBuzz
  equivalent.
- M07 blur is a splat approximation, not AE glyph blur semantics.
- M08/M09 expression paths need raw per-frame/per-unit telemetry before tuning.
- M17 has planning and scale-aware rasterization, but lacks `GPH_010` sharpness
  measurement telemetry.

## Gate Assessment

Phase 5 cannot pass the parity gate now. The AE references are present and the
new testkit contract anchors the required cases, frames, fonts, and telemetry
surface, but the native implementation remains approximate and the local
toolchain/font environment prevents running the new tests or validating
native-vs-AE diffs.

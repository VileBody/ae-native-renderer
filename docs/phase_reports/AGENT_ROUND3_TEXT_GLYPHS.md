# Agent Round3 Text Glyphs

Status date: 2026-05-03.

## Scope

Owned block: M05-M09 text glyph metrics / selector basics.

This pass used exact-font `TXT_030` and `TXT_040` first, with `TXT_010` and
`TXT_020` as guards. It used `text_telemetry.jsonl` from
`target/ae_agents/round2_integrated_trace_smoke/TXT_030` plus current
conformance outputs. `TXT_040` had no Round 2 trace-smoke sidecar, so current
Round 3 conformance telemetry is the first selector trace for that case.

Adobe docs checked:

- https://helpx.adobe.com/after-effects/using/formatting-characters-character-panel.html
- https://helpx.adobe.com/after-effects/using/animating-text.html
- https://ae-scripting.docsforadobe.dev/text/textdocument/

The docs confirm text animator properties and selector units/`Based On`
semantics, and that font size maps to composition pixels at 100% layer scale.
They do not provide exact font shaping/raster baseline formulas, so measured
glyph telemetry remained primary evidence.

## Diagnosis

First mismatch for exact-font cases was text-box placement, not font fallback:

- `TXT_030` and `TXT_040` resolve `Point-Light.ttf` via `DirectPath` with
  `fallback=false`.
- Round 2 `TXT_030` layout used baseline `285.593` and clamped overfull line
  centering to x=0, yielding glyph union `[3.0, 234.593, 578.854, 286.593]`.
- AE/native bboxes showed native was consistently too low in Point-Light cases:
  `TXT_040` frame 59 was AE `(0,212,512,257)` vs native `(6,238,512,283)`.
- Guard bboxes also supported a baseline correction: `TXT_010` frame 59 was AE
  `(0,214,512,327)` vs native `(4,202,512,315)`.

Selector/animator observations:

- `TXT_030` selector segmentation improved after text placement; current trace
  sees 11 non-space character units for `GLYPH MOTION`.
- `TXT_040` frame 0 remains a later selector/expression blocker: current trace
  gives expression weights `0.0`, which leaves the text unaffected in native,
  while AE golden is blank. Adobe docs say selector Amount 0 means animator
  properties do not affect characters, so this needs AE telemetry or render-core
  selector semantics work rather than another text-engine glyph change.

## Changes

Round 3 edits in `crates/text-engine/src/layout.rs`:

- first real-font baseline now starts at the text box vertical center;
- overfull lines now remain mathematically centered instead of clamping the
  x-offset to zero;
- added a focused Point-Light test for overfull text centering and baseline.

No render-core, expression-engine, or effects files were edited.

Note: the worktree already contained earlier text telemetry/font-resolution
changes in `font_db.rs`, `rasterize.rs`, and formatting-only differences in
`text_animator.rs` before this Round 3 edit.

## Metrics

Metric values are averaged over each case's selected comparison frames. Use RGB
and background-alpha-normalized metrics; raw RGBA remains dominated by the known
background alpha difference.

| Case | Run | RGB mean | RGB RMSE | RGB changed | BG-norm mean | BG-norm RMSE | BG-norm changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `TXT_010` | before | 9.085755 | 39.330581 | 114667 | 8.720305 | 41.066792 | 114680 |
| `TXT_010` | after | 8.184852 | 37.304239 | 105150 | 7.726469 | 38.599153 | 105195 |
| `TXT_020` | before | 8.660901 | 38.377645 | 108074 | 8.367676 | 39.763429 | 108081 |
| `TXT_020` | after | 7.245444 | 35.008286 | 95256 | 6.864629 | 35.924600 | 95365 |
| `TXT_030` | before | 10.815494 | 42.358021 | 204387 | 13.547671 | 50.391939 | 204419 |
| `TXT_030` | after | 8.128677 | 34.969623 | 176988 | 10.750017 | 43.466684 | 177111 |
| `TXT_040` | before | 6.823042 | 38.943447 | 73322 | 6.656466 | 38.970149 | 73325 |
| `TXT_040` | after | 4.250647 | 30.423745 | 51400 | 4.232892 | 30.682322 | 51431 |

Representative bbox deltas:

| Case/frame | Before native vs AE | After native vs AE |
| --- | --- | --- |
| `TXT_010` frame 59 | `(dx=4, dy=-12, dw=-4, dh=0)` | `(dx=0, dy=1, dw=-6, dh=0)` |
| `TXT_020` frame 59 | `(dx=6, dy=20, dw=-12, dh=-53)` | `(dx=6, dy=1, dw=-12, dh=0)` |
| `TXT_030` frame 0 | `(dx=2, dy=46, dw=-2, dh=38)` | `(dx=0, dy=7, dw=0, dh=46)` |
| `TXT_040` frame 59 | `(dx=6, dy=26, dw=-6, dh=0)` | `(dx=0, dy=0, dw=0, dh=0)` |

Current exact-font telemetry:

- `TXT_030`: baseline `256.0`, line width `586.053`, union
  `[-34.026, 205.0, 541.828, 257.0]`, `DirectPath`, `fallback=false`.
- `TXT_040`: baseline `256.0`, line width `638.388`, union
  `[-57.194, 212.0, 572.131, 257.0]`, `DirectPath`, `fallback=false`.

## Commands

`cargo` was not available locally, so tests and conformance were run in Docker:

```sh
docker run --rm --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'set -eu
export PATH=/usr/local/cargo/bin:$PATH
export CARGO_HOME=/tmp/cargo-home
export CARGO_TARGET_DIR=/tmp/ae-native-renderer-cargo-target-round3-text
apt-get update >/dev/null
apt-get install -y --no-install-recommends fontconfig >/dev/null
rustup component add rustfmt >/dev/null
cargo fmt -p text-engine
cargo test -p text-engine -- --nocapture'
```

Result: `text-engine` passed, 13 tests.

```sh
docker run --rm --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'set -eu
export PATH=/usr/local/cargo/bin:$PATH
export CARGO_HOME=/tmp/cargo-home
export CARGO_TARGET_DIR=/tmp/ae-native-renderer-cargo-target-round3-text
apt-get update >/dev/null
apt-get install -y --no-install-recommends fontconfig pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev >/dev/null
cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round3_text_glyphs --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040'
```

Result: `conformance-pack.done ok=true cases=4`.

## Next Blocker

Exact glyph/text-box placement is materially closer but not complete. The next
safe target is glyph raster/coverage and per-glyph animator/raster order:
`TXT_030` still has much lower native text coverage than AE on early frames even
after placement correction, while `TXT_040` settled placement is now exact.

Do not tune `TXT_040` frame-0 expression-selector behavior from text-engine
alone. Current evidence points at selector/expression animator semantics in
render-core, and Adobe docs vs AE golden behavior need a small dedicated
telemetry probe before changing that path.

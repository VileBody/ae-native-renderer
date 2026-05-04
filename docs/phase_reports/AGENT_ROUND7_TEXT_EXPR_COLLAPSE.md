# Agent Round7 Text Expression Collapse

Status date: 2026-05-04.

## Scope

Worker 5 scope for this pass was text/glyph/selector/expression/collapse
observability, without broad formula tuning and without final-PNG guessing.

The worktree was already dirty when this pass started. In the owned area,
existing local changes already provided:

- font resolution telemetry in `text-engine`;
- layout telemetry with glyph id, advance, bbox, baseline, line/word indices,
  line width, and text box;
- render-core text layout sidecars;
- selector sidecars with per-unit range/expression/final weights;
- position-expression sampled value traces;
- collapse traces for rasterized precomps, collapsed precomps, and collapsed
  text raster scale/sharpness.

This pass makes only a local glyph telemetry increment.

## Change

`GlyphLayoutTelemetry` now repeats the resolved font identity on each glyph
record:

- `font_path`;
- `font_family`;
- `font_style`;
- `font_postscript_name`;
- `font_fallback`;
- `font_resolution_source`;
- `font_size`.

This makes individual glyph rows in `text_telemetry.jsonl` self-contained. A
single glyph record now carries the character, actual font glyph id, resolved
font identity, font size, advance, bbox, baseline, line width, and text-box
rect. This is useful for `TXT_030`/`TXT_040` and collapsed text comparisons
where downstream analysis often filters directly to a glyph row instead of
joining back to top-level `font_resolution`.

No glyph placement, selector formula, expression formula, rasterization policy,
or collapse matrix policy was changed.

## Test Coverage

The existing Point-Light layout test now verifies that the first glyph record
itself carries:

- the repo-local `Point-Light.ttf` direct path;
- `FontResolutionSource::DirectPath`;
- `font_fallback=false`;
- the effective `font_size`;
- the actual `font_glyph_id`.

This keeps the check focused on observability rather than image output.

## Verification

Local `cargo` is not available in PATH, so verification used Docker with
`fontconfig` installed for the font metadata probes:

```sh
docker run --rm --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm -lc 'set -eu
export PATH=/usr/local/cargo/bin:$PATH
export CARGO_HOME=/work/target/ae_agents/round7_text_expr_collapse/cargo-home
export CARGO_TARGET_DIR=/work/target/ae_agents/round7_text_expr_collapse/cargo-target
apt-get update >/dev/null
apt-get install -y --no-install-recommends fontconfig >/dev/null
cargo test -p text-engine -- --nocapture
'
```

Result: `text-engine` passed, 13 tests.

## Exact AE-Golden Steps Needed Next

1. Rerun focused `TXT_030`, `TXT_040`, `EXP_010`, and `GPH_010` sidecars with
   the new glyph rows and compare glyph-by-glyph against AE golden bboxes,
   advances, and visible alpha bounds.
2. For `TXT_030`, capture or derive AE per-glyph blur/coverage evidence before
   changing the render-core animator blur radius/order. Existing telemetry
   already points at render-core animator application, not basic layout.
3. For `TXT_040`, add an AE expression-selector probe for frame 0 and the first
   reveal frames: `textIndex`, `textTotal`, raw amount, clamped amount, final
   selector weight, and whether zero/negative weight suppresses or preserves the
   base text.
4. For `EXP_010`, compare native `position_expressions` sampled positions
   against AE property samples at the same exact comp times before expanding the
   expression subset.
5. For `GPH_010`, compare `layer_text` vs `collapsed_text` glyph records under
   the same resolved font path and source glyph ids, then use collapse
   `effective_raster_scale` and `sharpness_probe` only after the glyph/source
   identity matches AE.
6. Keep final PNG metrics masked away from known background-alpha differences
   while diagnosing text/collapse primitives.

## Files Changed

- `crates/text-engine/src/layout.rs`
- `docs/phase_reports/AGENT_ROUND7_TEXT_EXPR_COLLAPSE.md`

# P2 ARE Cubic Scanline Static Pass

Date: 2026-05-08

Scope: remaining P2 text raster drift after TXT curve producer parity. This pass targets the ARE/BIB layer between `TXT_ARE_PathBuilder_40580` and `ARE_row_getter_8230`.

## Static Sources

- `target/reverse/predecoded/20260508_p2_bib_path_bridge/bib_path_bridge`
- `target/reverse/predecoded/20260508_p2_are_path_builder_rasterizer/are_path_builder_rasterizer`
- `target/reverse/predecoded/20260508_p2_are_path_builder_expanded/are_path_builder_expanded`
- `target/reverse/predecoded/20260508_p2_are_fill_core/are_fill_core`
- `target/reverse/predecoded/20260508_p2_are_curve_scanline_core/are_curve_scanline_core`
- `target/reverse/predecoded/20260508_p2_are_cubic_subdivision/are_cubic_subdivision`
- `target/reverse/predecoded/20260508_p2_are_cubic_math_helpers/are_cubic_math_helpers`

## Recovered Facts

- `ARE+0xc200` is a thin fill wrapper; the real builder is `ARE+0xa1ac`.
- `ARE+0xa1ac` dispatches the fill path to `ARE+0x9cb8`.
- `ARE+0x9cb8` builds the path/raster source through `ARE+0xe854`, then emits edge events through `ARE+0x95cc`.
- `ARE+0xe854` interprets path commands:
  - `0`: move
  - `1`/`3`: line/close-like segment handling
  - `2`: cubic segment
- Cubics go through `ARE+0x10380`.
- `ARE+0x10380` first splits cubic segments at x/y extrema via `ARE+0x11e28`.
- `ARE+0x11e28` computes cubic derivative roots on both axes and merges the sorted split times.
- `ARE+0x10a98` then projects each monotonic segment into row ranges.
- `ARE+0x430c` still owns the final event boundary rule: `floor(projected_min)` to `floor(projected_max) + 1`.

## Native Change

`crates/text-engine/src/rasterize.rs` now keeps a recovered ARE path segment layer in addition to the old flattened debug contours:

- line-only contours still use the proven reversed-TTF line path;
- curve contours keep cubic segments produced by the recovered TXT quadratic-to-cubic model;
- cubic rasterization now splits by x/y derivative roots and projects each monotonic cubic piece directly into 1/16-pixel strip events;
- the previous fixed-step curve flattening remains only as fallback/debug contour data.

This is a reverse-driven implementation of the `ARE+0x11e28 -> ARE+0x10380 -> ARE+0x10a98` shape, not metric-only tuning.

## Validation

Commands run:

```text
cargo test -p text-engine
cargo test -p render-core text
cargo test -p render-cli all_manifest_cases_have_native_recipes
cargo run -q -p render-cli -- render --scene target/ae_agents/p2_cov_w_native_scene_floor_end_20260508/scene.json --out target/ae_agents/p2_cov_w_are_cubic_native_20260508/rendered
python3 scripts/compare_text_row_spans.py --case COV_W --ae-row-getter-analysis target/ae_agents/p2_cov_w_are_row_getter_dense_20260508/analysis.json --native-text-telemetry target/ae_agents/p2_cov_w_are_cubic_native_20260508/rendered/text_telemetry.jsonl --out target/ae_agents/p2_row_compare_covw_are_cubic_20260508/row_compare.json
cargo run -q -p render-cli -- conformance-pack --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case GPH_010 --out target/ae_agents/p2_text_are_cubic_gate_20260508
```

`COV_W` line-only guard stayed accepted:

```text
normalized ink shape: 202 / 202
coverage byte exact:  202 / 202
```

Focused text gate stayed green:

```text
cases: TXT_010 TXT_020 TXT_030 TXT_040 GPH_010
ok:    true
```

Visible metric movement versus `p2_text_curve_producer_gate_20260508`:

```text
case     before visible mean   after visible mean
TXT_010  12.760945           12.531241
TXT_020   6.327912            6.410895
TXT_030   9.650311            7.073936
TXT_040   4.081952            3.671418
GPH_010   4.057992            4.002211
```

## Status

P2 text raster is now past "implemented approximate" for the curve edge path:

```text
TXT producer: implemented / instrumented / static-backed
ARE line scanline: accepted for COV_W
ARE cubic scanline: implemented / static-backed / gate-tested
```

Remaining P2 work:

- get an authoritative `COV_O` row-getter dense capture, not only producer path order;
- compare native cubic rows against that row-getter analysis;
- if rows still drift, finish the lower-level `ARE+0xfc04 / 0x125b8 / 0x1268c` incremental x-table parity instead of changing formulas by eye.

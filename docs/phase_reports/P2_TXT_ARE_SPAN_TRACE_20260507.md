# P2 TXT ARE Span Trace - 2026-05-07

## Scope

Goal: turn the remaining P2 text raster unknowns into a shared, reproducible trace surface before more native formula work:

- literal CoolType/BIB coverage rows;
- hinting/AA/subpixel policy;
- clipped span rounding;
- stroke/fill order;
- semitransparent fill temp-world transfer.

Five subagents analyzed static + probe surfaces independently. Orchestrator-owned shared files were kept out of agent write scope: `scripts/ae_trace_cooltype_text.py`, `docs/reverse_engineering/GHIDRA_PREDECODE_TASKS.json`, and native Rust crates.

## Agent Outcomes

- Coverage rows: recovered `TXT.dll` ARE chain to span writer `TXT.dll+0x3b8c0`; next shared trace target was `3de50/3b8c0`.
- Hinting/AA/subpixel: AE output is scalar grayscale coverage at TXT output, not LCD/RGB subpixel masks; fractional placement remains geometric.
- Clipped bounds: sourceRect/layout rounding is separate from final row spans; final writes are integer half-open spans.
- Stroke/fill: fill/stroke order byte controls pass order; ordinary fill+stroke uses the same pixel writer.
- Semitransparent fill: static Ghidra shows fill alpha `< 255` renders into a temp `PF_World` at full local alpha, then calls `PF_TransferRect` with opacity.

One collision was escalated and merged correctly: stroke/fill and semitransparent fill both touch the text-local temp-world transfer path.

## Shared Frida Surface

Added `--hook-profile txt-are-spans` to `scripts/ae_trace_cooltype_text.py`.

Hooks:

- `BEE_IMPORT_TXT_DrawChar_edf6b0`
- `TXTp_DrawChar3_ARE_42110`
- `TXT_DrawChar_outline_core_42b80`
- `TXT_ARE_Render_8bpc_3c360`
- `TXT_ARE_Render_8bpc_fill_3d200`
- `TXT_ARE_Render_8bpc_stroke_3d960`
- `TXT_ARE_OutputComposite_8bpc_3de50`
- `TXT_ARE_PixelWriter8_span_3b8c0`
- `TXT_IMPORT_PF_TransferRect_694f30`

## Dynamic Result

Command:

```bash
python3 scripts/ae_trace_cooltype_text.py \
  --ssh-host ae85 \
  --node http://85.239.48.31:8001 \
  --pack fixtures/ae_probe_pack/p2_text_coverage_rows_probe \
  --entry-script jsx/build_p2_text_coverage_rows_probe_project.jsx \
  --case COV_W \
  --duration 150 \
  --max-events 2200 \
  --hook-profile txt-are-spans \
  --out-dir target/dynamic_tools_85/p2_shared_are_spans_covw_stride_20260507 \
  --allow-render-failure
```

AE job:

- `ae_trace_cooltype_COV_W_20260507_234202`
- render id `a1713804e6be4f589efa4f5efa84fc0b`
- summary `target/dynamic_tools_85/p2_shared_are_spans_covw_stride_20260507/summary.json`

Hook counts:

- `TXT_ARE_PixelWriter8_span_3b8c0`: `1758`
- `TXT_ARE_Render_8bpc_3c360`: `2`
- `TXT_ARE_Render_8bpc_fill_3d200`: `2`
- `TXT_ARE_OutputComposite_8bpc_3de50`: `2`
- `TXT_DrawChar_outline_core_42b80`: `14`

Recovered coverage-plane ABI for opaque `COV_W`:

- coverage plane width: `109`
- coverage plane height: `68`
- coverage base: plane `+0x10`
- base mirror: plane `+0x28`
- row stride: plane `+0x30`, value `112`
- row getter: `ARE.dll+0x8230`
- type-2 spans carry byte coverage samples.
- observed coverage rows: `y=0..67`
- observed type-2 coverage samples: `409`

Sample spans:

```text
y=0  x=0..16    coverage=24 40 40 ... 3c
y=0  x=46..62   coverage=01 3e 40 ... 24
y=1  x=0..1     coverage=6f
y=1  x=15..17   coverage=00 4f
y=2  x=45..47   coverage=00 0d
```

No `PF_TransferRect` call appeared in this opaque fill run, as expected.

## Native Changes

Implemented the text-local semitransparent fill behavior recovered from Ghidra:

- if fill alpha is `1..254`, render glyph coverage into a temp `Canvas` with local alpha `255`;
- transfer temp canvas into the destination with recovered integer source-over and the original fill opacity;
- this prevents double-applying semitransparent alpha when glyph coverage overlaps before transfer.

Also changed bitmap writes to compute one integer raster origin per glyph and then write half-open integer spans. With the current `round()` origin this preserves existing placement while matching the recovered span ABI shape.

## Validation

Passed:

```bash
python3 -m py_compile scripts/ae_trace_cooltype_text.py
cargo test -p text-engine rasterize -- --nocapture
cargo test -p render-core text -- --nocapture
cargo test -p render-cli all_manifest_cases_have_native_recipes -- --nocapture
cargo run -p render-cli -- conformance-pack \
  --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case GPH_010 \
  --out target/ae_agents/p2_text_temp_world_gate_20260507
```

Focused text gate stayed `ok=true` for all five cases:

```text
TXT_010  rgb_visible=14.070786  alpha=15.021648
TXT_020  rgb_visible=5.858233   alpha=6.578621
TXT_030  rgb_visible=6.502396   alpha=5.624135
TXT_040  rgb_visible=3.730953   alpha=3.835255
GPH_010  rgb_visible=1.687255   alpha=1.585812
```

## Remaining P2 Work

This closes the shared row/span instrumentation and semitransparent fill implementation, but not full P2 parity.

Next concrete targets:

1. Run the same `txt-are-spans` profile on stroke cases and confirm live `TXT_ARE_Render_8bpc_stroke_3d960` span order.
2. Generate a JSX case that actually sets fill alpha through the same AE text path, then confirm `TXT_IMPORT_PF_TransferRect_694f30` live arguments.
3. Replace the current TTF outline supersample mask with CoolType-compatible row coverage once BIB row generation/hinting fields are fully recovered.
4. Lock clip rounding against the traced half-open span coordinates using native row telemetry, not final PNG only.

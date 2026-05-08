# P2-B Minimal AE JSX/Render Traces - 2026-05-08

## Scope

Agent: P2-B, minimal AE JSX/render trace owner.

Allowed write zone used:

- `docs/phase_reports/P2_AGENT_MINIMAL_AE_TRACES_20260508.md`
- `fixtures/ae_probe_pack/p2_text_coverage_rows_probe/jsx/build_COV_W_single.jsx`
- `fixtures/ae_probe_pack/p2_text_coverage_rows_probe/jsx/build_COV_I_single.jsx`
- `fixtures/ae_probe_pack/p2_text_coverage_rows_probe/jsx/build_COV_O_single.jsx`

No core Rust files were touched. I did not restart or modify the AE API/node.

## Single-Case JSX Added

Added thin one-case entry scripts for coverage rows:

- `fixtures/ae_probe_pack/p2_text_coverage_rows_probe/jsx/build_COV_W_single.jsx`
- `fixtures/ae_probe_pack/p2_text_coverage_rows_probe/jsx/build_COV_I_single.jsx`
- `fixtures/ae_probe_pack/p2_text_coverage_rows_probe/jsx/build_COV_O_single.jsx`

Each script builds exactly one comp, one text layer, and one render queue item. They intentionally mirror the existing coverage rows probe settings:

- comp `256x256`, 8 bpc
- `Montserrat-BoldItalic` fallback `Arial-BoldMT`
- font size `96`
- white fill, no stroke
- centered glyph at `[128, 150]`

## Why Old Dense COV_W Had 1758 But New Runs Often Have 1 Span

The old dense trace:

```text
target/dynamic_tools_85/p2_shared_are_spans_covw_stride_20260507/COV_W.jsonl
```

Summary file reports:

```text
event_count: 1823
TXT_ARE_PixelWriter8_span_3b8c0: 1758
```

Direct JSONL recount shows this is enter+leave accounting. Actual `cooltype_hook_enter` calls:

```text
lines: 1823
hook enter events: 891
TXT_ARE_PixelWriter8_span_3b8c0 enter events: 879
```

The old dense run installed only the broad ARE hooks:

- `TXTp_DrawChar3_ARE_42110`
- `TXT_DrawChar_outline_core_42b80`
- `TXT_ARE_Render_8bpc_3c360`
- `TXT_ARE_Render_8bpc_fill_3d200`
- `TXT_ARE_Render_8bpc_stroke_3d960`
- `TXT_ARE_OutputComposite_8bpc_3de50`
- `TXT_ARE_PixelWriter8_span_3b8c0`
- `TXT_IMPORT_PF_TransferRect_694f30`
- `BEE_IMPORT_TXT_DrawChar_edf6b0`

The current `txt-are-spans` profile also installs hot in-function PixelWriter hooks:

- `TXT_ARE_PixelWriter8_type2_load_3ba1b`
- `TXT_ARE_PixelWriter8_type2_stride_mul_3ba2e`
- `TXT_ARE_PixelWriter8_type2_stride_add_3ba5b`
- `TXT_ARE_PixelWriter8_type2_span_count_3ba71`
- `TXT_ARE_PixelWriter8_type2_span_ready_3ba74`

Observed result: with these inline hooks, COV_W usually reaches the first type-2 span and then the dense span stream does not continue. The single-case COV_W run reproduced this, so the short trace is not caused by the old multi-case JSX.

## New COV_W Single Run - Stopped After AE Error

Command:

```bash
python3 scripts/ae_trace_cooltype_text.py \
  --ssh-host ae85 \
  --node http://85.239.48.31:8001 \
  --pack fixtures/ae_probe_pack/p2_text_coverage_rows_probe \
  --entry-script jsx/build_COV_W_single.jsx \
  --case COV_W \
  --duration 150 \
  --max-events 4200 \
  --hook-profile txt-are-spans \
  --out-dir target/dynamic_tools_85/p2_minimal_cov_w_single_txt_are_spans_20260508_001 \
  --allow-render-failure
```

Trace:

```text
target/dynamic_tools_85/p2_minimal_cov_w_single_txt_are_spans_20260508_001/COV_W.jsonl
target/dynamic_tools_85/p2_minimal_cov_w_single_txt_are_spans_20260508_001/summary.json
```

S3:

```text
bucket: f7cef916-job-artifacts
root: ae_dynamic_traces/cooltype_text/20260508_143208
trace: ae_dynamic_traces/cooltype_text/20260508_143208/COV_W.jsonl
render id: 037633495f7e4f469097183c8c53351a
```

Hook counts from direct recount:

```text
lines: 109
hook enter events: 13
TXT_DrawChar_outline_core_42b80: 5
BEE_IMPORT_TXT_DrawChar_edf6b0: 1
TXTp_DrawChar3_ARE_42110: 1
TXT_ARE_Render_8bpc_3c360: 1
TXT_ARE_Render_8bpc_fill_3d200: 1
TXT_ARE_OutputComposite_8bpc_3de50: 1
TXT_ARE_PixelWriter8_span_3b8c0: 1
TXT_ARE_PixelWriter8_type2_load_3ba1b: 1
TXT_ARE_PixelWriter8_type2_stride_mul_3ba2e: 1
```

Useful data caught:

- ARE object for opaque fill:
  - `fill_enabled_0x08 = 1`
  - `stroke_enabled_0x09 = 0`
  - `fill_stroke_order_0x0a = 1`
  - `clip_shorts = top 0, left 0, bottom 68, right 109`
  - matrix tail includes `[-9.055999755859375, 68]`
- Coverage plane at `3ba2e`:
  - width `109`
  - height `68`
  - `base_0x10 == base_0x28`
  - stride `0x30 = 112`

Stop condition:

```text
target/ae_remote/ae_trace_cooltype_COV_W_20260508_143208/extracted/ae_trace_cooltype_COV_W_20260508_143208/logs/aerender_pack.stdout.log
```

contains:

```text
aerender ERROR An existing connection was forcibly closed by the remote host.
: Unable to receive at line 505
```

The API status said succeeded, but output summary reported `tiff_count=0` and `png_count=0`. Per task rule, I stopped and did not run COV_I or COV_O.

ESCALATE_TO_ORCHESTRATOR: COV_W single + current inline `txt-are-spans` hook set appears unsafe/unstable on AE85. Do not continue COV_I/O runs with this hook set until orchestrator approves a safer hook split.

## Existing Useful Minimal Traces

These existing traces are still valuable and did not require new AE jobs from this pass.

### Dense COV_W Producer Baseline

```text
target/dynamic_tools_85/p2_shared_are_spans_covw_stride_20260507/COV_W.jsonl
target/dynamic_tools_85/p2_shared_are_spans_covw_stride_20260507/summary.json
```

Direct recount:

```text
TXT_ARE_PixelWriter8_span_3b8c0 enter events: 879
TXT_ARE_Render_8bpc_fill_3d200 enter events: 1
TXT_ARE_OutputComposite_8bpc_3de50 enter events: 1
```

This is the best dense COV_W producer trace. Use this for row topology. Interpret the reported `1758` as enter+leave, not unique span calls.

### Stroke Consumer/Producer

Dense stroke-only:

```text
target/dynamic_tools_85/p2_stroke_live_strokeonly_trace_20260508_001/STR_LIVE_STROKE_ONLY.jsonl
```

Direct recount:

```text
TXT_ARE_PixelWriter8_span_3b8c0 enter events: 953
TXT_ARE_Render_8bpc_stroke_3d960 enter events: 1
```

Stride/sample-byte focused stroke:

```text
target/dynamic_tools_85/p2_stroke_live_strokeonly_stride_trace_20260508_001/STR_LIVE_STROKE_ONLY.jsonl
target/dynamic_tools_85/p2_stroke_live_strokeonly_sample_byte_trace_20260508_001/STR_LIVE_STROKE_ONLY.jsonl
```

The sample-byte trace caught:

```text
TXT_ARE_PixelWriter8_span_3b8c0 enter events: 9
TXT_ARE_PixelWriter8_type2_sample_byte_3ba80 enter events: 31
```

Known useful row from prior parser:

```text
role: stroke
y: 7
start_x: 6
end_x: 37
actual_coverage_sample_hex:
22404040404040404040404040404040404040404040404040404040404008
```

### Semitransparent Fill Consumer

Dense alpha-fill:

```text
target/dynamic_tools_85/p2_transfill_live_whta128_trace_20260508_001/TRFLIVE_WHT_A128_FILL_OPACITY.jsonl
```

Direct recount:

```text
TXT_ARE_PixelWriter8_span_3b8c0 enter events: 288
TXT_ARE_Render_8bpc_fill_3d200 enter events: 1
```

Sample-byte alpha-fill:

```text
target/dynamic_tools_85/p2_transfill_live_whta128_sample_byte_trace_20260508_001/TRFLIVE_WHT_A128_FILL_OPACITY.jsonl
```

Direct recount:

```text
TXT_ARE_PixelWriter8_span_3b8c0 enter events: 1
TXT_ARE_PixelWriter8_type2_sample_byte_3ba80 enter events: 9
```

Known useful row from prior parser:

```text
role: fill
source_pixel_raw: 80ffffff
y: 0
start_x: 0
end_x: 10
actual_coverage_sample_hex: 5bd0d0d0d0d0d0d0d0
```

## Recommendation

For dense COV_W/COV_I/COV_O, use a basic ARE span hook profile that matches the 2026-05-07 dense run: `3b8c0` plus render envelope/import hooks only, without inline hooks at `3ba1b/3ba2e/3ba5b/3ba71/3ba74/3ba80`.

For byte ownership, use separate one-case byte probes with very small max-events, as already done for stroke and semitransparent fill. Do not combine dense span capture and inline PixelWriter instruction hooks in one COV run on AE85 until the aerender connection error is understood.

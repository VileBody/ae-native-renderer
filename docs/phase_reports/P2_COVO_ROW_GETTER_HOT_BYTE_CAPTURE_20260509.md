# P2 COV_O Row Getter + Hot Byte Capture

Date: 2026-05-09

## Scope

Close the missing authoritative `COV_O` evidence for P2 text raster:

- capture real `ARE_row_getter_8230` rows for the curved `O` glyph;
- compare those rows against native `ttf_outline_are_scanline_16x`;
- run a broader ARE sampler/hot-byte trace only far enough to decide whether a native formula change is justified.

## Inputs

Single-glyph JSX:

```text
fixtures/ae_probe_pack/p2_text_coverage_rows_probe/jsx/build_COV_O_single.jsx
```

Fixture properties:

```text
text:       O
font:       Montserrat-BoldItalic
font size:  96
comp:       256x256
```

## Captures

Direct row getter:

```text
target/dynamic_tools_85/p2_cov_o_are_row_getter_dense_20260509_001/COV_O.jsonl
target/ae_agents/p2_cov_o_are_row_getter_dense_20260509/analysis.json
```

Native render + compare:

```text
target/ae_agents/p2_cov_o_row_getter_dense_native_20260509/rendered/text_telemetry.jsonl
target/ae_agents/p2_cov_o_row_getter_dense_20260509/row_compare.json
```

Broad ARE sampler / hot-byte trace:

```text
target/dynamic_tools_85/p2_cov_o_are_sampler_hot_bytes_20260509_001/COV_O.jsonl
target/ae_agents/p2_cov_o_are_sampler_hot_bytes_20260509/analysis.json
```

The broad trace is intentionally not a tuning source by itself. It is a direction finder for the next focused static/dynamic target.

## Commands

```bash
python3 scripts/ae_trace_cooltype_text.py \
  --node http://85.239.48.31:8001 \
  --pack fixtures/ae_probe_pack/p2_text_coverage_rows_probe \
  --entry-script jsx/build_COV_O_single.jsx \
  --case COV_O \
  --hook-profile are-row-getter \
  --duration 180 \
  --max-events 5000 \
  --post-render-trace-s 2 \
  --out-dir target/dynamic_tools_85/p2_cov_o_are_row_getter_dense_20260509_001 \
  --live-tail

python3 scripts/analyze_are_sampler_trace.py \
  target/dynamic_tools_85/p2_cov_o_are_row_getter_dense_20260509_001/COV_O.jsonl \
  --out target/ae_agents/p2_cov_o_are_row_getter_dense_20260509/analysis.json \
  --events 200

cargo run -q -p render-cli -- render \
  --scene target/ae_agents/p2_cov_o_final_row_pass_20260508/scene.json \
  --out target/ae_agents/p2_cov_o_row_getter_dense_native_20260509/rendered

python3 scripts/compare_text_row_spans.py \
  --case COV_O \
  --ae-row-getter-analysis target/ae_agents/p2_cov_o_are_row_getter_dense_20260509/analysis.json \
  --native-text-telemetry target/ae_agents/p2_cov_o_row_getter_dense_native_20260509/rendered/text_telemetry.jsonl \
  --out target/ae_agents/p2_cov_o_row_getter_dense_20260509/row_compare.json
```

Broad sampler command:

```bash
python3 scripts/ae_trace_cooltype_text.py \
  --node http://85.239.48.31:8001 \
  --pack fixtures/ae_probe_pack/p2_text_coverage_rows_probe \
  --entry-script jsx/build_COV_O_single.jsx \
  --case COV_O \
  --hook-profile are-sampler \
  --duration 180 \
  --max-events 20000 \
  --post-render-trace-s 2 \
  --out-dir target/dynamic_tools_85/p2_cov_o_are_sampler_hot_bytes_20260509_001 \
  --live-tail
```

Note: the broad run produced a very large JSONL and the local wrapper had to be stopped after the AE job completed. The JSONL was still complete enough for `analyze_are_sampler_trace.py`.

## Row Getter Facts

`ARE_row_getter_8230` fired cleanly:

```text
row_getter events: 142
row_getter_rows:   437
merged ink rows:   113
ink extent:        75x71
```

The first rows are real coverage bytes, not PNG-derived pixels:

```text
y=0  x=35..48  02183244506060605b50412d14
y=1  x=30..54  144c82b0d9fbfffffffffffffffffffffffff4d0a1713403
y=2  x=26..57  013788d0fefffffffffffffffffffffffffffffffffffffffffffff1a9580a
```

The counter hole is explicit in the AE rows:

```text
y=14 left  x=9..39
y=14 right x=42..71
y=15 left  x=8..33
y=15 right x=48..72
y=16 left  x=7..31
y=16 right x=50..72
```

## Native Compare

Current native compare against the row getter:

```text
AE merged ink rows:      113
native rows:             107
normalized ink common:    97
AE-only row keys:          16
native-only row keys:      10
normalized ink ratio:   0.8584070796460177
coverage exact rows:       1 / 97
```

This is no longer an instrumentation blocker. It is a formula/substrate blocker.

Representative mismatch:

```text
y=3 x=24..59
AE:     1e82dffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff29828
native: 1e74d1f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0e48a28
```

Native also has rows with a low `0x10` bridge through the `O` counter where AE has separate left/right spans.

## Rejected Experiment

I tested the tempting "nonzero winding vs parity pairing" hypothesis locally. It was rejected:

```text
before normalized ink ratio: 0.8584070796460177
after parity experiment:    0.8407079646017699
```

No production code change was kept. The evidence says the issue is not a simple fill-rule swap.

## Broad Sampler Facts

The broad ARE sampler trace captured:

```text
ARE_edge_project_78e4:       13951
ARE_sampler_eval_row_75d0:    2213
ARE_raster_lazy_row_b7e0:     1925
ARE_sampler_prepare_76dc:      345
ARE_row_getter_8230:           145
ARE_edge_insert_sorted_a850:     9
ARE_edge_bounds_accumulate_b6c0: 3
```

Useful conclusion:

- AE row getter already gives the final authoritative row bytes for `COV_O`.
- Native drift remains below TXT producer and final PF blend, inside the BIB/ARE curve edge-list or incremental x-table path.
- Broad `are-sampler` is too noisy for direct formula changes. The next trace should be focused on the already identified cubic follow-ups:
  - `ARE+0xfc04`
  - `ARE+0x125b8`
  - `ARE+0x1268c`
  - event/x-table nodes around rows with AE split spans and native `0x10` bridges.

## Status

P2 is now:

```text
COV_W line path: accepted / parity locked
COV_O row getter: captured / testable
COV_O native: instrumented, not parity
COV_O blocker: ARE/BIB cubic edge-list or incremental x-table parity
```

Next implementation rule:

```text
Do not tune COV_O by final PNG metrics.
Do not swap fill rules without row evidence.
Use focused Ghidra/Frida on the cubic incremental x-table path, then patch native.
```

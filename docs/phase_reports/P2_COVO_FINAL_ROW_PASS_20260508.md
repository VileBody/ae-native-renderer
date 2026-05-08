# P2 COV_O Final Row Pass - 2026-05-08

## Scope

Bounded final P2 pass for `COV_O` row evidence after the TXT producer,
ARE row getter, and ARE cubic scanline passes. The write scope was kept to
this report; no rasterizer or compare-script change was made because the
available COV_O evidence does not include authoritative dynamic row-getter
coverage rows.

Starting references:

- `docs/phase_reports/P2_TEXT_COVERAGE_BYTE_ADDRESS_TRACE_20260508.md`
- `docs/phase_reports/P2_ARE_CUBIC_SCANLINE_STATIC_20260508.md`
- `docs/phase_reports/P2_ARE_ROW_GETTER_DYNAMIC_20260508.md`
- current `crates/text-engine/src/rasterize.rs`

## Commands

```text
cargo run -q -p render-cli -- render --scene target/ae_agents/p2_cov_o_final_row_pass_20260508/scene.json --out target/ae_agents/p2_cov_o_final_row_pass_20260508/rendered

python3 scripts/compare_text_row_spans.py --case COV_O --ae-row-getter-analysis target/ae_agents/p2_cov_o_are_row_getter_existing_20260508/analysis.json --out target/ae_agents/p2_cov_o_final_row_pass_20260508/row_getter_only.json

python3 scripts/compare_text_row_spans.py --case COV_O --ae-jsonl target/dynamic_tools_85/p2_cov_rows_safe_COV_O_20260508/COV_O.jsonl --native-text-telemetry target/ae_agents/p2_cov_o_final_row_pass_20260508/rendered/text_telemetry.jsonl --out target/ae_agents/p2_cov_o_final_row_pass_20260508/row_compare_txt_are_spans.json

python3 scripts/compare_text_row_spans.py --case COV_O --ae-row-getter-analysis target/ae_agents/p2_cov_o_are_row_getter_existing_20260508/analysis.json --native-text-telemetry target/ae_agents/p2_cov_o_final_row_pass_20260508/rendered/text_telemetry.jsonl --out target/ae_agents/p2_cov_o_final_row_pass_20260508/row_compare_row_getter.json

cargo test -q -p text-engine recovered_txt_curve_producer_matches_cov_o_prefix
cargo test -q -p text-engine recovered_txt_are_origin_and_subrow_phase_match_cov_w_trace
cargo test -q -p text-engine

cargo run -q -p render-cli -- conformance-pack --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case GPH_010 --out target/ae_agents/p2_covo_final_row_pass_gate_20260508
```

## Artifacts

```text
target/ae_agents/p2_cov_o_final_row_pass_20260508/
target/ae_agents/p2_cov_o_final_row_pass_20260508/rendered/text_telemetry.jsonl
target/ae_agents/p2_cov_o_final_row_pass_20260508/row_getter_only.json
target/ae_agents/p2_cov_o_final_row_pass_20260508/row_compare_txt_are_spans.json
target/ae_agents/p2_cov_o_final_row_pass_20260508/row_compare_row_getter.json
target/ae_agents/p2_covo_final_row_pass_gate_20260508/report.json
```

Existing evidence inputs:

```text
target/dynamic_tools_85/p2_cov_rows_safe_COV_O_20260508/COV_O.jsonl
target/ae_agents/p2_cov_o_are_row_getter_existing_20260508/analysis.json
```

## COV_O Row Metrics

Direct row-getter analysis is missing COV_O rows:

```text
ARE row getter rows: 0
ARE row getter ink rows: 0
compare status: instrumentation-incomplete
```

The older TXT/ARE span stream is dense enough for a topology sanity check, but
it is not the authoritative `ARE_row_getter_8230` / `3ba5b` / `3ba80` byte
stream used by the COV_W lock.

```text
AE TXT/ARE spans:       493
AE type2 rows:          226
AE merged ink rows:     113
native rows:            107
normalized extents:     AE 75x71, native 75x71
normalized type2 shape: 2 / 226 common
normalized ink shape:   97 / 113 common
ink byte exact:         0 / 97
acceptance:             substrate-mismatch
```

The topology shape is encouraging enough to show the current ARE-shaped native
rows are in the same broad footprint, but the byte comparison is not a safe
tuning target: AE bytes come from the old span stream, not dynamic row-getter
coverage rows.

## Focused Gate

The focused TXT/GPH conformance gate stayed green and matched the previous
ARE cubic gate metrics exactly because no rasterizer change was made.

```text
case     before visible mean   after visible mean
TXT_010  12.531241           12.531241
TXT_020   6.410895            6.410895
TXT_030   7.073936            7.073936
TXT_040   3.671418            3.671418
GPH_010   4.002211            4.002211
```

`cargo test -q -p text-engine` passed: 29 tests.

## Decision

Stop: blocked by missing dynamic COV_O row getter.

No large jump was found in this pass, and no evidence-backed implementation
change was available. The correct continuation is to capture authoritative
`COV_O` rows through the same direct row-getter or hot coverage-byte route that
closed `COV_W`; until then, changing `rasterize.rs` would be metric guessing.

# P2 Documentation Scribe — Coverage Producer Round (Draft) — 2026-05-08

## Scope

Собираем один сводный draft-отчёт для этапа P2 coverage-пайплайна по фактам из:

- `docs/phase_reports/P2_TEXT_COVERAGE_BYTE_ADDRESS_TRACE_20260508.md`
- `DOCUMENTATION_DELEGATION.md`

Фокус: статическая и динамическая верификация источника coverage-байтов для `TXT_ARE_PixelWriter8`, и состояние верификации относительно существующего native diff.

Текущая цель: подготовить структуру фактов для оркестратора без интерпретации и без изменения статуса roadmap.

## Known Locked Facts

- `DOCUMENTATION_DELEGATION.md` закрепляет роль scribe как markdown-only редакции без кодовых изменений.
- `DOCUMENTATION_DELEGATION.md` указывает: не менять roadmap/status без явного решения оркестратора.
- `docs/phase_reports/P2_TEXT_COVERAGE_BYTE_ADDRESS_TRACE_20260508.md` фиксирует, что `TXT_ARE_PixelWriter8` для авторитетного чтения coverage-байт использует точку `TXT.dll+0x3ba80`.

## New Static Evidence

- В `docs/phase_reports/P2_TEXT_COVERAGE_BYTE_ADDRESS_TRACE_20260508.md` перечислены релевантные static-оффсеты:
  - `TXT.dll+0x3ba2e` (coverage-plane stride multiplication point),
  - `TXT.dll+0x3ba5b` (coverage-row pointer post-calc),
  - `TXT.dll+0x3ba80` (actual coverage-byte read point),
  - `TXT.dll+0x3ba71` / `0x3ba74` (установлены, но не срабатывали как Interceptor enter в фокусе трасс).
- В этой же записи указано, что прежний якорь `0x3ba1b` полезен, но “ранний”: срабатывает до того, как `RDX` становится coverage plane.
- Из `P2_TEXT_COVERAGE_BYTE_ADDRESS_TRACE_20260508.md` также перенесено: `TXT_ARE_PixelWriter8_span_3b8c0` включает статические инструкции (`mov rax...[rdx+0x10]`, `mov rdx,[rax+0x8]`, `imul ... [rdx+0x20]`, `add ... [rdx+0x10]`, `call 0x18003c980`).

## New Dynamic Evidence

- Trace pack `target/dynamic_tools_85/p2_stroke_live_strokeonly_sample_byte_trace_20260508_001/STR_LIVE_STROKE_ONLY.jsonl` обработан через `target/ae_agents/p2_row_compare_stroke_sample_byte_20260508/ae_rows.json`.
- Trace pack `target/dynamic_tools_85/p2_transfill_live_whta128_sample_byte_trace_20260508_001/TRFLIVE_WHT_A128_FILL_OPACITY.jsonl` обработан через `target/ae_agents/p2_row_compare_transfill_sample_byte_20260508/ae_rows.json`.
- Trace pack `target/dynamic_tools_85/p2_cov_w_sample_byte_trace_20260508_001/COV_W.jsonl` обработан через `target/ae_agents/p2_row_compare_covw_sample_byte_20260508/ae_rows.json`.
- Trace pack `target/dynamic_tools_85/p2_cov_w_span_count_trace_20260508_001/COV_W.jsonl` обработан через:
  - `target/ae_agents/p2_row_compare_covw_span_count_20260508/ae_rows.json`
  - `target/ae_agents/p2_row_compare_stroke_stride_fallback_20260508/ae_rows.json`
- Для dense legacy trace упомянуты:
  - `target/dynamic_tools_85/p2_shared_are_spans_covw_stride_20260507/COV_W.jsonl`
  - `target/ae_agents/p2_cov_w_native_scene_20260508/rendered/text_telemetry.jsonl`
  - `target/ae_agents/p2_row_compare_covw_dense_native_20260508/row_compare.json`
- `P2_TEXT_COVERAGE_BYTE_ADDRESS_TRACE_20260508.md` содержит конкретные hook-counts и примеры `actual_coverage_sample_hex`; эти значения уже зафиксированы в источнике и не дублируются здесь как новые выводы.

## Native Diff Summary

- `TBD` (подтверждение формулировки и выводов по этому блоку ждёт оркестратора).
- Зафиксировано в источнике:
  - авторитетным для сравнения bytes stream считаются данные из `3ba80`,
  - `3ba5b` использован как масштабируемый row-level эквивалент для уменьшения нагрузки на Frida,
  - merged `type1/type2` сравнение даёт ближе соответствие row topology, чем узкая только `type2` выборка (`P2_TEXT_COVERAGE_BYTE_ADDRESS_TRACE_20260508.md`).

## Open Questions

- `TBD` (критерий, какой именно `native diff` вариант считать закрывающим шагом для этой итерации, и нужно ли фиксировать это как “ready” в статическом статусе).
- `TBD` (нужно ли дополнять доказанную логику edge-case’ами для многострочных/мульти-тип row-структур в этой же фазе).

## Next Actions

- Оставить и передать оркестратору все числовые наблюдения из источника без редактуры формулировок.
- `TBD` (добавить/подтвердить, какие строки отчёта считаются финальными для статуса P2).
- `TBD` (запросить финальное заключение по “next diff step” и статусу roadmap после вставки агрегированных фактов).

## Artifacts

- `docs/phase_reports/P2_TEXT_COVERAGE_BYTE_ADDRESS_TRACE_20260508.md` (source evidence)
- `DOCUMENTATION_DELEGATION.md` (coordination constraints)
- `target/dynamic_tools_85/p2_stroke_live_strokeonly_sample_byte_trace_20260508_001/STR_LIVE_STROKE_ONLY.jsonl`
- `target/ae_agents/p2_row_compare_stroke_sample_byte_20260508/ae_rows.json`
- `target/dynamic_tools_85/p2_transfill_live_whta128_sample_byte_trace_20260508_001/TRFLIVE_WHT_A128_FILL_OPACITY.jsonl`
- `target/ae_agents/p2_row_compare_transfill_sample_byte_20260508/ae_rows.json`
- `target/dynamic_tools_85/p2_cov_w_sample_byte_trace_20260508_001/COV_W.jsonl`
- `target/ae_agents/p2_row_compare_covw_sample_byte_20260508/ae_rows.json`
- `target/dynamic_tools_85/p2_cov_w_span_count_trace_20260508_001/COV_W.jsonl`
- `target/ae_agents/p2_row_compare_covw_span_count_20260508/ae_rows.json`
- `target/ae_agents/p2_row_compare_stroke_stride_fallback_20260508/ae_rows.json`
- `target/dynamic_tools_85/p2_shared_are_spans_covw_stride_20260507/COV_W.jsonl`
- `target/ae_agents/p2_cov_w_native_scene_20260508/rendered/text_telemetry.jsonl`
- `target/ae_agents/p2_row_compare_covw_dense_native_20260508/row_compare.json`
- `target/ae_agents/p2_row_compare_stroke_stride_fallback_20260508/ae_rows.json`

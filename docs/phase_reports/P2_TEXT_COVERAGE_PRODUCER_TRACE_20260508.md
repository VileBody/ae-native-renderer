# P2 Text Coverage Producer Trace — 2026-05-08

Doc-scribe: markdown-only reporter for P2 coverage producer work.

## Scope

- Зафиксировать итог последнего producer-ряда трассировки покрытия `TXT_ARE_PixelWriter8` (round 4).
- Использованы только файлы артефактов и сводки, без правок runtime/кода.
- Исходные артефакты:  
  - `target/dynamic_tools_85/p2_text_producer_covw_r4_20260508/COV_W.jsonl`  
  - `target/dynamic_tools_85/p2_text_producer_covw_r4_20260508/summary.json`

## Runs

- Базовый/успешный render:
  - `target/ae_remote/ae_trace_cooltype_COV_W_20260508_145120/outputs/ae_trace_cooltype_COV_W_20260508_145120_outputs.zip`
  - TIFF-рендеров: `6`
- Failed/learning rounds:
  - `r1`: `target/dynamic_tools_85/p2_text_producer_covw_20260508` — hooks resolved BIB съели budget.
  - `r2` + `r3`: `target/dynamic_tools_85/p2_text_producer_covw_r2_20260508` — вероятный краш aerender на mid-instruction callsite hooks:  
    `An existing connection was forcibly closed by the remote host`.
- Stable fix adopted for r4:
  - исключены mid-instruction callsite hooks и resolved BIB/ARE hooks,
  - оставлены function-entry hooks + безопасный entry для `PixelWriter`.

## Stable Trace Result

- Всего событий: `1960`.
- `PixelWriter8`:
  - `enter`: `879`
  - `leave`: `879`
- Спановые типы:
  - `type2 = 409`
  - `type0 = 263`
  - `type1 = 207`
- Rows:
  - `68` строк
  - `y` диапазон `0..67`
- Coverage object (закрыт потребитель/plane ABI):
  - `row_getter`: `ARE.dll+0x8230`
  - `plane.width = 109`
  - `plane.height = 68`
  - `plane.stride = 112`
  - базовые указатели plane захвачены.
- `DAT_18087f778` / `DAT_18087f780` в snapshot’ах: `0` и остаются неактивными.
  Не трактовать как подтверждённые активные resolved producer.
- PathBuilder W outline:
  - count `18`
  - команды: `[0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,3]`
  - координаты есть в summary/log; полные выборки оставить в `jsonl`.
- Первые `type2` spanы:
  - `y=0, x=0..16`, bytes: `2440404040404040404040404040403c`
  - `y=0, x=46..62`, bytes: `013e4040404040404040404040404024`
  - `y=0, x=92..109`, bytes: `1e40404040404040404040404040404004`

## Important Guardrail

- `DAT_18087f778` и `DAT_18087f780` остаются `0` в текущих снапшотах;
  их нельзя считать активным покрытием producer только на основании прошлого плана.
- Надёжное чтение байтов покрытия зафиксировано через safe trace-путь (post-constructor),
  а не через speculative callsite-патчинг.

## Extracted Facts

- Для r1/r2/r3 стабильная проблема: неустойчивые hook-вставки и/или резолв BIB-слоя мешают стабильному полному проходу.
- `TXT_ARE_PixelWriter8` трасса r4 подтверждает правильную нагрузку span payload’ов и row ABI.
- `row_getter ARE.dll+0x8230` используется как потребительская точка доступа к строкам span-ячейкам.
- Продукционный payload байтов покрытий подтверждён в sample-трэйсах и агрегирован в `COV_W.jsonl`.

## Interpretation

- Мы закрыли consumer/plane ABI и получили authoritative byte stream через безопасный trace.
- Дальше для покрытия producer остаётся «ниже» BIB/ARE уровня:
  - первично: static/dynamic трассировка `ARE.dll+0x8230` row getter;
  - далее: конструкторы coverage object в BIB/ARE через function-boundary hooks/TTD;
  - не через mid-instruction callsite patching.

## Next Actions

- Зафиксировать r4 как базовый стабильный producer-набросок покрытия в связке с `summary.json`.
- Продолжить целевые hooks по function boundary вокруг:
  - `ARE.dll+0x8230` (row getter),
  - BIB/ARE coverage object constructors,
  - и связки, ведущей к заполнению plane байтов.
- Для `next static/dynamic target` использовать именно вышеуказанные границы, избегая callsite-патчей.

## Artifact Links

- `docs/phase_reports/P2_TEXT_COVERAGE_PRODUCER_TRACE_20260508.md`
- `target/dynamic_tools_85/p2_text_producer_covw_r4_20260508/COV_W.jsonl`
- `target/dynamic_tools_85/p2_text_producer_covw_r4_20260508/summary.json`
- `target/ae_remote/ae_trace_cooltype_COV_W_20260508_145120/outputs/ae_trace_cooltype_COV_W_20260508_145120_outputs.zip`
- `target/dynamic_tools_85/p2_text_producer_covw_20260508`
- `target/dynamic_tools_85/p2_text_producer_covw_r2_20260508`

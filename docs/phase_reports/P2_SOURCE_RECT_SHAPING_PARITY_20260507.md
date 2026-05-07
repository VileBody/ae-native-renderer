# P2 SourceRect Shaping Parity

Date: 2026-05-07

## Goal

Close the remaining M05/P2 layout blocker where native text passport rows used
raw glyph metrics while AE references were generated with
`sourceRectAtTime(prefix)` row partitioning.

## Static Findings

Ghidra analysis moved the relevant path from exported CoolType text APIs into
`TXT.dll`:

- `TXT_GridChar::GetGlyphMetricsPlus` owns glyph id, advance, and outline bbox
  cache at `0x3f8/0x418` and alternate-cache slots at `0x430/0x450`.
- `TXT_GridChar::GetCharacterAlignmentBounds` builds character alignment
  bounds from `GetGlyphMetricsPlus` advance plus virtual-font ascent/descent
  helpers.
- `FUN_180214120` wraps `CoreBBoxBatch`.
- `FUN_180214a00` wraps `CoreWidthsBatch`.

Static bundles:

- `target/reverse/predecoded/20260507_185422_txt_text_layout_callers_20260507`
- `target/reverse/predecoded/20260507_185616_txt_gridchar_metric_neighbors_20260507`

## Dynamic Findings

Frida probe on AE85 confirmed the static call chain during `sourceRectAtTime`:

```text
Scripting.aex -> TXT.dll -> TXT_GridChar -> CoreBBoxBatch/CoreWidthsBatch
```

Important runtime counts from `TXT_010`:

- `TXT_GridChar_GetGlyphMetricsPlus`: 444 events
- `TXT_GridChar_GetCharacterAlignmentBounds`: 204 events
- `TXT_FUN_sourceRect_batch_bboxes_214120`: 297 events
- `TXT_FUN_sourceRect_batch_widths_214a00`: 245 events
- `CoreBBoxBatch`: 511 events
- `CoreWidthsBatch`: 733 events

The remote pack timed out waiting for `pack_status.json`, but the trace hit the
event cap and was usable:

- `target/dynamic_tools_85/cooltype_txt_gridchar_probe_20260507/TXT_010.jsonl`
- `target/dynamic_tools_85/cooltype_txt_gridchar_probe_20260507/summary.json`

## Implementation

Native text telemetry now mirrors the AE reference contract:

1. Shape each line/prefix with `rustybuzz` so GPOS kerning is included.
2. Compute sourceRect width/top/height from shaped glyph outline bbox union.
3. Emit glyph passport rows as:

```text
before = sourceRect(prefix_before).width
through = sourceRect(prefix_through).width
advance = max(0, through - before)
x = centered_line_start + before
y = baseline + sourceRect(full_line).top
height = sourceRect(full_line).height
```

This keeps runtime raster placement on the previous fontdue path; only passport
telemetry/sourceRect rows changed.

## Results

Focused gate:

```text
target/ae_agents/p2_source_rect_shaping_gate_20260507
```

Text passport:

| Case | Frames | Mismatches | Max Delta |
| --- | ---: | ---: | ---: |
| `TXT_010` | 7 | 0 | `0.0000735521` |
| `TXT_020` | 7 | 0 | `0.0000678003` |
| `TXT_040` | 8 | 0 | `0.0` |
| `GPH_010` | 0 compared | 0 | no AE text passport refs |

Master conformance:

```text
target/ae_agents/p2_source_rect_shaping_master_20260507
cases=42 ok=true failures=0
```

## Status

Closed for P2 text passport/sourceRect row partition:

- GPOS-aware prefix widths
- whitespace folding into next visible row
- cumulative glyph rows
- AE-style line top/height from full-line sourceRect
- numeric passport comparison for `0` vs `0.0`

Still separate from P2 layout passport:

- exact CoolType raster coverage/pixel fill for final PNG text
- deeper `GPH_010` text passport references

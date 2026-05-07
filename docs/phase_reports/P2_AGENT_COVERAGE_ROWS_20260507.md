# P2-A Coverage Rows Static Brief

Date: 2026-05-07

Scope: CoolType/BIB literal coverage rows/spans before TXT ARE PF_World writer.
Owned artifacts only; no shared native code or shared `GHIDRA_PREDECODE_TASKS.json` edits.

## Static Evidence Read

Existing reports:

- `docs/phase_reports/P2_TXT_DRAWCHAR_ARE_PIXEL_WRITER_20260507.md`
- `docs/phase_reports/P2_TTF_OUTLINE_TEXT_COVERAGE_20260507.md`
- `docs/phase_reports/P2_BEE_TEXT_RASTER_FILL_PATH_20260507.md`

Existing Ghidra bundles:

- `target/reverse/predecoded/20260507_223624_txt_drawchar_are_vtable`
- `target/reverse/predecoded/20260507_223741_txt_drawchar_are_helpers`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers`
- `target/reverse/predecoded/20260507_224844_txt_drawchar_are_pixel_writers`

Existing dynamic traces:

- `target/dynamic_tools_85/txt_drawchar_ras020_are_vtable_20260507`
- `target/dynamic_tools_85/txt_drawchar_ras020_dynamic_ptrs_20260507`
- `target/dynamic_tools_85/txt_drawchar_ras020_path_ptrs_20260507`

## Recovered Chain

1. `TXT_DrawOutlinePlayerARE` records path commands into two parallel buffers.
   The vtable methods are:
   - `0x1800404e0`: move command, writes two f32 coords and command `0`.
   - `0x180040430`: line command, writes two f32 coords and command `1`.
   - `0x18003f050`: curve command, writes three f32 coord pairs and command `2` per point.
   - `0x18003ef60`: close command, re-emits current point and command `3`.
   - `0x18003f310`: render/flush dispatch by PF depth.

2. `0x18003f310` dispatches to the active ARE path:
   - 8 bpc: `0x18003c360`
   - 16 bpc: `0x18003c140`
   - f32: local branch in `0x18003f310`

3. 8 bpc fill path:
   - `0x18003c360` prepares PF_Pixel8 fill/stroke colors and calls `0x18003d200` for fill.
   - `0x18003d200` calls `0x180040580` with path point count, coord buffer, and command buffer.
   - `0x180040580` calls `DAT_18087f778(&out_path, count, coords, commands, ...)`, then retains the returned path handle through `DAT_18087c038`.
   - For captured `RAS_020`, prior report says `DAT_18087f778`/`DAT_18087f780` were zero in final fill-only run, so this is a version-sensitive helper path.

4. Path object factory:
   - `0x18003d200` passes the path handle to `0x18003fd40(&DAT_180841f48, path, DAT_180841f88)`.
   - `0x18003fd40` uses `DAT_18087bd40 + 8 + path_id` as the BIB table lookup, caches objects in a critical-section-protected list, and returns `object + 0x38` only if `object+0x34` is true.
   - The cached object layout inferred from `0x18003fd40`/`0x1800408d0`:
     - `+0x20`: BIB path object from `DAT_18087beb8`
     - `+0x28`: BIB path vtable/object API from `DAT_18087bec8`
     - `+0x30`: TXT/BIB version stamp
     - `+0x34`: valid flag set by callback
     - `+0x38`: returned client-facing method block

5. BIB object refresh:
   - `0x1800408d0(object, path_id, callback)` calls:
     - `DAT_18087beb8(lVar4)` -> BIB path object
     - `DAT_18087bec0(old)` -> release old BIB path object
     - `DAT_18087bec8(new)` -> vtable/API pointer
     - `callback(path_id, 1, object+0x38)` -> fills returned method block; bool result stored at `+0x34`.
   - Existing dynamic pointer evidence maps these to:
     - `DAT_18087beb8` -> `BIB.dll + 0x18150`
     - `DAT_18087bec0` -> `BIB.dll + 0x18130`
     - `DAT_18087bec8` -> `BIB.dll + 0x18140`
     - `DAT_18087c038` -> `BIB.dll + 0x0a5f0`
     - `DAT_18087c040` -> `BIB.dll + 0x0b080`

6. Coverage object creation:
   - `0x18003d200` calls `(*(code *)*path_method_block)(&coverage_obj, path_handle)`.
   - Return value is checked as BIB-style status.
   - `coverage_obj` is then passed implicitly to `0x18003de50` by placing it at `param_2 + 0x10` in the stack tuple.

7. Row/span iterator:
   - `0x18003de50(param_1, &path_tuple, src_color, PF_World)` loops y from `*(short *)(param_1+0x60)` to `*(short *)(param_1+0x64)`.
   - It skips alternating parity rows when `(y & 1) == *(uint *)(param_1+0x1c)`.
   - It calls row getter:
     - `row = (*(code **)(*(coverage_obj)+0x10))(*(coverage_obj+0x18), y)`
   - It iterates 8-byte row cells:
     - `row[0]`: span type/mode
     - `row[1]`: end x
     - start x is the current cursor, initialized from `*(short *)(param_1+0x62)`.
   - For each non-empty interval `[start_x, min(row_end_x, clip_right))`, it calls `0x18003b8c0`.

8. Pixel/span writer:
   - `0x18003b8c0(param_1, path_tuple, span_type, y, start_x, end_x, src_color, PF_World)`.
   - `span_type == 1`: full coverage span. If source alpha is `255`, it copies the source PF_Pixel8 dword directly; otherwise it calls full blend `0x18003cb10`.
   - `span_type == 2`: per-pixel coverage span. It loads coverage bytes from:
     - `coverage_base = *(coverage_obj+0x10)->? + 0x10`
     - `coverage_stride = *(coverage_obj+0x10)->? + 0x20`
     - address formula in decompile:
       `coverage_ptr = base + (y - top) * stride + (x - left)`
   - Each byte is passed to `0x18003c980(src, 0, coverage, dst)`.
   - `0x18003c980` implements source-over with effective alpha `src_alpha * coverage / 255` using AE byte rounding.

## Suspected Coverage Structures

`coverage_obj` is a small object returned through the BIB path method block:

```text
0x00  unknown / likely vtable or header
0x10  row getter function pointer holder, used by 0x18003de50
0x18  row getter context
```

`row` is an array of 8-byte cells, consumed until current x reaches clip right:

```text
struct AreCoverageSpanCell {
  int32_t span_type;  // 1 full span, 2 byte coverage span
  int32_t end_x;      // exclusive x, clipped by caller
};
```

For `span_type == 2`, the byte coverage plane appears reachable from `*(path_tuple+0x10)` via the object used by `0x18003b8c0`, with signed top/left from ARE object offsets `+0x60/+0x62`.

## Unanswered Questions

- Exact `coverage_obj` concrete type and ownership/release function.
- Exact member offsets for coverage byte plane base/stride inside the object behind `*(path_tuple+0x10)`.
- Whether row getter always returns monotonically increasing `end_x` cells and whether a sentinel row cell exists after clip right.
- Whether antialias rows are one byte per pixel, subpixel-expanded, or already collapsed to 8-bit alpha.
- Whether stroke and f32/16 bpc reuse the same row/span ABI with only different blend callbacks.

## Exact Dynamic Evidence Needed

Minimum useful trace should hook one small glyph (`COV_W` or `COV_I`) at:

- `TXT.dll + 0x3de50`: on entry, dump ARE clips `+0x60/+0x62/+0x64/+0x66`, the `coverage_obj` pointer at `path_tuple+0x10`, row getter pointer/context at `coverage_obj+0x10/+0x18`.
- The indirect row getter: for the first 8-16 y rows, dump returned row pointer and the first 12 `{type,end_x}` cells.
- `TXT.dll + 0x3b8c0`: sample calls with `span_type`, `y`, `start_x`, `end_x`, source PF_Pixel8, PF_World base/rowbytes.
- For `span_type == 2`, dump first 16 coverage bytes read for the span.

This is static/dynamic trace/buffer evidence only. No metric fitting.

## Dynamic Attempt

Created tiny probe pack:

- `fixtures/ae_probe_pack/p2_text_coverage_rows_probe/manifest.json`
- `fixtures/ae_probe_pack/p2_text_coverage_rows_probe/jsx/build_p2_text_coverage_rows_probe_project.jsx`

Cases in pack: `COV_W`, `COV_I`, `COV_O`; one render/trace attempt was run only for `COV_W`.

Command:

```sh
python3 target/ae_agents/p2_coverage_rows_20260507_231636/ae_trace_cooltype_text_p2_cov.py \
  --pack fixtures/ae_probe_pack/p2_text_coverage_rows_probe \
  --entry-script jsx/build_p2_text_coverage_rows_probe_project.jsx \
  --case COV_W \
  --hook-profile txt-drawchar \
  --duration 120 \
  --max-events 900 \
  --node http://85.239.48.31:8001 \
  --prefix ae_dynamic_traces/p2_coverage_rows \
  --out-dir target/dynamic_tools_85/p2_coverage_rows_20260507_231636 \
  --allow-render-failure
```

Job/render:

- job id: `p2_cov_rows_20260507_231821_COV_W`
- render id: `a5a7e6e992db49569292828ac0545bd9`
- status: succeeded
- output zip: `target/ae_remote/p2_cov_rows_20260507_231821_COV_W/outputs/p2_cov_rows_20260507_231821_COV_W_outputs.zip`
- trace log: `target/dynamic_tools_85/p2_coverage_rows_20260507_231636/COV_W.jsonl`
- trace summary: `target/dynamic_tools_85/p2_coverage_rows_20260507_231636/summary.json`
- trace S3 key: `ae_dynamic_traces/p2_coverage_rows/20260507_231821/COV_W.jsonl`

Trace result:

```text
event_count: 284
TXT_Font_HaveOutlines_47e20:     52
TXT_PlayCharOutlines:            44
TXT_PlayCharOutlines_impl:       44
TXT_DrawChar_outline_core_42b80: 52
BEE_IMPORT_TXT_DrawChar_edf6b0:   8
TXTp_DrawChar3_ARE_42110:         8
```

Dynamic pointer snapshot again confirmed the BIB bridge in initialized `AfterFX.exe`:

```text
DAT_18087c038 -> BIB.dll + 0x0a5f0
DAT_18087c040 -> BIB.dll + 0x0b080
DAT_18087beb8 -> BIB.dll + 0x18150
DAT_18087bec0 -> BIB.dll + 0x18130
DAT_18087bec8 -> BIB.dll + 0x18140
DAT_18087f778 -> 0
DAT_18087f780 -> 0
```

One stale `AfterFX.com` attach produced
`VirtualAllocEx returned 0x00000005`, but the render process still produced
TXT/BEE hook events and the render succeeded.

Limit: this run used the existing shared `txt-drawchar` profile, so it
confirmed the tiny `COV_W` DrawChar/outline route but did not dump the literal
`0x18003de50` row getter or `0x18003b8c0` span calls. No second AE attempt was
run.

## Proposed Patch, Not Applied

Add an owned or orchestrator-approved trace profile for `TXT.dll` offsets:

- `0x3de50`: on enter, dump `param_1+0x60/+0x62/+0x64/+0x66`, `path_tuple+0x10`,
  `coverage_obj+0x10/+0x18`, and row getter pointer.
- row getter indirect target: dump first rows as `{y,row_ptr,cells:[{type,end_x}]}`.
- `0x3b8c0`: dump `{span_type,y,start_x,end_x,src_color,pf_world_base,rowbytes}`.
- for `span_type == 2`: dump the first coverage bytes that feed `0x3c980`.

This should be implemented in an owned copy first, then promoted to shared
trace tooling only with orchestrator approval.

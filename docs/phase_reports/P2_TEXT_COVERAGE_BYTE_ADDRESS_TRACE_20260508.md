# P2 Text Coverage Byte Address Trace - 2026-05-08

## Goal

Close the next P2 text-raster blocker after row/stroke/alpha live lock:
verify the exact AE coverage-byte address used by `TXT_ARE_PixelWriter8`
instead of relying on heuristic plane snapshots.

## Static Anchor

Local `TXT.dll` disassembly for `TXT_ARE_PixelWriter8_span_3b8c0`:

```asm
18003ba12  mov     rax, qword ptr [rdx + 0x10] ; coverage object
18003ba1b  mov     rdx, qword ptr [rax + 0x8]  ; coverage plane
18003ba2e  imul    rbx, qword ptr [rdx + 0x20] ; row stride in hot formula
18003ba42  add     rbx, qword ptr [rdx + 0x10] ; coverage base
18003ba5b  add     rdi, qword ptr [rdx + 0x20] ; output PF rowbytes
18003ba80  movzx   r8d, byte ptr [rbx]         ; actual coverage byte
18003ba90  call    0x18003c980                 ; covered pixel blend
```

The earlier `3ba1b` target was useful but incomplete: it fires before `RDX`
becomes the coverage plane. The actionable dynamic hooks are now:

- `TXT.dll+0x3ba2e`: coverage-plane stride multiplication point.
- `TXT.dll+0x3ba5b`: output PF-world rowbytes point.
- `TXT.dll+0x3ba80`: actual coverage-byte read point.

## Live Trace

Trace:

```text
target/dynamic_tools_85/p2_stroke_live_strokeonly_sample_byte_trace_20260508_001/STR_LIVE_STROKE_ONLY.jsonl
```

Parser output:

```text
target/ae_agents/p2_row_compare_stroke_sample_byte_20260508/ae_rows.json
```

Hook counts:

```text
TXT_ARE_PixelWriter8_span_3b8c0:              9
TXT_ARE_PixelWriter8_type2_load_3ba1b:        1
TXT_ARE_PixelWriter8_type2_stride_mul_3ba2e:  1
TXT_ARE_PixelWriter8_type2_stride_add_3ba5b:  1
TXT_ARE_PixelWriter8_type2_sample_byte_3ba80: 31
```

Observed type-2 stroke row:

```json
{
  "y": 7,
  "start_x": 6,
  "end_x": 37,
  "coverage_len": 31,
  "coverage_stride_source": "0x30",
  "coverage_sample_hex": "00001900000023000000c07f1b0862010000f045120862010000c0245d1062",
  "actual_coverage_sample_hex": "22404040404040404040404040404040404040404040404040404040404008",
  "actual_coverage_first_ptr": "0x16210976726",
  "actual_coverage_last_ptr": "0x16210976744"
}
```

Important correction: the old `coverage_sample_hex` is the legacy heuristic
sample using the plane snapshot and `+0x30`; on this stroke case it is not the
bytes AE actually consumes. The authoritative bytes are from the `3ba80`
sample-byte hook.

Second trace:

```text
target/dynamic_tools_85/p2_transfill_live_whta128_sample_byte_trace_20260508_001/TRFLIVE_WHT_A128_FILL_OPACITY.jsonl
target/ae_agents/p2_row_compare_transfill_sample_byte_20260508/ae_rows.json
```

Observed alpha-fill type-2 row:

```json
{
  "pass_role": "fill",
  "source_pixel_raw": "80ffffff",
  "y": 0,
  "start_x": 0,
  "end_x": 10,
  "coverage_sample_hex": "5bd0d0d0d0d0d0d0d0b6",
  "actual_coverage_sample_hex": "5bd0d0d0d0d0d0d0d0",
  "actual_coverage_sample_count": 9
}
```

For this first-row fill sample, the legacy plane heuristic and the actual
`3ba80` byte stream agree on the prefix. That explains why earlier fill-only
traces looked sane, but it does not make the snapshot formula safe for stroke
or later rows.

Third trace:

```text
target/dynamic_tools_85/p2_cov_w_sample_byte_trace_20260508_001/COV_W.jsonl
target/ae_agents/p2_row_compare_covw_sample_byte_20260508/ae_rows.json
```

Observed full-opacity `COV_W` first row:

```json
{
  "pass_role": "fill",
  "source_pixel_raw": "ffffffff",
  "y": 0,
  "start_x": 0,
  "end_x": 16,
  "coverage_sample_hex": "2440404040404040404040404040403c",
  "actual_coverage_sample_hex": "2440404040404040404040404040403c"
}
```

This confirms the parser/hook pair works on the original coverage fixture too.
The current AE job only emitted the first type-2 span for this pack on the
GUI API route, so a dense multi-row fill capture still needs a smaller
single-comp JSX or a trace mode that does not stop at the first emitted render.

## Recovered Semantics

For this live stroke case:

- `3ba2e` sees a coverage plane with `width=137`, `height=89`,
  `base_0x10=0x16210976720`, `stride_0x20=0`, `stride_0x30=144`.
- `3ba80` proves the actual read pointer sequence starts at
  `base_0x10 + start_x`, then increments by one byte per covered pixel.
- Therefore `+0x30` is a useful plane snapshot field, but not a safe proxy for
  the hot coverage-byte address in every text path.
- Formula tuning must prefer `actual_coverage_sample_hex` when present; a fill
  first-row match is not enough to validate the plane snapshot heuristic.
- The row base may already be row-local by the time type-2 coverage is consumed;
  the next runner should prefer the `3ba80` byte stream when available.

## Code Changes

- `scripts/ae_trace_cooltype_text.py`
  - Added hooks for `3ba2e`, `3ba5b`, and `3ba80`.
  - Added wide register snapshots and memory samples for the type-2 path.

- `scripts/compare_text_row_spans.py`
  - Added AE row-span parser.
  - Added plane-probe summaries by hook.
  - Added reconstruction of `actual_coverage_sample_hex` from `3ba80`.

## Status

P2 text raster is now better instrumented, not parity-locked.

Closed in this pass:

- Static/dynamic location of actual coverage-byte read.
- Proof that `3ba80` is the authoritative byte stream.
- Parser support for row-level AE spans plus actual sample bytes.

Still open:

- Run the same `3ba80` sample-byte hook on a fill case with many type-2 rows.
  Current fill captures (`TRFLIVE_*`, `COV_W`) prove first-row byte parity, but
  the GUI API route emitted only one type-2 span for these focused packs.
- Compare `actual_coverage_sample_hex` against native `coverage_rows` for
  matched glyph rows.
- Use the diff to tune native coverage generation/hinting instead of the
  legacy plane `+0x30` heuristic.

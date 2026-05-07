# P2 Text Raster Agent Results

Date: 2026-05-07

## Summary

Five scoped agents ran static-first P2 text raster work and submitted at most one
small AE job each. They did not collide on shared native code or shared Ghidra
task files.

Current integration decision:

```text
do not merge broad native formula changes yet
do add one shared narrow Frida profile for TXT ARE span/coverage/PF_TransferRect
then implement the deterministic text-local changes behind focused gates
```

## Results By Lane

| Lane | Agent Report | Evidence | Decision |
| --- | --- | --- | --- |
| Coverage rows | `P2_AGENT_COVERAGE_ROWS_20260507.md` | Static maps ARE path to BIB object, row iterator `TXT.dll+0x3de50`, span writer `TXT.dll+0x3b8c0`, span cells `{span_type,end_x}`, and byte coverage path through `TXT.dll+0x3c980`. One `COV_W` trace succeeded but current profile did not hook row/span internals. | Next shared tracer must hook `3de50`, `3b8c0`, and row getter / coverage bytes. |
| Hinting / AA / subpixel | `P2_AGENT_HINTING_AA_SUBPIXEL_20260507.md` | Static span writer consumes scalar coverage; render of 12 fractional-position glyph cases shows neutral grayscale output (`R=G=B`) and geometric fractional centroid shifts. | Do not implement RGB/LCD masks. Keep scalar grayscale coverage and geometric subpixel placement; hinting formula remains open. |
| Clipped bounds | `P2_AGENT_CLIPPED_BOUNDS_20260507.md` | Static separates sourceRect rounding from final write clipping. AE render confirms half-open edge writes via RGB nonblack bbox, but alpha was full-comp opaque. | Native change candidate: single integer coverage origin plus half-open clipped loops. Needs row/span trace before parity lock. |
| Stroke / fill merge | `P2_AGENT_STROKE_MERGE_20260507.md` | Static shows `ARE+0x0a` order byte: `0` stroke-then-fill, nonzero fill-then-stroke. Stroke gate is alpha > 0 and width > 0. One render succeeded, existing trace missed live `TXT_DrawChar`. | Ordinary stroke merge is ordered passes through existing writer. Implementation should wait for coverage/stroke row trace. |
| Semi-transparent fill | `P2_AGENT_SEMITRANSPARENT_FILL_20260507.md` | Static proves `fill_alpha != full` path: render full-alpha fill into temp `PF_WorldX`, then `PF_TransferRect` with `CompositeModePlus.mode=2` and opacity `alpha/255` or `alpha/32768`. AE JSX did not actually create semi-transparent text fill. | Text-local temp-world/group-opacity candidate is valid statically; needs `PF_TransferRect` trace or real alpha project for dynamic lock. |

## Shared Collision / Escalation Review

Only one escalation was raised:

```text
stroke lane -> semi-transparent fill temp-world / PF_TransferRect branch
```

This is not a conflict. It points at the same text-local branch identified by
the semi-transparent fill lane. It should be handled once in shared tracer and
then integrated as text-local behavior, not as a broad M19 rewrite.

No agent found evidence that the previously recovered opaque
`blend_text_pixel_ae_u8` path is wrong.

## Artifacts

Reports:

```text
docs/phase_reports/P2_AGENT_COVERAGE_ROWS_20260507.md
docs/phase_reports/P2_AGENT_HINTING_AA_SUBPIXEL_20260507.md
docs/phase_reports/P2_AGENT_CLIPPED_BOUNDS_20260507.md
docs/phase_reports/P2_AGENT_STROKE_MERGE_20260507.md
docs/phase_reports/P2_AGENT_SEMITRANSPARENT_FILL_20260507.md
```

Probe packs:

```text
fixtures/ae_probe_pack/p2_text_coverage_rows_probe
fixtures/ae_probe_pack/p2_text_hinting_probe
fixtures/ae_probe_pack/p2_text_clip_probe
fixtures/ae_probe_pack/p2_text_stroke_probe
fixtures/ae_probe_pack/p2_text_transfill_probe
```

Key local target artifacts:

```text
target/dynamic_tools_85/p2_coverage_rows_20260507_231636/COV_W.jsonl
target/dynamic_tools_85/p2_coverage_rows_20260507_231636/summary.json
target/dynamic_tools_85/p2_hinting_20260507_231636/render_rgb_analysis.json
target/dynamic_tools_85/p2_hinting_20260507_231636/render_alpha_analysis.json
target/ae_agents/p2_clip_20260507_231703/clip_alpha_bbox.json
target/dynamic_tools_85/p2_stroke_20260507_231752/summary.json
target/dynamic_tools_85/p2_transfill_20260507_231836/measurement.json
```

## Next Shared Trace

Add a narrow profile to `scripts/ae_trace_cooltype_text.py`:

```text
txt-are-spans
```

Hooks:

```text
TXT.dll + 0x03c360  TXT_ARE_Render_8bpc
TXT.dll + 0x03d200  TXT_ARE_Render_8bpc_fill
TXT.dll + 0x03d960  TXT_ARE_Render_8bpc_stroke
TXT.dll + 0x03de50  TXT_ARE_OutputComposite_8bpc
TXT.dll + 0x03b8c0  TXT_ARE_PixelWriter8_span
TXT.dll IAT +0x694f30 PF_TransferRect
DAT_18087f780 resolved target, when nonzero
```

Minimum payloads:

```text
ARE clip shorts +0x60/+0x62/+0x64/+0x66
ARE matrix block +0x68
fill/stroke/order bytes +0x08/+0x09/+0x0a
span type, y, start_x, end_x
row pointer and first row cells from 3de50
first coverage bytes for span_type=2
source PF_Pixel8 and destination PF_World snapshot
PF_TransferRect composite mode and opacity
```

First cases to run:

```text
COV_W
STR_FILL_STROKE_STROKE_OVER
one real semi-transparent-fill case if we can create it; otherwise skip until a real AE project has fill alpha < full
```

## Native Candidates After Trace

Order of implementation:

1. Text coverage write origin: replace per-pixel `round(raster_x + bx)` with
   one integer origin plus half-open clipped loops.
2. Text-local semitransparent fill: temp world, full-alpha fill, one normal
   transfer with opacity.
3. Ordinary stroke/fill: two ordered passes with order byte semantics.
4. Coverage rows: replace the current TTF 4x supersample coverage only after
   the BIB row/span ABI is dumped.

Do not implement LCD/RGB subpixel masks for this AE path.

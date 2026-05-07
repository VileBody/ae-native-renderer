# P2 Agent Stroke Merge

Date: 2026-05-07

## Scope

Task P2-D: inspect `TXT_DrawChar` ARE stroke path and fill/stroke merge policy.
No shared native code or shared Ghidra task files were edited.

## Static Inputs Read

- `docs/phase_reports/P2_BEE_TEXT_RASTER_FILL_PATH_20260507.md`
- `docs/phase_reports/P2_TXT_DRAWCHAR_ARE_PIXEL_WRITER_20260507.md`
- `target/reverse/predecoded/20260507_211128_bee_text_render_node_vtable_impls/.../13_BEE_TextRenderNode_render_payload_d8_5c0c20_1805c0c20/{decompile.c,disassembly.txt}`
- `target/reverse/predecoded/20260507_222041_txt_drawchar_core/.../01_TXT_DrawChar_entry_413d0_1800413d0/{decompile.c,disassembly.txt}`
- `target/reverse/predecoded/20260507_222041_txt_drawchar_core/.../03_TXTp_DrawChar3_caller_42266_180042266/{decompile.c,disassembly.txt}`
- `target/reverse/predecoded/20260507_222041_txt_drawchar_core/.../04_TXT_text_bbox_caller_42b80_180042b80/{decompile.c,disassembly.txt}`
- `target/reverse/predecoded/20260507_223741_txt_drawchar_are_helpers/.../01_TXT_ARE_OutlinePlayer_ctor_3e310_18003e310/{decompile.c,disassembly.txt}`
- `target/reverse/predecoded/20260507_223741_txt_drawchar_are_helpers/.../04_TXT_ARE_Render_8bpc_3c360_18003c360/{decompile.c,disassembly.txt}`
- `target/reverse/predecoded/20260507_223741_txt_drawchar_are_helpers/.../05_TXT_ARE_Render_16bpc_3c140_18003c140/decompile.c`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers/.../01_TXT_ARE_Render_8bpc_fill_3d200_18003d200/{decompile.c,disassembly.txt}`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers/.../02_TXT_ARE_Render_8bpc_stroke_3d960_18003d960/{decompile.c,disassembly.txt}`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers/.../03_TXT_ARE_Render_16bpc_fill_3cf50_18003cf50/decompile.c`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers/.../04_TXT_ARE_Render_16bpc_stroke_3d760_18003d760/decompile.c`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers/.../05_TXT_ARE_OutputComposite_3df40_18003df40/decompile.c`
- `target/reverse/predecoded/20260507_224844_txt_drawchar_are_pixel_writers/.../01_TXT_ARE_OutputComposite_8bpc_3de50_18003de50/decompile.c`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers/.../06_TXT_ARE_PathObjectFactory_3fd40_18003fd40/decompile.c`
- `target/reverse/predecoded/20260507_224844_txt_drawchar_are_pixel_writers/.../05_TXT_ARE_BIBPathObject_refresh_408d0_1800408d0/decompile.c`

## BEE Callsite Arguments

`BEE_TextRenderNode_render_payload_d8_5c0c20` still owns the high-level gates:

- `fill_enabled = grid_char[0x1a0].alpha > 0` unless render-order gating suppresses fill.
- `stroke_enabled = grid_char[0x1b0].alpha > 0 && grid_char[0x100] > 0` unless render-order gating suppresses stroke.
- Stroke width comes from `TXT_GridChar + 0x100`.
- Line join comes from `TXT_GridChar + 0x108`.
- Miter limit is passed as `DAT_180f608f8`, matching the previous live fill sample value `2.5`.
- Orientation comes from `TXT_GridChar + 0x390`.
- Fill/stroke colors are copied from `TXT_GridChar + 0x1a0` and `+0x1b0`.
- The byte at `TXT_GridChar + 0x3d0` is passed as `TXT_DrawChar` arg8 and becomes ARE byte `+0x0a`.

Static inference: `TXT_GridChar + 0x3d0` is the text stroke/fill ordering flag,
likely the AE `TextDocument.strokeOverFill` value. Dynamic did not capture live
entries in this pass, so this exact AE-property mapping remains unconfirmed.

## ARE Object Fields

`TXT_ARE_OutlinePlayer_ctor_3e310` lays out the key ARE state:

```text
+0x08  fill enabled
+0x09  stroke enabled
+0x0a  fill/stroke order byte
+0x0b  field/render context bool from TXT_DrawChar arg0
+0x20  fill PF_PixelFloat
+0x30  stroke PF_PixelFloat
+0x40  stroke width double
+0x48  AIMLineJoinType
+0x50  miter limit double, converted to float before BIB rasterizer call
+0x58  PF_World pointer
+0x68  glyph/text matrix payload converted to f32
+0x3d0 extra render feature flag
```

The constructor also disables stroke when `stroke_width <= 0`. If the field flag
is set, it also suppresses very small strokes when `stroke_width * matrix_scale`
falls below `DAT_180698440`.

## Fill/Stroke Order

`TXT_ARE_Render_8bpc_3c360` and `TXT_ARE_Render_16bpc_3c140` have the same
order policy:

```text
if ARE[0x0a] == 0:
  stroke pass, if ARE[0x09]
  fill pass,   if ARE[0x08]
else:
  fill pass,   if ARE[0x08]
  stroke pass, if ARE[0x09]
```

So byte `+0x0a` means fill/stroke order:

- `0`: stroke first, then fill. This visually means fill over stroke.
- nonzero: fill first, then stroke. This visually means stroke over fill.

Each pass composites into the same destination world through the existing ARE
pixel writer path. No separate final fill/stroke merge operator was found for
the ordinary opaque path; order is simply pass order plus the recovered
PF_Pixel8/PF_Pixel16/f32 writer.

## Stroke Path

`TXT_ARE_Render_8bpc_stroke_3d960` and 16 bpc `3d760` are direct analogs.
They:

1. Read the accumulated outline/path buffer from the ARE object.
2. Call the dynamic BIB/ARE rasterizer pointer `DAT_18087f780`.
3. Pass stroke width, line join, miter limit, matrix payload, clip/bounds, and
   field/context flags into that rasterizer.
4. Wrap the returned path/raster object through `TXT_ARE_PathObjectFactory_3fd40`
   / `TXT_ARE_BIBPathObject_refresh_408d0`.
5. Composite spans into the destination world via `TXT_ARE_OutputComposite_8bpc_3de50`
   or `TXT_ARE_OutputComposite_16bpc_3dd60`.

The direct stroke helper does not allocate a temp `PF_World`; it renders the
stroke path into the destination world argument it receives.

## Temp World / Composite Policy

`TXT_ARE_Render_8bpc_fill_3d200` and 16 bpc `3cf50` contain the important
shared-alpha branch:

- If ARE `+0x10` is zero, fill is rasterized and composited directly.
- If ARE `+0x10` is nonzero and fill alpha is not full (`0xff` for 8 bpc,
  `0x8000` for 16 bpc), the helper allocates a same-size temporary `PF_WorldX`.
- It forces the local color alpha to full coverage, calls the stroke-style
  helper with width/source from ARE `+0x10`, composites that into the temp, then
  transfers the temp back to the real destination using `PF_TransferRect`.
- `PF_CompositeModePlus` is initialized with mode `2`; opacity is derived from
  the original fill alpha (`alpha / 255` for 8 bpc, `alpha / 32768` for 16 bpc).

This branch looks like a semi-transparent fill / internal coverage transfer
policy rather than the ordinary visible text stroke pass. It was not dynamically
hit in this attempt, but it is enough to flag cross-agent risk.

## ESCALATE_TO_ORCHESTRATOR

Yes.

Reason: static stroke/fill inspection uncovered a conditional semi-transparent
fill temp-world path in `3d200`/`3cf50` that allocates a `PF_WorldX` and uses
`PF_TransferRect`. This overlaps the semi-transparent fill agent's owned
alpha/composite policy. Dynamic confirmation is still needed, but native work
should not lock semi-transparent text fill semantics independently of that
agent.

## Probe Pack

Created:

```text
fixtures/ae_probe_pack/p2_text_stroke_probe/
```

Cases:

```text
STR_FILL_ONLY
STR_STROKE_ONLY
STR_FILL_STROKE_FILL_OVER
STR_FILL_STROKE_STROKE_OVER
```

Each case is a single `W`, Montserrat-BoldItalic, 96 px. Stroke cases use a
thick red stroke (`strokeWidth = 14`). Fill+stroke cases use green fill and red
stroke with `strokeOverFill` toggled.

No semi-transparent stroke case was added because the public `TextDocument`
surface used here exposes `strokeColor` as RGB; a cheap, reliable stroke-alpha
setter was not identified during this pass.

## Dynamic Attempt

One remote render/trace attempt was run:

```text
job_id: p2_stroke_20260507_231752
node:   http://85.239.48.31:8001
pack:   fixtures/ae_probe_pack/p2_text_stroke_probe
cases:  STR_FILL_ONLY, STR_STROKE_ONLY,
        STR_FILL_STROKE_FILL_OVER, STR_FILL_STROKE_STROKE_OVER
trace:  existing txt-drawchar profile
```

Artifacts:

```text
target/ae_agents/p2_stroke_20260507_231752/
target/dynamic_tools_85/p2_stroke_20260507_231752/
```

Result:

- Remote render succeeded; no busy/crash/modal/timeout.
- AE produced 12 TIFF frames and local conversion produced 12 PNGs.
- Render queue manifest confirms all four `STR_*` comps rendered.
- Basic image stats confirm visibly different fill/stroke outputs.
- Frida trace attached and installed `TXT.dll`/BEE IAT hooks, but captured no
  live hook-enter events:

```text
target/dynamic_tools_85/p2_stroke_20260507_231752/STR_BATCH.jsonl
event_count: 13
hook_event_counts: {}
installed hooks include TXTp_DrawChar3_ARE_42110 and BEE_IMPORT_TXT_DrawChar_edf6b0
```

Interpretation: the existing `txt-drawchar` profile was useful enough to install
the expected hooks, but this run missed the actual render process/window. I did
not run a second remote attempt.

## Proposed Dynamic Hooks

Next dynamic pass should keep the same small `STR_*` pack but adjust tracing:

- Attach across `AfterFX.exe`, `AfterFX.com`, and `aerender.exe` for the full
  render duration without stopping immediately after `ae_remote_pack.py` exits.
- Add live hooks for `BEE.dll+0x5c0c20` and the direct callsite
  `BEE.dll+0x5c100a` to capture `TXT_DrawChar` args before IAT dispatch.
- Add `TXT.dll+0x3c360`, `+0x3c140`, `+0x3d200`, `+0x3d960`, `+0x3cf50`,
  `+0x3d760`, `+0x3de50`, `+0x3dd60`, and `+0x3df40`.
- On module snapshot, resolve and hook `DAT_18087f780` target to observe BIB
  rasterizer parameters for stroke width/join/miter and returned coverage.
- Hook/import-log `PF_TransferRect` to confirm the semi-transparent fill temp
  world path and its opacity/mode.

## Native Implementation Readiness

Can implement/report now, without more dynamic details:

- Text draw planning can expose stroke-enabled when stroke alpha > 0 and stroke
  width > 0.
- Store/pass fill color, stroke color, stroke width, line join, miter limit,
  orientation, and fill/stroke order in the native `TXT_DrawChar` boundary.
- For ordinary fill+stroke, execute two ordered passes into the same destination
  using existing recovered AE integer source-over writer:
  `order=0 -> stroke then fill`, `order!=0 -> fill then stroke`.
- Preserve miter default `2.5` where no text-specific value is available.

Still needs dynamic/native coverage details:

- Exact BIB/CoolType stroke coverage rows and how stroke expansion maps to
  native outlines.
- Confirmed AE property mapping for `TXT_GridChar + 0x3d0` to `strokeOverFill`.
- Exact `AIMLineJoinType` enum values emitted for text stroke styles.
- Whether/when ARE `+0x10` is set, and exact `PF_TransferRect` behavior for
  semi-transparent fill.
- Literal high-bit-depth output semantics if 16 bpc/f32 parity becomes required.


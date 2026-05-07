# P2-E Semi-Transparent Fill Temp World

Date: 2026-05-07

## Scope

Lane: semi-transparent text fill in the final TXT ARE raster path.

Owned outputs:

```text
fixtures/ae_probe_pack/p2_text_transfill_probe/
target/ae_agents/p2_transfill_20260507_231836/
target/dynamic_tools_85/p2_transfill_20260507_231836/
```

No shared native code, shared tracer, or shared `GHIDRA_PREDECODE_TASKS.json`
was edited.

## Static Evidence

Primary function:

```text
target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers/
  txt_drawchar_are_lower_helpers/
  01_TXT_ARE_Render_8bpc_fill_3d200_18003d200/decompile.c
```

Important offsets:

```text
TXT.dll+0x03d200  TXT_ARE_Render_8bpc_fill
TXT.dll+0x03cf50  TXT_ARE_Render_16bpc_fill
TXT.dll+0x03de50  TXT_ARE_OutputComposite_8bpc
TXT.dll+0x03dd60  TXT_ARE_OutputComposite_16bpc
TXT.dll+0x03b8c0  TXT_ARE_PixelWriter8_span
TXT IAT +0x694f30 PF_TransferRect
```

8 bpc behavior:

```text
if fill_alpha == 255:
  render directly into destination PF_World
else:
  allocate PF_WorldX<PF_Pixel8>(dest_width, dest_height, 1)
  local_fill.alpha = 255
  render fill into temp world
  PF_CompositeModePlus()
  composite_mode.mode = 2
  composite_mode.opacity = original_fill_alpha * 0x3b808081
  PF_TransferRect(..., src=temp+0x8, dst=dest+0x8, mode=&composite_mode)
```

The decompile lines that matter:

```text
3d200: alpha test against -1 / 255
3d27f..3d2a9: PF_WorldX<PF_Pixel8> allocation
3d2a9: local fill alpha rewritten to 0xff
3d388: OutputComposite writes into temp when present
3d39a..3d3c5: CompositeModePlus init, opacity write, mode = 2
3d42e: PF_TransferRect call via TXT IAT +0x694f30
```

`0x3b808081` is `0.003921568859`, effectively `1 / 255`.

16 bpc has the same shape:

```text
alpha full sentinel: 0x8000
local fill alpha rewritten to 0x8000
opacity = original_alpha * 0x38000000
```

`0x38000000` is `1 / 32768`.

## Formula Candidate

This is a text-local group opacity path, not a replacement for the recovered
opaque per-pixel writer.

For 8 bpc fill alpha `< 255`:

```text
temp = transparent PF_World, same dimensions/depth as dst
full = fill_color with alpha forced to 255

for each fill span:
  temp = TXT_ARE_PixelWriter8(temp, full, coverage)

dst = PF_TransferRect_Normal(
  dst,
  temp,
  opacity = original_fill_alpha / 255.0,
  composite_mode = 2
)
```

For isolated, non-overlapping pixels this is usually equivalent to scaling
source alpha before source-over:

```text
effective_alpha = coverage * fill_alpha / 255
```

The reason AE uses a temp world is still important: overlapping glyphs/spans are
first accumulated at full fill opacity, then the whole fill result receives one
opacity transfer. A direct per-span `src_alpha *= fill_alpha` can double-apply
opacity in overlap/merge cases.

RGB in the temp world is straight PF text output, produced by the already
recovered `TXT_ARE_PixelBlend8_*` writer. The temp is not pre-multiplied by the
original fill alpha; the original alpha is only passed as the `PF_TransferRect`
opacity.

## Native Change Candidate

Do not change shared M19/core composite as part of P2-E.

In the text raster path only:

```text
if fill_alpha == 255:
  keep current recovered_txt_are_pf_pixel8_integer_source_over_v1 path
else:
  create a temporary transparent Canvas/PF-world-equivalent
  render fill glyphs into temp with color alpha = 255 using blend_text_pixel_ae_u8
  composite temp onto destination once with normal source-over and opacity
    fill_alpha / 255.0
  trace output_semantics:
    recovered_txt_are_pf_pixel8_temp_world_transferrect_opacity_v1
```

Implementation note: if `PF_TransferRect` exact integer rounding is not already
available in a local text helper, add a text-scoped helper first. Do not route
this through a broad shared M19 policy change until the PF transfer rounding is
locked.

## Probe Pack

Added:

```text
fixtures/ae_probe_pack/p2_text_transfill_probe/manifest.json
fixtures/ae_probe_pack/p2_text_transfill_probe/README.md
fixtures/ae_probe_pack/p2_text_transfill_probe/jsx/build_p2_text_transfill_probe_project.jsx
fixtures/ae_probe_pack/p2_text_transfill_probe/scripts/measure_transfill_probe.py
```

Cases include white/red text with alpha `25`, `50`, `128`, `255` over
transparent, black, and blue backgrounds.

Local syntax check:

```text
python3 -m py_compile \
  fixtures/ae_probe_pack/p2_text_transfill_probe/scripts/measure_transfill_probe.py \
  scripts/ae_remote_pack.py \
  scripts/ae_trace_cooltype_text.py
```

## Remote Attempt

Command shape:

```text
python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/p2_text_transfill_probe \
  --entry-script jsx/build_p2_text_transfill_probe_project.jsx \
  --node http://85.239.48.31:8001 \
  --job-id p2_transfill_20260507_231836 \
  --prefix ae_remote_packs/p2_transfill \
  --output-dir target/ae_agents \
  --case TRF_WHT_A25_TRANSPARENT \
  --case TRF_WHT_A128_TRANSPARENT \
  --case TRF_WHT_A255_TRANSPARENT \
  --case TRF_WHT_A128_BLACK \
  --case TRF_RED_A128_BLUE
```

Result:

```text
render_id: edc3e49012ed4260a0053ba37df94d1e
status: succeeded
output: target/ae_agents/p2_transfill_20260507_231836/
measurement: target/dynamic_tools_85/p2_transfill_20260507_231836/measurement.json
```

The attempt did not crash, hang, hit a modal, or time out.

## Dynamic Result

The JSX API route did not actually create semi-transparent text fill. The
rendered `A25`, `A128`, and `A255` white cases are identical at the sampled
level:

```text
TRF_WHT_A25_TRANSPARENT   maxRGB=255 maxA=255
TRF_WHT_A128_TRANSPARENT  maxRGB=255 maxA=255
TRF_WHT_A255_TRANSPARENT  maxRGB=255 maxA=255
TRF_WHT_A128_BLACK        maxRGB=255 maxA=255
TRF_RED_A128_BLUE         maxR=255 maxB=255 maxA=255
```

So the probe pack is useful as scaffolding, but this render does not prove the
alpha branch dynamically. AE scripting appears to ignore `TextDocument.fillColor`
alpha and the best-effort `doc.fillOpacity` assignment.

## Proposed Trace Additions

Existing `scripts/ae_trace_cooltype_text.py` profiles do not hook the concrete
temp-world branch or `PF_TransferRect`. Proposed additions only; not applied:

```javascript
// TXT_HOOKS
{module: "TXT.dll", name: "TXT_ARE_Render_8bpc_fill_3d200", offset: 0x03d200,
 surface: "txt_are_fill_transferrect"},
{module: "TXT.dll", name: "TXT_ARE_Render_16bpc_fill_3cf50", offset: 0x03cf50,
 surface: "txt_are_fill_transferrect"},
{module: "TXT.dll", name: "TXT_ARE_OutputComposite_8bpc_3de50", offset: 0x03de50,
 surface: "txt_are_output_composite"},
{module: "TXT.dll", name: "TXT_ARE_PixelWriter8_span_3b8c0", offset: 0x03b8c0,
 surface: "txt_are_pixel_writer"}

// IAT hook, TXT.dll import slot used by 3d200/3cf50
{module: "TXT.dll", name: "TXT_IMPORT_PF_TransferRect_694f30", iatOffset: 0x694f30,
 surface: "txt_pf_transferrect"}
```

Minimum payloads:

```text
3d200 enter:
  param_3 PF_Pixel8 bytes, especially byte 0 alpha
  param_4 destination PF_World dimensions/rowbytes/data ptr

3d200 before PF_TransferRect:
  temp PF_World ptr/dimensions/data ptr
  CompositeModePlus raw 32 bytes
  mode int, opacity float
  source/dest world ptrs passed on stack

PF_TransferRect enter:
  first four register args
  stack args +0x20..+0x60
  decoded source world, dest world, composite mode, rect/field
```

## Blockers

Dynamic alpha-case generation is not locked. Need either:

```text
1. a reliable AE scripting route to set Character panel Fill Opacity / style alpha, or
2. the proposed native hooks while rendering a real project known to contain fill alpha < 255.
```

No escalation is needed from this pass:

```text
ESCALATE_TO_ORCHESTRATOR: no
```

The current recovered `blend_text_pixel_ae_u8` remains valid for normal opaque
text based on this static pass. The semi-transparent fill change should be
implemented as a text-local temp-world/group-opacity path, not as a shared M19
alpha policy change.

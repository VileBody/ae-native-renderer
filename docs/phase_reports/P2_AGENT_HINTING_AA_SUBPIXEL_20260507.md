# P2 Agent Hinting / AA / Subpixel

Date: 2026-05-07

## Scope

This pass targets P2-B: first facts for CoolType/TXT hinting, grid-fit,
antialias, and subpixel policy below the recovered `TXT_DrawChar` boundary.

Owned artifacts:

```text
fixtures/ae_probe_pack/p2_text_hinting_probe
target/ae_agents/p2_hinting_20260507_231636
target/dynamic_tools_85/p2_hinting_20260507_231636
```

No shared native code and no shared `GHIDRA_PREDECODE_TASKS.json` were edited.

## Static Brief

Previously closed layers:

- `BEE_TextRenderNode -> TXT_DrawChar -> TXTp_DrawChar3_ARE` is the active final
  text fill path for outline fonts.
- `TXT_DrawChar` is `TXT.dll + 0x413d0`.
- The render-time ARE outline player vtable is `TXT.dll + 0x6983e0`.
- The final 8 bpc pixel writer uses AE-style integer source-over; coverage
  generation remains the open layer.

Static files inspected:

```text
docs/phase_reports/P2_TXT_DRAWCHAR_ARE_PIXEL_WRITER_20260507.md
docs/phase_reports/P2_TTF_OUTLINE_TEXT_COVERAGE_20260507.md
docs/phase_reports/P2_SOURCE_RECT_SHAPING_PARITY_20260507.md
docs/phase_reports/P2_TEXT_RASTER_COVERAGE_20260507.md
docs/phase_reports/P2_TXT_DRAWCHAR_NATIVE_BOUNDARY_20260507.md
docs/phase_reports/P2_BEE_TEXT_RASTER_FILL_PATH_20260507.md
docs/phase_reports/GHIDRA_COOLTYPE_GLYPH_METRICS_20260505.md
```

Static bundles inspected:

```text
target/reverse/predecoded/20260507_222041_txt_drawchar_core
target/reverse/predecoded/20260507_223624_txt_drawchar_are_vtable
target/reverse/predecoded/20260507_223741_txt_drawchar_are_helpers
target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers
target/reverse/predecoded/20260507_224108_txt_drawchar_are_pixel_writers
target/reverse/predecoded/20260507_211128_bee_text_render_node_vtable_impls
target/reverse/predecoded/20260507_171815_cooltype_text_layout_surface_static_20260507
target/reverse/predecoded/20260507_171957_cooltype_text_layout_core_static_20260507
```

Key functions and offsets:

| Area | Function | Finding |
| --- | --- | --- |
| TXT entry | `TXT_DrawChar`, `0x1800413d0` | Uses `TXT_Font::HaveOutlines(font_dict, true)` and global `DAT_180879d72` to choose `TXTp_DrawChar3_ARE`; fallback `TXTp_DrawChar1` routes through an 8 bpc temp world for non-8 bpc output. |
| ARE setup | `TXTp_DrawChar3`, `0x180042110` | Builds `TXT_DrawOutlinePlayerARE`, initializes identity matrix, feature-gates `AE_3DTextExtrusionV2`, then calls `FUN_180042b80` to play outlines into the ARE player. |
| Outline player ctor | `FUN_18003e310`, `0x18003e310` | Copies fill/stroke flags, colors, stroke width, PF world, and matrix scale into the ARE object. Converts six matrix components from f64 to f32 at offsets around `0x68..0x7c`. |
| Outline core | `FUN_180042b80`, `0x180042b80` | Copies up to 32 floats from the vector-float payload, builds a 12-byte glyph request, obtains outline/path records, transforms path points by `glyph_matrix * param_4`, and emits move/line/curve/close calls to the outline player. |
| Render dispatch | `FUN_18003f310`, `0x18003f310` | Dispatches by PF depth: 8 bpc `0x18003c360`, 16 bpc `0x18003c140`, f32 local path. Fill/stroke order comes from bytes at ARE object `+0x08/+0x09/+0x0a`. |
| 8 bpc fill | `FUN_18003d200`, `0x18003d200` | If stroke alpha temp is needed, allocates temp `PF_WorldX<PF_Pixel8>`, calls stroke first, then converts BIB path to output spans and composites into PF world. |
| 8 bpc stroke | `FUN_18003d960`, `0x18003d960` | Calls `DAT_18087f780` with path point count, path pointers, stroke width as float, orientation/flag byte, matrix block at ARE `+0x68`, and current clip/span data. This is the strongest static candidate for BIB coverage generation. |
| 8 bpc output | `FUN_18003de50`, `0x18003de50` | Iterates BIB span rows and delegates to `FUN_18003b8c0`. |
| 8 bpc spans | `FUN_18003b8c0`, `0x18003b8c0` | Span type `1` is full-coverage run; span type `2` reads one byte per pixel from the BIB coverage buffer and calls coverage-scaled blend. This is grayscale coverage, not an RGB/LCD triplet at the TXT ARE output layer. |
| BEE payload | `FUN_1805c0c20`, `BEE.dll + 0x5c0c20` | Builds `char_matrix`, multiplies `char_parent * text_matrix`, clips bounds, translates by output rect origin, then passes glyph id, two matrices, `font_dict`, `vector<float>` payload, fill/stroke flags, and `PF_World` to `TXT_DrawChar`. |
| CoolType outlines | `CTTextGetOutlines/V2`, `0x1802a0ab0/0x1802a0950` | Public CoolType outline APIs wrap `CoreTextOutlines/V2`. Useful for text grid build, but final render path reaches BIB span generation through TXT ARE path helpers. |
| CoreText outlines | `CoreTextOutlines/V2`, `0x180124e0c/0x1801257d4` | Rebuilds layout, transforms glyph positions, batches bboxes, then emits outlines. It does not itself expose final raster coverage rows. |

Known matrix and subpixel-related facts:

- `BEE_TextRenderNode_render_payload_d8` passes a glyph/char matrix from
  `TXT_GridChar + 0x10..0x58`, optionally multiplied by char-parent and the
  render text matrix.
- It then translates the text matrix by negative output-rect origin before
  `TXT_DrawChar`.
- In the live `RAS_020` evidence, `glyph_matrix` was `[96,0,0,0,96,0,0,0,1]`
  for a 96 px Montserrat `W`, while `text_matrix` carried fractional/global
  placement.
- `TXT_ARE_OutlinePlayer_ctor_3e310` stores matrix components as f32 and also
  stores short-sized clip/world bounds from the PF world.
- `FUN_180042b80` applies `glyph_matrix * local_identity_or_setup_matrix` to
  outline points before sending the path to BIB/TXT output.
- `FUN_18003d960` passes ARE object `+0x68` to `DAT_18087f780`; this block is
  the likely matrix/subpixel input to BIB coverage generation.

Unknown flags and fields:

- `DAT_180879d72`: global path selector for outline-font ARE path.
- `DAT_18087f780`: BIB path coverage/raster helper pointer; dynamic prior
  identified it as `BIB.dll`-owned, but the callee ABI and coverage-buffer
  structure remain open.
- ARE object byte `+0x0b`: copied from `TXT_DrawChar` first bool and forwarded
  to BIB span generation; likely render/orientation/context policy, not yet
  semantically named.
- `vector<float>` payload from `TXT_GridChar + 0x3a8`: copied into up to 32
  floats in `FUN_180042b80`; no hinting/AA meaning found yet.
- `font_dict` structure: only observed as a retained `CCTFontDict` reference.
  No native code changes should depend on its layout yet.

Early static conclusion:

The active final fill path likely generates grayscale coverage in BIB/TXT, not
LCD/RGB subpixel masks. `TXT_ARE_PixelWriter8_span_3b8c0` consumes either
full-run spans or a single 8-bit coverage byte per pixel. If subpixel behavior
exists, it is probably geometric subpixel positioning before coverage, not
per-channel LCD composition at the PF pixel writer.

Dynamic questions for the tiny render:

1. Do fractional text-layer positions change alpha coverage continuously or
   snap at integer/half-integer boundaries?
2. Do small vertical/horizontal stems keep width stable across 0.00/0.25/0.50/
   0.75 px, suggesting grid-fit/hinting?
3. Are rendered edge pixels neutral grayscale in RGB with alpha-only coverage,
   supporting no LCD color-fringing in final 8 bpc output?
4. Does large text show smooth subpixel translation while small text snaps,
   which would indicate size-dependent hinting/grid-fit policy?

## Probe Pack

Created:

```text
fixtures/ae_probe_pack/p2_text_hinting_probe/manifest.json
fixtures/ae_probe_pack/p2_text_hinting_probe/README.md
fixtures/ae_probe_pack/p2_text_hinting_probe/jsx/build_p2_text_hinting_probe_project.jsx
```

Cases:

```text
HNT_010..HNT_013  glyph I  size 12  position fractions 0.00/0.25/0.50/0.75
HNT_020..HNT_023  glyph H  size 24  position fractions 0.00/0.25/0.50/0.75
HNT_030..HNT_033  glyph W  size 96  position fractions 0.00/0.25/0.50/0.75
```

All cases are 8 bpc, one frame, white fill only, no stroke, transparent output.

## Dynamic Run

Completed first tiny render:

```text
job_id: p2_hinting_20260507_231636_render
render_id: b1595ca32ce340f4a5034c25285e196d
node: http://85.239.48.31:8001
cases: HNT_010..HNT_033
```

Command:

```text
python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/p2_text_hinting_probe \
  --entry-script jsx/build_p2_text_hinting_probe_project.jsx \
  --node http://85.239.48.31:8001 \
  --job-id p2_hinting_20260507_231636_render \
  --prefix ae_remote_packs/p2_hinting_20260507_231636 \
  --output-dir target/ae_agents/p2_hinting_20260507_231636 \
  --case HNT_010 --case HNT_011 --case HNT_012 --case HNT_013 \
  --case HNT_020 --case HNT_021 --case HNT_022 --case HNT_023 \
  --case HNT_030 --case HNT_031 --case HNT_032 --case HNT_033 \
  --poll-interval-s 5 --timeout-s 900
```

Remote status:

```text
success=true
tiff=12
local_converted_png=12
```

Primary artifacts:

```text
target/ae_agents/p2_hinting_20260507_231636/p2_hinting_20260507_231636_render/outputs/p2_hinting_20260507_231636_render_outputs.zip
target/ae_agents/p2_hinting_20260507_231636/p2_hinting_20260507_231636_render/extracted/p2_hinting_20260507_231636_render
target/dynamic_tools_85/p2_hinting_20260507_231636/render_alpha_analysis.json
target/dynamic_tools_85/p2_hinting_20260507_231636/render_alpha_analysis.md
target/dynamic_tools_85/p2_hinting_20260507_231636/render_rgb_analysis.json
target/dynamic_tools_85/p2_hinting_20260507_231636/render_rgb_analysis.md
```

AE render log confirmed:

```text
Output Module: TIFF Sequence with Alpha
Channels: RGB + Alpha
Depth: Millions of Colors+
Color: Premultiplied
enabled render queue items: 12
```

## Tiny Render Facts

Important caveat: the decoded TIFF/PNG outputs have `alpha=255` for every
`192x192` pixel. This run therefore does not provide a direct alpha-buffer
coverage dump. Evidence below uses white text RGB over black background as a
foreground coverage proxy only.

Neutral-color / LCD-subpixel evidence:

- Every nonzero foreground pixel had `R=G=B`.
- `rgb_channel_delta_max_all_pixels = 0` for all 12 cases.
- No color-fringing / RGB LCD mask was visible in final 8 bpc output.

Fractional-position evidence:

| Group | Case | RGB bbox | Foreground sum | RGB centroid | Unique RGB max values |
| --- | --- | --- | ---: | --- | ---: |
| `I` 12 px | `HNT_010` | `[63, 67, 64, 75]` | `4304` | `63.500,71.266` | `2` |
| `I` 12 px | `HNT_011` | `[63, 67, 65, 76]` | `4344` | `63.750,71.566` | `9` |
| `I` 12 px | `HNT_012` | `[63, 68, 65, 76]` | `4345` | `64.000,71.823` | `6` |
| `I` 12 px | `HNT_013` | `[63, 68, 65, 76]` | `4345` | `64.250,72.059` | `9` |
| `H` 24 px | `HNT_020` | `[56, 63, 71, 79]` | `40240` | `63.500,71.068` | `9` |
| `H` 24 px | `HNT_021` | `[56, 63, 71, 80]` | `40372` | `63.744,71.353` | `16` |
| `H` 24 px | `HNT_022` | `[56, 63, 72, 80]` | `40372` | `64.000,71.602` | `12` |
| `H` 24 px | `HNT_023` | `[57, 63, 72, 80]` | `40372` | `64.256,71.850` | `17` |
| `W` 96 px | `HNT_030` | `[42, 68, 149, 135]` | `920009` | `95.233,101.039` | `132` |
| `W` 96 px | `HNT_031` | `[42, 69, 149, 136]` | `920546` | `95.483,101.312` | `141` |
| `W` 96 px | `HNT_032` | `[42, 69, 150, 136]` | `920585` | `95.733,101.560` | `129` |
| `W` 96 px | `HNT_033` | `[43, 69, 150, 136]` | `920583` | `95.983,101.810` | `143` |

Deltas from fraction `0.00`:

- `I` 12 px centroid moved by about `+0.25/+0.50/+0.75` in x and
  `+0.30/+0.56/+0.79` in y; coverage sum stayed within `+41`.
- `H` 24 px centroid moved by about `+0.244/+0.500/+0.756` in x and
  `+0.285/+0.534/+0.782` in y; coverage sum stayed exactly `+132` for all
  nonzero fractions.
- `W` 96 px centroid moved by about `+0.250/+0.500/+0.750` in x and
  `+0.273/+0.521/+0.771` in y; coverage sum changed by only `+537..+576` on a
  `~920k` foreground sum.

Interpretation:

- Final output is grayscale AA, not RGB/LCD subpixel AA, for this AE render
  path and output module.
- Fractional text positions are preserved in coverage placement; they do not
  simply snap to integer pixels before rasterization.
- Small and medium stems show stable total coverage with changing gray levels,
  which is consistent with gray antialiasing plus some grid-fit/stem stability.
  The tiny render is not enough to recover the hinting formula.

## Deterministic Native Consequence

Native can take one deterministic next step now:

```text
Do not implement LCD/subpixel RGB masks for this path.
Keep coverage scalar/grayscale and treat subpixel as geometric coverage placement.
```

This is supported by both:

- static `TXT_ARE_PixelWriter8_span_3b8c0`, which consumes one coverage byte per
  pixel for span type `2`; and
- tiny render RGB evidence, where all text-edge pixels are neutral grayscale.

Native should not yet hard-code a CoolType hinting/grid-fit formula. The next
needed fact is the BIB coverage-buffer ABI around `DAT_18087f780` and the span
object consumed by `FUN_18003de50`/`FUN_18003b8c0`.

## Proposed Next Patch Scope

No shared patch was applied. If a tracer change is allowed in a future pass,
propose adding a local profile that hooks:

```text
TXT.dll + 0x18003d960  FUN_18003d960  BIB stroke/path coverage call wrapper
TXT.dll + 0x18003de50  FUN_18003de50  8 bpc span iterator
TXT.dll + 0x18003b8c0  FUN_18003b8c0  8 bpc span writer
DAT_18087f780 resolved target in BIB.dll
```

Dump only:

```text
span type
row y
x start/end
coverage buffer pointer/stride
first N coverage bytes
ARE matrix block at +0x68
ARE clip bounds +0x60..0x66
```

This would answer the remaining ABI/policy question without modifying native
code.

## Escalation

No `ESCALATE_TO_ORCHESTRATOR` was hit.

Known blocker/risk remains: `font_dict`, `vector<float>` payload, and
`DAT_18087f780`/BIB span ABI are observed but not decoded. Do not write native
code that depends on their structure until the span/coverage trace lands.

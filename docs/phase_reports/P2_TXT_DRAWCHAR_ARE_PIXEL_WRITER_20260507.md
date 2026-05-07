# P2 TXT_DrawChar ARE Pixel Writer

Date: 2026-05-07

## Scope

This pass closes the first concrete native implementation layer below the
recovered `TXT_DrawChar` boundary:

```text
BEE_TextRenderNode
  -> TXT_DrawChar
  -> TXTp_DrawChar3_ARE
  -> TXT_DrawChar_outline_core_42b80
  -> TXT_DrawOutlinePlayerARE vtable
  -> PF_World pixel writer
```

The goal was to stop treating text output alpha/composite as a guess. We traced
the final render-time single-glyph path, predecoded the concrete `TXT.dll`
handlers, and implemented the recovered 8 bpc `PF_Pixel8` source-over blend in
native text rasterization.

## Dynamic Evidence

Focused traces:

```text
target/dynamic_tools_85/txt_drawchar_ras020_are_vtable_20260507/RAS_020.jsonl
target/dynamic_tools_85/txt_drawchar_ras020_dynamic_ptrs_20260507/RAS_020.jsonl
target/dynamic_tools_85/txt_drawchar_ras020_path_ptrs_20260507/RAS_020.jsonl
```

The final render-time `RAS_020` glyph is Montserrat `W`:

```text
process: AfterFX.com
glyph_id: 331
glyph_matrix: [96, 0, 0, 0, 96, 0, 0, 0, 1]
outline_player_vtable: TXT.dll + 0x6983e0
```

The old sourceRect/bounds player uses `TXT.dll+0x6989c8`; that is not the final
pixel fill vtable. The render-time ARE vtable handlers are:

```text
0x18003ecd0
0x18003ee00
0x1800404e0  move command
0x180040430  line command
0x18003f050  curve command
0x18003ef60  close command
0x18003f310  render/flush
```

Runtime dynamic pointers show the current CPU-only AE path is bridged through
BIB path helpers:

```text
TXT_bib_make_path_DAT_18087beb8    -> BIB.dll + 0x18150
TXT_bib_release_path_DAT_18087bec0 -> BIB.dll + 0x18130
TXT_bib_path_vtable_DAT_18087bec8  -> BIB.dll + 0x18140
TXT_path_retain_DAT_18087c038      -> BIB.dll + 0x0a5f0
TXT_path_release_DAT_18087c040     -> BIB.dll + 0x0b080
```

`DAT_18087f778` / `DAT_18087f780` were zero in the captured final fill-only
case, so they are not the active path for this `RAS_020` run.

## Static Evidence

Predecoded bundles:

```text
target/reverse/predecoded/20260507_223624_txt_drawchar_are_vtable
target/reverse/predecoded/20260507_223741_txt_drawchar_are_helpers
target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers
target/reverse/predecoded/20260507_224844_txt_drawchar_are_pixel_writers
```

`TXT_ARE_OutlinePlayer_vtable_06_3f310` dispatches by PF depth:

```text
8 bpc  -> 0x18003c360
16 bpc -> 0x18003c140
f32    -> local f32 writer path
```

Fill/stroke order is controlled by bytes on the ARE object:

```text
param_1 + 0x08: fill enabled
param_1 + 0x09: stroke enabled
param_1 + 0x0a: stroke/fill order
```

8 bpc color conversion constants:

```text
0x437f0000 = 255.0
0x3f000000 = 0.5
```

The 8 bpc blend functions are:

```text
0x18003cb10  full coverage blend
0x18003c980  coverage-scaled blend
```

Recovered scalar shape:

```text
mul_u8_ae(a, b):
  x = a * b + 0x80
  ((x >> 8) + x) >> 8

src_alpha:
  coverage == 255 ? src_alpha : mul_u8_ae(src_alpha, coverage)

out_alpha:
  255 - mul_u8_ae(255 - src_alpha, 255 - dst_alpha)

channel:
  dst_premul = mul_u8_ae(dst_channel, dst_alpha)
  out_premul = dst_premul + div255_ae((src_channel - dst_premul) * src_alpha)
  out_channel = PFp_G_fcd_table[out_alpha, out_premul]
```

The native implementation uses the same integer multiply/divide shape and a
mathematical equivalent of the `PFp_G_fcd_table` unpremultiply. A future Frida
dump can replace that with the literal table if byte-for-byte paranoia is
needed.

## Native Implementation

`crates/text-engine/src/rasterize.rs`

- `blend_bitmap` now composites through `blend_text_pixel_ae_u8`.
- The blend uses recovered AE integer alpha math instead of float straight-RGBA
  source-over.
- Text raster trace now records:

```text
output_semantics = recovered_txt_are_pf_pixel8_integer_source_over_v1
pf_world_semantics = TXT_DrawChar ARE PF_Pixel8 fill/composite recovered from TXT.dll
```

Coverage generation is still the existing native outline supersample backend.
This pass changes how covered pixels are written, not how CoolType/BIB generates
coverage rows.

## Tests

Focused checks:

```text
python3 -m py_compile scripts/ae_trace_cooltype_text.py scripts/ae_remote_pack.py
cargo test -p raster-cpu
cargo test -p text-engine
```

Added unit test:

```text
recovered_txt_are_pixel8_blend_matches_reverse_formula
```

## P2 Status

Closed in this pass:

```text
TXT_DrawChar final ARE vtable identified
8 bpc PF_Pixel8 text source-over formula implemented
fill/stroke order flags located
BIB path bridge pointers captured dynamically
pixel-writer Ghidra tasks made repeatable
```

Still open for full P2 parity:

```text
literal CoolType/BIB coverage rows
hinting/grid-fit and antialias/subpixel policy
clipped-bounds rounding at coverage generation time
stroke path and fill/stroke merge
semi-transparent fill temp-world/PF_TransferRect path
16 bpc / f32 text output if we need high-bit-depth parity
literal PFp_G_fcd_table dump if 8 bpc unpremultiply needs byte-for-byte lock
```

So P2 is no longer blocked by "what does text alpha/composite do". It is now
blocked by the narrower raster-coverage path below `TXT_DrawChar`.

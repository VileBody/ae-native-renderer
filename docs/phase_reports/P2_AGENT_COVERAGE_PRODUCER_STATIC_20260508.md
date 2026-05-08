# P2-A Static Coverage Producer Analysis

Date: 2026-05-08

Agent: P2-A static/Ghidra coverage producer analysis.

Scope: find the producer side of the byte coverage plane consumed by
`TXT_ARE_PixelWriter8_span_3b8c0`. No native code or shared tracer edits were
made.

## Inputs Read

- `docs/phase_reports/P2_TEXT_COVERAGE_BYTE_ADDRESS_TRACE_20260508.md`
- `docs/phase_reports/P2_TXT_ARE_SPAN_TRACE_20260507.md`
- `docs/reverse_engineering/GHIDRA_PREDECODE_TASKS.json`
- `docs/phase_reports/P2_AGENT_BIB_COVERAGE_STATIC_20260508.md`
- `docs/phase_reports/P2_AGENT_COVERAGE_ROWS_20260507.md`
- `docs/phase_reports/P2_BEE_TEXT_RASTER_FILL_PATH_20260507.md`
- `docs/phase_reports/P2_TXT_DRAWCHAR_NATIVE_BOUNDARY_20260507.md`
- `docs/phase_reports/P2_TXT_DRAWCHAR_ARE_PIXEL_WRITER_20260507.md`
- `docs/phase_reports/P2_AGENT_STROKE_MERGE_20260507.md`
- `target/reverse/predecoded/20260507_222041_txt_drawchar_core`
- `target/reverse/predecoded/20260507_223624_txt_drawchar_are_vtable`
- `target/reverse/predecoded/20260507_223741_txt_drawchar_are_helpers`
- `target/reverse/predecoded/20260507_223902_txt_drawchar_are_lower_helpers`
- `target/reverse/predecoded/20260507_224844_txt_drawchar_are_pixel_writers`
- `target/reverse/predecoded/20260507_171815_cooltype_text_layout_surface_static_20260507`
- `target/reverse/predecoded/20260507_172115_cooltype_text_layout_core_static_v2_20260507`
- local DLLs under `target/reverse/ae_2026`.

Local static corpus contains `TXT.dll`, `CoolType.dll`, and `BEE.dll`, but no
matching `ARE.dll` or `BIB.dll`. Exact ARE/BIB producer bodies are therefore
live-resolved hook targets, not locally decompilable static bodies yet.

## Executive Result

The byte coverage plane is produced below the TXT bridge, not in
`TXT_ARE_PixelWriter8`. TXT owns path assembly, BIB handle wrapping, coverage
object construction, and span consumption. The likely producer candidates are:

| Module | Offset / dynamic target | Role |
| --- | ---: | --- |
| `TXT.dll` | `+0x40580` | Fill path builder; calls dynamic `DAT_18087f778` with path coords/commands. |
| dynamic, likely ARE/BIB | `*(TXT.dll+0x87f778)` | Fill path/raster handle producer; must be resolved at call time. |
| `TXT.dll` | `+0x3d960` | 8 bpc stroke pass; calls dynamic `DAT_18087f780`. |
| dynamic, likely ARE/BIB | `*(TXT.dll+0x87f780)` | Stroke offset/raster handle producer; must be resolved at call time. |
| `TXT.dll` | `+0x3fd40` | Maps BIB handle through `DAT_18087bd40 + 8 + handle`; returns method block. |
| `TXT.dll` | `+0x408d0` | Refreshes BIB object using `BIB.dll+0x18150/0x18130/0x18140`, then calls callback from `DAT_180841f88`. |
| dynamic method block | `method_block[0]` | Coverage object constructor: `(*method_block)(&coverage_obj, handle)`. |
| `ARE.dll` | `+0x8230` | Live row getter: `row_getter(row_context, y)` returns `{type,end_x}` cells. |
| unknown ARE/BIB plane writer | below row getter/constructor | Fills the byte plane read by `TXT.dll+0x3ba80`. |

CoolType appears raster-adjacent but not the final byte-plane producer on the
recovered path. `CTTextGetOutlines`/`CTTextGetOutlinesV2`
(`CoolType.dll+0x2a0ab0/+0x2a0950`) call core outline bodies
`+0x124e0c/+0x1257d4`. `CTTextGetTextImage` (`CoolType.dll+0x2a14b0`)
decompiles to `xor eax,eax; ret`, so it is not the active coverage generator
for this TXT ARE route.

## Call / Data Flow

```text
BEE.dll+0x5c0c20
  BEE_TextRenderNode payload
  - filters renderable glyphs
  - passes glyph id, matrices, fill/stroke flags/colors, stroke width/join
  - calls BEE IAT TXT_DrawChar

TXT.dll+0x413d0
  TXT_DrawChar entry
    -> TXT.dll+0x42110 TXTp_DrawChar3_ARE
      -> TXT.dll+0x3e310 constructs TXT_DrawOutlinePlayerARE
      -> TXT.dll+0x42b80 outline core
         -> vtable path ops:
            +0x404e0 move
            +0x40430 line
            +0x3f050 curve
            +0x3ef60 close
         -> +0x3f310 render/flush

TXT.dll+0x3c360
  8 bpc render dispatch
  - order byte ARE+0x0a controls fill/stroke pass order
  - fill calls +0x3d200
  - stroke calls +0x3d960

fill:
  TXT.dll+0x3d200
    -> TXT.dll+0x40580
       -> call *(TXT.dll+0x87f778)(&path_handle, count, coords, commands, ...)
    -> TXT.dll+0x3fd40(&DAT_180841f48, path_handle, DAT_180841f88)
       -> TXT.dll+0x408d0 if cache refresh needed
       -> returns method_block
    -> method_block[0](&coverage_obj, path_handle)
    -> TXT.dll+0x3de50(ARE, {path_handle, method_block, coverage_obj}, src, PF_World)

stroke:
  TXT.dll+0x3d960
    -> call *(TXT.dll+0x87f780)(&path_handle, count, coords, commands,
                               stroke_width, join, miter, matrix, bounds...)
    -> TXT.dll+0x3fd40 / method_block[0] / +0x3de50

consume:
  TXT.dll+0x3de50
    -> coverage_obj+0x10 row_getter(row_context=coverage_obj+0x18, y)
       live target: ARE.dll+0x8230
    -> iterate 8-byte cells {s32 type, s32 end_x}
    -> TXT.dll+0x3b8c0 for each clipped span

  TXT.dll+0x3b8c0 type 2
    coverage_plane = coverage_obj+0x08
    coverage byte stream is authoritative at +0x3ba5b/+0x3ba80
```

The consumer ABI is now well constrained:

```c
struct CoverageObject {
    uint64_t unknown_00;
    void *plane;                 // +0x08
    SpanCell *(*row_getter)(void *ctx, int y); // +0x10
    void *row_context;           // +0x18
};

struct SpanCell {
    int32_t type;   // 0 transparent, 1 solid ink, 2 byte coverage
    int32_t end_x;  // exclusive
};
```

The live plane snapshot seen in prior traces:

```text
coverage_obj+0x08 plane/context = 0x296e493a428
coverage_obj+0x10 row_getter    = ARE.dll+0x8230
coverage_obj+0x18 row_context   = 0x296e493a350

plane+0x08 width                = 109
plane+0x0c height               = 68
plane+0x10 base                 = 0x296e6beb520
plane+0x28 base mirror          = 0x296e6beb520
plane+0x30 stride               = 112
```

But fresh byte hooks prove the actual byte stream must be taken from
`TXT.dll+0x3ba5b`/`+0x3ba80`, not from a generic `plane+0x30` snapshot formula.

## Candidate Producer Addresses

### `TXT.dll+0x40580` fill path builder

Static callsite:

```asm
1800405f1  mov r10, qword ptr [0x18087f778]
18004061d  ... rcx=&out_handle, rdx=count, r8=coords, r9=commands
180040621  call r10
180040629  mov rcx, qword ptr [rsp+0x48] ; returned handle
180040631  call qword ptr [0x18087c038]  ; retain
```

Question closed by hook: is `DAT_18087f778` ever nonzero in the render process,
and what handle/object does it return before coverage construction?

### `TXT.dll+0x3d960` stroke producer call

Static callsite:

```asm
18003d9f9  mov r10, qword ptr [0x18087f780]
18003da61  stack p5 = stroke_width_f32
18003da52  stack p8 = line_join
18003da45  stack p10 = miter_limit_f32
18003da67  r9 = path command base
18003da6a  r8 = path coord base
18003da6d  rdx = path count
18003da70  rcx = &out_handle
18003da78  call r10
```

Question closed by hook: whether stroke expands outlines before raster rows and
how width/join/miter/matrix enter the producer.

### `TXT.dll+0x3fd40` / `TXT.dll+0x408d0` BIB method block

`+0x3fd40` maps a BIB handle to a cached method block. `+0x408d0` refreshes it:

```asm
180040922  call qword ptr [0x18087beb8] ; live BIB.dll+0x18150
18004093a  call qword ptr [0x18087bec0] ; live BIB.dll+0x18130
180040947  call qword ptr [0x18087bec8] ; live BIB.dll+0x18140
180040960  call rbp                    ; callback from DAT_180841f88
```

Question closed by hook: what method block entries are installed, especially
entry `+0x00`, the coverage object constructor.

### Dynamic `method_block[0]`

Both fill and stroke do:

```asm
mov rax, [method_block]
lea rcx, [&coverage_obj]
mov rdx, handle
call rax
```

Question closed by hook: exact coverage object allocation/fill moment; dump
`coverage_obj`, `plane`, `row_getter`, `row_context`, and first plane bytes
immediately after construction.

### `ARE.dll+0x8230` row getter

Known live target from shared traces. It returns the row-cell pointer consumed
by `TXT.dll+0x3de50`.

Question closed by hook: whether row cells and/or coverage bytes are generated
lazily per row, and how row_context maps to the byte plane.

## Frida Hook Targets

Use a tiny single glyph case first (`COV_W`, then stroke-only). Keep a
per-coverage-object map keyed by `coverage_obj` and `row_context`.

| Target | Dump | Question |
| --- | --- | --- |
| `BEE.dll+0x5c0c20` | glyph id, matrices, fill/stroke flags, stroke width, line join, miter, font pointer, PF world | Confirms upstream glyph/pass payload for producer correlation. |
| `TXT.dll+0x3c360` | `RCX=ARE`, `RDX=PF_World`; dump ARE `+0x08/+0x09/+0x0a/+0x0b`, clip `+0x60..+0x66`, matrix `+0x68`, path bases `+0x3a8/+0x3b8/+0x3c0` | Establishes fill/stroke order and path buffer bounds before producer calls. |
| `TXT.dll+0x3d200` | `RCX=ARE`, `R8=PF_Pixel8 fill`, `R9=PF_World`; dump `*(TXT+0x87f778)`, BIB slots, path count/coords/commands | Confirms fill pass reaches BIB/ARE producer boundary. |
| `TXT.dll+0x40580` | onEnter: `RCX=&out`, `RDX=count`, `R8=coords`, `R9=commands`, stack p5/p6/p7/p8; dump first 32 coord f32s and command dwords | Captures fill producer input path before dynamic call. |
| `TXT.dll+0x40621` | instruction callsite; dump `R10`, `RCX/RDX/R8/R9`, stack p5..p8 before call; after nearby return, dump `[out]` | Resolves actual `DAT_18087f778` target and returned handle at the real call moment. |
| dynamic `*(TXT.dll+0x87f778)` | hook if nonzero; same args as `+0x40580`, onLeave status and output handle | Names module+offset for fill producer body, likely `ARE.dll`/`BIB.dll`. |
| `TXT.dll+0x3d960` | `RCX=ARE`, `RDX=&bounds`, `R8=stroke pixel`, `XMM3=stroke_width`; dump join `ARE+0x48`, miter `ARE+0x50`, path buffers, `*(TXT+0x87f780)` | Captures stroke producer input. |
| `TXT.dll+0x3da78` | instruction callsite; dump `R10`, `RCX=&out`, `RDX=count`, `R8=coords`, `R9=commands`, stack stroke args `+0x20..+0x88` | Resolves actual stroke rasterizer target and returned handle. |
| dynamic `*(TXT.dll+0x87f780)` | hook if nonzero; args/return as above | Names stroke producer body and validates stroke coverage source. |
| `TXT.dll+0x3fd40` | onEnter: cache root, handle, callback; onLeave: `RAX=method_block`; dump method block qwords `+0x00..+0x40` with module offsets | Finds coverage constructor target. |
| `TXT.dll+0x408d0` | onEnter: object, handle, callback; dump `DAT_18087beb8/bec0/bec8`; onLeave: object `+0x20/+0x28/+0x30/+0x34/+0x38` | Maps BIB path object and method-block install. |
| `TXT.dll+0x40960` | instruction callsite for callback; dump `RBP`, `RCX=handle`, `EDX=1`, `R8=object+0x38`; after return dump first method-block entries | Closes `DAT_180841f88` callback behavior. |
| dynamic `method_block[0]` | onEnter: `RCX=&coverage_obj`, `RDX=handle`; onLeave: status, `coverage_obj`, object words, plane words, row getter target | Captures coverage object construction and candidate plane allocation. |
| `TXT.dll+0x3de50` | `RCX=ARE`, `RDX=tuple`, `R8=src`, `R9=PF_World`; dump tuple `+0/+8/+0x10`, coverage object, row getter/context | Correlates producer object to consumer row iteration. |
| `TXT.dll+0x3deaa` | after row getter call; dump `RAX=row_ptr`, `EBP=y`, first 16 `{type,end_x}` cells | Captures row cells without per-pixel noise. |
| `ARE.dll+0x8230` | onEnter `RCX=row_context`, `EDX=y`; onLeave row ptr and cells; also dump mapped plane bytes for that y if known | Determines whether row/coverage data are lazy-generated. |
| `TXT.dll+0x3ba5b` | `RBX=actual coverage ptr`, `R8=start_x`, stack end_x; dump row bytes length `end_x-start_x` | Authoritative byte stream validation against producer dump. |
| `TXT.dll+0x3ba80` | sampled first N pixels only; dump `RBX` and byte read | Pixel-level validation for edge cases. |
| `BIB.dll+0x18150` | args/return from `DAT_18087beb8` live mapping | Names/validates path object creation. |
| `BIB.dll+0x18140` | arg path object, return vtable/API pointer | Finds BIB object methods adjacent to method block. |
| `BIB.dll+0x0a5f0` / `+0x0b080` | handle/object retain/release args | Ownership sanity; helps avoid stale handles in logs. |

For each pointer dump, record `{absolute, module, offset}`. For byte planes,
dump compact samples only: header qwords `0x00..0x40`, bytes at `base`, and row
slice at the consumer pointer.

## Static Findings By Module

### `TXT.dll`

TXT is the bridge and consumer. It builds path buffers and wraps BIB/ARE
handles but does not itself generate type-1/type-2 row cells. The row-cell type
assignment is only consumed in `+0x3de50/+0x3b8c0`.

### `ARE.dll`

Only live evidence is available locally. `ARE.dll+0x8230` is the row getter
stored at `coverage_obj+0x10`. This is the best current producer-adjacent hook
because it returns row cells immediately before TXT consumes them.

### `BIB.dll`

Live-resolved support calls:

```text
DAT_18087c038 -> BIB.dll+0x0a5f0
DAT_18087c040 -> BIB.dll+0x0b080
DAT_18087beb8 -> BIB.dll+0x18150
DAT_18087bec0 -> BIB.dll+0x18130
DAT_18087bec8 -> BIB.dll+0x18140
```

No local `BIB.dll` is present under `target/reverse/ae_2026`, so these bodies
cannot yet be predecoded. They should be captured/copied from the same AE build
before static closure.

### `CoolType.dll`

CoolType contributes glyph layout/outlines:

```text
CTTextGetOutlines    CoolType.dll+0x2a0ab0 -> CoreTextOutlines    +0x124e0c
CTTextGetOutlinesV2  CoolType.dll+0x2a0950 -> CoreTextOutlinesV2  +0x1257d4
CTTextGetTextImage   CoolType.dll+0x2a14b0 -> returns 0
```

This supports the current model: outlines come from CoolType/TXT, final scalar
coverage bytes come from BIB/ARE scan conversion.

## Unknowns / Escalations

ESCALATE_TO_ORCHESTRATOR: static `TXT.dll+0x40580/+0x3d960` unambiguously call
`DAT_18087f778/f780`, but several live module snapshots recorded those slots as
`0`. Yet the same overall traces reached `TXT_ARE_Render_8bpc_fill_3d200` and
`TXT_ARE_PixelWriter8_span_3b8c0`. This could be process/init timing, a stale
snapshot, or an alternate active path. The next hook must sample the slots at
the actual callsites (`TXT+0x40621`, `TXT+0x3da78`) before concluding these
pointers are unused.

Remaining unknowns:

- Exact bodies and semantic names for `ARE.dll+0x8230`,
  `*(TXT.dll+0x87f778)`, and `*(TXT.dll+0x87f780)`.
- Exact `DAT_180841f88` callback target and method-block layout beyond
  `method_block[0]`.
- Whether the byte plane is fully allocated/filled by `method_block[0]` or
  lazily by `ARE.dll+0x8230`.
- Precise reconciliation of static `plane+0x20` stride read in `+0x3b8c0` with
  live snapshots showing useful row stride at `plane+0x30`.
- Exact hinting/grid-fit policy before BIB/ARE scan conversion.

## Recommended Next Static Step

Acquire matching `ARE.dll` and `BIB.dll` from the same AE 2026 installation used
by the traces. Predecode:

```text
ARE.dll+0x8230
BIB.dll+0x18150
BIB.dll+0x18140
BIB.dll+0x0a5f0
BIB.dll+0x0b080
the live module+offset resolved from TXT.dll+0x87f778
the live module+offset resolved from TXT.dll+0x87f780
the live method_block[0] target
```

The dynamic hook plan above is the minimum run needed to name those offsets
without guessing.

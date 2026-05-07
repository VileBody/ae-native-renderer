# P2 Text Row / Stroke / Alpha Live Lock - 2026-05-08

## Scope

Close the next P2 text-raster layer without metric guessing:

- add native coverage-row sidecars at the same integer/clipped write boundary;
- use static TXT/BIB evidence to locate the remaining coverage generator;
- replace crashy multi-case JSX probes with atomic one-case probes;
- live-lock stroke and Text Animator fill opacity through AE85 + Frida.

## Static Boundary

`P2_AGENT_BIB_COVERAGE_STATIC_20260508.md` confirms TXT does not generate
coverage bytes itself. TXT consumes coverage objects produced by the BIB/ARE
bridge:

- fill path: `TXT.dll+0x40580 -> DAT_18087f778`;
- stroke path: `TXT.dll+0x3d960 -> DAT_18087f780`;
- BIB handle is mapped through `TXT.dll+0x3fd40`;
- coverage object row getter is live-resolved as `ARE.dll+0x8230`;
- `TXT.dll+0x3de50 -> 0x3b8c0` walks rows and writes PF_Pixel8 spans.

One unresolved static/live mismatch remains: static disasm around
`TXT.dll+0x3b8c0` appears to read a stride-like field at `plane+0x20`, while
live traces show the valid byte stride at `plane+0x30`. The next targeted hook
is `TXT.dll+0x3ba1b` to sample the exact `RDX` pointer at the load site.

## Native Row Telemetry

Implemented in `crates/text-engine/src/rasterize.rs`:

- `DrawCharPlan.coverage_rows`;
- `CoverageRowSpan` with schema, glyph row index, glyph id, y, half-open
  x-range, length, FNV-1a64 hash, sample hex, and full hex for short spans;
- row collection uses the same integer origin and half-open clip policy as the
  native pixel writer.

This moves text tuning from final-PNG-only comparison to row/span comparison.

Focused native gate:

```text
cargo run -p render-cli -- conformance-pack \
  --case TXT_010 --case TXT_020 --case GPH_010 \
  --out target/ae_agents/p2_text_row_sidecar_gate_20260508
```

Result: `ok=true`, and `text_telemetry.jsonl` contains
`ae-native-renderer.text-coverage-row.v1` rows. Example: Montserrat `W` glyph id
`331` emits `120` native coverage row spans in `TXT_010`.

## JSX Crash Fix

The first live probe packs were unsafe because their entry scripts built all
risky variants before remote `--case` filtering. AE could crash or return a
zero-byte output even when one case was requested.

Changed probe shape:

- `p2_text_stroke_live_probe` default entry builds only `STR_LIVE_FILL_ONLY`;
- stroke-only/fill-over/stroke-over each have separate one-case entry scripts;
- `p2_text_transfill_live_probe` default entry builds only the opaque control;
- alpha cases each have separate one-case entry scripts;
- suspicious tight-tracking/duplicate-layer variants were kept out of the live
  default path.

Smoke results on AE85 GUI API `:8001`:

- `p2_stroke_live_fillonly_smoke_20260508_001`: succeeded, one local PNG;
- `p2_stroke_live_strokeonly_smoke_20260508_001`: succeeded, one local PNG;
- `p2_transfill_live_whta128_smoke_20260508_001`: succeeded, one local PNG.

## Stroke Live Lock

Frida command:

```text
python3 scripts/ae_trace_cooltype_text.py \
  --ssh-host ae85 --node http://85.239.48.31:8001 \
  --pack fixtures/ae_probe_pack/p2_text_stroke_live_probe \
  --entry-script jsx/build_STR_LIVE_STROKE_ONLY.jsx \
  --case STR_LIVE_STROKE_ONLY \
  --duration 80 --max-events 2200 \
  --hook-profile txt-are-spans \
  --out-dir target/dynamic_tools_85/p2_stroke_live_strokeonly_trace_20260508_001 \
  --allow-render-failure
```

Key observed facts:

- real stroke pass: `TXT_ARE_Render_8bpc_stroke_3d960`;
- `TXT_ARE_PixelWriter8_span_3b8c0` events: `1909`;
- span types: type `2` coverage `389`, type `1` full spans `219`, type `0`
  transparent/no-op `345`;
- source pixel: `raw=ffff0000`, decoded as `a=255,r=255,g=0,b=0`;
- coverage plane: `137x89`, `stride_0x30=144`, `stride_0x20=0`;
- row getter: `ARE.dll+0x8230`;
- ARE stroke config: fill disabled, stroke enabled, stroke width `14`,
  clip `0,0,137,96`, matrix `[1,0,0,1,4.944,82]`.

## Alpha Fill Live Lock

Frida command:

```text
python3 scripts/ae_trace_cooltype_text.py \
  --ssh-host ae85 --node http://85.239.48.31:8001 \
  --pack fixtures/ae_probe_pack/p2_text_transfill_live_probe \
  --entry-script jsx/build_TRFLIVE_WHT_A128_FILL_OPACITY.jsx \
  --case TRFLIVE_WHT_A128_FILL_OPACITY \
  --duration 80 --max-events 2600 \
  --hook-profile txt-are-spans \
  --out-dir target/dynamic_tools_85/p2_transfill_live_whta128_trace_20260508_001 \
  --allow-render-failure
```

Key observed facts:

- real fill pass: `TXT_ARE_Render_8bpc_fill_3d200`;
- no runtime `PF_TransferRect` enter-events for this case; the hook was only
  installed;
- source pixel at writer is direct alpha: `raw=80ffffff`,
  decoded as `a=128,r=255,g=255,b=255`;
- fill color payload stores alpha-like component first:
  `[0.5019608, 1, 1, 1]`;
- coverage plane: `38x46`, `stride_0x30=48`, `stride_0x20=0`;
- row getter: `ARE.dll+0x8230`.

Native changed accordingly: normal text fill opacity now blends as direct
PF_Pixel8 source alpha, not as "full-alpha temp world then TransferRect".

## Verification

Rust/tests:

```text
cargo test -p text-engine rasterize -- --nocapture
cargo test -p render-core text -- --nocapture
cargo test -p render-cli text_passport -- --nocapture
```

Focused text conformance:

```text
cargo run -p render-cli -- conformance-pack \
  --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case GPH_010 \
  --out target/ae_agents/p2_text_row_alpha_gate_20260508
```

Result: `ok=true` for all 5 cases. Primary visible mean averages:

| Case | Mean |
| --- | ---: |
| `TXT_010` | `14.070786` |
| `TXT_020` | `5.858233` |
| `TXT_030` | `6.502396` |
| `TXT_040` | `3.730953` |
| `GPH_010` | `1.687255` |

## Remaining P2 Work

Next concrete target:

1. Hook `TXT.dll+0x3ba1b` to resolve the exact plane pointer/stride load.
2. Add AE row-span parser that normalizes `TXT_ARE_PixelWriter8_span_3b8c0`
   events into the native `CoverageRowSpan` shape.
3. Compare AE/native rows by case/pass/y/x-range before touching AA/hinting.
4. Only after row ABI matches, implement deeper BIB/CoolType coverage parity:
   hinting, AA/subpixel, clipped rounding, stroke merge.

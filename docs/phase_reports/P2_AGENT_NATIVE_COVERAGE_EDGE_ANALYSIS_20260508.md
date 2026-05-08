# P2 Native Coverage Edge Analysis - 2026-05-08

Agent: P2-C native coverage/edge analysis

## Scope

Goal: use the current row/edge artifacts to decide which native coverage change
is most likely to improve P2 text raster parity after the next producer trace.

No native raster math was changed in this pass. No shared analyzer script was
edited.

Inputs:

- `crates/text-engine/src/rasterize.rs`
- `scripts/compare_text_row_spans.py`
- `scripts/analyze_text_coverage_edges.py`
- `target/ae_agents/p2_row_compare_covw_dense_native_20260508/row_compare.json`
- `target/ae_agents/p2_row_compare_covw_dense_native_20260508/edge_analysis.json`

## Current Native Coverage Backend

The active outline backend is `ttf_outline_nonzero_supersample`:

- source outline: `ttf_parser::Face::outline_glyph`
- fill rule: nonzero winding
- curve handling: local fixed-step flattening
- mask bounds: scaled glyph bbox, `floor(x_min/y_min)` and `ceil(x_max/y_max)`
- AA: fixed `OUTLINE_COVERAGE_SUPERSAMPLE = 4`
- sample grid: centered `4 x 4` subpixel points
- coverage quantization: `round(covered / 16 * 255)`
- placement: `raster_x = glyph_x + x_min`, `raster_y = baseline - y_max`
- row placement/output: `x.round()` / `y.round()` integer origin
- native row spans: contiguous nonzero coverage bytes only

This means native can only produce the 4x ladder:

```text
0, 16, 32, 48, 64, 80, 96, 112, 128, 143, 159, 175, 191, 207, 223, 239, 255
```

That ladder is visible in the dense artifact: native rows have 16 unique byte
values across `3833` bytes.

## Current Row/Byte Deltas

From `edge_analysis.json`:

```text
case: COV_W
AE merged ink rows: 202
native rows: 203
matched-y rows with same run count: 67 / 68
matched runs with edge comparison: 199
exact edge match: 139 / 199
one-pixel edge variants: 60 / 199
overlap byte-exact rows: 0 / 199
```

Edge deltas among the 199 comparable runs:

| native start - AE start | native end - AE end | count | read |
| --- | --- | ---: | --- |
| 0 | 0 | 139 | same run bounds, bytes still differ |
| +1 | 0 | 35 | native starts one pixel later |
| 0 | -1 | 17 | native ends one pixel earlier |
| +1 | -1 | 8 | native narrower by one pixel on both sides |

Row/coverage lengths:

| source | rows | total bytes | avg bytes/row | extent width |
| --- | ---: | ---: | ---: | ---: |
| AE merged ink | 202 | 3903 | 19.32 | 109 |
| native | 203 | 3833 | 18.88 | 108 |

Byte distribution notes:

- Native: 16 unique values, exactly the expected 4x quantization ladder.
- AE merged rows: 80 unique values across `3903` bytes. Some bytes are `0`
  inside merged edge/full/edge ink rows because AE emits type-2 edge spans
  adjacent to type-1 solid spans, while native row spans currently skip zero
  coverage bytes.
- In the top 24 overlap byte deltas, positive deltas dominate (`174` positive
  vs `20` negative). This is not a full distribution, but the examples show
  native often overestimates edge coverage after alignment.

Representative examples:

```text
norm_y=0, run [0,16]:
AE     24 40 ... 40 3c
native 20 40 ... 40 40

norm_y=1, middle run [46,62]:
AE     33 ff ... ff 00
native 40 ff ... ff af

norm_y=2, run near x=45:
AE     00 0d ff ... ff 00
native bf ff ... ff bf
```

The topology is now close enough that the main unknown is coverage-generation
policy, not TXT writer/composite semantics.

## Controlled Implementation Hypotheses

Do not implement these until a producer trace gives the missing evidence. The
right next trace is a dense `COV_W` capture with authoritative `3ba5b`/`RBX`
or `3ba80` coverage bytes, not the legacy plane snapshot.

| Hypothesis | Evidence needed from producer trace | Expected metric movement | Risk |
| --- | --- | --- | --- |
| Origin rounding should preserve fractional placement into coverage generation, not only round final bitmap origin | For each glyph, capture BIB/ARE input matrix or coverage plane left/top plus first rows. Compare AE edge movement under fractional text positions against native `raster_x/raster_y` and row start/end. | Edge deltas should move from `(+1,0)`, `(0,-1)`, `(+1,-1)` toward `(0,0)`; common normalized shape should increase beyond current 140 common merged rows. Byte parity may improve only at edges. | Medium. Changing placement can shift all text cases and can regress cases where current integer placement accidentally matches. |
| Edge inclusion policy differs: AE keeps/writes zero or near-zero edge cells inside type-2 spans, while native emits only nonzero contiguous runs | Dense authoritative type-2 byte rows, including leading/trailing zeros, plus final image diff to determine whether zero bytes are meaningful for row topology only or for rendered pixels. | Row span topology may match AE better: native rows could gain AE-like edge cells, reducing one-pixel narrow runs. Pixel metrics may not move if zeros remain no-ops. | Low to medium. Safe for trace parity, but can pollute row telemetry unless output blend still skips zero coverage. |
| Supersample grid is not fixed 4x centered points; AE appears analytic or higher/finer grid | Authoritative AE coverage bytes for several rows with exact outline/matrix inputs. Check whether bytes are constrained to 4x/8x ladders or arbitrary values. | Overlap byte equality should improve substantially; native unique byte count should grow beyond 16 and edge bytes like `101`, `116`, `77`, `56` become reachable. | Medium/high. Higher sampling costs runtime; analytic rasterization changes many bytes and needs caching/perf guardrails. |
| Gamma/quantization differs after geometric coverage | Trace rows where geometry is otherwise aligned and compare predicted linear area values against AE bytes. Need rows with same start/end and mostly simple diagonal/stem edges. | If geometry already matches, byte deltas shrink without major row topology changes. Current positive deltas could reduce if AE applies non-linear or different rounding around partial pixels. | Medium. A gamma LUT can overfit one font/size unless validated across multiple glyphs and opacity cases. |
| Outline flattening tolerance is too coarse or not CoolType-compatible | Producer trace of simple curves and diagonals; compare native rows before/after only changing flattening tolerance locally. Need evidence that mismatches cluster on curves rather than straight stems. | Byte deltas on curved outer edges should improve; straight vertical/horizontal stems should barely move. Little change expected to run counts. | Low/medium if made adaptive and cached, but it may cost raster time and is unlikely to explain the 4x-byte ladder by itself. |
| Fill rule differs from nonzero winding | Capture glyphs with holes/overlaps and compare AE span interiors against nonzero/even-odd predictions. Current `W` evidence does not isolate this. | For `COV_W`, expected movement is minimal. For counters/overlapping contours, wrong fill rule would flip whole interior spans. | Low immediate value for P2 `COV_W`; high visual risk if changed globally without counter-glyph evidence. |

## Recommended Order After Producer Trace

1. First classify AE bytes by ladder. If dense authoritative bytes are not
   constrained to native's 4x ladder, supersample/analytic coverage is the
   highest-probability native change.
2. Separately compare edge cells including zero bytes. If AE spans include
   leading/trailing zero coverage but final pixels do not change, keep blend
   semantics unchanged and consider a trace-only topology adjustment.
3. Only then test origin rounding. The current one-pixel edge pattern is real,
   but byte mismatch exists even when bounds match, so placement alone is not
   enough.
4. Treat gamma/quantization as a second-stage fit after geometry and sample
   policy are identified.

## Analyzer Notes

I did not edit `scripts/compare_text_row_spans.py` or
`scripts/analyze_text_coverage_edges.py`.

Useful private/local analyzer additions for a future pass:

- report value-ladder classification for AE/native bytes;
- include edge-zero accounting separately from rendered nonzero pixels;
- split byte deltas by edge position vs interior type-1/full-span region;
- prefer `actual_coverage_sample_hex` whenever present and label legacy plane
  bytes as non-authoritative.

These can be added as an agent-local script first, then promoted by the
orchestrator if the producer trace confirms they are useful.

## Native Patch Readiness

Most likely P2 text-raster gain after trace:

```text
supersample grid / analytic coverage policy
```

Reason: current native topology is already close after AE ink-row merging, but
overlap byte equality is zero across all 199 comparable runs and native byte
values are visibly limited by the 4x quantization ladder. Origin/edge inclusion
should be tested too, but they mostly explain the remaining one-pixel topology
errors, not the broad byte mismatch on already-aligned runs.

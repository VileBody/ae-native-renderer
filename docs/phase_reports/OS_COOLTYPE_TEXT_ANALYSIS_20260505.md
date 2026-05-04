# CoolType Text Analysis Orchestration

Date: 2026-05-05

## Iteration Goal

Move `M05`/`M07` text/glyph work from "CoolType targets selected" to
"implementation-ready passports":

```text
function targets
  -> slot/signature passports
  -> glyph metric record contract
  -> native telemetry gap list
  -> first testable implementation slice
```

This iteration is not expected to lock AE text raster parity. It should make
text metric tuning factual enough that we can compare glyph rows and selector
units before interpreting final PNG diffs.

## Raw Inputs

- `target/reverse/predecoded/20260505_005411_cooltype_glyph_metrics_intake/cooltype_glyph_metrics/`
- `target/reverse/predecoded/20260505_005459_cooltype_glyph_metrics_core_expanded/cooltype_glyph_metrics_core/`
- `docs/phase_reports/GHIDRA_COOLTYPE_GLYPH_METRICS_20260505.md`

## Agent Assignments

| Agent | Scope | Required output |
| --- | --- | --- |
| A / wrapper signatures | `CTFontInstanceInterface` and `CTFontInstanceInterfaceV2` slot tables. | Slot map, likely signatures, return/null/error conventions, global guard behavior. |
| B / glyph id + features | `GetGlyphID`, `GetGlyphIDs`, `ApplyFeatures`, `ProcessFeatures`, core glyph id batch. | Glyph id path, batch record shape, feature processing range semantics, telemetry fields. |
| C / width bbox baseline | `GetWidth(s)`, `GetBBox(es)`, `GetBaselineDeltas`, hmtx/vmtx, fixed scale. | Metric formula passport and implementation guardrails. |
| D / CTText glyph rows | `CTTextGetGlyphsV2`, `CTTextGetTextGlyphs`, `CTGlyphAccess`. | CTText glyph pointer/row contract and raster-adjacent unknowns. |
| E / native telemetry plan | `crates/text-engine`, `render-core`, conformance sidecars. | Native field gap list, first implementation slice, tests/commands. |

## Shared Guardrails

Agents must escalate instead of silently assuming if they hit any of these:

- a glyph record layout wider or different than the observed 12-byte
  `(glyph id + two metric slots)` pattern;
- an alternate scale other than `DAT_180322b20 = 1/65536`;
- writing-direction semantics that swap not only width axis but bbox/baseline
  coordinate order;
- CTText glyph rows carrying composer/shaping data that cannot be reproduced
  from `CTFontInstanceGetGlyphIDs` alone;
- raster coverage, antialiasing, hidden RGB, or premult policy leaking into
  what looks like a glyph metric formula;
- feature processing changing glyph count/order, not just glyph ids;
- font fallback or family/style resolution changing Point-Light/Montserrat
  identity before metric comparison.

## Acceptance For This Iteration

We can move to implementation once the agent reports answer these questions:

1. Which native telemetry fields are currently missing from CoolType's metric
   contract?
2. Which fields can be emitted without changing render math?
3. Which fields require a shaping/layout backend decision, not just telemetry?
4. What is the smallest Rust change that makes `TXT_010..TXT_040` comparable
   at glyph-row level?
5. Which final-pixel diffs remain blocked by raster/alpha/collapse rather than
   glyph metric formulas?

## Expected Progress

Best-case result for this iteration:

- `M05` reaches `instrumented/testable` with a concrete telemetry patch plan.
- `M07` text animator gets cleaner unit references: glyph-run order, selector
  unit rects, and final transform records can be audited separately.
- `GPH_010` collapse sharpness remains blocked until glyph placement and text
  raster substrate are measured independently.

Non-goals for this iteration:

- full CoolType replacement;
- full paragraph composer parity;
- arbitrary OpenType/CID edge cases;
- pixel-perfect text antialiasing.

## Agent Result Summary

Status: completed. This analysis pass is enough to start the next native
instrumentation slice. It is not enough to tune final text pixels.

### A / Wrapper Signatures

Accepted findings:

- `CTFontInstanceInterfaceV2_register` at `0x180290bc0` registers 24 slots.
- `CTFontInstanceInterface_register` at `0x1802907d8` registers 25 slots.
- `ProcessFeaturesV2` is on `CTFontInstanceInterface`, not on
  `CTFontInstanceInterfaceV2`. Do not treat V2 as a strict superset.
- Normal CoolType wrappers return `0` on success; many bad inputs go through
  non-return fatal helpers, not error codes.
- Public glyph metric rows are 12 bytes; internal temp rows often use 24 bytes.
- Most wrappers use global shutdown/lifetime guard `DAT_1804064e8`, with
  critical section at `+0x28`, refcount at `+0x58`, and event at `+0x50`.

Key slot/proc map:

| API | Proc | Meaning |
| --- | --- | --- |
| `GetGlyphID V2` | `0x1802920b0` | encoded input -> glyph id + consumed units |
| `GetGlyphIDs V2` | `0x180292260` | batch glyph ids into 12-byte public rows |
| `GetWidth` | `0x180292900` | single x/y advance with writing-direction selection |
| `GetWidths` | `0x180292d20` | batch x/y advances in 12-byte rows |
| `GetBBox` | `0x1802915e0` | single glyph bbox |
| `GetBBoxes` | `0x180291900` | batch glyph bboxes |
| `GetBaselineDeltas` | `0x180291cd0` | baseline pair delta |
| `ApplyFeatures V2` | `0x180290f80` | mutates glyph rows and optional metric slots |
| `ProcessFeatures` | `0x180294420` | feature run processing |
| `ProcessFeaturesV2` | `0x1802941a0` | feature run processing, old interface slot |
| `CTTextGetGlyphsV2` | `0x1802a02a0` | CTText-owned glyph pointer groups |
| `CTTextGetTextGlyphs` | `0x1802a1380` | CTText 48-byte row extraction |

### B / Glyph Id And Features

Accepted findings:

- `GetGlyphID_V2` input should be modeled as encoded bytes/units, not Unicode
  scalar index. `outConsumed` is consumed input length, not glyph count.
- `CoreUnicodeToGlyph` uses font dict at `fontInstance + 0x30`, encoding/code
  page object at `+0x170`, and writing-direction flag at `+0x178`.
- `CoreGlyphIDsBatch` writes internal glyph ids at internal row `+8`, stride
  `24`; the wrapper copies them into external 12-byte rows at `+0`.
- `ApplyFeatures` / `ProcessFeatures` can mutate glyph ids, change count/order,
  split runs, and apply ligatures/small-caps/component-specific behavior.
- We must not assume `codepoint == glyph_run_index == glyph_id`.

Required telemetry before glyph parity tuning:

- raw input bytes or UTF-16 units;
- consumed units per glyph;
- pre-feature glyph id rows;
- feature tags/script/language/range;
- post-feature glyph run order/count;
- source cluster/range mapping.

### C / Width, BBox, Baseline

Accepted findings:

- Metric scale is locked for this pass:

```text
DAT_180322b20 = 1.52587890625e-05 = 1 / 65536
float_metric = fixed_i32 * DAT_180322b20
```

- Width rows:

```text
12-byte public record:
  +0 u32 glyph_id
  +4 x/horizontal advance slot
  +8 y/vertical advance slot
```

- Single `GetWidth` returns x unless `fontInstance + 0x178 == 1`, then y.
- BBox output order is `[xmin, ymin, xmax, ymax]`, not `[x, y, w, h]`.
- BBoxes may be transformed through `fontInstance + 0x1c`; Type3 and synthetic
  scalar paths have special handling.
- Baseline deltas use:

```text
axis_index = baseline_index * 2 + (writing_dir == 1 ? 1 : 0)
delta_fixed = baselines[from_axis] - baselines[to_axis]
```

Baseline indices are bounded to `< 8`; writing direction is bounded to `< 2`.

### D / CTText Rows

Accepted findings:

- Final AE-like row/raster parity should key off CTText, not only
  CTFontInstance.
- `CTTextGetNumGlyphs` derives count as `text + 0x50 / 0xc`.
- CTText source rows are 12-byte records copied from `text + 0x48`.
- `CTTextGetTextGlyphs` extracts 48-byte output rows; capacity is `param_8`.
- Observed output row fields include floored x/y raster origins, payload pointer
  or null/sentinel, copied payload fields, and flags.
- `CTTextGetGlyphsV2` returns three CTText-owned pointer groups; callers must
  honor `CTTextReleaseGlyphPointers`.

Unknowns that remain:

- exact raster coverage buffer format;
- antialiasing/hinting kernel;
- exact lifetime rules behind indirect release calls;
- translator/features/language/access-path object internals beyond glyph id.

### E / Native Telemetry Plan

Accepted findings:

- Native already emits font resolution, `font_glyph_id`, scalar `advance`,
  bbox, baseline, selector weights, and animator contribution for active
  transform/blur/expression paths.
- Missing or partial fields: explicit `glyph_run_index`, glyph-run mapping per
  selector unit, `advance_x/y`, baseline delta, source cluster/range,
  pre/post-shape stage labels, final opacity for opacity-only animator paths,
  and per-unit/glyph blur/matrix aliases.
- We can add most of this as instrumentation without changing render math.

## Orchestrator Decision

Move to native instrumentation, not formula tuning.

Approved first implementation slice:

```text
CoolType glyph passport in text sidecar
```

Scope:

- no pixel math change;
- no shaping backend change yet;
- additive JSON telemetry fields only;
- source-label current metrics as `fontdue` / `not_cooltype_verified`.

Files likely touched:

- `crates/render-core/src/layer_eval.rs`
- optional follow-up: `crates/text-engine/src/layout.rs`
- optional follow-up: `crates/text-engine/src/glyph.rs`

Required new `text_telemetry.jsonl` fields:

- per selector unit: `glyph_run_indices`, `char_indices`, `font_glyph_ids`,
  `glyph_bboxes`, `glyph_advances`, `baselines`;
- per unit contribution: `final_matrix`, `final_opacity_alpha_scale`,
  `blur_radius_px`, even for opacity-only paths;
- layout glyphs: explicit `glyph_run_index`, `advance_x`, `advance_y`,
  `metric_source`, and `cooltype_parity`.

Commands for the implementation slice:

```bash
cargo test -p render-core text_animator
cargo test -p text-engine real_layout
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/native_text_cooltype_passport \
  --case TXT_030 --case TXT_040
```

## Current Status After Analysis

| Module | Before | After |
| --- | --- | --- |
| `M05` glyph layout | targets selected | implementation-ready telemetry passport |
| `M06` range selector | approximate | can be checked against glyph-run-linked units |
| `M07` glyph animator | approximate | can be checked with unit/glyph refs plus final matrix/opacity/blur |
| `M17` collapse text | blocked by glyph/raster substrate | still blocked, but next probe can separate glyph placement from raster sharpness |

Do not tune final PNGs yet. The next useful milestone is sidecar parity:
native glyph/unit records should line up with AE probes well enough that pixel
diffs can be attributed to glyph metrics, raster coverage, alpha/composite, or
collapse separately.

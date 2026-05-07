# P2 Text Raster Subagent Orchestration

Date: 2026-05-07

## Goal

Parallelize the remaining P2 text raster work without turning the AE node or
shared renderer files into a collision point.

Current P2 state:

```text
closed:
  sourceRect/layout passports
  TXT_DrawChar boundary
  TTF outline fallback coverage
  8 bpc TXT ARE PF_Pixel8 source-over writer

open:
  literal CoolType/BIB coverage rows
  hinting/grid-fit / AA / subpixel policy
  clipped-bounds rounding at coverage generation time
  stroke path and fill/stroke merge
  semi-transparent fill temp-world / PF_TransferRect behavior
```

## Shared Rules

All agents must follow this sequence:

```text
static analysis first
  -> short brief in owned report
  -> tiny probe pack if static brief justifies it
  -> one remote render/trace attempt
  -> final evidence and next native change candidate
```

Remote AE node:

```text
http://85.239.48.31:8001
ssh alias: ae85
```

Render jobs must use unique `job_id` prefixes and own output directories.
Agents must stop and escalate on:

```text
AE modal / crash repair dialog
render timeout
API busy/lock ambiguity
AfterFX crash
Frida attach failure that would require broad shared-script changes
any new shared ABI/policy issue
```

Shared files are not agent-owned:

```text
crates/**
scripts/ae_trace_cooltype_text.py
docs/reverse_engineering/GHIDRA_PREDECODE_TASKS.json
fixtures/ae_probe_pack/cooltype_text_raster/**
```

Agents may propose changes to these files in their reports, but the
orchestrator integrates them after review.

## Agent Board

| Agent | Scope | Owned report | Owned probe pack | Job prefix |
| --- | --- | --- | --- | --- |
| Franklin `019e040c-2f2c-7383-b30f-6367eb98b2dd` | Literal CoolType/BIB coverage rows | `docs/phase_reports/P2_AGENT_COVERAGE_ROWS_20260507.md` | `fixtures/ae_probe_pack/p2_text_coverage_rows_probe/` | `p2_cov_rows_` |
| Nietzsche `019e040c-81e0-7a63-bd22-e14235b20525` | Hinting/grid-fit/AA/subpixel | `docs/phase_reports/P2_AGENT_HINTING_AA_SUBPIXEL_20260507.md` | `fixtures/ae_probe_pack/p2_text_hinting_probe/` | `p2_hinting_` |
| Socrates `019e040c-e4d7-7882-9f41-c2ebddc39a70` | Clipped-bounds rounding | `docs/phase_reports/P2_AGENT_CLIPPED_BOUNDS_20260507.md` | `fixtures/ae_probe_pack/p2_text_clip_probe/` | `p2_clip_` |
| Wegener `019e040d-4e4b-7cb1-9ac2-0bde30e4bdd2` | Stroke path / fill-stroke merge | `docs/phase_reports/P2_AGENT_STROKE_MERGE_20260507.md` | `fixtures/ae_probe_pack/p2_text_stroke_probe/` | `p2_stroke_` |
| Locke `019e040d-971a-7171-b7b6-f93e1d90566b` | Semi-transparent fill / PF_TransferRect | `docs/phase_reports/P2_AGENT_SEMITRANSPARENT_FILL_20260507.md` | `fixtures/ae_probe_pack/p2_text_transfill_probe/` | `p2_transfill_` |

## Collision Guardrails

Known shared substrates:

```text
PF_World layout/depth/rowbytes
TXT/BIB path object ABI
font dict ABI
glyph/text matrix convention
coverage row/span buffer ABI
PF_TransferRect opacity/composite policy
stroke/fill order byte semantics
```

If any agent finds a fact that changes one of these, they must label it:

```text
ESCALATE_TO_ORCHESTRATOR
```

and avoid making behavior changes locally.

## Initial Node Check

At orchestration start:

```text
curl http://85.239.48.31:8001/health
=> {"status":"ok"}
```

## Integration Gate

No native P2 changes should be merged until at least one of these is true:

```text
1. static decompile proves the formula/branch and focused Rust tests cover it;
2. dynamic trace proves the ABI/branch and focused probe PNG/TIFF confirms it;
3. agent result is report-only and explicitly marks implementation blocked.
```

After any native change:

```text
python3 -m py_compile scripts/ae_trace_cooltype_text.py scripts/ae_remote_pack.py
cargo test -p text-engine
cargo test -p render-core
python3 scripts/run_master_conformance_gate.py --no-fail
```

The master gate can be deferred only if the change is docs/probe-only.

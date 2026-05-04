# Ghidra BEE Predecode Intake

Status date: 2026-05-05

## Result

`BEE.dll` was imported into a dedicated local Ghidra project:

```text
target/reverse/ghidra_projects/agent_bee_temporal/agent_bee_temporal.gpr
```

The focused prepared bundle was then generated at:

```text
target/reverse/predecoded/20260505_002125_bee_temporal_scheduler_intake/bee_temporal_scheduler
```

This closes the previous artifact gap where `bee_temporal_scheduler` had target
RVAs but no dedicated predecoded function bundles.

## Command

```bash
mkdir -p target/reverse/ghidra_projects/agent_bee_temporal
/Applications/ghidra_12.0_PUBLIC/support/analyzeHeadless \
  target/reverse/ghidra_projects/agent_bee_temporal \
  agent_bee_temporal \
  -import target/reverse/ae_2026/core_composite_alpha/BEE.dll \
  -overwrite

python3 scripts/prepare_ghidra_bundles.py \
  --task bee_temporal_scheduler \
  --include-disabled \
  --run-name bee_temporal_scheduler_intake
```

## Target Map

| Label | Function |
| --- | --- |
| `BEE_candidate_4ba230` | `BEE_RenderOptions::GetShutterSampleInfo` |
| `BEE_candidate_4ba2f0` | `BEE_RenderOptions::GetShutterStartTime` |
| `BEE_candidate_4b96d0` | `BEE_RenderOptions::GetFrameShutterRange` |
| `BEE_candidate_4bac20` | `BEE_RenderOptions::GetTime` |
| `BEE_candidate_454d00` | `BEE_GetLayerROAndTimeFromCompRO` |
| `BEE_candidate_7d72c0` | `BEE_WorkQueue_CheckoutLayerFrame` |

Each target has:

```text
decompile.c
disassembly.txt
metadata.md
callers.tsv
callees.tsv
data_refs.tsv
refs.tsv
```

## Why It Matters

This is the first concrete BEE-side bundle for:

- motion blur shutter range and sample scheduling;
- comp-time to layer-time conversion;
- expression host time context;
- work-queue layer frame checkout.

Agents should use this before tuning motion blur or generic expression timing.

## Remaining Gap

`CoolType.dll` is still only collected as a binary/string/source dependency.
Exact CoolType function targets for glyph metrics are not selected yet.

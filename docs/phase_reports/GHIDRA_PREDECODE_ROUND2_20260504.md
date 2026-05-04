# Ghidra Predecode Round 2

Status date: 2026-05-04

## Result

Prepared bundle run:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

The run used:

```text
scripts/prepare_ghidra_bundles.py
scripts/ghidra/PreparedReverseBundle.java
docs/reverse_engineering/GHIDRA_PREDECODE_TASKS.json
```

All 14 enabled tasks completed with return code `0`. The disabled
`bee_temporal_scheduler` task remains disabled until `BEE.dll` is imported into
a Ghidra project. `extendscript_expression_engine` is also disabled until exact
generic ExtendScript function targets are selected; the AE-specific expression
host pointers are covered by `scripting_expression_host`.

## Bundle Coverage

| Task | Targets |
| --- | ---: |
| `core_alpha_gpufoundation` | 8 |
| `geometry_transform_gpufoundation` | 8 |
| `geometry_transform_wrapper` | 4 |
| `posterize_time` | 4 |
| `time_displace_temporal` | 6 |
| `blur_gpufoundation_kernels` | 8 |
| `box_blur_aex` | 8 |
| `drop_shadow_aex` | 8 |
| `glow_aex` | 7 |
| `imagerenderer_gaussian_composite` | 6 |
| `minimax_aex` | 7 |
| `turbulent_displace_aex` | 9 |
| `basic_text_aex` | 10 |
| `scripting_expression_host` | 14 |

Total: 107 target entries. Six `scripting_expression_host` entries are
intentional `.rdata` pointer targets (`<data/no function>`) with incoming refs,
not functions.

## Known Extraction Notes

- `core_alpha_gpufoundation/02_GF_Transfer_18001e3e0/decompile.c` contains a
  Ghidra decompiler failure: `Low-level Error: Free varnode has multiple
  descendants`.
- The same `GF_Transfer` bundle still has `metadata.md`, `disassembly.txt`,
  `callees.tsv`, `callers.tsv`, `data_refs.tsv`, and `refs.tsv`.
- `scripting_expression_host` includes BEE/time bridge pointers such as
  `PTR_GetTimePalSuite`, `PTR_BEE_FpLongToTime`, and
  `PTR_BEE_CompToLayerTime`. For these entries, read `data_target.md` first.
- No raw decompiler output was copied into tracked docs; agent-facing raw
  artifacts stay under ignored `target/reverse/predecoded/`.

## Handoff

The next agent round should read only the relevant task folder plus the phase
brief. Suggested split:

| Agent | Folder |
| --- | --- |
| Alpha/composite | `core_alpha_gpufoundation`, `blur_gpufoundation_kernels` |
| Geometry/collapse/motion | `geometry_transform_wrapper`, `geometry_transform_gpufoundation` |
| Temporal/motion blur | `posterize_time`, `time_displace_temporal` |
| Blur/shadow/glow | `box_blur_aex`, `drop_shadow_aex`, `glow_aex`, `imagerenderer_gaussian_composite` |
| Minimax/turbulent | `minimax_aex`, `turbulent_displace_aex` |
| Text/expression | `basic_text_aex`, `scripting_expression_host` |

Each agent should answer concrete parity questions with addresses, inferred
formula/policy, uncertainty level, and the next probe or native test needed.

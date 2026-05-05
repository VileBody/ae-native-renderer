# Ghidra Predecode Bundles

Status date: 2026-05-05

This is the intermediate layer before sending analysis tasks to agents. Instead
of asking every agent to import/open Ghidra projects, run one prepared pass that
writes function-level artifacts into ignored `target/reverse/predecoded/`.

## Why

Agents should spend time answering parity questions, not fighting Ghidra
startup, project locks, or address collection. A predecode bundle gives them:

- `metadata.md`: program, image base, function entry/name/body;
- `decompile.c`: bounded Ghidra C output;
- `disassembly.txt`: instruction listing for the function body;
- `callees.tsv`, `callers.tsv`, `data_refs.tsv`, `refs.tsv`: edges and literal
  references.

Raw decompiler output stays under `target/`. Durable findings should still be
summarized in `docs/phase_reports/`.

## Commands

List tasks:

```bash
python3 scripts/prepare_ghidra_bundles.py --list
```

Run one task:

```bash
python3 scripts/prepare_ghidra_bundles.py --task geometry_transform_wrapper
```

Run an area:

```bash
python3 scripts/prepare_ghidra_bundles.py --area effects --skip-missing
```

Run all enabled tasks:

```bash
python3 scripts/prepare_ghidra_bundles.py --all --skip-missing
```

The runner uses:

```text
Ghidra home default: /Applications/ghidra_12.0_PUBLIC
Shared lock:         /tmp/ae-native-renderer-ghidra.lockdir
Manifest:            docs/reverse_engineering/GHIDRA_PREDECODE_TASKS.json
Script:              scripts/ghidra/PreparedReverseBundle.java
Output:              target/reverse/predecoded/<timestamp>_<run_name>/
```

Set `GHIDRA_HOME` or pass `--ghidra-home` if the install lives elsewhere.

## Current Task Map

The task manifest currently covers:

| Task | Area | Main question |
| --- | --- | --- |
| `core_alpha_gpufoundation` | `core_alpha` | Premult, alpha gain, transfer/composite substrate. |
| `geometry_transform_gpufoundation` | `geometry` | Matrix construction, bounds, sampler quality, motion blur matrix path. |
| `geometry_transform_wrapper` | `geometry` | Geometry2 AE wrapper and param mapping. |
| `posterize_time` | `temporal` | Bucket formula and frame-rate checkout. |
| `time_displace_temporal` | `temporal` | Time/source sampling references. |
| `blur_gpufoundation_kernels` | `effects_blur` | Shared GF blur kernels and alpha options. |
| `box_blur_aex` | `effects_blur` | Box Blur wrapper and delayed `GF::FastBoxBlur` calls. |
| `drop_shadow_aex` | `effects_shadow` | Offset, softness, shadow alpha/composite candidates. |
| `drop_shadow_followup` | `effects_shadow` | M10 upstream direction/distance producer and local shadow helper context. |
| `glow_aex` | `effects_glow` | Glow mask/render wrapper and IR routing. |
| `glow_aex_followup` | `effects_glow` | M11 8/16/32 bpc bodies and Glow-local threshold/radius/composite helpers. |
| `imagerenderer_gaussian_composite` | `effects_glow` | Real `IR_GaussianBlur` implementation and blend dispatch. |
| `imagerenderer_composite_workers` | `effects_glow` | Concrete ImageRenderer composite worker bodies selected by the worker selector. |
| `minimax_aex` | `effects_minimax` | 8/16/32 bpc callbacks, GPU path, comparators. |
| `turbulent_displace_aex` | `effects_turbulent` | Param setup, lookup tables, 1D/all kernel dispatch. |
| `basic_text_aex` | `text` | Text/glyph/CoolType candidates. |
| `cooltype_glyph_metrics` | `text` | Concrete CoolType proc bodies for glyph id, widths, bboxes, baselines, feature processing, and CTText glyph access. |
| `cooltype_glyph_metrics_core` | `text` | Core callees behind CoolType glyph metrics, including fixed-point scale and hmtx/vmtx lookup. |
| `scripting_expression_host` | `expression` | Scripting.aex host bridge, ExtendScript delay-loads, BEE/time pointer table. |
| `extendscript_expression_engine` | `expression` | Disabled until exact generic ExtendScript function targets are selected. |
| `bee_temporal_scheduler` | `temporal` | BEE shutter/time/layer checkout targets; run with `--include-disabled` when the local `agent_bee_temporal` project exists. |

## Agent Handoff Pattern

Give each agent a concrete folder and questions. Example:

```text
Read target/reverse/predecoded/<run>/drop_shadow_aex/.
Answer:
1. Which function computes dx/dy from direction+distance?
2. Is softness routed to GF::FastBoxBlur or another kernel?
3. What alpha/premult flags are passed into BoxBlurOptions and composite?
4. List exact addresses and uncertainty labels.
```

Keep agent outputs as formula notes and implementation candidates, not raw
decompiler dumps.

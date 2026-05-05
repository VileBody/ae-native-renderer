# Reverse Engineering Workflow

This directory documents the safe reverse-engineering lane for AE parity work.
It is intentionally documentation-first: formulas, fixture intent, native test
targets, and AE golden links should be committed here or in nearby phase reports;
copied Adobe binaries, Ghidra projects, and raw headless logs should stay under
ignored `target/reverse/`.

## Scope

Use this workflow when a native parity blocker needs evidence from AE binaries,
AE probes, or both. Keep the chain explicit:

```text
formula hypothesis
  -> fixture or probe case
  -> native unit/conformance test
  -> AE golden PNGs or probe telemetry
  -> phase report / reverse note
```

The goal is not to collect interesting disassembly. The goal is to turn one
small recovered behavior into a deterministic check that another agent can run
or audit without reopening the same binary.

## Repository Layout

- `target/reverse/ae_2026/`: local copied AE `.aex` / `.dll` inputs, hashes,
  Ghidra logs, and other ignored scratch artifacts. Example current inputs:
  `Transform.aex` and `GPUFoundation.dll`.
- `target/reverse/ghidra_scripts/`: local headless helper scripts used during
  exploration. Commit only if the team decides a script is reusable and free of
  proprietary material.
- `docs/reverse_engineering/`: stable workflow docs and cross-links.
  Start M19/color-alpha work from `M19_REVERSE_LOCK.md`, then use
  `MATH_CONTRACTS_GUARDRAILS.md` for cross-module guardrails. For remaining
  module work, use `MODULE_HYPOTHESIS_VALIDATION_PLANS.md` before implementing
  formula candidates.
- `docs/phase_reports/`: dated findings and module-specific reverse notes, for
  example `AE_REVERSE_GEOMETRY2_GHIDRA.md`.
  The current artifact intake checklist is
  `docs/phase_reports/ARTIFACT_INTAKE_20260505.md`.
- `fixtures/conformance/`: small native scenes and manifest entries for
  focused parity fixtures.
- `fixtures/ae_conformance_pack/`: AE-generated pack, manifest, checked-in test
  assets, and AE golden metadata.
- `fixtures/ae_probe_pack/`: larger exploratory AE probe packs and captured
  output metadata.

Do not commit or redistribute Adobe binaries. The repository `.gitignore`
already ignores `target/`, `.aex`, `.dll`, `.aep`, and most generated media; keep
new binary copies inside ignored locations anyway.

## Parallel Safety

Many agents may work at the same time, so reverse work needs ownership by
artifact path and by tool state.

- Own a narrow topic before writing docs, such as `geometry2_matrix`,
  `minimax_radius`, or `motion_blur_sampling`.
- Use per-topic output paths:
  `target/reverse/<agent_or_topic>/<YYYYMMDD_HHMMSS>/` for raw logs and
  `docs/phase_reports/<TOPIC>_<TOOL_OR_METHOD>.md` for durable conclusions.
- Never rewrite another agent's phase report in place unless your task is to
  edit that report. Add an addendum section with a date or create a new report.
- If running Ghidra headless against a shared project, take
  `/tmp/ae-native-renderer-ghidra.lock` with `flock`.
- If you do not want to wait on the lock, create a separate Ghidra project dir
  under `target/reverse/ghidra_projects/<topic>/` and import binaries there.
- Put large generated outputs in `target/reverse/...`, not in docs.
- Commit only summarized evidence: symbol names, addresses, pseudocode
  summaries, hashes, command recipes, and conclusions. Avoid long pasted
  decompiler listings.

## Naming Output Docs

Use predictable names so agents can discover work with `rg`:

```text
docs/phase_reports/AE_REVERSE_<MODULE>_<METHOD>.md
docs/phase_reports/AE_PROBE_<MODULE>_<ROUND_OR_DATE>.md
docs/phase_reports/AGENT_<MODULE>_NATIVE_DIFF.md
```

Prefer upper snake case for phase reports. Include at the top:

- status date;
- AE version or node/source;
- input file hashes when binaries were used;
- exact fixture/probe/golden paths;
- whether the result is observed, inferred, or still speculative.

If a formula becomes canonical, link the phase report from the module doc
(`docs/EFFECTS.md`, `docs/MOTION_BLUR.md`, etc.) or from
`docs/MATH_PARITY_STATUS.md` in a separate documentation task.

## Formula To Golden Checklist

1. Write the recovered or hypothesized formula in a phase report, including
   coordinate conventions, units, matrix order, premultiplication/alpha policy,
   and any inferred signs.
2. Add or identify a minimal fixture that isolates the behavior. Use
   `fixtures/conformance/scenes/*.json` for native micro-scenes, or add a case
   to the AE conformance/probe pack when AE must produce new truth data.
3. Link the fixture to a native check. For native conformance pack checks, the
   runner writes `scene.json`, native frames, copied AE frames, diffs, and
   `metrics.json` under `target/ae_conformance_native/...`.
4. Produce or point to AE goldens:
   `fixtures/conformance/ae-reference/<case>/` for small checked fixtures, or
   `fixtures/ae_conformance_pack/ae_goldens/png/<case_id>/` for pack cases.
5. Record the metric outcome and threshold intent. Distinguish "measured" from
   "enforced": a measured diff proves only that the comparison ran.
6. Update the phase report with links back to formula, fixture, native test, and
   AE golden paths. A future agent should not need to guess which evidence
   supported the change.

## Command Recipes

Run a focused native conformance pack case:

```bash
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_conformance_native_<topic> \
  --case <CASE_ID>
```

Run a small fixture comparison once AE reference PNGs exist:

```bash
cargo run -p render-cli -- render \
  --scene fixtures/conformance/scenes/<case>_scene.json \
  --out target/conformance/native/<case>

cargo run -p render-cli -- compare \
  --native target/conformance/native/<case> \
  --reference fixtures/conformance/ae-reference/<case> \
  --out target/conformance/diff/<case> \
  --frames <N> \
  --threshold-mean <MEAN> \
  --threshold-max <MAX>
```

See `GHIDRA_WORKFLOW.md` for the required locking pattern around Ghidra
headless runs.

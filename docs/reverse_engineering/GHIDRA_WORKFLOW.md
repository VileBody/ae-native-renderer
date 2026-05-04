# Ghidra Workflow

Use Ghidra only for focused parity questions that cannot be answered from AE
goldens or scripted probes. Keep imports and logs local, serialize shared
project access, and publish compact findings in Markdown.

## Local Inputs

Copied AE binaries live under ignored `target/reverse/ae_2026/`. Current reverse
notes reference:

```text
target/reverse/ae_2026/Transform.aex
target/reverse/ae_2026/GPUFoundation.dll
```

Before analysis, record hashes in the phase report:

```bash
shasum -a 256 target/reverse/ae_2026/Transform.aex \
  target/reverse/ae_2026/GPUFoundation.dll
```

Do not move these binaries into `docs/`, `fixtures/`, or any tracked source
tree. If a new AE build is copied, place it in a versioned ignored directory
such as `target/reverse/ae_2027/` or
`target/reverse/ae_2026_<build_id>/`.

## Project Ownership

Use one of two safe modes:

- Shared project mode: take `/tmp/ae-native-renderer-ghidra.lock` with `flock`
  for the entire `analyzeHeadless` invocation.
- Isolated project mode: create a unique project directory under
  `target/reverse/ghidra_projects/<topic>/` and run without the shared lock.

Use the lock whenever the command can touch a project directory another agent
might also use. Prefer isolated project dirs for long experiments.

## Headless Pattern

Set `GHIDRA_HOME` to your local Ghidra install. Keep scripts and logs under
`target/reverse`.

```bash
export GHIDRA_HOME=/path/to/ghidra
export TOPIC=geometry2_matrix
mkdir -p "target/reverse/ghidra_projects/$TOPIC" \
  "target/reverse/$TOPIC"

flock /tmp/ae-native-renderer-ghidra.lock \
  "$GHIDRA_HOME/support/analyzeHeadless" \
  "$PWD/target/reverse/ghidra_projects/$TOPIC" \
  "$TOPIC" \
  -import "$PWD/target/reverse/ae_2026/GPUFoundation.dll" \
  -overwrite \
  -postScript DumpGpuTransformTargeted.java \
  -scriptPath "$PWD/target/reverse/ghidra_scripts" \
  -log "$PWD/target/reverse/$TOPIC/ghidra.log"
```

For read-only script reruns against an existing shared project, still use the
lock:

```bash
flock /tmp/ae-native-renderer-ghidra.lock \
  "$GHIDRA_HOME/support/analyzeHeadless" \
  "$PWD/target/reverse/ghidra_projects/shared_ae_2026" \
  ae_2026 \
  -process GPUFoundation.dll \
  -postScript DumpGpuTransformTargeted.java \
  -scriptPath "$PWD/target/reverse/ghidra_scripts" \
  -log "$PWD/target/reverse/<topic>/ghidra_rerun.log"
```

On macOS machines without `flock`, use an atomic directory lock:

```bash
LOCK=/tmp/ae-native-renderer-ghidra.lockdir
while ! mkdir "$LOCK" 2>/dev/null; do sleep 1; done
trap 'rmdir "$LOCK" 2>/dev/null || true' EXIT

"$GHIDRA_HOME/support/analyzeHeadless" \
  "$PWD/target/reverse/ghidra_projects/<project>" \
  <project> \
  -process <program> \
  -noanalysis \
  -postScript <script>.java \
  -scriptPath "$PWD/target/reverse/<script_dir>" \
  -log "$PWD/target/reverse/<topic>/ghidra.log"
```

For repeatable agent handoff, prefer the prepared bundle runner before assigning
analysis tasks:

```bash
python3 scripts/prepare_ghidra_bundles.py --list
python3 scripts/prepare_ghidra_bundles.py --task geometry_transform_wrapper
```

The bundle workflow is documented in
`docs/reverse_engineering/GHIDRA_PREDECODE_BUNDLES.md`; task definitions live in
`docs/reverse_engineering/GHIDRA_PREDECODE_TASKS.json`.

## What To Extract

Extract the smallest evidence that answers the parity question:

- exported symbol names and demangled names;
- function addresses or RVAs needed to relocate the finding;
- constants, branch strings, and table shapes;
- matrix/vector layout and operation order;
- pseudocode summaries written in your own words;
- uncertainty labels such as `observed`, `inferred`, or `needs AE probe`.

Avoid committing raw decompiler dumps or long instruction listings. Keep raw
logs in `target/reverse/<topic>/` and summarize findings in
`docs/phase_reports/AE_REVERSE_<MODULE>_GHIDRA.md`.

## Formula Validation Loop

Ghidra findings are not the finish line. Close the loop with runtime evidence:

1. Translate the finding into a formula note with units, signs, order, and
   coordinate space.
2. Build or identify a fixture that isolates one term of the formula.
3. Run native tests or the conformance pack and store outputs under `target/`.
4. Compare against AE PNG goldens or probe telemetry.
5. Record metric deltas, threshold proposal, and residual uncertainty in the
   phase report.

Example linkage:

```text
Formula:
  docs/phase_reports/AE_REVERSE_GEOMETRY2_GHIDRA.md
Fixture:
  fixtures/conformance/scenes/collapse_transform_probe_scene.json
Native output:
  target/ae_conformance_native_<topic>/<CASE>/metrics.json
AE golden:
  fixtures/ae_conformance_pack/ae_goldens/png/<CASE>/
Native test:
  focused unit/conformance test named after the formula term
```

## When To Lock

Take `/tmp/ae-native-renderer-ghidra.lock` for:

- importing or re-importing binaries into any shared Ghidra project;
- running auto-analysis on a shared project;
- running post-scripts that save project state;
- deleting, upgrading, compacting, or renaming a project;
- any command where you are unsure whether Ghidra will write.

The lock is not needed for:

- reading committed Markdown docs;
- hashing copied binaries;
- inspecting raw logs under your own `target/reverse/<topic>/`;
- isolated project dirs that no other agent uses.

## Publishing Results

A good Ghidra phase report includes:

- `Status date: YYYY-MM-DD`;
- input binary paths and SHA-256 hashes;
- exact headless command or script names;
- relevant function/symbol identifiers;
- recovered formula or behavior;
- fixture/native/golden links;
- next validation step.

Name raw outputs by topic and timestamp:

```text
target/reverse/<topic>/<YYYYMMDD_HHMMSS>/ghidra.log
target/reverse/<topic>/<YYYYMMDD_HHMMSS>/symbols.json
target/reverse/<topic>/<YYYYMMDD_HHMMSS>/notes.txt
```

Name durable docs by module and method:

```text
docs/phase_reports/AE_REVERSE_GEOMETRY2_GHIDRA.md
docs/phase_reports/AE_REVERSE_MOTION_BLUR_GHIDRA.md
docs/phase_reports/AE_REVERSE_TURBULENT_DISPLACE_GHIDRA.md
```

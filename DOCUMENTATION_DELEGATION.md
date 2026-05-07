# Documentation Delegation Policy

## Goal

Save high-reasoning orchestration budget for math, reverse engineering, code
changes, and experiment design.

Markdown reports should be drafted by a low-cost documentation subagent when
possible. The orchestrator provides curated facts, snippets, paths, and
decisions; the documentation agent turns them into readable `.md`.

## Default Roles

### Orchestrator

- Owns technical decisions, math conclusions, code changes, and final claims.
- Extracts short evidence bundles from logs instead of handing over huge raw
  traces.
- Gives the documentation agent exact paths, metrics, command summaries, and
  accepted wording for uncertain points.
- Reviews and edits the final markdown before commit.
- Commits/pushes the final state.

### Documentation Scribe

Preferred settings:

- reasoning/intelligence: `low`;
- or model: `gpt-5.3-codex-spark` when available and appropriate.

Responsibilities:

- Draft or update markdown only.
- Preserve exact numbers, paths, case ids, commit ids, and module names from the
  orchestrator bundle.
- Mark uncertainty explicitly as `open`, `partial`, or `not verified`.
- Avoid inventing conclusions, formulas, or status changes.
- Avoid changing code, fixtures, generated artifacts, or roadmap semantics
  unless explicitly assigned.

## Evidence Bundle Format

When delegating docs, the orchestrator should provide a compact bundle like:

```text
Doc target:
  docs/phase_reports/EXAMPLE.md

Purpose:
  What this report should prove or preserve.

Facts:
  - exact metric/result/path
  - exact metric/result/path

Commands:
  - command summary, not huge stdout

Artifacts:
  - target/.../report.json
  - target/.../trace.jsonl

Conclusions:
  - accepted conclusion
  - open question

Do not claim:
  - thing not yet verified
```

## Guardrails

- Do not use a documentation subagent to interpret raw Ghidra/Frida evidence
  independently. It may summarize already-selected snippets.
- Do not let docs drift ahead of implementation.
- If the scribe sees a contradiction, missing artifact, or ambiguous metric, it
  must report it back instead of guessing.
- If a doc update affects roadmap status, the orchestrator must approve the
  status wording.
- If a doc includes commands, prefer reproducible commands and paths over prose.

## Practical Rule

For substantial `.md` updates:

1. Orchestrator does the experiment and decides what changed.
2. Orchestrator passes a small evidence bundle to the low-cost scribe.
3. Scribe drafts the markdown.
4. Orchestrator reviews, fixes technical wording, and commits.

For tiny one-line process-log entries, the orchestrator may update the file
directly.

# AE Remote Golden Loop

Goal: make AE evidence cheap. Instead of preparing packs locally, manually moving
them to the Windows machine, rendering, downloading, importing, and then tuning,
we want one repeatable loop:

```text
native repo
  -> generate JSX/probe pack
  -> upload pack/artifacts to S3
  -> enqueue reserved AE node job
  -> AE renders goldens/probes under a render-only lock
  -> upload/download result archive
  -> ingest into fixtures
  -> run native conformance + visual contact sheet
```

## What Slowed Us Down

The bottleneck was not the number of agents. It was the manual AE loop.
Formula work needs AE evidence, but every new probe required a human transfer
step. That made agents spend time adding instrumentation and reports while
actual formula tuning waited for goldens.

There is also a smaller runtime bottleneck on the Windows render node: the AE
lock must protect only the AE session. S3/download/upload work should happen
outside the lock so the next job can prepare while the current job renders.

## Current Node Facts

Reserved test node:

```text
http://85.239.48.31:8000
```

Health is reachable. The current API exposes:

```text
GET  /health
POST /render
GET  /render/{render_id}
POST /pack-render
GET  /pack-render/{render_id}
POST /jobs
```

The production render-node runtime already has an in-process render lock around
AE work. I tightened the source runtime in `blast_mj_final` so job preparation
and S3 output upload happen outside the global AE lock, while the critical AE
session remains serialized:

```text
outside lock:
  job dir cleanup
  JSX/media download
  debug JSX copy

inside lock:
  pre reset
  AfterFX.com -r render.jsx
  ae_status wait
  aerender
  output stability wait
  post reset

outside lock:
  output mp4 upload
  full job artifacts upload/cleanup
```

This keeps the important invariant: one AE render/session at a time per node.

Operational requirements found during the first remote smoke:

```text
AfterFX.exe must be running in the interactive Administrator session.
The FastAPI/uvicorn node should also run from that same interactive session.
AE scripting file/network access must be enabled:
  Pref_SCRIPTING_FILE_NETWORK_SECURITY = "1"
```

If `AfterFX.com -r script.jsx` returns success but no status file appears,
first run a minimal JSX file-write probe. On 2026-05-04 the root cause was AE
prefs/security plus a bad generated wrapper string. The wrapper generator now
uses a raw Python template and has a regression test for JS escape sequences.
The Windows `restart_node_workflow.ps1` now also enforces the scripting pref
before starting AE.

## Pack Job Shape

For conformance/probe work we need a slightly different job than production MP4
rendering. The pack job should accept:

```json
{
  "job_id": "ae_probe_roundN_85_YYYYMMDD_HHMMSS",
  "pack_s3_uri": "s3://.../ae_probe_pack.zip",
  "entry_script": "ae_probe_pack/run_pack.jsx",
  "output_s3_bucket": "...",
  "output_s3_key": "ae_probe_outputs/<job_id>/outputs.zip",
  "timeout_s": 7200
}
```

The node should:

```text
download pack.zip
unpack into C:\ae_jobs\<job_id>\
run the pack script through AfterFX.com or build project + aerender queue
collect png/tiff/logs/status into outputs.zip
upload outputs.zip to S3
return render_id/status/output_s3_uri
```

This is separate from production MP4 output. The production `/render` path can
stay focused on `render_full.jsx -> output.mp4`.

The smoke command that proved the loop:

```bash
PYTHONUNBUFFERED=1 python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/glow_shadow \
  --job-id ae_pack_smoke_glo010_20260504_165021 \
  --case GLO_010 \
  --poll-interval-s 10 \
  --timeout-s 900
```

Result:

```text
remote render_id: ee4e58e2a1fb460ca379394758983299
remote output: s3://f7cef916-job-artifacts/ae_remote_packs/ae_pack_smoke_glo010_20260504_165021/ae_pack_smoke_glo010_20260504_165021_outputs.zip
local zip: target/ae_remote/ae_pack_smoke_glo010_20260504_165021/outputs/ae_pack_smoke_glo010_20260504_165021_outputs.zip
local extracted: target/ae_remote/ae_pack_smoke_glo010_20260504_165021/extracted
local converted PNG: 30 frames
```

## Fast Development Workflow

For each math module:

```text
1. Decompose one primitive.
2. Generate minimal AE probe case(s).
3. Enqueue pack on 85.
4. Ingest returned zip.
5. Run native conformance + manual contact sheet.
6. Tune exactly one formula family.
7. Re-run the same cases.
```

AE is single-lane per node, but everything around it is parallel:

```text
while AE renders:
  agents prepare next probe packs
  native tests run on existing goldens
  manual review sheets are generated
  docs/status tables are updated
```

## Highest-Leverage Next Steps

1. Add a remote pack runner command in this repo:

```text
scripts/ae_remote_pack.py
  pack --pack fixtures/ae_probe_pack/... --job-id ...
  submit --node http://85.239.48.31:8000
  wait --render-id ...
  download --output target/ae_remote/<job_id>/outputs.zip
  ingest --kind probe|conformance
```

2. Add or expose a pack endpoint on the Windows node:

```text
POST /pack-render
GET  /pack-render/{render_id}
```

3. Generate visual review artifacts after every run:

```text
contact_sheet.png
top_diffs/
metrics.json
manual_review.md
```

4. Keep formula tuning serialized per module, but keep probe generation and AE
rendering queued continuously.

5. Promote a case only when it reaches:

```text
implemented -> instrumented -> AE golden exists -> formula tuned -> parity locked
```

## Practical Rule

Agents should not spend much time guessing AE math without fresh AE outputs.
When a module is blocked by unknown AE behavior, the next task is a probe pack,
not another approximation.

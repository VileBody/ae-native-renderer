# Production Visual Acceptance, 2026-07-17

## Scope

This pass covers the reproducible archived-JSX visual acceptance run for the
active native-readiness corpus:

- `trendy_5th_production`
- full-footage `brat_5th`
- 32 production `/bigtest` F1-F5 cases

The OFX/Sapphire execution route remains out of scope. The renderer path is the
native approximate implementation.

## Runner

Use the unified JSX corpus runner:

```bash
python3 scripts/run_jsx_production_corpus.py \
  --archive-root /Users/ergin/Desktop/archives_and_duplicates/dup_blast_family/blast_mj_final/out/visual_tool_examples_s3 \
  --out target/visual_acceptance/jsx_runner_acceptance \
  --cache-root target/visual_acceptance/jsx_runner_cache \
  --render-cli target/debug/render-cli \
  --case trendy_5th_production \
  --case brat_5th_strobe_subtitles
```

The runner now supports recursive job archive lookup, sibling `output.mp4`
reference lookup, and explicit `control_frames` / `control_times`.

## Results

The final production pass contains 33 cases: 32 `/bigtest` F1-F5 jobs and one
full-footage Brat job. All 33 rendered with required audio capability, no
`not_implemented` findings and no `unsupported` findings. This is native
approximate acceptance, not a pixel-perfect claim.

The earlier Trendy pass remains the subtitle/style reference. It rendered six
frame-locked controls without unsupported visual layers; its residual is visual
tuning rather than missing production semantics.

Generated artifacts:

- `target/visual_acceptance/jsx_runner_acceptance/index.json`
- `target/visual_acceptance/jsx_runner_acceptance/trendy_5th_production/side-by-side.mp4`
- `target/visual_acceptance/ae_refs_20260717_full32/full_video_audio_summary.json`
- `target/visual_acceptance/ae_refs_20260717_full32/full_video_audio_bounded/`
- `target/visual_acceptance/ae_refs_20260717_full32/full_video_audio_tail/`
- `target/visual_acceptance/ae_refs_20260717_full32/f3_final_3/`

The tracked, secret-free evidence index is
`fixtures/bot_corpus/production_acceptance_20260717.json`. It records the 33
orchestrator job IDs and aggregate SHA-256 checksums so the AE outputs and job
archives can be re-harvested from artifact storage.

## Visual Notes

- Trendy is structurally close: text timing, word order, position and overall
  red treatment line up. The main residual differences are global AE analog
  texture / moire, grade intensity, and small text softness/stroke differences.
- Brat subtitle/strobe mechanics and full footage composition are both covered
  by the final reference job.

## Remaining

- Pixel-level tuning of text metrics, masks/mattes and plugin-derived native
  approximations remains separate from P0/P1 readiness.
- OFX and proprietary plugin workers remain intentionally out of scope.
- Production rollout still requires the normal container/manager canary; it is
  not part of this visual acceptance result.

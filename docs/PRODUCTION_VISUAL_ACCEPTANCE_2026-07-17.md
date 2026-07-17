# Production Visual Acceptance, 2026-07-17

## Scope

This pass covers the first reproducible archived-JSX visual acceptance run for
public bot styles that already have local AE job artifacts:

- `trendy_5th_production`
- `brat_5th_strobe_subtitles`

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

- `trendy_5th_production`: rendered 6 frame-locked control frames and generated
  side-by-side MP4. Capability status is partial only because audio mux is still
  reported as `import.skip_audio` / `layer.audio`; no unsupported visual layers
  were reported.
- `brat_5th_strobe_subtitles`: rendered 6 frame-locked control frames and
  generated side-by-side MP4. The archived AE job contains text/strobe/audio
  only (`scene.assets=0`); it is valid for subtitle/strobe parity, but not for
  full footage parity.

Generated artifacts:

- `target/visual_acceptance/jsx_runner_acceptance/index.json`
- `target/visual_acceptance/jsx_runner_acceptance/trendy_5th_production/side-by-side.mp4`
- `target/visual_acceptance/jsx_runner_acceptance/brat_5th_strobe_subtitles/side-by-side.mp4`

## Visual Notes

- Trendy is structurally close: text timing, word order, position and overall
  red treatment line up. The main residual differences are global AE analog
  texture / moire, grade intensity, and small text softness/stroke differences.
- Brat subtitle/strobe mechanics line up enough for this artifact, but the
  comparison cannot validate underlying footage because the AE archive has no
  video source layers.

## Remaining

- Add real `/bigtest` F1-F5 AE job archives to the same manifest once their
  production job ids are available or after a fresh team bot `/bigtest` run.
- Add a Brat production reference with actual footage if we want full-frame
  footage parity rather than subtitle/strobe-only acceptance.
- Improve the runner contact sheet to include all control frames per case, not
  only the first comparison frame.

# P2 Text Stroke Live Probe

Minimal AE85 probe for live-locking TXT ARE stroke/fill ordering with the
existing `txt-are-spans` hook profile.

Cases render one 8 bpc text glyph per comp. Run them through their one-case
entry scripts; the default entry script intentionally builds only the fill-only
control so remote `--case` filtering never has to discard risky queued comps.

- `STR_LIVE_FILL_ONLY`: `jsx/build_p2_text_stroke_live_probe_project.jsx`
- `STR_LIVE_STROKE_ONLY`: `jsx/build_STR_LIVE_STROKE_ONLY.jsx`
- `STR_LIVE_FILL_OVER`: `jsx/build_STR_LIVE_FILL_OVER.jsx`
- `STR_LIVE_STROKE_OVER`: `jsx/build_STR_LIVE_STROKE_OVER.jsx`

The fill/stroke combined cases differ only by `TextDocument.strokeOverFill`.

Smoke one case first:

```bash
python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/p2_text_stroke_live_probe \
  --entry-script jsx/build_STR_LIVE_STROKE_ONLY.jsx \
  --node http://85.239.48.31:8001 \
  --case STR_LIVE_STROKE_ONLY \
  --job-id p2_stroke_live_strokeonly_smoke_YYYYMMDD_HHMMSS
```

# Type 2-6 Production Corpus

`fixtures/bot_corpus/jsx_production_manifest.json` defines ten frame-locked
production comparisons for `TYPE_2` through `TYPE_6`. Every control frame is
rendered from the archived `render.jsx` and compared against the AE `output.mp4`
at the same composition frame. Rust is left; AE is right.

Run it with the archived S3 job folders:

```sh
python3 scripts/run_jsx_production_corpus.py \
  --archive-root /path/to/scenes_full_palette \
  --out target/jsx-production-corpus
```

The runner saves two frame controls and a short `side-by-side.mp4` per case,
plus `index.json` and a 2x5 `contact-sheet.png`. It uses `compsSpec.fps` from
the JSX (rather than assuming 24 fps), and does not rebase layer keyframes.

The archived sample set has multiple independent `TYPE_2` and `TYPE_4` layers,
but only one exported `TYPE_3`, `TYPE_5`, and `TYPE_6` layer. Those three types
therefore use two production states of that layer, explicitly marked in the
manifest. The renderer remains `partial` for these jobs only because production
audio muxing is not implemented yet; the visual frame controls are rendered.

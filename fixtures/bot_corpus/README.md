# Bot Approximation Corpus

This corpus is the local acceptance surface for the Blast bot, not a generic AE
test suite. Each case is an unmodified-shape planner payload wrapped in
`blast.bot-render.v1`; `render-cli adapt-bot-payload` converts it to the native
JSON API request.

Run every native case:

```bash
python3 scripts/run_bot_visual_corpus.py --out target/bot-corpus
```

Place the corresponding AE MP4s downloaded from the production job archive or
S3 under `fixtures/bot_corpus/references/<case>.mp4`, then rerun with
`--require-reference`. The runner writes `native.mp4`, `side-by-side.mp4`, a
contact sheet, exact control PNGs, and a per-case `review.json`. AE is a visual
reference only; the native render is always produced directly from the adapted
bot payload.

The corpus deliberately has a standalone case for each Scenes `TYPE_1` through
`TYPE_6`. New selectable presets must add a case before they are exposed.

# Agent Round 8 Core Alpha Bezier

Worker: Core / Metrics / Alpha / Bezier foundation.

## Summary

Round 7 added `foreground_rgb`, which removes a lot of full-frame noise by
masking background-looking pixels out of RGB comparison. The remaining gap was
visibility: raw `rgb`, `rgba`, and even foreground-masked RGB do not answer
whether a difference survives normal source-over compositing onto a known
background.

This pass adds a local `testkit` diagnostic for that question:
`diff_rgb8_over_background`.

## Added Helper

`diff_rgb8_over_background(a, b, background_rgb)` composites each RGBA pixel in
both buffers over the same RGB background, then computes the existing
`DiffMetrics` over the composited RGB channels.

Per channel it uses straight-alpha source-over semantics:

```text
out = round((src * alpha + background * (255 - alpha)) / 255)
```

This intentionally keeps the helper small and local to `image_diff.rs`. It does
not change conformance pass/fail thresholds or wire new report fields into
`render-cli`.

## Test Coverage

Two focused tests were added:

- Invisible RGB/background-alpha case: raw `rgb` sees a `255` max diff and raw
  `rgba` sees two changed pixels, but compositing over the sampled background
  produces zero visible RGB difference.
- Partial-alpha premult case: straight red `[255, 0, 0, 128]` compared with
  premult-looking red `[128, 0, 0, 128]` over black produces a visible red
  difference of `64`, so the helper still catches foreground color/alpha
  representation errors.

## Manual Inspection

I manually inspected the representative byte cases rather than only relying on
test counts.

For `[255, 0, 0, 0]` versus `[0, 0, 255, 0]` over background `[5, 5, 6]`, both
pixels display as `[5, 5, 6]`. The raw RGB diff is numerically huge, but a human
looking at the composited frame would see no red/blue difference. The new helper
matches that visual meaning.

For `[5, 5, 6, 255]` versus `[5, 5, 6, 0]` over `[5, 5, 6]`, both sides also
display as the same background RGB. This is exactly the kind of background
alpha-only mismatch that can dominate `rgba` while carrying no visible AE parity
signal.

For `[255, 0, 0, 128]` versus `[128, 0, 0, 128]` over black, the displayed
colors are approximately `[128, 0, 0]` and `[64, 0, 0]`. That is visibly
different: the second pixel has the shape of a premultiplied RGB buffer being
interpreted as straight color. The helper therefore does not just make numbers
smaller; it separates invisible metadata/background mismatches from errors that
would actually change the preview.

One caveat: this diagnostic assumes straight/unassociated RGBA input. It is most
useful as a triage metric, especially when compared over black and white or over
the frame's sampled corner background. It should not become the sole pass/fail
gate without a broader decision about the renderer's stored alpha convention.

## AE Parity Impact

AE parity work needs to distinguish three cases that can look similar in raw
buffer diffs:

- invisible transparent RGB storage differences;
- background alpha bookkeeping differences;
- visible straight-vs-premult or blend math errors at partial alpha.

`diff_rgb8_over_background` gives foundation-level tooling for that split. It
complements `foreground_rgb`: one asks "is this foreground content?", the other
asks "does this differ after display compositing?"

## Next Steps

- Wire this as an optional conformance report metric once the report schema is
  stable enough for another field.
- Evaluate over at least black, white, and sampled-corner backgrounds when
  diagnosing straight-vs-premult failures.
- For Bezier/ease fixtures, add tiny opacity/position samples at deterministic
  times around eased keyframes, then compare `rgba`, `foreground_rgb`, and
  composited RGB. This should make ease curve timing errors visible without
  confusing them with transparent-pixel storage noise.

## Verification

- `docker run --rm -v "$PWD":/app -w /app rust:1-bookworm cargo test -p testkit image_diff`
  passed: 7 tests passed, 0 failed.

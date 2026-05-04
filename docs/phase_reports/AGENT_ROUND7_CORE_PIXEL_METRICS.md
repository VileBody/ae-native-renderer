# Agent Round 7 Core Pixel Metrics

Worker: Core Pixel / Metrics / Bezier direction.

## Summary

Before this pass, conformance output exposed full-frame `rgba`, `rgb`, `alpha`,
and `background_alpha_normalized` metrics. Those are useful, but formula tuning
could still be noisy when invisible/background pixels carried alpha or RGB
state that was not foreground content.

This pass adds a foreground-masked RGB metric in `testkit` and exposes it from
the conformance pack report as `foreground_rgb`.

## Metric Semantics

`diff_rgb8_foreground_masked` compares RGB channels only for pixels where at
least one side has foreground coverage:

- pixel alpha is greater than zero; and
- pixel RGB is not equal to that side's background RGB.

The conformance runner passes each frame's top-left/corner RGB as the native and
AE background RGB. The compared pixel count is stored in `total_pixels`, so
`mean_abs_diff`, `rmse_abs_diff`, and `changed_pixel_ratio` are scoped to the
foreground mask rather than the full frame.

## Report Usage

Use these report paths for formula-tuning triage:

- per-frame: `frames[].metrics.foreground_rgb`
- case summary: `summary.metrics.foreground_rgb`

The existing `rgba`, `rgb`, `alpha`, and `background_alpha_normalized` keys are
unchanged. Pass/fail thresholds still use the legacy RGBA summary fields.

## Checks

- `cargo test -p testkit` was run via Docker because local `cargo` is not in
  PATH. Result: failed in an existing non-metrics Phase 5 font assumption test
  (`phase5_font_assumptions_match_ae_manifest`); 30 tests passed, including the
  new image diff tests.
- Focused `cargo test -p testkit image_diff` via Docker passed: 5/5.
- `cargo test -p render-cli --no-run` via Docker passed after installing the
  GStreamer dev packages needed by `media-gst`.

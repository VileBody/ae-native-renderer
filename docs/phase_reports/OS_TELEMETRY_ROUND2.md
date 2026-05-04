# OS Telemetry Round 2

Status date: 2026-05-03.

This round handled four follow-up work streams after M19 metric normalization:

```text
1. Temporal adjustment stack / STK_030
2. Effects telemetry and time-aware params
3. Geometry2 / Turbulent Displace telemetry
4. Text font/glyph observability and Point-Light in Docker
```

## Point-Light

The local archive `youworkforthem-T9068-point.zip` was used to extract:

```text
fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf
```

Docker font probe identified it as:

```text
family=Point,Point Light
style=Light,Regular
fullname=Point Light
postscript=Point-Light
```

`TXT_030` and `TXT_040` conformance recipes now use the direct repo-local TTF
path instead of the fontconfig family name, so Docker no longer falls back to
DejaVu Sans for those native text cases.

Smoke run:

```text
target/ae_agents/point_font_smoke/report.json
```

## Temporal Stack

Status: old Round 1 temporal blocker is fixed.

Changes:

- adjustment-layer telemetry in `FrameRenderTrace.adjustment_effects`;
- render-sequence JSON profile serialization for the trace;
- Posterize Time on adjustment layers now freezes lower/input sampling at bucket
  time while downstream effects after Posterize can evaluate animated params at
  current comp time.

Guard result:

- `TMP_020` keeps `rgb.mean_abs_diff=0.0`;
- `STK_030` native frame `0` and `1` are no longer byte-identical, matching the
  AE observation that those frames should differ.

Report:

```text
docs/phase_reports/AGENT_TEMPORAL_STACK_TELEMETRY.md
```

## Effects

Status: diagnostic time-aware patch complete; formula tuning still blocked on
intermediate sidecars.

Changes:

- BoxBlur2 `radius` and `iterations` now sample at effect time;
- Glow `threshold`, `radius`, and `intensity` now sample at effect time;
- lightweight debug trace/hash helpers for Box Blur, Drop Shadow, Glow, and
  Minimax.

Fresh `EFF_070` is no longer static. Native sampled frame hashes differ across
the selected frames, and `metrics.rgb` changes over time.

Report:

```text
docs/phase_reports/AGENT_EFFECTS_TELEMETRY_TIME_AWARE.md
```

## Warps And Fields

Status: Geometry2 mapping fix plus telemetry; Turbulent telemetry only.

Changes:

- Geometry2 `0004`/`0008` axis scale controls override uniform `0003` when
  present;
- Geometry2 debug data exposes raw params, resolved params, matrix/inverse,
  sample UV, sampler mode, edge policy, and OOB count;
- Turbulent Displace field telemetry exposes resolved params, probe
  noise/displacement/source UV, field hash, OOB count, sampler mode, and edge
  policy.

No Turbulent formula tuning was done.

Report:

```text
docs/phase_reports/AGENT_WARPS_FIELDS_TELEMETRY.md
```

## Text

Status: exact Point-Light native font path and first font/glyph telemetry are in
place.

Changes:

- font resolution telemetry reports requested id, resolved path, source, and
  fallback state;
- layout telemetry reports text box, line height, and per-glyph metrics;
- real layout stores actual `fontdue` glyph id instead of Unicode codepoint.

`TXT_030` and `TXT_040` can now be rerun as exact-font diagnostic cases.

Report:

```text
docs/phase_reports/AGENT_TEXT_FONT_TELEMETRY.md
```

## Verification

Completed in Docker:

```text
cargo fmt -p render-cli -- --check
cargo test -p effects
cargo test -p text-engine
cargo test -p render-core posterize -- --nocapture
cargo test -p effects posterize_time -- --nocapture
render-cli conformance-pack --case TXT_030 --case TXT_040
render-cli conformance-pack --case TMP_020 --case STK_030
render-cli conformance-pack --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_050 --case EFF_070
```

The current integrated `effects` crate test run passes `30 passed; 0 failed`.

## Next Round

Rerun a focused Round 2 native diff batch with the new split metrics and
telemetry:

```text
TMP_020, STK_030,
EFF_010, EFF_020, EFF_030, EFF_050, EFF_070,
EFF_040, EFF_060,
TXT_030, TXT_040
```

Then start formula work only where the first divergent intermediate is known:

- Drop Shadow direction/offset;
- Glow threshold/mask;
- Minimax enum/channel/neighborhood;
- Geometry2 matrix/pixel-center/sampler after param mapping;
- Turbulent field model after field telemetry comparison;
- glyph/selector/expression tuning after exact-font rerun.

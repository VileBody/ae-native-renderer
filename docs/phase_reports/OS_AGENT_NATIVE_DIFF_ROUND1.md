# OS Native Diff Round 1

Status date: 2026-05-03.

This is the OS/critic summary after five agents ran focused
`render-cli conformance-pack` batches against the AE conformance PNGs.

The reports are:

- `AGENT_1_CORE_PIXEL_NATIVE_DIFF.md`
- `AGENT_2_TEMPORAL_GRAPH_NATIVE_DIFF.md`
- `AGENT_3_EFFECTS_NATIVE_DIFF.md`
- `AGENT_4_WARPS_FIELDS_NATIVE_DIFF.md`
- `AGENT_5_TEXT_GLYPHS_EXPRESSIONS_NATIVE_DIFF.md`

Native artifacts are under:

```text
target/ae_agents/
```

## Global Finding

The first cross-agent blocker is `M19` output alpha/background policy.

All agents found the same pattern:

```text
native background corner: [5, 5, 6, 255]
AE golden corner:         [5, 5, 6, 0]
```

This makes raw RGBA diffs hit `max_abs_diff=255` and inflates mean/RMSE for
nearly every case before the module-owned math can be judged. The current
metrics are useful as a render baseline, but not as formula-tuning evidence.

Next global patch should add or expose:

- RGB-only, alpha-only, and RGBA metrics;
- corner/background alpha diagnostics;
- matte/background policy in each case report;
- an AE-compatible alpha normalization mode or explicit masked metric for
  operator diagnosis.

Until that is done, no full RGBA metric should be used to tune text, effects,
warps, or motion formulas.

## Agent Decisions

| Agent | Scope | Decision | First Real Blocker After `M19` |
| --- | --- | --- | --- |
| 1 Core Pixel | `M01`, `M03`, `M04`, `M19` | Baseline measured; global blocker confirmed | `INT_020` Bezier/ease secondary; `EFF_040` matrix/UV/sampling secondary |
| 2 Temporal | `M02`, `M15`, `M16`, `M17`, `M18` | `TMP_010`/`TMP_020` RGB match AE; do not tune isolated Posterize | `STK_030` adjustment Posterize time routing; `TMP_030` needs motion sample telemetry |
| 3 Effects | `M10`, `M11`, `M13` | Effects measured; no formula tuning from RGBA | Drop Shadow direction/offset, Glow threshold/alpha mask, Minimax enum/channel, BoxBlur/Glow animated params |
| 4 Warps | `M12`, `M14` | Warps measured; field telemetry required | Geometry2 `0003` vs `0004`/`0008` mapping; Turbulent displacement field model |
| 5 Text | `M05`, `M06`, `M07`, `M08`, `M09`, text-side `M17` | Text measured; no formula tuning from current metrics | `Point-Light` resolves to DejaVu Sans in Docker; glyph/selector/expression/collapse telemetry missing |

## Confirmed Useful Signals

### Temporal

`TMP_010` and `TMP_020` match AE exactly in RGB according to Agent 2. That means
the basic layer/source-time mapping and isolated Posterize Time bucket behavior
are likely correct for the current selected frames.

`TMP_020` confirms 6 fps Posterize Time in a 30 fps comp holds frames:

```text
0..4 -> frame 0
5    -> frame 5
10..11 -> frame 10
20..21 -> frame 20
30..31 -> frame 30
```

Do not tune isolated `M15` from the raw RGBA failure.

### Adjustment Stack

`STK_030` exposes a real timing problem:

```text
native frame 0 vs 1: byte-identical
AE frame 0 vs 1:     different
```

The likely issue is that native quantizes the whole adjustment effect stack
after Posterize Time, while AE appears to allow downstream effects after
Posterize Time to keep changing at comp time. This needs per-effect time routing
telemetry before a formula patch.

### Effects

Agent 3 found stack order is not the first proven bug. The next useful effect
patches are diagnostic:

- emit Box Blur, Drop Shadow, Glow, and Minimax intermediates;
- switch BoxBlur2/Glow animated numeric params to time-aware sampling where the
  parser currently uses static `param_f32_any`;
- retest isolated `EFF_010`, `EFF_020`, `EFF_030`, `EFF_050`, then `EFF_070`.

### Geometry And Turbulence

Agent 4 found a likely Geometry2 mapping issue before matrix tuning:

```text
0003 = 82
0004 = 120
0008 = 72
```

Native currently resolves uniform `0003` first and can ignore width/height style
controls. Add resolved-param and matrix/UV telemetry before changing formulas.

Turbulent Displace should not be tuned from final pixels. It needs field output:

```text
noise, dx, dy, displaced UV, sampled source/hash, edge stats, evolution
```

### Text

Agent 5 found the current Docker/native run resolves `Point-Light` to
`DejaVu Sans`. `TXT_030` and `TXT_040` cannot be used for tight AE text parity
until the exact font is available or font telemetry proves the right font.

Next text work is telemetry, not formula tuning:

- resolved font path/family/postscript/fallback flag;
- actual font glyph id, advance, bbox, baseline;
- selector weights per frame;
- per-glyph transform, opacity, blur radius;
- expression selector amount and property expression sampled values;
- collapse raster scale and sharpness probes.

## Next Allowed Work

1. Add conformance metric normalization for `M19`:
   RGB-only, alpha-only, corner alpha, matte policy, and optionally
   AE-compatible transparent background normalization.
2. Add first telemetry checkpoints:
   temporal time routing, effect intermediates, Geometry2 params/matrix/UV,
   Turbulent field maps, and text font/glyph/selector/expression records.
3. Apply only tiny diagnostic correctness patches that have isolated evidence:
   BoxBlur2/Glow time-aware params and, after telemetry confirms it, Geometry2
   AE parameter mapping.
4. Rerun all five agent batches and promote modules only from isolated cases.

Formula tuning is not yet authorized for Glow, Drop Shadow, Minimax,
Turbulent Displace, glyph animator, expression selector, or collapse sharpness.

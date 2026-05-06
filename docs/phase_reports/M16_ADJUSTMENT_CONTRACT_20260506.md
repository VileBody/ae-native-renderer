# M16 Adjustment Contract

Date: 2026-05-06.

## Question

`EFF_041` proved the isolated Geometry2 `0012=2` sampler/matrix path, but
`STK_031` still diverged badly. The blocker was therefore not "retune
Geometry2", but "what canvas/origin does AE feed to Geometry2 when it is on an
adjustment layer?"

## Probe Ladder

Added focused conformance cases:

| Case | Purpose |
| --- | --- |
| `ADJ_010` | no-op adjustment over centered coordinate field |
| `ADJ_011` | no-op adjustment over full-frame color bars |
| `ADJ_020` | Geometry2 identity over centered coordinate field |
| `ADJ_021` | Geometry2 identity over full-frame color bars |
| `ADJ_030` | Geometry2 translate over centered coordinate field |
| `ADJ_031` | Geometry2 translate over full-frame color bars |
| `ADJ_040` | Geometry2 rotate/scale/bicubic over centered coordinate field |
| `ADJ_041` | Geometry2 rotate/scale/bicubic over full-frame color bars |
| `ADJ_042` | Geometry2 rotate/scale/bicubic over full-frame checker |
| `ADJ_050` | Geometry2 rotate/scale/bicubic over checker plus coordinate |
| `ADJ_051` | Geometry2 rotate/scale/bicubic over checker plus alpha square |
| `ADJ_052` | Geometry2 rotate/scale/bicubic over checker plus premult probe |

AE jobs:

```text
ae_conformance_adjustment_contract_20260506_160601
ae_conformance_adjustment_contract_extra_20260506_162523
```

Native reports:

```text
target/ae_agents/m16_adjustment_contract_20260506_160601/report.json
target/ae_agents/m16_adjustment_contract_full_origin0_20260506/report.json
target/ae_agents/m12_m16_geometry2_bicubic_after_adjustment_origin_20260506/report.json
target/ae_agents/stk030_after_adjustment_origin_20260506/report.json
```

## Finding

Native Geometry2 used `layer_space_origin(input)`, which infers the origin from
the first non-transparent pixel. That is correct for the current layer-effect
path that made `EFF_041` pass, but wrong for adjustment-layer Geometry2.

For a centered 256x256 coordinate field inside a 512x512 comp,
`layer_space_origin(input)` becomes `(128, 128)`. AE adjustment Geometry2 uses
the adjustment/comp canvas origin `(0, 0)`.

## Native Change

Adjustment-layer Geometry2 now passes an internal native-only parameter:

```json
{ "__native_layer_space_origin": [0.0, 0.0] }
```

This only affects adjustment application. Isolated/layer Geometry2 keeps the
existing alpha-bounds origin behavior.

## Metrics

Primary metric is `rgb_straight_source_over_ae_background`.

| Case | Before | After |
| --- | ---: | ---: |
| `ADJ_040` centered coordinate rotate/scale | `20.167969` | `0.087540` |
| `STK_031` Geometry2 bicubic adjustment | `8.511274` | `0.032308` |
| `STK_030` template-like stack | `40.052743` previous M12 run | `3.741755` |
| `EFF_041` isolated Geometry2 bicubic | `0.043613` | `0.043613` |

`ADJ_010`, `ADJ_020`, and `ADJ_030` are exactly `0.0` on the visible RGB
metric, confirming that no-op, identity, and translate adjustment cases were
already correct.

The remaining high-frequency checker cases are not the same blocker:

| Case | After |
| --- | ---: |
| `ADJ_041` color bars full frame | `1.080157` |
| `ADJ_042` checker full frame | `8.454229` |
| `ADJ_050` checker plus coordinate | `5.448621` |
| `ADJ_051` checker plus alpha square | `7.700708` |
| `ADJ_052` checker plus premult probe | `7.022895` |

Because checker-only is already worse than checker+coordinate, this residual is
a high-frequency sampler/edge stress case, not a new adjustment layer ordering
contract.

## Status

M16 has a concrete implemented contract for Geometry2-on-adjustment origin
routing. The next stack residuals in `STK_030` are now small enough to route to
the actual downstream modules:

1. `M13` Minimax radius/channel/edge tuning.
2. `M14` Turbulent Displace field model.
3. `M15` Posterize Time boundary checks if temporal frames still drift.
4. High-frequency sampler residuals for checker-like content, if templates need
   that level of texture parity.

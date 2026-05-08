# P0 Master Gate Refresh - 2026-05-08

## Scope

Ran the current master conformance gate without formula tuning.

Command:

```bash
python3 scripts/run_master_conformance_gate.py --out target/ae_agents/p0_master_gate_refresh_20260508_230416
```

Artifacts:

```text
target/ae_agents/p0_master_gate_refresh_20260508_230416/report.json
target/ae_agents/p0_master_gate_refresh_20260508_230416/dashboard.json
target/ae_agents/p0_master_gate_refresh_20260508_230416/dashboard.md
```

Runner result:

```text
conformance-pack.done ok=true cases=19 report=target/ae_agents/p0_master_gate_refresh_20260508_230416/report.json
master-gate.dashboard json=target/ae_agents/p0_master_gate_refresh_20260508_230416/dashboard.json md=target/ae_agents/p0_master_gate_refresh_20260508_230416/dashboard.md
```

## Summary

Primary metric: `rgb_straight_source_over_ae_background`

```text
cases:       19
accepted:    2
approximate: 16
tuning:      1
regressions: 0
missing:     0
```

Source report summary:

```text
cases_requested: 19
cases_rendered:  19
failures:        0
elapsed_ms:      224173.18875
```

## Template Status

| Template | Status | Counts |
| --- | --- | --- |
| `template_4th` | tuning | approximate: 8, tuning: 1 |
| `impulse_2nd` | approximate | approximate: 8 |
| `scenes_3rd` | approximate | accepted: 2, approximate: 10 |

`template_4th` is tuning because `TXT_010` is outside the approximate band but below the regression guardrail. `impulse_2nd` and `scenes_3rd` remain approximate.

## Accepted Cases

| Case | Primary | Max | Notes |
| --- | ---: | ---: | --- |
| `TMP_020` | 0.000000 | 0 | Posterize Time numbered-frame boundaries |
| `EFF_060` | 0.166828 | 2 | Turbulent Displace coordinate-field warp |

## Tuning Case

| Case | Primary | Max | Changed Pixel Ratio | Notes |
| --- | ---: | ---: | ---: | --- |
| `TXT_010` | 12.531241 | 255 | 0.070856 | Montserrat word reveal |

`TXT_010` is the only non-accepted/non-approximate case. It is tuning, not regression, under the current policy thresholds (`approximate_mean: 10.0`, `regression_mean: 20.0`).

## Worst Cases by Primary Mean

| Rank | Case | Status | Primary | Max | Changed Pixel Ratio | Notes |
| ---: | --- | --- | ---: | ---: | ---: | --- |
| 1 | `TXT_010` | tuning | 12.531241 | 255 | 0.070856 | Montserrat word reveal |
| 2 | `TXT_030` | approximate | 7.073936 | 255 | 0.105707 | Point-Light glyph animator position/scale/rotation/blur |
| 3 | `TXT_020` | approximate | 6.410895 | 255 | 0.044334 | Montserrat character and line reveal |
| 4 | `CMP_010` | approximate | 5.862451 | 188 | 0.247990 | alpha/composite/sampling audit |
| 5 | `GPH_010` | approximate | 4.002211 | 255 | 0.051170 | nested precomp and collapse-transform text sharpness |
| 6 | `TXT_040` | approximate | 3.671418 | 255 | 0.023391 | expression-selector bounce pattern |
| 7 | `EFF_020` | approximate | 1.628605 | 125 | 0.191063 | Glow on luma ramp |
| 8 | `INT_020` | approximate | 0.406353 | 250 | 0.010081 | bezier/ease position and opacity interpolation |

## Worst Cases by Max Abs Diff

Several approximate cases still hit `max_abs_diff: 255` while staying inside their mean guardrails:

```text
EFF_041 primary=0.043613 changed=0.769562
TXT_010 primary=12.531241 changed=0.070856
TXT_020 primary=6.410895 changed=0.044334
TXT_030 primary=7.073936 changed=0.105707
TXT_040 primary=3.671418 changed=0.023391
STK_031 primary=0.032308 changed=0.811630
ADJ_040 primary=0.087540 changed=0.728474
GPH_010 primary=4.002211 changed=0.051170
```

## Gate Read

The refreshed master gate completed cleanly with no missing cases and no regressions. Current release posture is blocked from all-approximate/accepted by one tuning text case, `TXT_010`; the accepted set improved to include both `TMP_020` and `EFF_060`.

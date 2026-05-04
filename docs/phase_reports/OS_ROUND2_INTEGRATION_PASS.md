# OS Round 2 Integration Pass

Status date: 2026-05-03.

Purpose: integrate the four Round 2 worker streams into one conformance path so
formula tuning can start from case-local native frames, AE frames, diffs,
metrics, and telemetry sidecars.

## Code Integration

Integrated into `render-cli conformance-pack`:

- selected-frame native render per manifest case;
- AE golden copy into each case output;
- diff PNG and split metrics;
- `effects_debug` sidecars from render traces;
- Geometry2 and Turbulent Displace field sidecars;
- case-local JSONL trace sidecars:
  - `adjustment_effects.jsonl`;
  - `text_telemetry.jsonl`;
  - `expression_telemetry.jsonl`;
  - `collapse_telemetry.jsonl`.

Point-Light is repo-local at:

```text
fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf
```

`TXT_030` and `TXT_040` use that direct TTF path in native recipes, so Docker
native renders no longer fall back to DejaVu Sans for the Point cases.

## Verification

Docker clean dependency run:

```text
cargo fmt -p render-cli -- --check
cargo test -p render-cli conformance_pack -- --nocapture
```

Result:

```text
3 passed; 0 failed
```

Earlier integrated core check:

```text
cargo test -p render-core -- --nocapture
```

Result:

```text
29 passed; 0 failed
```

## Full Round 2 Metric Batch

Output:

```text
target/ae_agents/round2_integrated/report.json
```

Result:

```text
cases=15
failures=0
elapsed_ms=71080
```

Metrics:

| Case | RGB mean | RGB max | Background-normalized mean |
| --- | ---: | ---: | ---: |
| `TMP_020` | 0.0000 | 0 | 0.0000 |
| `STK_030` | 20.2231 | 255 | 52.2771 |
| `EFF_010` | 0.1635 | 116 | 1.2643 |
| `EFF_020` | 12.6578 | 137 | 15.3025 |
| `EFF_030` | 0.0000 | 0 | 0.0000 |
| `EFF_050` | 3.5333 | 250 | 2.6674 |
| `EFF_070` | 1.8913 | 126 | 2.8430 |
| `EFF_040` | 8.9347 | 250 | 7.0474 |
| `EFF_060` | 2.8989 | 200 | 2.8006 |
| `TXT_010` | 9.0858 | 250 | 8.7203 |
| `TXT_020` | 8.6609 | 250 | 8.3677 |
| `TXT_030` | 10.8155 | 250 | 13.5477 |
| `TXT_040` | 6.8230 | 250 | 6.6565 |
| `EXP_010` | 1.3041 | 250 | 1.1888 |
| `GPH_010` | 18.6581 | 250 | 16.0434 |

This full batch was run before the case-local JSONL sidecar patch. The render
math and metrics are still the current baseline; the follow-up smoke below
confirms sidecar writing through the same runner.

## Trace Sidecar Smoke

Output:

```text
target/ae_agents/round2_integrated_trace_smoke/report.json
```

Command scope:

```text
STK_030, EFF_040, EFF_060, TXT_030, EXP_010, GPH_010
```

Result:

```text
cases=6
failures=0
elapsed_ms=40936
```

Metrics:

| Case | RGB mean | RGB max | Background-normalized mean |
| --- | ---: | ---: | ---: |
| `STK_030` | 20.2231 | 255 | 52.2771 |
| `EFF_040` | 8.9347 | 250 | 7.0474 |
| `EFF_060` | 2.8989 | 200 | 2.8006 |
| `TXT_030` | 10.8155 | 250 | 13.5477 |
| `EXP_010` | 1.3041 | 250 | 1.1888 |
| `GPH_010` | 18.6581 | 250 | 16.0434 |

Trace sidecars produced:

```text
target/ae_agents/round2_integrated_trace_smoke/STK_030/adjustment_effects.jsonl        36
target/ae_agents/round2_integrated_trace_smoke/TXT_030/text_telemetry.jsonl            14
target/ae_agents/round2_integrated_trace_smoke/EXP_010/expression_telemetry.jsonl       8
target/ae_agents/round2_integrated_trace_smoke/GPH_010/text_telemetry.jsonl             8
target/ae_agents/round2_integrated_trace_smoke/GPH_010/collapse_telemetry.jsonl        12
```

Effect/warp sidecars produced:

```text
target/ae_agents/round2_integrated_trace_smoke/EFF_040/effects_debug/frame_00000/
target/ae_agents/round2_integrated_trace_smoke/EFF_060/effects_debug/frame_00000/
target/ae_agents/round2_integrated_trace_smoke/EFF_060/effects_debug/frame_00015/
target/ae_agents/round2_integrated_trace_smoke/EFF_060/effects_debug/frame_00030/
target/ae_agents/round2_integrated_trace_smoke/EFF_060/effects_debug/frame_00045/
target/ae_agents/round2_integrated_trace_smoke/effects_debug/STK_030/
```

## Critic Decision

Allowed to proceed:

- `M16/STK_030`: old temporal blocker is cleared. Keep `TMP_020` as guard.
- `M12/Geometry2`: ready for formula tuning from matrix/UV sidecars.
- `M05-M09 text`: exact Point-Light path and glyph/layout telemetry are ready
  enough to start glyph metric and text-box placement tuning.
- `M17 expressions/collapse`: expression and collapse sidecars exist; tuning can
  start on selected supported traits, not arbitrary JS.

Still gated:

- `M10/M11/M13 effects`: start formula tuning only case-by-case from isolated
  effect sidecars. Do not tune the whole `STK_030` stack first.
- `M14 Turbulent Displace`: field telemetry exists, but the next step is field
  model comparison, not final-pixel tuning.
- `GPH_010 collapse`: use as sharpness/matrix diagnostic, not parity-locked
  evidence yet.

## Next Work Order

1. Tune `EFF_040` Geometry2 matrix/anchor/pixel-center/sampler.
2. Tune isolated effects in order: `EFF_030`, `EFF_010`, `EFF_020`, `EFF_050`,
   then `EFF_070`.
3. Tune text glyph metrics and text-box placement on `TXT_030`/`TXT_040`.
4. Tune expression trait values on `EXP_010`.
5. Re-run `STK_030` and `GPH_010` only after their primitive dependencies move.

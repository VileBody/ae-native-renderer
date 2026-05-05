# Step 5 Reverse Implementation Readiness

Date: 2026-05-05.

This pass started the "reverse implemented" push across the five module lanes.
The goal was not to tune formulas yet. The goal was to make each lane cite
machine-readable evidence before formula patches are accepted.

## Scope

| Lane | Modules | Result |
| --- | --- | --- |
| Core pixel substrate | `M01`, `M03`, `M04`, `M19` | Added an explicit M19 alpha/composite gate and required-case policy. |
| Temporal graph / motion | `M02`, `M15`, `M16`, `M17`, `M18` | Added temporal contract reports for source time, Posterize, adjustment bucket/live split, and motion blur sample passports. |
| Effects / morphology | `M10`, `M11`, `M13` | Added Glow Based On diagnostics and Minimax intermediate hashes/alpha stats. |
| Warps / procedural fields | `M12`, `M14` | Added Geometry2 property mapping and Turbulent field-state guardrails, plus a sidecar evidence checker. |
| Text / expressions / collapse | `M05`, `M06`, `M07`, `M08`, `M09`, text-side `M17` | Added text-passport diagnostics for font instances, CoolType scope, selector/expression gaps, and collapse gaps. |

## Artifacts

Focused native run:

```text
target/ae_agents/step5_reverse_evidence/report.json
target/ae_agents/step5_reverse_evidence/warps_fields_evidence_check.json
```

Commands:

```bash
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/step5_reverse_evidence \
  --case PRI_010 --case CMP_010 --case STK_010 --case STK_020 \
  --case TMP_010 --case TMP_020 --case TMP_030 --case STK_030 \
  --case EFF_040 --case EFF_060 \
  --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_050 --case EFF_070 \
  --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 \
  --case GPH_010 --case EXP_010

python3 scripts/check_warps_fields_evidence.py \
  --out target/ae_agents/step5_reverse_evidence \
  --write-json target/ae_agents/step5_reverse_evidence/warps_fields_evidence_check.json
```

Result:

```text
conformance-pack.done ok=true cases=21
warps-fields evidence check ok=true
```

## New Gates

### M19 Alpha/Composite Gate

`report.json` now contains:

```text
m19_alpha_composite_gate.schema = m19.alpha_composite_gate.v1
required_cases = PRI_010, CMP_010, STK_010, STK_020
primary_visible_metric = rgb_straight_source_over_ae_background
raw_rgba_tuning_allowed = false
premult_contract_locked = false
```

In the focused run all required cases were measured and `required_cases_ok=true`,
but the gate remains diagnostic-only until the premult/straight contract is
locked.

### Temporal Contract Reports

Each case now writes:

```text
<out>/<case>/temporal_contract.json
```

The focused run checked:

| Case | Contract | Result |
| --- | --- | --- |
| `TMP_010` | `m02.source-time-passport.v1` | passed |
| `TMP_020` | `m15.posterize-numbered-source-exact.v1` | passed |
| `TMP_030` | `m18.motion-blur-sampling-passport.v1` | passed |
| `STK_030` | `m15.m16.adjustment-posterize-bucket-live-split.v1` | passed |

These reports are guardrails for formula tuning. They do not mean M15/M18 are
AE parity locked.

### Warps/Fields Evidence

`scripts/check_warps_fields_evidence.py` validates isolated sidecars before
M12/M14 tuning:

- `EFF_040`: Geometry2 property mapping, matrices, UV samples.
- `EFF_060`: Turbulent field state, field hash, field samples.
- `STK_030`: adjustment order plus Geometry2/Turbulent checkpoints.

The current run passed all checks. This makes `EFF_040` and `EFF_060` the
primary evidence entrypoints instead of `STK_030` final pixels.

## Reverse Implemented Remaining Work

| Lane | Remaining blockers |
| --- | --- |
| Core pixel substrate | Lock premult/straight policy, source-over flags, gamma/color-space, and M03 pixel-center/OOB evidence. |
| Temporal graph / motion | Add AE bucket-boundary refs, adjustment intermediate refs, and shutter sample/weight refs. |
| Effects / morphology | After M19, tune Drop Shadow mask/composite, Glow Based On/IR Gaussian/composite, and Minimax fractional radius/direction/Dont Shrink Edges. |
| Warps / procedural fields | Tune M12 against EFF_040 UV/matrix refs; replace M14 field approximation with AE-shaped FracAll/Frac1D model once field/kernel refs are available. |
| Text / expressions / collapse | Tune M05 sourceRect layout first; add CoolType glyph-id/raster refs, selector amount refs, expression property refs, and collapsed text/vector deferred-raster refs. |

## Acceptance For Next Formula Patches

Every Step 5 formula patch must cite:

1. The module packet from Step 4.5.
2. The gate or sidecar it is changing against.
3. The before/after metrics from `target/ae_agents/step5_reverse_evidence` or a
   newer focused run.
4. Any substrate blocker it is explicitly excluding.

Final PNG improvement alone is not enough for reverse-implemented acceptance.

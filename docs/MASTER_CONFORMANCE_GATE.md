# Master Conformance Gate

The master gate turns the AE conformance pack into a single release-style
dashboard. It does not replace focused module probes; it answers whether the
current renderer state is accepted, approximate, tuning, or regression across
the cases that matter for `template_4th`, `impulse_2nd`, and `scenes_3rd`.

## Inputs

- Policy: `fixtures/ae_conformance_pack/master_gate_policy.json`
- Runner: `scripts/run_master_conformance_gate.py`
- Native/AE frame diffs: `cargo run -p render-cli -- conformance-pack`

Each gate case has one of these roles:

- `isolated`: one primitive/operator module under test.
- `composed_stack`: a small stack that catches operator interaction drift.
- `template_frame`: a template-shaped slice used as a finish-line guardrail.

## Status Bands

The primary metric is:

```text
rgb_straight_source_over_ae_background
```

`accepted` means the case is inside the tight target band. `approximate` means
the case is inside the current known-good guardrail but still needs formula
tuning before parity lock. `tuning` means it is outside the approximate band
but not yet a hard regression. `regression` means the case crossed the module
guardrail or broke a temporal contract.

The bands are intentionally per-case. A text or collapsed-precomp case can be
rough and still not be a regression for a Turbulent Displace iteration.

## Commands

Run the master gate:

```bash
python3 scripts/run_master_conformance_gate.py \
  --out target/ae_agents/master_gate_$(date +%Y%m%d_%H%M%S)
```

Reclassify an existing conformance report without rerendering:

```bash
python3 scripts/run_master_conformance_gate.py \
  --report target/ae_agents/p0_master_gate_m14_20260506/report.json \
  --out target/ae_agents/p0_master_gate_m14_20260506
```

Outputs:

```text
<out>/report.json      raw conformance-pack report
<out>/dashboard.json   machine-readable gate dashboard
<out>/dashboard.md     human-readable gate dashboard
```

## Current Baseline

Latest run:

```text
target/ae_agents/p0_master_gate_m14_20260506/dashboard.md
```

Summary:

```text
cases: 19
accepted: 1
approximate: 18
regressions: 0
```

Template dashboards:

```text
template_4th: approximate
impulse_2nd:  approximate
scenes_3rd:   approximate
```

Notable module cases:

```text
TMP_020 accepted, primary=0.000000
EFF_060 approximate, primary=2.898929
STK_030 approximate, primary=3.741755
EFF_050 approximate, primary=0.073972
EFF_041 approximate, primary=0.043613
```

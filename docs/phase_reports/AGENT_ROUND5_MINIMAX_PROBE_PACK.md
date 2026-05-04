# Agent Round 5 Minimax Probe Pack

Scope: AE probe pack only. No formula tuning, no `crates/effects` edits, and no
changes to the existing `fixtures/ae_conformance_pack`.

## Inputs inspected

- `docs/phase_reports/AGENT_ROUND4_MINIMAX.md`
- `fixtures/ae_conformance_pack/manifest.json`
- `fixtures/ae_conformance_pack/jsx/build_conformance_project.jsx`
- Current EFF_050 tuple: `{ "0001": 2, "0002": 12, "0003": 1 }`

Round 4 identified the unresolved blocker as AE enum/channel semantics for
`ADBE Minimax`, especially whether operation value `2` and channel value `1`
mean what native currently assumes.

## Files added

- `fixtures/ae_probe_pack/minimax/README.md`
- `fixtures/ae_probe_pack/minimax/manifest.json`
- `fixtures/ae_probe_pack/minimax/jsx/build_minimax_probe_project.jsx`
- `fixtures/ae_probe_pack/minimax/scripts/measure_minimax_probe.py`
- `fixtures/ae_probe_pack/minimax/ae_goldens/png/.gitkeep`
- `fixtures/ae_probe_pack/minimax/ae_goldens/metadata/.gitkeep`
- `docs/phase_reports/AGENT_ROUND5_MINIMAX_PROBE_PACK.md`

## Probe matrix

The JSX creates an AE-generated alpha-square-like source:

- `256x256` source precomp
- centered `128x128` opaque white square
- placed into `512x512` probe comps at position `[256, 256]`
- scale `[70, 70]`

Cases:

- `MINIMAX_PRE`
- `MINIMAX_OP1_CH1_R0`
- `MINIMAX_OP1_CH1_R12`
- `MINIMAX_OP1_CH2_R0`
- `MINIMAX_OP1_CH2_R12`
- `MINIMAX_OP2_CH1_R0`
- `MINIMAX_OP2_CH1_R12_EFF050`
- `MINIMAX_OP2_CH2_R0`
- `MINIMAX_OP2_CH2_R12`

This covers radius `0` identity and radius `12` outputs for operation values
`1/2` and channel values `1/2`. The EFF_050 tuple is explicitly represented by
`MINIMAX_OP2_CH1_R12_EFF050`.

## Render instructions

In After Effects:

```text
File > Scripts > Run Script File...
fixtures/ae_probe_pack/minimax/jsx/build_minimax_probe_project.jsx
```

Then render the queued PNG sequences to:

```text
fixtures/ae_probe_pack/minimax/ae_goldens/png/<case_id>/<case_id>_[#####].png
```

The JSX writes property metadata to:

```text
fixtures/ae_probe_pack/minimax/ae_goldens/metadata/minimax_property_dump.json
```

After rendering, measure the final PNGs:

```text
python3 fixtures/ae_probe_pack/minimax/scripts/measure_minimax_probe.py \
  --pack fixtures/ae_probe_pack/minimax
```

Measurement output:

```text
fixtures/ae_probe_pack/minimax/ae_goldens/metadata/minimax_measurements.json
```

## Desired measurements

For every rendered case, collect:

- final PNG path
- RGB non-background bbox
- alpha nonzero bbox
- row samples at `y=256`, `x=211..300`

Compare every radius `0` case against `MINIMAX_PRE` first. Then use radius `12`
differences across operation/channel values to identify AE's operation enum and
channel enum before touching native Minimax formula logic.

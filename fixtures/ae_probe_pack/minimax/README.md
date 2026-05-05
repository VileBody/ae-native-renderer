# AE Minimax Probe Pack

Self-contained After Effects probe pack for resolving `ADBE Minimax` operation
and channel enum semantics before tuning native formulas.

This pack intentionally lives outside `fixtures/ae_conformance_pack` and does
not modify existing conformance fixtures.

## Probe Shape

The JSX builds an alpha-square-like source inside AE:

- source precomp: `256x256`
- opaque white centered square: `128x128`
- probe comp: `512x512`, `30 fps`, `2 seconds`
- source placement: position `[256, 256]`, scale `[70, 70]`

That matches the EFF_050-style transformed edge region, where row `y=256`
crosses the square around `x=211..300`.

## Cases

The render matrix includes a pre-effect source plus radius `0` identity probes
and radius `12` probes for operation values `1/2` and channel values `1/2`.

The EFF_050 parameter tuple is covered by:

```json
{ "0001": 2, "0002": 12, "0003": 1 }
```

## How To Render In AE

1. Open After Effects.
2. Run `File > Scripts > Run Script File...`.
3. Select `fixtures/ae_probe_pack/minimax/jsx/build_minimax_probe_project.jsx`.
4. The script creates:
   - `MINIMAX_PRE`
   - `MINIMAX_OP1_CH1_R0`
   - `MINIMAX_OP1_CH1_R12`
   - `MINIMAX_OP1_CH2_R0`
   - `MINIMAX_OP1_CH2_R12`
   - `MINIMAX_OP2_CH1_R0`
   - `MINIMAX_OP2_CH1_R12_EFF050`
   - `MINIMAX_OP2_CH2_R0`
   - `MINIMAX_OP2_CH2_R12`
5. Render the queued case comps as PNG sequences into:

```text
fixtures/ae_probe_pack/minimax/ae_goldens/png/<case_id>/<case_id>_[#####].png
```

6. Keep the metadata dump produced by the JSX:

```text
fixtures/ae_probe_pack/minimax/ae_goldens/metadata/minimax_property_dump.json
```

## Measurements

After rendering PNGs, run:

```text
python3 fixtures/ae_probe_pack/minimax/scripts/measure_minimax_probe.py \
  --pack fixtures/ae_probe_pack/minimax
```

The script writes:

```text
fixtures/ae_probe_pack/minimax/ae_goldens/metadata/minimax_measurements.json
```

Required fields for native parity work:

- pre/post RGB bbox
- pre/post alpha bbox
- row samples at `y=256`, `x=211..300`
- final PNG path for each rendered case
- metadata for `ADBE Minimax` property index, display name, match name, value,
  and value type

## Interpretation Goal

Compare radius `0` cases to `MINIMAX_PRE` to confirm identity behavior, then
compare the radius `12` operation/channel matrix to determine whether AE's enum
values match the current native assumptions for operation and channel selection.

## Discriminator Probe

`jsx/build_minimax_discriminator_project.jsx` is the smaller follow-up probe for
formula work. It builds 128x128 cases under `MMD_*` without UI alerts:

- fractional radius thresholds around `.5`;
- direction `0004 = 1/2/3`;
- channel lanes `0003 = 1..6`;
- operation order `0001 = 1..4`;
- `Don't Shrink Edges` polarity for `0005 = 0/1`.

Render through the remote pack runner:

```text
python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/minimax \
  --entry-script jsx/build_minimax_discriminator_project.jsx \
  --node http://85.239.48.31:8000 \
  --job-id minimax_discriminator_YYYYMMDD
```

Then measure the extracted outputs:

```text
python3 fixtures/ae_probe_pack/minimax/scripts/measure_minimax_discriminator.py \
  target/ae_remote/minimax_discriminator_YYYYMMDD/extracted \
  --output target/ae_remote/minimax_discriminator_YYYYMMDD/minimax_discriminator_measurements.json
```

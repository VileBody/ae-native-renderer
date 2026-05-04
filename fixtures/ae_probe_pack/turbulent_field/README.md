# Turbulent Displace Field Probe Pack

Self-contained After Effects probe pack for `ADBE Turbulent Displace` field
identification. This pack is intentionally separate from
`fixtures/ae_conformance_pack`; do not copy these cases into the shared
conformance pack until AE goldens are ready to be regenerated.

## Contents

- `manifest.json` describes the suites, variants, sample grid, and expected
  render outputs.
- `scripts/generate_assets.py` generates deterministic 512x512 primitive PNGs.
- `scripts/measure_turbulent_vectors.py` decodes AE coordinate-field PNG renders
  into source-coordinate and dx/dy telemetry for formula fitting.
- `assets/primitives/` contains the generated source images.
- `jsx/build_turbulent_field_probe_project.jsx` creates one AE comp per probe
  variant and queues PNG sequence renders.

## Generate Assets

Run from the repository root:

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/generate_assets.py
```

Generated assets:

- `coordinate_field.png`: `R = x mod 256`, `G = y mod 256`, `B = 32px tile
  parity`, `A = 255`.
- `sparse_impulse_grid.png`: one-pixel impulses every 32 px, with colored center
  axes.
- `hard_edge_alpha_ramp.png`: hard vertical/horizontal RGB edges plus a
  horizontal alpha ramp.
- `unique_checkerboard.png`: 16x16 unique colored 32 px cells with 4 px internal
  parity.

## Build And Render In AE

1. Open After Effects.
2. Run `File > Scripts > Run Script File...`.
3. Select `fixtures/ae_probe_pack/turbulent_field/jsx/build_turbulent_field_probe_project.jsx`.
4. Render queued case comps as PNG sequences.
5. Default output path:

```text
fixtures/ae_probe_pack/turbulent_field/ae_goldens/png8/<case_id>/<case_id>_[#####].png
```

The builder also writes:

```text
fixtures/ae_probe_pack/turbulent_field/ae_goldens/logs/turbulent_field_builder_log.txt
```

Keep that log with the renders. It includes an `ADBE Turbulent Displace`
property dump and notes when AE did not expose seed, pinning, or resize controls
by searchable names.

For 16-bit confirmation, rebuild or reopen the generated project, set
`File > Project Settings > Color > Depth` to 16 bits per channel, and render the
same queued comps to a sibling path such as `ae_goldens/png16`.

## Probe Coverage

The JSX creates variants for:

- amount sweep: `0, 1, 10, 45, 100`
- size sweep: `1, 8, 16, 32, 65, 128, 256`
- complexity sweep: `1, 2, 3, 4, 6`
- static evolution sweep: `0, 45, 90, 180, 360, 720`
- animated evolution: `0 -> 180` over 2 seconds; sample frames
  `0, 1, 15, 30, 45, 59`
- displacement type values: `1..9`
- seed sweep: `0, 1, 2, 10, 999`, if AE exposes a seed/random control
- pinning/edge variants over `hard_edge_alpha_ramp`
- resize-layer variants near borders
- sampler check over both `coordinate_field` and `unique_checkerboard`
- sparse impulse grid sanity case

## Inferring dx/dy From Coordinate-Field Renders

Use only coordinate-field probe outputs for field-vector estimation. For each
sampled output pixel `(ox, oy)`:

1. Read rendered `RGBA` at `(ox, oy)`.
2. If alpha is zero or nearly zero, mark the sample as probable
   out-of-bounds/transparent and do not infer a trusted vector from RGB.
3. Decode modulo coordinates:

```text
sx_mod = R
sy_mod = G
parity = B >= 128
```

4. Candidate source coordinates are:

```text
sx = sx_mod + 256 * kx
sy = sy_mod + 256 * ky
```

where `kx` and `ky` are chosen from values that place the source near the
512x512 source extent, usually `0` or `1`.

5. Prefer candidates whose expected parity matches the blue channel:

```text
expected_parity = ((floor(sx / 32) + floor(sy / 32)) mod 2) == 1
```

6. If more than one candidate matches parity, choose the one with the smallest
   displacement magnitude from the output pixel:

```text
dx = sx - ox
dy = sy - oy
```

7. Record both the inferred vector and the raw observed `RGBA`. Near modulo
   seams, hard edges, alpha transitions, or subpixel interpolation, keep the raw
   color because fractional/interpolated values may be the evidence needed to
   identify AE sampler behavior.

Required sample grid:

- corners, edge midpoints, and center
- every 32 px grid point
- dense `17x17` center patch
- full 16 px border band for pinning/resize/edge behavior

Acceptance target: identify AE parameter mapping and field vectors from this
pack before changing the native Turbulent Displace formula, sampler, pinning, or
edge policy.

## Measure Rendered Vectors

After AE PNG renders exist, run:

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/measure_turbulent_vectors.py \
  --pack fixtures/ae_probe_pack/turbulent_field
```

For an extracted render bundle, point `--pack` at the extracted
`ae_probe_pack/turbulent_field` root. The default report path is:

```text
<pack>/ae_goldens/metadata/turbulent_vector_measurements.json
```

By default the report stores case and frame summaries. Add `--include-samples`
only when the full per-pixel sample dump is needed for deeper field fitting.

# Agent Round 5 Turbulent Field Probe Pack

Scope: AE coordinate-field micro-pack for `ADBE Turbulent Displace`.

Owned files touched:

- `fixtures/ae_probe_pack/turbulent_field/**`
- `docs/phase_reports/AGENT_ROUND5_TURBULENT_FIELD_PROBE_PACK.md`

No `crates/effects` files, Turbulent Displace formula code, sampler code, or
existing `fixtures/ae_conformance_pack` files were changed.

## Input Reviewed

Read `docs/phase_reports/AGENT_ROUND4_TURBULENT_FIELD_PROBES.md`.

Round 4 established that current native Turbulent behavior is still approximate
and underdetermined from final-pixel `EFF_060` metrics alone. Required next
evidence is AE field telemetry over controlled coordinate/edge/sampler sources,
covering amount, size, complexity, evolution, displacement type, seed, pinning,
resize layer, and sampler policy.

## Probe Pack Added

Root:

```text
fixtures/ae_probe_pack/turbulent_field/
```

File list:

```text
fixtures/ae_probe_pack/turbulent_field/README.md
fixtures/ae_probe_pack/turbulent_field/manifest.json
fixtures/ae_probe_pack/turbulent_field/scripts/generate_assets.py
fixtures/ae_probe_pack/turbulent_field/jsx/build_turbulent_field_probe_project.jsx
fixtures/ae_probe_pack/turbulent_field/assets/primitives/assets_manifest.json
fixtures/ae_probe_pack/turbulent_field/assets/primitives/coordinate_field.png
fixtures/ae_probe_pack/turbulent_field/assets/primitives/sparse_impulse_grid.png
fixtures/ae_probe_pack/turbulent_field/assets/primitives/hard_edge_alpha_ramp.png
fixtures/ae_probe_pack/turbulent_field/assets/primitives/unique_checkerboard.png
```

Generated primitive assets are deterministic 512x512 RGBA PNGs:

- `coordinate_field.png`: `R=x mod 256`, `G=y mod 256`, `B=32px tile parity`,
  `A=255`.
- `sparse_impulse_grid.png`: one-pixel white impulses every 32 px with colored
  center axes.
- `hard_edge_alpha_ramp.png`: vertical/horizontal hard RGB edges plus horizontal
  alpha ramp.
- `unique_checkerboard.png`: unique 32 px cells with 4 px intra-cell parity for
  sampler identification.

## Probe Coverage

The AE builder creates one comp per variant for:

| Suite | Coverage |
| --- | --- |
| `TD_AMOUNT_SWEEP` | amount `0,1,10,45,100`; fixed size `65`, complexity `2`, evolution `0` |
| `TD_SIZE_SWEEP` | size `1,8,16,32,65,128,256`; fixed amount `45` |
| `TD_COMPLEXITY_SWEEP` | complexity `1,2,3,4,6`; fixed amount `45`, size `65` |
| `TD_EVOLUTION_STATIC` | evolution `0,45,90,180,360,720` |
| `TD_EVOLUTION_ANIM` | evolution `0 -> 180` over `2 s`; sample frames `0,1,15,30,45,59` |
| `TD_DISPLACEMENT_TYPE` | displacement type property index `1`, values `1..9` |
| `TD_SEED_SWEEP` | seed/random values `0,1,2,10,999` if AE exposes a matching control |
| `TD_PINNING_EDGE` | pinning off/on by name search over hard-edge/alpha-ramp source |
| `TD_RESIZE_LAYER` | resize-layer off/on by name search near coordinate-field borders |
| `TD_SAMPLER_CHECK` | amount `1,2,4` over coordinate-field and unique-checkerboard sources |
| `TD_IMPULSE_GRID` | sparse impulse flow sanity case |

The JSX writes `ae_goldens/logs/turbulent_field_builder_log.txt` with an
`ADBE Turbulent Displace` property dump and any unavailable seed/pinning/resize
control notes. This is intentionally evidence-gathering only; failed name
matches are logged rather than patched around in native code.

## Render Instructions

Generate or refresh primitives:

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/generate_assets.py
```

Build the AE project:

1. Open After Effects.
2. Run `File > Scripts > Run Script File...`.
3. Select:

```text
fixtures/ae_probe_pack/turbulent_field/jsx/build_turbulent_field_probe_project.jsx
```

4. Render the queued case comps as PNG sequences.

Default 8-bit output:

```text
fixtures/ae_probe_pack/turbulent_field/ae_goldens/png8/<case_id>/<case_id>_[#####].png
```

Recommended 16-bit pass:

1. Rebuild or reopen the generated project.
2. Set `File > Project Settings > Color > Depth` to 16 bits per channel.
3. Render the same queued comps to:

```text
fixtures/ae_probe_pack/turbulent_field/ae_goldens/png16/<case_id>/<case_id>_[#####].png
```

Keep the builder log beside both render sets.

## Field Derivation Notes

For coordinate-field outputs, infer field vectors by decoding each sampled output
pixel `(ox, oy)`:

```text
sx_mod = R
sy_mod = G
sx = sx_mod + 256 * kx
sy = sy_mod + 256 * ky
dx = sx - ox
dy = sy - oy
```

Choose `kx` and `ky` candidates that place `(sx, sy)` near the 512x512 source.
Use the blue channel parity check:

```text
((floor(sx / 32) + floor(sy / 32)) mod 2) == (B >= 128)
```

If multiple candidates match, choose the smallest displacement magnitude from
the output pixel. If alpha is zero or nearly zero, record the sample as probable
transparent/out-of-bounds and preserve raw RGBA rather than trusting decoded RGB.

Required samples:

- corners, edge midpoints, and center;
- every 32 px grid point;
- dense `17x17` center patch;
- full 16 px border band for pinning, resize, and edge behavior.

## Verification

Commands run:

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/generate_assets.py
```

```sh
python3 -m json.tool fixtures/ae_probe_pack/turbulent_field/manifest.json >/tmp/turbulent_manifest_check.json
python3 -m json.tool fixtures/ae_probe_pack/turbulent_field/assets/primitives/assets_manifest.json >/tmp/turbulent_assets_manifest_check.json
```

```sh
file fixtures/ae_probe_pack/turbulent_field/assets/primitives/*.png
```

Result: all primitive PNGs are `512 x 512`, `8-bit/color RGBA`,
non-interlaced.

```sh
node --check < fixtures/ae_probe_pack/turbulent_field/jsx/build_turbulent_field_probe_project.jsx
```

Result: JSX parses as JavaScript. AE execution was not run in this environment.

## Status

Round 5 adds the requested self-contained AE probe pack and documentation only.
It deliberately does not change Turbulent Displace native formulas or add these
cases to the existing shared conformance pack.

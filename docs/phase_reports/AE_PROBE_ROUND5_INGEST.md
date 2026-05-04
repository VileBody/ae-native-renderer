# AE Probe Round 5 Ingest

Source zip:

`/Users/ergin/Desktop/blast_mj_final/out/ae_probe_downloads/ae_probe_round5_85_20260504_011829_tiff_png_outputs_85.zip`

Imported copy:

`target/ae_probe_ingest/ae_probe_round5_85_20260504_011829_tiff_png_outputs_85.zip`

Extracted root:

`fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack`

## Validation

The AE return pack contains the expected probe outputs plus logs and embedded
probe sources.

| Probe area | PNG frames | TIFF frames | Notes |
| --- | ---: | ---: | --- |
| Glow / Drop Shadow | 18 | 18 | one frame per case |
| Minimax | 9 | 9 | property dump and measurements present |
| Turbulent Displace | 108 | 108 | includes 60-frame evolution animation |

Generated Minimax measurement file:

`fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/minimax/ae_goldens/metadata/minimax_measurements.json`

## Evidence-Backed Fixes

### M13 Minimax

The AE probe answered the enum/channel blocker:

| AE tuple | Observed behavior |
| --- | --- |
| `0001=1`, `0003=1`, radius 12 | RGB minimum/darken; alpha shape unchanged |
| `0001=1`, `0003=2`, radius 12 | alpha minimum/erode |
| `0001=2`, `0003=1`, radius 12 | RGB maximum; EFF_050 tuple stays close to identity |
| `0001=2`, `0003=2`, radius 12 | alpha maximum/dilate |

Patch:

- `0001=1` now maps to `minimum`; `0001=2` maps to `maximum`.
- `0003=1` now maps to RGB-only morphology; alpha is preserved.
- `0003=2` now maps to alpha morphology.

This moves Minimax from enum-blocked to formula-tuning territory.

### M10 Drop Shadow

The raw AE shadow case `DSH_020` has bbox `[219,219]-[330,330]` from source
bbox `[200,200]-[311,311]`, so `direction=135`, `distance=28` resolves to
`dx=+19`, `dy=+19`. The softened AE case expands to `[209,209]-[340,340]`,
which corresponds to blur radius 10 for `softness=18`.

Patch:

- native shadow offset now uses `dx=-cos(direction)*distance`,
  `dy=sin(direction)*distance`, with truncation.
- native shadow softness now maps to `blur_radius(softness / 2) + 1` for
  non-zero softness.

## Turbulent Probe Findings

The coordinate-field probe is usable. Decoded AE samples show:

- `TD_AMOUNT_SWEEP_A000` is exact pass-through.
- `amount=45,size=65,complexity=2,evolution=0` center displacement is about
  `(+8,-6)`, grid mean magnitude about `16.9`, p95 about `33.0`.
- `amount=100` center displacement is about `(+17,-13)`, grid mean magnitude
  about `29.1`.
- `random seed` is exposed at property `0010` and materially changes the field.
- displacement type `0001` values `1..9` materially change the vector basis.
- pinning value `0` was rejected by AE; value `1` was accepted.
- resize layer `0013` changes edge/output behavior.
- size value `1` was rejected by AE as out of range `2..1000`, so
  `TD_SIZE_SWEEP_S001` must not be treated as a true size-1 golden.

Next Turbulent work should add model slots for displacement type, seed, pinning,
resize/edge policy, and sampler behavior before amplitude-only tuning.

## Glow Probe Findings

Glow/Drop Shadow outputs imported cleanly. The Glow `based on` enum probes
produce distinct alpha/RGB bboxes on the alpha/luma split source, so the next
Glow pass should first map `0001` behavior and threshold source semantics.
No Glow formula patch was made in this ingest pass.

## Conformance After Patch

Command target:

`target/ae_probe_ingest/round5_patch_conformance/report.json`

| Case | RGB mean before | RGB mean after | RGB RMSE before | RGB RMSE after | Notes |
| --- | ---: | ---: | ---: | ---: | --- |
| `EFF_010` | `0.1635` | `0.1301` | `3.3191` | `3.2835` | Drop Shadow offset/softness closer |
| `EFF_050` | `3.5333` | `0.0487` | `29.3622` | `1.4252` | Minimax enum/channel blocker cleared |
| `STK_030` | `3.6281` | `3.7145` | `21.8641` | `21.1593` | mixed; Turbulent/stack still dominate |

Background-alpha-normalized Minimax also improved sharply:

`EFF_050`: mean `2.6674 -> 0.0540`, RMSE `25.4462 -> 1.5585`.

## Verification

- `cargo test -p effects -- --nocapture` in Docker: 39 passed.
- `cargo fmt -p effects -- --check` in Docker: passed.
- `render-cli conformance-pack` selected cases:
  `EFF_010`, `EFF_050`, `STK_010`, `STK_020`, `STK_030`: ok.

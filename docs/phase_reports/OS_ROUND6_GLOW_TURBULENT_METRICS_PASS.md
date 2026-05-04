# OS Round 6 Glow/Turbulent/Metrics Pass

Date: 2026-05-04

## Scope

This pass starts the next work wave across the current blockers:

- `M19` conformance metric normalization.
- `M11` Glow threshold/source controls.
- `M14` Turbulent Displace field telemetry before formula tuning.

The intent is to move modules from "implemented approximate" toward
"instrumented/testable" and only then into AE formula tuning.

## M19 Metric Audit

No source patch was needed in this pass. The current testkit and conformance
runner already expose the metrics needed before effect tuning:

- `rgba`
- `rgb`
- `alpha`
- `background_alpha_normalized`
- `background_corner` metadata

The key tuning rule remains: use `rgb` and `background_alpha_normalized` before
interpreting full-RGBA errors, because AE and native frames often share visible
background RGB while alpha differs.

Potential next M19 improvement: add an explicit foreground/coverage-weighted RGB
metric for cases where partially transparent foregrounds dominate the visible
error.

## M11 Glow Controls

AE Round 5 probes showed that `ADBE Glo2` property `0001` is a real branch
point for `Glow Based On`.

Patch:

- `crates/effects/src/glow.rs`
  - Added `GlowBasedOn`.
  - Added `0001` / `based_on` / `basedOn` / `Glow Based On` parsing.
  - `0001=1` maps to `color_channels`.
  - `0001=2` maps to `alpha_channel`.
  - Missing `0001` keeps the legacy combined threshold source so existing
    conformance cases do not regress.
  - `glow_debug_trace` now reports resolved `based_on`.
- `docs/EFFECTS.md`
  - Documented the new `ADBE Glo2` control surface.

This is not final Glow parity. It only adds the missing source selector needed
before tuning threshold mask, blur kernel/radius, intensity, and blend behavior.

## M14 Turbulent Telemetry

Added a repeatable vector-measurement tool:

- `fixtures/ae_probe_pack/turbulent_field/scripts/measure_turbulent_vectors.py`
- `fixtures/ae_probe_pack/turbulent_field/README.md`

The script decodes coordinate-field AE PNG renders into inferred source
coordinates and dx/dy telemetry. By default it writes compact case/frame
summaries; `--include-samples` enables the full per-pixel sample dump. It writes:

```text
<pack>/ae_goldens/metadata/turbulent_vector_measurements.json
```

Generated measurement report from the Round 5 AE probe bundle:

```text
fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field/ae_goldens/metadata/turbulent_vector_measurements.json
```

Summary:

| Case | Mean Mag | P95 Mag | Max Mag | Center dx/dy |
| --- | ---: | ---: | ---: | --- |
| `TD_AMOUNT_SWEEP_A000` | `0.000` | `0.000` | `0.000` | `(0, 0)` |
| `TD_AMOUNT_SWEEP_A045` | `11.038` | `26.000` | `126.194` | `(8, -6)` |
| `TD_SIZE_SWEEP_S065` | `11.038` | `26.000` | `126.194` | `(8, -6)` |
| `TD_DISPLACEMENT_TYPE_01` | `11.038` | `26.000` | `126.194` | `(8, -6)` |
| `TD_DISPLACEMENT_TYPE_09` | `9.585` | `25.000` | `121.000` | `(0, 5)` |
| `TD_SEED_SWEEP_999` | `11.347` | `31.000` | `149.030` | `(1, -4)` |
| `TD_RESIZE_LAYER_ON` | `16.517` | `44.045` | `121.807` | `(15, -12)` |

This report is the next input for actual Turbulent formula fitting.

## Verification

Commands:

```sh
docker run --rm -v "$PWD":/app -w /app rust:1-bookworm /bin/sh -lc '/usr/local/cargo/bin/cargo test -p effects glow -- --nocapture'
docker run --rm -v "$PWD":/app -w /app rust:1-bookworm /bin/sh -lc '/usr/local/cargo/bin/cargo test -p effects'
docker run --rm -v "$PWD":/app -w /app rust:1-bookworm /bin/sh -lc '/usr/local/cargo/bin/rustup component add rustfmt >/tmp/rustfmt-install.log 2>&1 && /usr/local/cargo/bin/cargo fmt -p effects -- --check'
docker run --rm -v "$PWD":/app -w /app rust:1-bookworm /bin/sh -lc 'apt-get update >/tmp/apt-update.log && apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev libfreetype6-dev libharfbuzz-dev >/tmp/apt-install.log && /usr/local/cargo/bin/cargo run -p render-cli -- conformance-pack --case EFF_020 --case EFF_070 --out target/ae_probe_ingest/glow_based_on_conformance'
```

Results:

- `cargo test -p effects glow`: pass, 5 tests.
- `cargo test -p effects`: pass, 41 tests.
- `cargo fmt -p effects -- --check`: pass.
- Focused conformance: `ok=true`, 2 cases.

Focused conformance metrics:

| Case | RGB Mean | RGB RMSE | BG-Alpha Mean | BG-Alpha RMSE |
| --- | ---: | ---: | ---: | ---: |
| `EFF_020` | `12.6578` | `32.1739` | `15.3025` | `45.0600` |
| `EFF_070` | `1.8913` | `10.2863` | `2.8430` | `19.5030` |

These match previous default-case numbers, which is expected because the shared
conformance cases do not set `0001`; they continue through the legacy combined
source branch.

## Next Gates

1. `M11` Glow formula tuning:
   - generate or extract intermediate AE masks for `0001=1`, `0001=2`, and
     default;
   - tune source threshold, radius/kernel mapping, intensity, and blend in that
     order;
   - keep `EFF_020` as isolated static gate and `EFF_070` as animated gate.

2. `M14` Turbulent formula fitting:
   - compare native field telemetry against
     `turbulent_vector_measurements.json`;
   - fit amount scale, size/frequency, complexity/octaves, evolution phase,
     displacement type basis, seed offsets, and sampler rounding separately;
   - retest `EFF_060` before using `STK_030`.

3. `M19` metric hardening if needed:
   - add foreground/coverage-weighted RGB metric only if the next formula pass
     still needs better signal separation than `rgb` and
     `background_alpha_normalized`.

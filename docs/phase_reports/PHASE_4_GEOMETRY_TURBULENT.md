# Phase 4 Geometry2 + Turbulent Displace

Date: 2026-05-03

## Scope

Worker Phase 4 covers:

- `M12` / `ADBE Geometry2`
- `M14` / `ADBE Turbulent Displace`
- composed stack `STK_030` in the presence of `M15` Posterize Time,
  `M13` Minimax, `M16` adjustment ordering, and `M19` sampling/color/alpha.

Write scope used:

- `crates/testkit/src/phase4.rs`
- `crates/testkit/src/lib.rs`
- `docs/phase_reports/PHASE_4_GEOMETRY_TURBULENT.md`

No effect implementation files were changed.

## AE Golden Frame Selection

From `fixtures/ae_conformance_pack/manifest.json`:

| Case | Title | Selected frames |
| --- | --- | --- |
| `EFF_040` | Geometry2 coordinate-field transform | `0` |
| `EFF_060` | Turbulent Displace coordinate-field warp | `0, 15, 30, 45` |
| `STK_030` | Geometry2 -> Posterize Time -> Minimax -> Turbulent Displace adjustment stack | `0, 1, 5, 10, 15, 20, 30, 45, 59` |

All selected PNGs exist under
`fixtures/ae_conformance_pack/ae_goldens/png/{EFF_040,EFF_060,STK_030}` and
open as `512x512`, matching the pack composition.

## Geometry2 Decomposition

Current implementation: `crates/effects/src/geometry.rs`.

Pipeline:

1. Params:
   - `anchor` from `anchor`, `anchorPoint`, `Anchor Point`, `0001`
   - `position` from `position`, `Position`, `0002`
   - `scale` from named 2D scale, uniform `0003`, or width/height `0004`/`0008`
   - `rotation` from `rotation`, `Rotation`
   - scalar numbered params can be sampled at effect time.
2. Matrix / inverse mapping:
   - output pixel `(x, y)` is translated around `position`
   - inverse rotation uses `-rotation`
   - inverse scale divides by `scale / 100`
   - source coordinate is rebuilt around `anchor`
3. Sampling:
   - nearest-neighbor by `round()`
   - out-of-bounds samples become transparent by skipping writes
4. Final pixels:
   - one output canvas, same dimensions as input
   - identity transform returns input clone

Parity risk:

- AE Transform/Geometry2 likely requires bilinear sampling and exact pixel-center
  convention checks. Current implementation has matrix-like behavior, but the
  sampling convention is still approximate.
- Required next telemetry is `matrix`, `inverse_matrix`, and `sample_uv`, then a
  coordinate-field UV diff against `EFF_040_00000.png`.

## Turbulent Displace Decomposition

Current implementation: `crates/effects/src/turbulent_displace.rs`.

Pipeline:

1. Params:
   - `amount` from `amount`, `Amount`, `0002`
   - `size` from `size`, `Size`, `0003`
   - `complexity` from `complexity`, `Complexity`, `0005`
   - `evolution` from `evolution`, `Evolution`, `0006`, sampled at effect time
2. Displacement field:
   - current field is deterministic sine turbulence
   - `amount` is clamped to `0..200`, then scaled by `0.25`
   - `size` is clamped to at least `1`
   - `complexity` is rounded and clamped to `1..6` octaves
   - evolution is converted to radians and used as procedural phase
3. Sampling:
   - displaced source coordinate is `(x + dx, y + dy)`
   - nearest-neighbor by `round()`
   - out-of-bounds samples become transparent by skipping writes
4. Final pixels:
   - one output canvas, same dimensions as input
   - zero amount returns input clone

Parity risk:

- The current sine turbulence is explicitly an approximation, not an AE
  displacement model. Blind formula tuning against final PNGs would be noisy.
- The next useful step is displacement-field parity: expose/debug `noise`, `dx`,
  `dy`, `uv`, sampled source, and final pixels for `EFF_060` frames
  `0, 15, 30, 45`.
- Complexity octave boundaries and seed/evolution semantics must be tested before
  tuning the pixel formula.

## STK_030 Context

`STK_030` uses modules `M12`, `M13`, `M14`, `M15`, `M16`, and `M19`.

Expected order from the manifest title:

```text
Geometry2 -> Posterize Time -> Minimax -> Turbulent Displace
```

Frame selection intentionally covers:

- frame `0` and `1` for immediate posterize/source-time boundary behavior
- frame `5`, `10`, `15`, `20`, `30`, `45`, `59` for temporal/evolution spread
- Turbulent frames shared with isolated `EFF_060`: `0`, `15`, `30`, `45`

`STK_030` cannot be used as the first tuning target because final pixels combine
Geometry2 sampling, Posterize Time quantization, Minimax edge/channel behavior,
Turbulent displacement, adjustment ordering, and alpha/color assumptions.

## Added Checks

Added `crates/testkit/src/phase4.rs` with focused checks:

- manifest frame selection for `EFF_040`, `EFF_060`, `STK_030`
- selected AE golden PNG existence and `512x512` dimensions
- Geometry2 passport requires coordinate/animated-parameter coverage plus
  `matrix`, `inverse_matrix`, `sample_uv`
- Turbulent Displace passport prioritizes coordinate/procedural/temporal coverage
  plus `noise`, `dx`, `dy`, `uv`, `sampled_source`
- `STK_030` manifest keeps the expected module context and named effect order

These are readiness checks, not AE pixel-parity assertions.

## Commands Run

Local host:

```sh
which cargo rustup rustc just make mise direnv
```

Result: `cargo`, `rustup`, and `rustc` are not installed on the host shell.
`make`, `docker`, and a cached `rust:1-bookworm` image are available.

Targeted tests:

```sh
docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v /Users/ergin/Desktop/ae-native-renderer:/work -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p testkit phase4 -- --nocapture'
```

Result: pass, `5 passed; 0 failed`.

```sh
docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v /Users/ergin/Desktop/ae-native-renderer:/work -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p effects geometry -- --nocapture'
```

Result: pass, `3 passed; 0 failed`.

```sh
docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v /Users/ergin/Desktop/ae-native-renderer:/work -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p effects turbulent -- --nocapture'
```

Result: pass, `4 passed; 0 failed`.

One mistyped exploratory command failed before the corrected targeted runs:

```sh
cargo test -p effects geometry turbulent_displace -- --nocapture
```

Cargo accepts only one test-name filter in that position.

## Pass / Fail

Pass:

- selected AE golden files are present and match manifest dimensions
- phase4 testkit checks pass
- existing Geometry2 unit tests pass
- existing Turbulent Displace unit tests pass

Fail / not yet pass:

- no native-vs-AE pixel diff was run for `EFF_040`, `EFF_060`, or `STK_030`
- no displacement-field telemetry exists yet for Turbulent Displace
- no matrix/UV telemetry exists yet for Geometry2
- current Turbulent formula remains a deterministic placeholder, not AE parity

## Blockers

Phase 4 cannot pass the AE parity gate now.

Blockers:

1. Geometry2 needs matrix/UV telemetry and likely sampler/pixel-center tuning
   against `EFF_040`.
2. Turbulent Displace needs a displacement-field parity harness before formula
   tuning. The present sine turbulence should not be blindly tuned from final
   pixels.
3. `STK_030` depends on unsettled `M12`, `M13`, `M14`, `M15`, `M16`, and `M19`;
   it should be treated as a composed regression target after isolated operator
   fields are inspectable.

Gate status: not passable yet; readiness checks are now in place.

# Agent Effects Telemetry Time-Aware Patch

Date: 2026-05-03

Scope: M10 Box Blur / Drop Shadow, M11 Glow, M13 Minimax. No Drop Shadow,
Glow, or Minimax formula tuning was done beyond making BoxBlur2 and Glow sample
animated numeric params at effect time.

## Code Changes

- `ADBE Box Blur2` now passes `EffectContext.time` into `BoxBlurParams` and uses
  `param_f32_at_any` for `radius` and `iterations`.
- Box Blur `iterations` is now sampled at time but still not applied as repeated
  passes; that remains formula work and was intentionally not tuned here.
- `ADBE Glo2` now passes `EffectContext.time` into `GlowParams` and uses
  `param_f32_at_any` for `threshold`, `radius`, and `intensity`.
- Added focused unit coverage proving BoxBlur2 and Glow animated params resolve
  different scalar values at `t=0.0`, midpoint, and `t=1.0`.
- Added lightweight deterministic debug helpers without full PNG dumps:
  - `box_blur_debug_trace(input, params, time)` reports resolved radius,
    iterations, kernel radius, input hash, horizontal pass hash, and output hash.
  - `drop_shadow_debug_trace(input, params)` reports resolved color, opacity,
    normalized opacity, direction, distance, softness, `dx`, `dy`, blur radius,
    and hashes for input, source alpha, raw offset shadow, blurred shadow, and
    final composite.
  - `glow_debug_trace(input, params, time)` reports resolved threshold, radius,
    intensity, kernel radius, and hashes for input, threshold source, blurred
    glow, intensity-scaled glow, and final blend.
  - `minimax_debug_trace(input, params, time)` reports resolved operation,
    channels, radius, kernel radius, input hash, and output hash.

The shared `canvas_debug_hash` is a deterministic FNV-1a-style `u64` over
canvas dimensions and RGBA bytes. It is intended for first-divergent-intermediate
triage, not as a cryptographic artifact hash.

## Next Hook Shape

The next low-noise integration point is a per-effect JSON sidecar emitted by the
conformance runner or render-core effect loop:

```json
{
  "case": "EFF_070",
  "frame": 30,
  "time": 1.0,
  "layer_id": "animated_glow",
  "effect_index": 0,
  "match_name": "ADBE Glo2",
  "trace": {
    "params": { "threshold": 160.0, "radius": 32.5, "intensity": 0.5, "kernel_radius": 16 },
    "hashes": {
      "input_rgba": "0x...",
      "threshold_source_rgba": "0x...",
      "blurred_glow_rgba": "0x...",
      "intensity_scaled_glow_rgba": "0x...",
      "final_rgba": "0x..."
    }
  }
}
```

Use one sidecar per frame, layer, and effect under a path like
`effects_debug/<case>/<frame>/<layer-index>_<effect-index>_<match-name>.json`.
This should be enough to identify the first divergent intermediate before
adding any PNG dumps.

## Tests

Host cargo is still unavailable:

```text
cargo --version
zsh:1: command not found: cargo
```

Docker formatting:

```text
rustup component add rustfmt
cargo fmt -p effects
```

Full effects test command:

```text
cargo test -p effects
```

Result: build succeeded; 29 tests passed and 1 unrelated out-of-scope test
failed:

```text
turbulent_displace::tests::zero_amount_field_telemetry_has_zero_displacement
left: 6408590644140840149
right: 10692903102008353237
```

Targeted touched-module tests:

```text
cargo test -p effects box_blur
cargo test -p effects glow
cargo test -p effects drop_shadow
cargo test -p effects minimax
```

Result:

```text
box_blur: 3 passed
glow: 3 passed
drop_shadow: 2 passed
minimax: 4 passed
```

Targeted conformance command:

```text
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/effects_telemetry_time_aware \
  --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_050 --case EFF_070
```

Result:

```text
conformance-pack.done ok=true cases=5 report=target/ae_agents/effects_telemetry_time_aware/report.json
```

`ok=true` means measured with no thresholds supplied; it is not an AE parity
pass.

## Fresh `metrics.rgb`

From `target/ae_agents/effects_telemetry_time_aware/report.json`:

| Case | Frames | RGB max | RGB mean | RGB RMSE | RGB changed ratio | RGB changed pixels |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| EFF_010 | 1 | 116 | 0.1635 | 3.3191 | 0.0228 | 5,977 |
| EFF_020 | 1 | 137 | 12.6578 | 32.1739 | 0.2732 | 71,618 |
| EFF_030 | 1 | 0 | 0.0000 | 0.0000 | 0.0000 | 0 |
| EFF_050 | 1 | 250 | 3.5333 | 29.3622 | 0.0148 | 3,875 |
| EFF_070 | 6 | 126 | 1.8913 | 10.2863 | 0.0810 | 127,409 |

Expected interpretation under `metrics.rgb`:

- `EFF_010`: RGB error is relatively small but nonzero; Drop Shadow
  direction/offset remains the likely next isolated primitive to instrument.
- `EFF_020`: RGB error remains high; Glow threshold/mask alpha participation is
  still the first suspected primitive, not radius animation.
- `EFF_030`: RGB is exact for this fixture; raw RGBA failure is alpha/background
  only.
- `EFF_050`: RGB touches a small region but with large max/RMSE; Minimax
  operation/channel/neighborhood semantics remain suspect.
- `EFF_070`: native output is no longer static. RGB metrics evolve frame by
  frame, proving animated BoxBlur2/Glow params are now sampled at time.

`EFF_070` per-frame RGB:

| Frame | Time | RGB max | RGB mean | RGB RMSE | RGB changed ratio | RGB changed pixels |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 0.0000 | 125 | 1.6738 | 10.0872 | 0.0616 | 16,158 |
| 10 | 0.3333 | 125 | 1.7437 | 10.1579 | 0.0680 | 17,836 |
| 20 | 0.6667 | 126 | 1.8226 | 10.2323 | 0.0750 | 19,662 |
| 30 | 1.0000 | 126 | 1.9079 | 10.3072 | 0.0810 | 21,235 |
| 45 | 1.5000 | 126 | 2.0376 | 10.4103 | 0.0942 | 24,707 |
| 59 | 1.9667 | 126 | 2.1624 | 10.5169 | 0.1061 | 27,811 |

Native `EFF_070` PNG hashes are distinct across sampled frames:

| Frame | Native SHA-256 prefix |
| ---: | --- |
| 0 | `3845a7f9a3c816da` |
| 10 | `bcce66570f097a92` |
| 20 | `06246c7a6a5aac9d` |
| 30 | `5765e63bf11a80ec` |
| 45 | `c5eb62f7fbbaa57c` |
| 59 | `6ed55f2549b3ea29` |

## Gate

The tiny time-aware param patch is complete. Do not tune Drop Shadow, Glow, or
Minimax formulas from final PNGs yet. The next useful step is to wire the debug
trace sidecars into the conformance run and rank first divergent intermediates
for `EFF_010`, `EFF_020`, `EFF_050`, and then `EFF_070`.

# Agent Round 4 Glow / Drop Shadow

Date: 2026-05-04

Scope: M11 Glow and Drop Shadow intermediate probes for `EFF_010`,
`EFF_020`, and animated `EFF_070`.

## Decision

No formula tuning patch was made.

The current evidence is native-intermediate-only. It proves native resolved
params and hashes, but it does not prove the first AE divergent intermediate.
Changing Glow threshold/mask behavior, Glow blur/composite math, Drop Shadow
direction, softness kernel, or opacity units from the final PNGs would be
final-pixel-only tuning.

## Files Changed

- Added this report: `docs/phase_reports/AGENT_ROUND4_GLOW_SHADOW.md`.
- Generated `target/ae_agents/round4_glow_shadow/`.

No source edits were made by this pass. `crates/effects/src/glow.rs` and
`crates/effects/src/drop_shadow.rs` were already dirty on entry; I inspected
them and left those existing changes intact.

## Commands

```text
docker run --rm --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm -lc '
  set -e
  export PATH=/usr/local/cargo/bin:$PATH
  export DEBIAN_FRONTEND=noninteractive
  apt-get update >/tmp/apt-update.log
  apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev libfreetype6-dev libharfbuzz-dev fontconfig >/tmp/apt-install.log
  export CARGO_TARGET_DIR=/work/target/ae_agents/round4_glow_shadow/cargo-target
  cargo test -p effects glow -- --nocapture
  cargo test -p effects drop_shadow -- --nocapture
  cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round4_glow_shadow --case EFF_010 --case EFF_020 --case EFF_070
'
```

Results:

- `cargo test -p effects glow -- --nocapture`: pass, 3 passed, 32 filtered.
- `cargo test -p effects drop_shadow -- --nocapture`: pass, 2 passed, 33 filtered.
- `conformance-pack`: `target/ae_agents/round4_glow_shadow/report.json`
  contains `ok=true` for `EFF_010`, `EFF_020`, and `EFF_070`.

Note: the container was stopped after the files were written because the
process remained running silently after `report.json` and case metrics existed.
The metrics files and report are complete and parseable.

## Before / After Metrics

Before source: `target/ae_agents/round3_final_integrated`.
After source: `target/ae_agents/round4_glow_shadow`.

| Case | Before RGB max | Before RGB mean | Before BG-alpha-norm mean | After RGB max | After RGB mean | After BG-alpha-norm mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `EFF_010` | 116 | 0.1635 | 1.2643 | 116 | 0.1635 | 1.2643 |
| `EFF_020` | 137 | 12.6578 | 15.3025 | 137 | 12.6578 | 15.3025 |
| `EFF_070` | 126 | 1.8913 | 2.8430 | 126 | 1.8913 | 2.8430 |

`EFF_030` remains the split-metric guard from Round 3: RGB mean `0.0000` and
background-alpha-normalized mean `0.0000`. It was not rerun in this focused
Round 4 block because the requested conformance cases were `EFF_010`,
`EFF_020`, and `EFF_070`.

## Native Sidecar Evidence

Generated native sidecars:

- `target/ae_agents/round4_glow_shadow/effects_debug/EFF_010/0/EFF_010_drop_shadow_0_ADBE_Drop_Shadow.json`
- `target/ae_agents/round4_glow_shadow/effects_debug/EFF_020/0/EFF_020_glow_0_ADBE_Glo2.json`
- `target/ae_agents/round4_glow_shadow/effects_debug/EFF_070/{0,10,20,30,45,59}/...`

Resolved native values:

| Case | Operator | Native intermediate evidence |
| --- | --- | --- |
| `EFF_010` | Drop Shadow | `direction=135`, `distance=28`, `dx=-20`, `dy=20`, `softness=18`, `blur_radius=9`, `opacity=180`, `opacity_normalized=0.705882`; hashes include `source_alpha_rgba`, `raw_offset_shadow_rgba`, `blurred_shadow_rgba`, and `final_rgba`. |
| `EFF_020` | Glow | `threshold=120`, `radius=35`, `kernel_radius=18`, `intensity=1.25`; native `threshold_source_rgba == input_rgba`, so the current implementation admits the full opaque luma ramp. |
| `EFF_070` | Animated Glow | Radius samples are time-aware: frame 0 `radius=10`, frame 30 `radius=32.5`, frame 59 `radius=54.25`; native `threshold_source_rgba == input_rgba` at sampled frames. |

## First Divergence Probe Spec

Current blocker: AE goldens are final PNGs only. Native sidecars contain useful
intermediate hashes, but there are no AE intermediate hashes/images to compare
against. The minimal next fixture/probe should export AE-side buffers that map
one-to-one to the existing native sidecar names.

Glow probe:

- Fixture input: existing `luma_ramp`, fully opaque, with the current
  `EFF_020` params `ADBE Glo2 {"0002":120,"0003":35,"0004":1.25}`.
- Also include an alpha-separated variant: same RGB ramp with alpha below the
  threshold in some bright pixels and above the threshold in some dark pixels.
- Export AE intermediates or equivalent isolated PNGs/hashes:
  `input_rgba`, `threshold_source_rgba`, `blurred_glow_rgba`,
  `intensity_scaled_glow_rgba`, `final_rgba`.
- Include resolved `Glow Based On` / threshold source enum if available.
- First yes/no check: does AE `threshold_source_rgba` equal full input, luma
  threshold only, alpha threshold only, or a combined rule?

Drop Shadow probe:

- Fixture input: existing `alpha_square`, current `EFF_010` params
  `ADBE Drop Shadow {"0001":[0,0,0,1],"0002":180,"0003":135,"0004":28,"0005":18,"0006":0}`.
- Add a no-softness variant with identical direction/distance and `softness=0`
  to isolate raw offset before kernel questions.
- Add a shadow-only variant (`"0006":1`) to isolate composite from shadow
  generation.
- Export AE intermediates or equivalent isolated PNGs/hashes:
  `input_rgba`, `source_alpha_rgba`, `raw_offset_shadow_rgba`,
  `blurred_shadow_rgba`, `final_rgba`.
- First yes/no check: for `direction=135,distance=28`, is AE raw offset
  `dx=-20,dy=20`, `dx=20,dy=20`, or another convention? Only after that should
  softness kernel and opacity normalization be tuned.

## Next Action

Add the AE-side Glow and Drop Shadow intermediate probes above to the
conformance pack. Then compare AE vs native at the first divergent named
intermediate before making any formula patch.

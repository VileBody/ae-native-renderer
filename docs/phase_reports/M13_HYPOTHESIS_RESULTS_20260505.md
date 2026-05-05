# M13 Minimax Hypothesis Results - 2026-05-05

Agent D scope: `ADBE Minimax` only. Local Rust toolchain was used; Docker was
not used. No M10/M11/M12/M14 files were edited.

## Inputs Read

- `docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md`, section
  `M13 Minimax`
- `docs/reverse_engineering/effect_math_temporal_noise_distort.md`, section
  `Minimax`
- `docs/EFFECTS.md`, Minimax parameter contract
- `fixtures/ae_probe_pack/minimax/README.md`
- `fixtures/ae_probe_pack/minimax/manifest.json`
- `fixtures/ae_conformance_pack/ae_goldens/metadata/effect_property_dump.json`
- `crates/effects/src/minimax.rs`
- `crates/render-cli/src/conformance_pack.rs`
- `crates/render-core/src/layer_eval.rs` Minimax debug sidecar writer

## Finite Hypotheses

H1 Operation enum:

- H1a accepted for probed values: `0001=1` is `minimum`, `0001=2` is
  `maximum`.
- H1b native candidate for unprobed values: `0001=3` is
  `minimum_then_maximum`, `0001=4` is `maximum_then_minimum`.
- Rejected for probed values: inverted mapping where `0001=2` means minimum.
- Needs new probe: AE confirmation for `0001=3/4` stage order.

H2 Channel enum:

- H2a accepted for probed values/current contract: `0003=1` is color/RGB,
  preserving alpha in the native stage; `0003=2` is alpha and color.
- H2b native candidate for unprobed values: `0003=3` red, `0003=4` green,
  `0003=5` blue, `0003=6` alpha.
- Rejected for EFF_050: `0003=1` as alpha-only. The EFF_050 tuple is
  `operation=maximum`, `channel=color`.
- Needs new probe: AE confirmation for individual channel modes `3..6`.

H3 Direction enum:

- H3a native candidate: `0004=1` horizontal then vertical, `0004=2`
  horizontal only, `0004=3` vertical only.
- Accepted only for the default observed value: conformance and property dump
  use `0004=1`.
- Needs new probe: AE direction sweep for `0004=2/3` on impulse and ramp
  inputs.

H4 Radius handling:

- H4a current native candidate: `kernel_radius = round(radius).clamp(0, 32)`.
- Alternatives still open: floor, ceil, or fractional morphology.
- Accepted only for integer-radius cases in this run: `12 -> 12`, `6 -> 6`,
  `0 -> identity`.
- Needs new probe: fractional sweep. The existing EFF_050/STK_020 cases cannot
  discriminate floor vs round vs fractional because all Minimax radii are
  integers.

H5 Edge and `Don't Shrink Edges` behavior:

- H5a current native candidate: clipped sample window at image bounds
  (`clip_to_image_bounds`); `0005` is parsed and reported but not applied.
- Alternatives still open: transparent fill outside bounds, repeated/clamped
  edge pixels, shrink-edge branch, and special `Don't Shrink Edges` branch.
- Needs new probe: current conformance cases are centered and use `0005=0`, so
  they do not isolate edge or `0005` behavior.

## Hypothesis-Pack Runs

EFF_050 isolated:

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M13 \
  --candidate m13_current_round_radius_clip_edges_eff050 \
  --status instrumented \
  --gate isolated \
  --question "Does current Minimax enum mapping and round-radius clipped morphology explain EFF_050?" \
  --hypothesis "operation 2 = maximum, channel 1 = color/RGB, direction default = horizontal+vertical, radius = round, edge = clip to image bounds, Don't Shrink Edges parsed only" \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md#M13 \
  --evidence docs/reverse_engineering/effect_math_temporal_noise_distort.md#Minimax \
  --note "Agent D M13 baseline isolated pack" \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/m13_eff050_current \
  --case EFF_050
```

Result: completed, `ok=true`, status `measured`.

STK_020 stack:

```bash
cargo run -p render-cli -- hypothesis-pack \
  --module M13 \
  --candidate m13_current_round_radius_clip_edges_stk020 \
  --status instrumented \
  --gate stack \
  --question "Does current Minimax candidate remain explainable in the Blur then Minimax stack STK_020?" \
  --hypothesis "operation 2 = maximum, channel 1 = color/RGB, direction default = horizontal+vertical, radius = round, edge = clip to image bounds, Don't Shrink Edges parsed only" \
  --evidence docs/reverse_engineering/MODULE_HYPOTHESIS_VALIDATION_PLANS.md#M13 \
  --evidence docs/reverse_engineering/effect_math_temporal_noise_distort.md#Minimax \
  --note "Agent D M13 baseline stack pack" \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/m13_stk020_current \
  --case STK_020
```

Result: completed, `ok=true`, status `measured`.

## Evidence Summary

EFF_050 frame 0:

- Metrics: RGBA mean `0.06379222869873047`, max `92`, changed pixels `356`;
  alpha mean `0.049633026123046875`, RGB mean `0.068511962890625`.
- PNG hashes:
  - native `9ace43ba28e7fb36b8b07b769e51a45ad681d85cbf4de985ad7179a6b8bdfec1`
  - AE `181511d85b9cbb942682352939b3510b3a5ebdd6dea72c51b0374dc95f38e373`
  - diff `d86830fbb3a32edbdb124f76833fb3f558cdfb82c3a2fcba34178f899afa9724`
- Minimax params: `operation=maximum`, `channels=color`,
  `direction=horizontal_and_vertical`, `radius=12.0`, `kernel_radius=12`,
  `dont_shrink_edges=false`, `edge_policy=clip_to_image_bounds`.
- Intermediate hashes:
  - input `0xe784ce0090798f71`
  - first pass `0xd8f9035fa9378df1`
  - first stage `0x7b8048b048231591`
  - second stage `0x7b8048b048231591`
  - output `0x7b8048b048231591`
- Alpha stats: input/output nonzero pixels `8100`, full pixels `7921`,
  alpha mean `7.829212188720703`, alpha sum `2052381`. This confirms the
  color-only native stage preserves alpha for the EFF_050 tuple.

STK_020 frame 0:

- Metrics: RGBA mean `7.005674362182617`, max `187`, changed pixels `64380`;
  alpha mean `9.030303955078125`, RGB mean `6.330797831217448`.
- PNG hashes:
  - native `ac8c7f05891340d7117a3a0664cdaadd750acaeea77c2f761d4c8f0615a8742b`
  - AE `83160feeac940d88c787cf47b9d1227467ddc7a6bbb2ebe0d2659911036b2561`
  - diff `887fd0fc7e102f76e98ad0181f6bfcfbb2462464df7c17901432fd83910e6024`
- Left layer Minimax after blur:
  - params: `operation=maximum`, `channels=color`, `kernel_radius=6`
  - hashes: input `0xc84cc9fa7a30cd25`, first pass
    `0x3c399de476498505`, output `0xff6eb8d7f89639a5`
  - alpha stats: input/output nonzero pixels `46188`, alpha mean
    `30.73968505859375`, alpha sum `8058224`
- Right layer Minimax before blur:
  - params: `operation=maximum`, `channels=color`, `kernel_radius=6`
  - hashes: input `0x86c56a4aa856d765`, first pass
    `0xf115e7c211aaeb65`, output `0x8d4def139dac5365`
  - alpha stats: input/output nonzero pixels `31684`, alpha mean
    `30.820541381835938`, alpha sum `8079420`

STK_020 is useful as a stack regression but not as a clean M13 discriminator:
the Minimax stages are color-only and preserve alpha in native sidecars, while
the case's large residual includes Box Blur stages and stack composition. No
global alpha/sampler/OOB policy was changed.

## Accepted / Rejected / Needs New Probe

Accepted:

- `0001=1 minimum`, `0001=2 maximum`.
- `0003=1 color/RGB` for the EFF_050/STK_020 tuple.
- Integer radii in current cases resolve to matching integer kernel radii.
- Existing `minimax_debug_trace` sidecars are sufficient for current M13
  evidence; no local candidate switch was needed for these two runs.

Rejected:

- `0001=2` as minimum for EFF_050.
- `0003=1` as alpha-only for EFF_050.
- Using STK_020 final PNG alone to tune M13 edge/radius/channel behavior.

Needs new probe:

- `0001=3/4` AE stage order.
- `0003=3..6` AE individual channel labels and exact lane behavior.
- `0004=2/3` direction mapping.
- Fractional radius policy: floor vs round vs ceil vs fractional morphology.
- Edge sample policy and `0005 Don't Shrink Edges` behavior.

ORCHESTRATOR_BLOCKER:

- STK_020 residual parity should not be assigned to M13 from final pixels alone.
  If stack parity must be improved before isolated M10/M19/global alpha evidence
  is available, route that to the owning orchestrator rather than changing
  Minimax or global alpha/sampler policy in M13.

## Next Discriminator

Add a focused AE Minimax probe pack with TIFF/PNG measurements and row/column
samples:

- Source A: small center impulse with distinct RGBA lanes.
- Source B: horizontal and vertical 1D ramps.
- Source C: hard alpha/RGB edge touching the layer bounds.
- Sweep operations `0001=1..4`.
- Sweep channels `0003=1..6`.
- Sweep directions `0004=1..3`.
- Sweep `0005=0/1`.
- Sweep radii `0, 0.25, 0.49, 0.5, 0.51, 0.75, 1, 1.49, 1.5, 1.51, 2, 12`.

Key discriminator examples:

- Radius `0.5` on an impulse separates floor identity from round radius `1`.
- Radius `1.5` separates floor radius `1` from round radius `2`.
- Off-center impulse with `0004=2/3` separates horizontal and vertical mapping.
- Boundary impulse with `0005=0/1` separates clipped window, transparent fill,
  repeated edge, and `Don't Shrink Edges` behavior.

## Verification

```bash
cargo fmt --all
cargo test -p effects minimax
cargo test -p render-cli conformance_pack
```

Results:

- `cargo fmt --all`: passed.
- `cargo test -p effects minimax`: passed, 13 tests.
- `cargo test -p render-cli conformance_pack`: passed, 15 tests.

Artifacts:

- `target/ae_agents/m13_eff050_current/report.json`
- `target/ae_agents/m13_eff050_current/hypothesis_report.json`
- `target/ae_agents/m13_eff050_current/effects_debug/EFF_050/0/EFF_050_minimax_0_ADBE_Minimax.json`
- `target/ae_agents/m13_stk020_current/report.json`
- `target/ae_agents/m13_stk020_current/hypothesis_report.json`
- `target/ae_agents/m13_stk020_current/effects_debug/STK_020/0/`

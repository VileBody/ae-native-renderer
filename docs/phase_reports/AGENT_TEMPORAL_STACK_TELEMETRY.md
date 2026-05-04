# Agent Temporal Stack Telemetry

Status date: 2026-05-03.

## Scope

Worked only in render-core temporal/adjustment timing plus this report. Effects
formulas, text-engine, Geometry2/Turbulent implementations, and font assets were
not changed.

## Changes

- Added adjustment-layer trace records to `FrameRenderTrace`:
  - composition and layer id;
  - effect index and matchName;
  - comp time;
  - adjustment layer-local time;
  - lower-stack resample time;
  - per-effect param time;
  - deterministic input/output canvas hash;
  - Posterize Time frame rate, bucket id, and bucket time on the Posterize
    effect record.
- Serialized `adjustment_effects` into render-sequence frame profiles.
- Split adjustment Posterize Time routing:
  - the first Posterize Time on an adjustment layer chooses the lower-stack
    resample time;
  - effects up to and including that Posterize Time evaluate at bucket time;
  - effects after Posterize Time evaluate params at current comp time.
- Left isolated/non-adjustment Posterize Time routing unchanged.

## Focused Test

Added `adjustment_posterize_keeps_downstream_param_time_live_inside_bucket` in
`crates/render-core/src/layer_eval.rs`.

The test renders frame 5 in a 10 fps comp with:

- a moving lower solid;
- an adjustment-layer Posterize Time at 1 fps;
- a downstream animated Minimax radius.

Expected behavior:

- lower stack samples bucket frame 0;
- downstream Minimax radius evaluates at comp time 0.5;
- frame output differs from the unexpanded bucket input;
- trace records `comp_time=0.5`, `lower_stack_time=0.0`, Posterize
  `param_time=0.0`, and downstream Minimax `param_time=0.5`.

This is the previously suspected failure mode: under the old global
`effects_time` routing, the downstream Minimax param time would also be 0.0.

## Verification

Host `cargo` is not installed, so tests were run in Docker.

Passed:

```text
docker run --rm --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; rustup component add rustfmt >/tmp/rustfmt-install.log && cargo fmt -p render-core -- --check'

docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p render-core posterize -- --nocapture'

docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p effects posterize_time -- --nocapture'
```

The render-core filter ran 3 tests: the new adjustment-stack test plus the
existing layer and adjustment Posterize guards. The effects filter ran the 3
Posterize Time unit tests.

Also passed:

```text
docker build -t ae-native-renderer:temporal-stack .

docker run --rm -u $(id -u):$(id -g) -v "$PWD:/work" -w /work \
  ae-native-renderer:temporal-stack conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/temporal_stack_telemetry \
  --case TMP_020 --case STK_030
```

Conformance smoke result:

- `TMP_020`: `rgb.mean_abs_diff=0.0`, `rgb.max_abs_diff=0`,
  `background_alpha_normalized.mean_abs_diff=0.0`.
- `STK_030`: `rgb.mean_abs_diff=20.22311528523763`, `rgb.max_abs_diff=255`.

`STK_030` frame 0/1 hash check:

```text
native frame 0: 6e8d465056b5ea730a8b016e100a4c1e5e18e34bdaf4a4e768e380d5d1c4e36a
native frame 1: 9de878ea9a5d54a4b0569665b39c17c89fe80eed3955a0f0683feb39c2ce4aa5
AE frame 0:     fd740d8277ee8bf982eec6c2e3cd29c0e14badc8a9d1039ef2d4c8843dec4372
AE frame 1:     ddc84ddbbac6e444e6ad0d46236412a2b7063ba9b518cb01bbf21e7f38c8ff77
```

Native frame 0/1 are no longer byte-identical. AE frame 0/1 also differ.

## Rerun Recommendation

`STK_030` should be rerun in the next Round 2/native-diff batch. The specific
temporal blocker from Round 1 is fixed: downstream adjustment effects after
Posterize Time can now animate inside a Posterize bucket. The case still has a
large RGB diff, so remaining work should move to operator/formula telemetry for
Geometry2, Minimax, and Turbulent Displace rather than more global time routing.

# Agent Round 2 Temporal Stack

Status date: 2026-05-03.

## Scope

Worked in the Temporal/STK write scope:

- `crates/render-core/src/layer_eval.rs`
- `crates/render-core/src/render_sequence.rs`
- this report under `docs/phase_reports/`

No effects, text, geometry, Minimax, or Turbulent Displace formula tuning was
done.

## Commands

Host `cargo` was unavailable, so all Rust commands ran in Docker.

Focused conformance, written to `target/ae_agents/round2_temporal_stack/`:

```text
docker run --rm -v "$PWD:/work" -w /work ae-native-renderer:round2-dev sh -lc '
  export CARGO_HOME=/work/target/round2_cargo_home
  export CARGO_TARGET_DIR=/work/target/round2_cargo
  cargo run -p render-cli -- conformance-pack \
    --pack fixtures/ae_conformance_pack \
    --out target/ae_agents/round2_temporal_stack \
    --case TMP_020 --case STK_030
'
```

Targeted tests:

```text
cargo test -p render-core posterize -- --nocapture
cargo test -p render-core adjustment_effect_sidecar -- --nocapture
```

## TMP_020 Guard

Fresh `target/ae_agents/round2_temporal_stack/report.json` confirms the Round 2
guard remains clean:

| case | rgb mean | rgb max | background-alpha-normalized mean | background-alpha-normalized max |
| --- | ---: | ---: | ---: | ---: |
| `TMP_020` | 0.0 | 0 | 0.0 | 0 |

Raw RGBA still contains the known background-alpha mismatch
(`rgba.mean_abs_diff=47.8125`), but RGB and background-normalized metrics are
zero.

## STK_030 Metrics

Fresh `STK_030` summary:

| metric | mean_abs_diff | max_abs_diff | changed_pixel_ratio |
| --- | ---: | ---: | ---: |
| `rgb` | 20.22311528523763 | 255 | 0.4101460774739583 |
| `background_alpha_normalized` | 52.277102364434135 | 255 | 0.9844415452745225 |
| `rgba` | 52.27721044752333 | 255 | 0.9844415452745225 |

Selected frame RGB means:

| frame | time | rgb mean | rgb max | rgb changed pixels |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 0.0 | 20.36194101969401 | 255 | 108939 |
| 1 | 0.03333333333333333 | 19.037492116292317 | 255 | 103649 |
| 5 | 0.16666666666666666 | 20.52179718017578 | 255 | 109414 |
| 10 | 0.3333333333333333 | 20.609602610270183 | 255 | 109512 |
| 15 | 0.5 | 20.622950236002605 | 255 | 109073 |
| 30 | 1.0 | 20.451995849609375 | 255 | 106983 |
| 45 | 1.5 | 20.763179779052734 | 255 | 108122 |
| 59 | 1.9666666666666666 | 19.057242075602215 | 255 | 103608 |

Selected PNG SHA-256 hashes:

| frame | native | AE |
| ---: | --- | --- |
| 0 | `6e8d465056b5ea730a8b016e100a4c1e5e18e34bdaf4a4e768e380d5d1c4e36a` | `fd740d8277ee8bf982eec6c2e3cd29c0e14badc8a9d1039ef2d4c8843dec4372` |
| 1 | `9de878ea9a5d54a4b0569665b39c17c89fe80eed3955a0f0683feb39c2ce4aa5` | `ddc84ddbbac6e444e6ad0d46236412a2b7063ba9b518cb01bbf21e7f38c8ff77` |
| 5 | `14292afb8db0bf439d8f95f4aacf999bbc290efb373460d7b6035ffd78ad9f5c` | `5e754c312cc7148be815240762a3a95e566ad67e793493584d4ccd74d2c1405a` |
| 10 | `ae478c00bab8dce20bb0135539a0fe1e11acf318b4b1f81266ac58caece394a4` | `83318177d243f2e91b6b5360a1702d3ece40f544b98a2233cbf350fee45d489c` |
| 15 | `6c62d79220cfd66f1cb1e8107705a4a96471b2634b31e8856ad952c2c948c813` | `5309a7b7990a375efd9c387e6591c538ae1656dfcedc7adde0641ce691b4b0eb` |
| 30 | `a217512a9cca891a6222ae11c56c33c0623e770197bee8c2f2d76a2ac81d3e91` | `e1ad37204c4ffa18b00a8c723edadeb1aee2eedf7bc2da27069060e6b57cafad` |
| 45 | `f6bdd1d0dd75e81b09e92574df45bac3df23d5b2ee539da20f99be7093142b7b` | `6ff2181b2c9b483365dd7c99903352e27f801c15a75665afd8f1ae43c38c5413` |
| 59 | `a040e2da6891c11a9859d68efabcc179a53685001a6da71b85ebb3ab70943bb7` | `4385208381e0f00bfcfd00afb071e9fa96e0ba6e1632a0f1db761321026ae5b4` |

Native frame `0` and `1` are still not byte-identical, matching the Round 2
temporal fix.

## Trace Sidecar

Fresh conformance output currently emits only:

```text
target/ae_agents/round2_temporal_stack/report.json
target/ae_agents/round2_temporal_stack/TMP_020/metrics.json
target/ae_agents/round2_temporal_stack/STK_030/metrics.json
```

No case-local adjustment trace sidecar is emitted by the conformance runner.

Added a minimal render-sequence sidecar in `render_sequence.rs`:

```text
adjustment_effects.jsonl
```

Each JSONL row is one frame/effect record with:

```text
frame, time, composition, layer_id, effect_index, match_name,
comp_time, layer_time, lower_stack_time, bucket_time, param_time,
input_hash, output_hash, posterize
```

The sidecar does not change timing logic. A direct `render-cli render` attempt
against the saved `STK_030/scene.json` could not generate the sidecar because
that CLI path rejects primitive image assets as non-video footage. The source
serializer and unit coverage are in place; wiring the conformance runner itself
to call traced rendering remains a render-cli integration task outside this
agent's write scope.

## Trace Evidence

`cargo test -p render-core posterize -- --nocapture` passed 3 tests, including
`adjustment_posterize_keeps_downstream_param_time_live_inside_bucket`.

That guard renders frame 5 at 10 fps and asserts:

- lower stack samples bucket time `0.0`;
- Posterize Time record uses `param_time=0.0`;
- downstream Minimax record uses live `param_time=0.5`;
- downstream Minimax input/output hashes differ.

`cargo test -p render-core adjustment_effect_sidecar -- --nocapture` passed and
verifies the JSONL sidecar writes one record per adjustment effect with bucket
time, param time, and input/output hashes.

## Gate Decision

`M16` is cleared as a temporal blocker.

Reasons:

- `TMP_020` remains RGB-clean and background-alpha-normalized clean.
- `STK_030` native frame `0`/`1` are distinct, so the old frozen-native temporal
  symptom has not returned.
- The adjustment Posterize guard proves lower-stack bucket resampling while
  downstream adjustment effects evaluate animated params at current comp time.
- Remaining `STK_030` RGB error is stable stack/operator divergence and should
  be routed to `M12` Geometry2, `M13` Minimax, and `M14` Turbulent Displace.

There is no current evidence for a remaining global adjustment timing blocker.

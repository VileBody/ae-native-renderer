# Agent Round 7 Warps Fields

Scope: Worker 4, M12/M14 Geometry / Turbulent / Procedural Fields. This pass
only adds measurable AE-vs-native Turbulent field comparison plumbing. No native
Turbulent formula tuning was done.

## Inputs Reviewed

- `fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field/ae_goldens/metadata/turbulent_vector_measurements.json`
- `fixtures/ae_probe_pack/turbulent_field/scripts/measure_turbulent_vectors.py`
- `crates/effects/src/turbulent_displace.rs`
- Round 5/Round 6 turbulent reports and the AE builder log.

Relevant current native telemetry:

- `turbulent_displace_field_telemetry` reports resolved params, 9 fixed probe
  samples, a field hash, sampler mode, edge policy, and out-of-bounds count.
- Existing unit tests cover param parsing, resolved bounds, seed/evolution
  determinism, and current EFF_060 sidecar hashes.
- The API does not yet emit arbitrary AE measurement points, so this pass keeps
  Rust untouched and uses a Python mirror of the current field model for sample
  comparison.

## Tool Added

Added:

```text
fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py
```

The tool accepts Round 5 `turbulent_vector_measurements.json`, maps each case id
to AE probe params, and compares AE decoded source offsets against the current
native sampled source offsets at the same output points.

The Round 5 measurement JSON is compact and does not include
`frames[].samples`. When samples are absent, the tool resolves each recorded PNG
path and reuses `measure_turbulent_vectors.measure_frame(..., include_samples=True)`
to reconstruct the per-sample AE vectors before comparing.

Native side note: the current script mirrors `crates/effects/src/turbulent_displace.rs`.
That is good enough to produce fitting numbers now, but the next API step should
export arbitrary sample-point telemetry from Rust or a tiny native CLI so this
mirror cannot drift.

## Representative Results

Command:

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py \
  --measurements fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field/ae_goldens/metadata/turbulent_vector_measurements.json \
  --output /tmp/turbulent_native_compare_round7.json
```

Result summary, excluding AE samples whose coordinate parity decode failed:

| Case | Samples | Mean abs dx err | Mean abs dy err | Mean abs mag err | Mean vector err | P95 vector err | Center AE dx/dy | Center native dx/dy |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| `TD_AMOUNT_SWEEP_A000` | 2502 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | `(0,0)` | `(0,0)` |
| `TD_AMOUNT_SWEEP_A001` | 2474 | 0.192 | 0.253 | 0.435 | 0.435 | 1.000 | `(0,34)` | `(0,0)` |
| `TD_AMOUNT_SWEEP_A010` | 2481 | 2.532 | 2.293 | 2.092 | 3.942 | 7.211 | `(2,-1)` | `(1,2)` |
| `TD_AMOUNT_SWEEP_A045` | 2489 | 8.582 | 7.899 | 6.667 | 13.344 | 29.275 | `(8,-6)` | `(5,8)` |
| `TD_AMOUNT_SWEEP_A100` | 2498 | 15.934 | 15.622 | 13.142 | 25.535 | 55.946 | `(17,-13)` | `(11,18)` |
| `TD_SIZE_SWEEP_S065` | 2489 | 8.582 | 7.899 | 6.667 | 13.344 | 29.275 | `(8,-6)` | `(5,8)` |
| `TD_DISPLACEMENT_TYPE_01` | 2489 | 8.582 | 7.899 | 6.667 | 13.344 | 29.275 | `(8,-6)` | `(5,8)` |
| `TD_DISPLACEMENT_TYPE_09` | 2489 | 0.000 | 10.730 | 8.570 | 10.730 | 28.000 | `(0,5)` | `(0,6)` |
| `TD_SEED_SWEEP_999` | 2477 | 8.313 | 8.212 | 7.499 | 13.547 | 31.401 | `(1,-4)` | `(2,-6)` |
| `TD_RESIZE_LAYER_ON` | 2499 | 17.128 | 15.640 | 13.326 | 26.307 | 55.227 | `(15,-12)` | `(13,21)` |

Aggregate representative run:

| Case frames | Samples | Mean abs dx err | Mean abs dy err | Mean abs mag err | Mean vector err | Max P95 vector err |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 10 | 24887 | 6.993 | 7.653 | 6.513 | 12.066 | 55.946 |

Interpretation:

- `amount=0` is aligned as pass-through.
- Small amount samples are mostly close except modulo/parity decode ambiguity at
  center for `A001`; parity-failed samples are counted separately and skipped
  from aggregate error by default.
- Default displacement type/size/amount has a strong dy sign/basis mismatch:
  AE center `(8,-6)` vs native sampled `(5,8)`.
- Type 9 center is close but its full-frame dy distribution is still off, so a
  center-only fit would be misleading.
- Seed 999 is directionally closer at center than the default case, but
  full-field error remains high enough to require seed coordinate/phase fitting.
- Resize-on currently mostly inherits the same field mismatch, plus larger
  amount/size makes the error easier to see.

## Params Mapped

The comparison harness maps these AE probe controls into the native field model:

| AE control | Native field | Current comparison mapping |
| --- | --- | --- |
| `0001` Displacement | `displacement_type` | Parsed from `TD_DISPLACEMENT_TYPE_XX`, clamped `1..9` by native mirror |
| `0002` Amount | `amount` | Parsed from amount/sampler/resize suites |
| `0003` Size | `size` | Parsed from size/resize/sampler suites; `S001` uses AE default `100` because AE rejected value `1` |
| `0004` Offset (Turbulence) | `offset` | Uses AE property default `(256,256)` from the builder log |
| `0005` Complexity | `complexity` | Parsed from complexity suite or suite fixed params |
| `0006` Evolution | `evolution` | Static value from case id; animation maps frame `n` to `n * 3` degrees |
| `0010` Random Seed | `random_seed` | Parsed from seed suite |
| `0012` Pinning | `pinning` | Default `3` for coordinate-field cases |
| `0013` Resize Layer | `resize_layer` | Parsed from resize suite |

## Next Tuning Order

1. Replace the Python model mirror with native arbitrary-point telemetry, or add
   a tiny Rust-backed report path, before changing the formula.
2. Fit sampler/decode trust first using `TD_SAMPLER_CHECK_COORD_*`; keep parity
   mismatches separate from trusted coordinate samples.
3. Fit coordinate origin and sign/basis for displacement type 1 using
   `TD_AMOUNT_SWEEP_A010/A045/A100` and `TD_SIZE_SWEEP_S065`.
4. Fit amount scale after the sign/origin issue is resolved; current magnitude
   error is mixed with direction error.
5. Fit size/frequency across `TD_SIZE_SWEEP_*`, excluding `S001` or treating it
   as AE default `100`.
6. Fit displacement type bases using `TD_DISPLACEMENT_TYPE_01..09`, with type 9
   as a separate vertical/scalar branch.
7. Fit random seed coordinate offsets and phase with `TD_SEED_SWEEP_*`.
8. Fit complexity/octave weighting and evolution phase/periodicity after the
   static field basis is closer.
9. Revisit pinning/resize edge policy only after the interior field error drops;
   edge behavior is otherwise dominated by the same field mismatch.

## Verification

Commands:

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py --help
```

Result: passed and printed CLI usage.

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py \
  --measurements fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field/ae_goldens/metadata/turbulent_vector_measurements.json \
  --output /tmp/turbulent_native_compare_round7.json
```

Result: passed; wrote `/tmp/turbulent_native_compare_round7.json`; compared 10
representative case frames and 24,887 trusted samples.

```sh
python3 -m py_compile fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py
```

Result: passed.

Rust was not touched in this pass, so `cargo test -p effects turbulent` was not
required.

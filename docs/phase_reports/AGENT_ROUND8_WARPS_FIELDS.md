# Agent Round 8 Warps / Geometry / Turbulent Field

Scope: Agent 4, M14 `ADBE Turbulent Displace` field comparison tooling and
analysis only.

No Rust formula, sampler, temporal, text, render-core, or unrelated effects code
was edited in this pass. `crates/effects/src/turbulent_displace.rs` was already
dirty in the shared worktree and was intentionally left untouched.

## Inputs Reviewed

- `fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py`
- `fixtures/ae_probe_pack/turbulent_field/scripts/measure_turbulent_vectors.py`
- `fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field/ae_goldens/metadata/turbulent_vector_measurements.json`
- Round 7 Warps/Fields report and the turbulent probe README/JSX case setup.

The Round 5 measurement JSON is compact: case/frame summaries are present, but
full `frames[].samples` are not. The comparator therefore resolves each recorded
PNG and reconstructs sample rows with
`measure_turbulent_vectors.measure_frame(..., include_samples=True)`.

## Tool Update

Updated:

```text
fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py
```

New capability:

- report schema bumped to
  `ae-native-renderer.turbulent-native-vector-comparison.v2`;
- every ok case-frame now gets a signed permutation scan over native sampled
  vectors: identity, x/y flips, xy swap, and 90-degree rotations;
- the JSON report includes grouped fitting hints by suite
  (`amount`, `size`, `displacement_type`, `seed`, `complexity`, `evolution`,
  `resize_layer`, `sampler_check`);
- `analysis.next_tuning_order` is emitted directly by the tool;
- `--csv-output` writes a flat case-frame CSV with group, center vectors, error
  metrics, parity-skip count, and signed-basis winner.

This is still a fitting/reporting harness. It mirrors the current Rust field
model for comparison; it does not tune the native formula.

## Round 5 Full Run

Command:

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py \
  --measurements fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field/ae_goldens/metadata/turbulent_vector_measurements.json \
  --all-cases \
  --output /tmp/turbulent_native_compare_round8_all.json \
  --csv-output /tmp/turbulent_native_compare_round8_all.csv
```

Result:

| Case frames | Trusted samples | Mean abs dx err | Mean abs dy err | Mean abs mag err | Mean vector err | Max P95 vector err | Max vector err |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 102 | 253,549 | 8.447 | 8.227 | 7.453 | 13.700 | 75.664 | 149.215 |

Representative case rows:

| Case | Samples | Mean vector err | P95 vector err | Center AE dx/dy | Center native dx/dy | Skipped parity |
| --- | ---: | ---: | ---: | --- | --- | ---: |
| `TD_AMOUNT_SWEEP_A000` | 2502 | 0.000 | 0.000 | `(0,0)` | `(0,0)` | 0 |
| `TD_AMOUNT_SWEEP_A010` | 2481 | 3.942 | 7.211 | `(2,-1)` | `(1,2)` | 21 |
| `TD_AMOUNT_SWEEP_A045` | 2489 | 13.344 | 29.275 | `(8,-6)` | `(5,8)` | 13 |
| `TD_AMOUNT_SWEEP_A100` | 2498 | 25.535 | 55.946 | `(17,-13)` | `(11,18)` | 4 |
| `TD_DISPLACEMENT_TYPE_09` | 2489 | 10.730 | 28.000 | `(0,5)` | `(0,6)` | 13 |
| `TD_SIZE_SWEEP_S256` | 2501 | 33.918 | 75.664 | `(30,-24)` | `(5,8)` | 1 |
| `TD_RESIZE_LAYER_ON` | 2499 | 26.307 | 55.227 | `(15,-12)` | `(13,21)` | 3 |

Grouped fitting hints from the full run:

| Group | Case frames | Samples | Mean vector err | Max P95 vector err | Worst case |
| --- | ---: | ---: | ---: | ---: | --- |
| `amount` | 5 | 12,444 | 8.667 | 55.946 | `TD_AMOUNT_SWEEP_A100@0` |
| `size` | 7 | 17,437 | 15.526 | 75.664 | `TD_SIZE_SWEEP_S256@0` |
| `displacement_type` | 9 | 22,398 | 12.904 | 40.000 | `TD_DISPLACEMENT_TYPE_02@0` |
| `seed` | 5 | 12,447 | 14.160 | 33.136 | `TD_SEED_SWEEP_10@0` |
| `complexity` | 5 | 12,450 | 14.120 | 32.249 | `TD_COMPLEXITY_SWEEP_C01@0` |
| `evolution_static` | 6 | 14,936 | 14.696 | 36.401 | `TD_EVOLUTION_STATIC_E720@0` |
| `evolution_anim` | 60 | 148,995 | 13.998 | 33.242 | `TD_EVOLUTION_ANIM@59` |
| `resize_layer` | 2 | 4,998 | 26.307 | 55.227 | `TD_RESIZE_LAYER_OFF@0` |
| `sampler_check` | 3 | 7,444 | 2.329 | 6.083 | `TD_SAMPLER_CHECK_COORD_A004@0` |

## Type-1 Basis/Sign Diagnostic

The type-1 signed-permutation scan covers 28 case-frames and 69,701 trusted
samples. Aggregate result:

| Candidate | Mean vector err | Mean cosine | Opposite direction fraction | Max P95 vector err |
| --- | ---: | ---: | ---: | ---: |
| `identity` | 14.108 | -0.006 | 0.473 | 75.664 |
| `flip_xy` | 14.004 | 0.006 | 0.468 | 75.107 |
| `flip_x` | 14.009 | 0.019 | 0.454 | 75.107 |
| `flip_y` | 14.045 | -0.019 | 0.486 | 75.664 |

Best candidate is `flip_xy`, but it improves type-1 mean vector error by only
0.7%. This is the important Round 8 result: do not tune amplitude first, and do
not assume a trivial y-sign fix. The current native field is not just mirrored;
its noise basis/origin/phase is not aligned with AE.

## Manual Inspection

I manually inspected representative Round 5 PNG rows by decoding raw `RGBA`
facts at center and nearby samples, then comparing those AE decoded vectors with
the current native sampled vectors.

`TD_AMOUNT_SWEEP_A001`:

- Center `(256,256)` raw `RGBA=(0,34,34,255)` decodes to AE source
  `(256,290)`, so AE dx/dy is `(0,34)`, but parity is false. Native sampled
  dx/dy is `(0,0)` with a tiny native field `(0.11,0.18)`.
- Nearby `(264,256)` raw `RGBA=(8,20,20,255)` decodes to AE `(0,20)` and also
  shows subpixel/interpolation/parity ambiguity at tiny amount.
- Visual/semantic read: amount 1 is dominated by modulo decode ambiguity around
  the coordinate-field seams. It is useful for sampler trust, not for formula
  fitting.

`TD_AMOUNT_SWEEP_A045`:

- Center raw `RGBA=(8,250,255,255)` decodes to AE source `(264,250)`, dx/dy
  `(8,-6)`. Native samples from `(261,264)`, dx/dy `(5,8)`, native field
  `(4.73,8.04)`.
- The center is visually/semantically a clear direction mismatch: AE pulls from
  right/up, while native pulls from right/down.
- Nearby rows prove this is not one global y flip. At `(248,256)`, AE is
  `(6,-8)` while native is `(-3,2)`. At `(264,256)`, AE is `(9,-4)` while native
  is `(5,-7)`. At `(256,248)`, AE is `(9,-8)` while native is `(-7,-6)`.
  Different points disagree on x and y in different ways, matching the scan's
  "not a simple signed permutation" conclusion.

`TD_AMOUNT_SWEEP_A100`:

- Center raw `RGBA=(17,243,255,255)` decodes to AE source `(273,243)`, dx/dy
  `(17,-13)`. Native is `(11,18)`.
- This is the same center direction problem as A045, amplified by amount. The
  center y sign is opposite, but the surrounding points again show mixed x/y
  disagreement, so amplitude scale would only enlarge the wrong field.

`TD_DISPLACEMENT_TYPE_09`:

- Center raw `RGBA=(0,5,0,255)` decodes to AE `(0,5)`. Native is `(0,6)`, so the
  center looks close.
- Nearby `(248,256)` is AE `(0,6)` vs native `(0,0)`, `(264,256)` is AE `(0,5)`
  vs native `(0,-1)`, and `(256,248)` is AE `(0,5)` vs native `(0,-7)`.
- Visual/semantic read: type 9's center can fool a center-only check. It is
  correctly vertical-only in shape, but its vertical scalar distribution is
  still wrong across the field.

`TD_SIZE_SWEEP_S256`:

- Center raw `RGBA=(30,232,255,255)` decodes to AE `(30,-24)`. Native remains
  `(5,8)` at the center because the current mirror samples the same center
  noise phase at the offset.
- Nearby center-patch samples are also far apart: `(248,256)` AE `(29,-26)` vs
  native `(0,6)`, `(264,256)` AE `(32,-21)` vs native `(9,7)`.
- Visual/semantic read: size/frequency behavior cannot be tuned by amount
  scaling. AE size changes the interior field strongly where native's centered
  basis remains too similar to the default case.

## Exact Next Tuning Order

1. Add Rust-backed arbitrary-point field telemetry or keep the Python mirror
   locked to tests before formula work. This prevents tuning against a stale
   mirror.
2. Fit type-1 noise basis/origin/phase first. The signed permutation scan gives
   only 0.7% improvement, so global flip/swap is not the correction.
3. Fit amount scale using `TD_AMOUNT_SWEEP_A010/A045/A100` only after the type-1
   basis decision. `A000` is pass-through; `A001` is decode/sampler evidence.
4. Fit size/frequency using the size sweep, excluding `TD_SIZE_SWEEP_S001` or
   treating it as AE default-size fallback.
5. Fit displacement type branches with `TD_DISPLACEMENT_TYPE_01..09`; keep type
   9 as a separate vertical/scalar branch because center parity is misleading.
6. Fit seed phase/coordinate offsets with `TD_SEED_SWEEP_*`.
7. Fit complexity octave weighting.
8. Fit evolution phase and temporal period after static field basis/size/seed
   are closer.
9. Revisit resize/pinning edge policy after interior field error drops; current
   resize-layer error is mostly inherited field mismatch.

## Verification

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py --help
```

Passed; printed usage including new `--csv-output`.

```sh
python3 fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py \
  --measurements fixtures/ae_probe_pack/ae_outputs_round5_20260504_011829/ae_probe_pack/turbulent_field/ae_goldens/metadata/turbulent_vector_measurements.json \
  --all-cases \
  --output /tmp/turbulent_native_compare_round8_all.json \
  --csv-output /tmp/turbulent_native_compare_round8_all.csv
```

Passed; compared 102 case-frames and 253,549 trusted samples. Wrote:

- `/tmp/turbulent_native_compare_round8_all.json`
- `/tmp/turbulent_native_compare_round8_all.csv`

```sh
python3 -m py_compile fixtures/ae_probe_pack/turbulent_field/scripts/compare_turbulent_native_vectors.py
```

Passed.

Rust was not touched by this pass, so `cargo test -p effects turbulent` was not
required or run.

## Caveats

- The comparator still mirrors native Rust formulas in Python. The next tooling
  improvement should replace or cross-check this with native arbitrary-point
  telemetry.
- Parity-failed decoded samples remain excluded from aggregate error by default.
  The center of `TD_AMOUNT_SWEEP_A001` is intentionally inspected manually above
  but should not drive formula tuning.
- The signed-permutation scan tests basis/sign hypotheses only. It does not
  search continuous origin, phase, frequency, or noise-function parameters.

# Conformance Goldens

Conformance fixtures live under `fixtures/conformance/` and are intentionally
separate from central render integration. They provide small deterministic
scenes and a manifest format for AE parity checks without requiring checked-in
AE outputs.

## Layout

- `fixtures/conformance/manifest.json` lists each case, feature area, scene,
  frame indexes, pending AE reference PNG paths, and provisional thresholds.
- `fixtures/conformance/scenes/*.json` contains micro-scenes for Bezier/ease,
  effects, collapse transformations, and motion blur.
- `fixtures/conformance/ae-reference/<case>/` contains `.keep` placeholders for
  future AE PNG exports named `frame_000000.png`, `frame_000001.png`, etc.

The manifest schema is represented by `testkit::ConformanceManifest`. Its
validator checks case uniqueness, safe repository-relative paths, frame
metadata, and threshold sanity. It does not require AE PNGs to exist while a
case has `status: "pending_ae_export"`.

## Adding AE References

When a fixture is ready to become an enforced golden:

1. Recreate the micro-scene in AE with the same composition size, fps, duration,
   colors, transforms, effect matchNames, and motion-blur settings.
2. Export straight PNGs into the matching
   `fixtures/conformance/ae-reference/<case>/` directory. The manifest frames
   are the must-inspect frames; today's compare CLI still expects a contiguous
   sequence from `frame_000000.png` through the highest listed frame.
3. Change that case's `ae_reference.status` to `ready` and fill
   `exported_from` with the AE project, script, or export recipe used.
4. Run the native render and compare commands with the manifest thresholds.

Example:

```bash
cargo run -p render-cli -- render \
  --scene fixtures/conformance/scenes/bezier_ease_probe_scene.json \
  --out target/conformance/native/bezier_ease_probe

cargo run -p render-cli -- compare \
  --native target/conformance/native/bezier_ease_probe \
  --reference fixtures/conformance/ae-reference/bezier_ease \
  --out target/conformance/diff/bezier_ease_probe \
  --frames 12 \
  --threshold-mean 0.75 \
  --threshold-max 8
```

Keep thresholds feature-specific. Bezier/ease and collapse probes should trend
tight as math parity improves. Effects and motion blur start looser because AE
filter kernels, sampling, and premultiplication details can differ before the
native implementations are tuned.

## AE Conformance Pack Runner

The generated AE pack under `fixtures/ae_conformance_pack/` already contains
clean AE PNG goldens. It does not require launching AE again for native-side
checks.

Run native recipes against the pack:

```bash
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_conformance_native
```

Run a focused case:

```bash
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_conformance_native_eff030 \
  --case EFF_030
```

The runner maps every pack `case.id` to a deterministic native scene recipe,
renders only `frames_to_compare`, copies the matching AE PNGs next to the native
frames, and writes:

```text
target/ae_conformance_native/report.json
target/ae_conformance_native/<CASE>/scene.json
target/ae_conformance_native/<CASE>/native/<CASE>_00000.png
target/ae_conformance_native/<CASE>/ae/<CASE>_00000.png
target/ae_conformance_native/<CASE>/diff/<CASE>_00000_diff.png
target/ae_conformance_native/<CASE>/metrics.json
```

`metrics.json` keeps the original raw RGBA fields (`max_abs_diff`,
`mean_abs_diff`, `rmse_abs_diff`, `changed_pixels`, `total_pixels`) at both
summary and per-frame level for compatibility. It also records
`metric_contract.schema = "m19.rgb_alpha_metric_policy.v1"` with explicit
straight/premult diagnostic flags, then structured diagnostic metrics:

- `rgba`: the same raw RGBA metric set.
- `rgb`: RGB-only comparison, useful when transparent-background alpha policy
  dominates raw RGBA.
- `alpha`: alpha-only comparison.
- `background_alpha_normalized`: RGBA comparison with alpha ignored only for
  pixels whose RGB matches the detected native and AE background-corner RGB.
- `foreground_rgb`: RGB comparison over the union foreground mask.
- `rgb_under_alpha_policy`: RGB comparison after straight RGBA8 source-over
  projection onto the AE/reference background RGB. This is the preferred M19
  diagnostic when invisible RGB or premult-looking partial-alpha pixels dominate
  raw metrics.
- `rgb_over_native_background` and `rgb_over_ae_background`: explicit projection
  variants for background-sensitivity checks.
- `background_corner`: per-frame native/AE corner RGBA plus
  `rgb_matches_alpha_differs` when the background RGB matches but alpha differs.

Use `rgb`, `alpha`, `background_alpha_normalized`, and
`rgb_under_alpha_policy` together before formula tuning. `metric_contract.flags`
records that no unpremultiply is applied and that the premult/straight contract
is diagnostic, not locked. Without thresholds, case status is `measured`; that
means frames and diffs were produced, not that AE parity has been achieved. Add
`--threshold-mean`, `--threshold-max`, and optionally `--fail-on-diff` when a
module is ready to become an enforced gate.

The shared guardrails for alpha/premult, gamma/color, edge sampling, time,
quality/bpc, CPU/GPU path, and parameter mapping live in
`docs/reverse_engineering/MATH_CONTRACTS_GUARDRAILS.md`.

## Worker E Step 4 EXP/GPH/CMP/TMP Run

Expression/collapse diagnostics were checked with:

```bash
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/worker_e_step4_after \
  --case EXP_010 --case GPH_010 --case CMP_010 --case TMP_010 --case TMP_020
```

Measured after metrics match the before run because the change is telemetry
only:

| Case | RGBA mean | RGBA max | Sidecar check |
| --- | ---: | ---: | --- |
| `EXP_010` | `1.3197853565` | `255` | 8 expression telemetry records with named subset context. |
| `GPH_010` | `14.5752954483` | `255` | 12 collapse records with matrix reports and deferred-raster checkpoints. |
| `CMP_010` | `4.3968381882` | `188` | Composite metrics unchanged. |
| `TMP_010` | `0.0` | `0` | Temporal baseline unchanged. |
| `TMP_020` | `0.0` | `0` | Posterize baseline unchanged. |

## Step 4 Integrated Math Run

The integrated Step 4 pass was run through the native runner after all worker
patches were merged:

```bash
cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/step4_math_after_current \
  --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 \
  --case INT_020 --case TMP_010 --case TMP_020 --case TMP_030 \
  --case EFF_010 --case EFF_020 --case EFF_030 --case EFF_070 \
  --case STK_030 --case EXP_010 --case GPH_010 --case CMP_010
```

Result:

```text
ok=true
cases=16
report=target/ae_agents/step4_math_after_current/report.json
```

The report should be compared to the local baseline with:

```bash
python3 scripts/compare_conformance_reports.py \
  target/ae_agents/step4_math_baseline/report.json \
  target/ae_agents/step4_math_after_current/report.json
```

The biggest measurable movement is in `text_passport.max_abs_delta`, not final
PNG mean. That distinction is intentional: Step 4 prioritizes turning modules
into localized, tuneable math objects before changing broad final-pixel
formulas.

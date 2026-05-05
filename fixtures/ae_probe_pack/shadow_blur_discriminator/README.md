# AE Shadow / Blur Discriminator Probe Pack

Runnable After Effects probe pack for resolving Drop Shadow behavior against
Box Blur candidates.

The pack is intentionally outside `fixtures/ae_conformance_pack`; it creates AE
probe outputs and measurements only.

## Coverage

- Direction sweep: all quadrants plus fractional projections at `30`, `120`,
  `210`, and `300` degrees.
- Softness sweep: `0, 1, 2, 4, 8, 12, 18, 32`.
- Box Blur discriminator candidates:
  - `radius = softness * 1.4`, `iterations = 1`
  - `radius = ceil(softness / 2.71)`, `iterations = 3`
- Alpha-only blur probe using a PNG whose transparent pixels contain RGB noise.
- Colored translucent composite probe with `Shadow Only` both on and off.

## Generate Required Asset

Before running the JSX builder in AE, generate the deterministic noise asset:

```text
python3 fixtures/ae_probe_pack/shadow_blur_discriminator/scripts/measure_shadow_blur_discriminator.py \
  --pack fixtures/ae_probe_pack/shadow_blur_discriminator \
  --generate-assets
```

This writes:

```text
fixtures/ae_probe_pack/shadow_blur_discriminator/assets/noisy_rgb_under_alpha.png
fixtures/ae_probe_pack/shadow_blur_discriminator/assets/assets_manifest.json
```

## Build And Render In AE

1. Open After Effects.
2. Run `File > Scripts > Run Script File...`.
3. Select:

```text
fixtures/ae_probe_pack/shadow_blur_discriminator/jsx/build_shadow_blur_discriminator_project.jsx
```

The script creates source comps, case comps under
`AE_SHADOW_BLUR_DISCRIMINATOR/cases`, render queue entries, and:

```text
fixtures/ae_probe_pack/shadow_blur_discriminator/ae_goldens/metadata/shadow_blur_discriminator_property_dump.json
```

Render the queued case comps as PNG sequences with RGB + Alpha:

```text
fixtures/ae_probe_pack/shadow_blur_discriminator/ae_goldens/png/<case_id>/<case_id>_[#####].png
```

Frame `0` is sufficient for every case.

## Measure AE Outputs

After AE rendering:

```text
python3 fixtures/ae_probe_pack/shadow_blur_discriminator/scripts/measure_shadow_blur_discriminator.py \
  --pack fixtures/ae_probe_pack/shadow_blur_discriminator
```

Outputs:

```text
fixtures/ae_probe_pack/shadow_blur_discriminator/ae_goldens/metadata/shadow_blur_discriminator_measurements.json
fixtures/ae_probe_pack/shadow_blur_discriminator/ae_goldens/metadata/shadow_blur_discriminator_measurements.csv
```

The metrics include alpha bbox offsets, alpha spread radius by threshold,
per-channel RGB/alpha checks, Box Blur pair comparisons, and overlap samples for
the colored translucent composite cases.

## Local Self-Test Without AE

The measurement script can generate synthetic dummy PNGs and then measure them:

```text
python3 fixtures/ae_probe_pack/shadow_blur_discriminator/scripts/measure_shadow_blur_discriminator.py \
  --pack fixtures/ae_probe_pack/shadow_blur_discriminator \
  --self-test-dir fixtures/ae_probe_pack/shadow_blur_discriminator/ae_goldens/self_test_png
```

This validates the JSON/CSV measurement path without requiring After Effects.

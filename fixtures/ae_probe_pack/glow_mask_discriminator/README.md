# AE Probe Pack: Glow Mask Discriminator

This probe pack creates runnable After Effects compositions for measuring Glow
source-mask, radius, intensity, and RGB/alpha compositing behavior. It does not
contain AE goldens; render the queued comps in AE, then run the measurement
script against the PNG outputs.

## Scope

- Source-mask threshold discriminator at `Glow Threshold = 120`,
  `Glow Radius = 0`, `Glow Intensity = 1`.
- `Glow Based On` comparison for default/property `0001` absent, `0001 = 1`,
  and `0001 = 2`.
- RGBA sample pixels covering dark high-alpha, bright low-alpha, dark low-alpha,
  threshold edge, and control samples.
- Radius impulse sweep for `0`, `0.5`, `1`, `2`, `5`, and `10`.
- Intensity impulse sweep for `0`, `0.5`, `1`, `1.25`, and `2`.
- Composite discriminator over transparent, opaque black, and 50% alpha black
  backgrounds.

All source imagery is generated procedurally by the JSX. No files from
`fixtures/ae_conformance_pack` are modified or required.

## How To Build In AE

1. Open After Effects.
2. Run `File > Scripts > Run Script File...`.
3. Select `jsx/build_glow_mask_discriminator_project.jsx`.
4. The script creates:
   - procedural precomps under `AE_PROBE_GLOW_MASK_DISCRIMINATOR/precomps`;
   - one label-free comp per probe case under
     `AE_PROBE_GLOW_MASK_DISCRIMINATOR/cases`;
   - `master_glow_mask_discriminator_probe_reel` for quick visual review;
   - render queue entries for every case comp.

## What To Export

Render the queued case comps as PNG sequences with alpha if available:

```text
ae_probe_outputs/png/<case_id>/<case_id>_[#####].png
```

Recommended output settings:

- Format: PNG sequence.
- Channels: RGB + Alpha.
- Color: Straight/unmatted when the AE output module exposes it.
- Depth: 8 bpc is enough for this discriminator.
- Frame range: frame 0 only for every case in this pack.

The master reel is optional and is only for human preview:

```text
ae_probe_outputs/preview/master_glow_mask_discriminator_probe_reel.mov
```

## Measure

From the repository root after copying AE renders back into this pack:

```sh
python3 fixtures/ae_probe_pack/glow_mask_discriminator/scripts/measure_glow_mask_discriminator.py \
  --pack fixtures/ae_probe_pack/glow_mask_discriminator
```

The script writes:

```text
fixtures/ae_probe_pack/glow_mask_discriminator/measurements/glow_mask_discriminator_measurements.json
fixtures/ae_probe_pack/glow_mask_discriminator/measurements/mask_samples.csv
fixtures/ae_probe_pack/glow_mask_discriminator/measurements/radius_profile.csv
fixtures/ae_probe_pack/glow_mask_discriminator/measurements/intensity_scale.csv
fixtures/ae_probe_pack/glow_mask_discriminator/measurements/rgb_alpha_split.csv
fixtures/ae_probe_pack/glow_mask_discriminator/measurements/composite_samples.csv
```

Quick script self-test without AE renders:

```sh
python3 fixtures/ae_probe_pack/glow_mask_discriminator/scripts/measure_glow_mask_discriminator.py --self-test
```

## Probe Reading Notes

The source-mask cases compare rendered sample centers before and after Glow. A
sample is marked as changed when any RGBA channel differs from the source by more
than the configured tolerance. The key discriminator samples are:

- `bright_low_alpha`: high RGB, alpha below 120.
- `dark_high_alpha`: low RGB, alpha above 120.
- `dark_low_alpha`: low RGB, alpha below 120.

If `bright_low_alpha` changes but `dark_high_alpha` does not, the mask likely
follows RGB/luma. If `dark_high_alpha` changes but `bright_low_alpha` does not,
the mask likely follows alpha. If both change, the rule is combined or the enum
selects a different source.

Radius and intensity cases use a single white impulse at `(256, 256)`. The
measurement script records horizontal profile samples, ring means, and
RGB/alpha split values so native implementations can compare spread and scale
without visually inspecting PNGs.

## Contract

This pack is intentionally not wired into native conformance. After AE renders
are produced, compare the JSON/CSV measurements to native-side measurements
before changing Glow formulas or accepting new goldens.

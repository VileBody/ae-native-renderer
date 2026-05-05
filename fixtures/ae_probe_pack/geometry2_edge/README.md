# AE Geometry2 Edge Probe Pack

Runnable After Effects probe pack for `ADBE Geometry2` pixel-center, edge/OOB,
and `0012` Sampling evidence. It is intentionally isolated from the conformance
pack and writes all generated output under this folder.

## What It Builds

The JSX imports four deterministic 64x64 PNG primitives:

- `coord_ramp`: decodes sampled source UV from RGB.
- `checker_1px`: separates bilinear smoothing from bicubic/ringing behavior.
- `edge_impulse`: exposes sampler footprint and edge contribution.
- `alpha_hidden_rgb_border`: transparent border pixels carry nonzero hidden RGB
  so transparent-black, clamp, and hidden-edge bleed can be distinguished when
  AE preserves straight RGB.

The render matrix is:

- transforms: identity, subpixel translate, scale 90%, small +5 degree rotation
- source edge targets near `-0.5`, `0`, `0.5`, `width - 1`, `width - 0.5`,
  and `width`
- `ADBE Geometry2-0012` sweep: value `1` Bilinear, value `2` Bicubic

## Generate Assets

Run once before opening the JSX in After Effects:

```sh
python3 fixtures/ae_probe_pack/geometry2_edge/scripts/measure_geometry2_edge_probe.py \
  --pack fixtures/ae_probe_pack/geometry2_edge \
  --generate-assets
```

This writes:

```text
fixtures/ae_probe_pack/geometry2_edge/assets/primitives/*.png
fixtures/ae_probe_pack/geometry2_edge/assets/primitives/assets_manifest.json
```

## Render In AE

1. Open After Effects.
2. Run `File > Scripts > Run Script File...`.
3. Select:

```text
fixtures/ae_probe_pack/geometry2_edge/jsx/build_geometry2_edge_probe_project.jsx
```

4. Render the queued PNG sequences. The builder points them at:

```text
fixtures/ae_probe_pack/geometry2_edge/ae_goldens/png8/<case_id>/<case_id>_[#####].png
```

The JSX also writes:

```text
fixtures/ae_probe_pack/geometry2_edge/ae_goldens/metadata/geometry2_edge_property_dump.json
fixtures/ae_probe_pack/geometry2_edge/ae_goldens/logs/geometry2_edge_builder_log.txt
```

## Measure AE Output

After AE renders are back in the repo:

```sh
python3 fixtures/ae_probe_pack/geometry2_edge/scripts/measure_geometry2_edge_probe.py \
  --pack fixtures/ae_probe_pack/geometry2_edge
```

Outputs:

```text
fixtures/ae_probe_pack/geometry2_edge/ae_goldens/metadata/geometry2_edge_measurements.json
fixtures/ae_probe_pack/geometry2_edge/ae_goldens/metadata/geometry2_edge_samples.csv
```

For a compact run with per-sample JSON omitted:

```sh
python3 fixtures/ae_probe_pack/geometry2_edge/scripts/measure_geometry2_edge_probe.py \
  --pack fixtures/ae_probe_pack/geometry2_edge \
  --no-json-samples
```

## Local Self-Test

The self-test generates synthetic bilinear PNGs in a temporary pack and verifies
the measurement path without requiring After Effects:

```sh
python3 fixtures/ae_probe_pack/geometry2_edge/scripts/measure_geometry2_edge_probe.py \
  --self-test
```

## Interpretation

Use `inferred_pixel_center` to decide whether AE's Geometry2 path aligns output
sample coordinates to integer pixel centers or half-pixel centers.

Use `oob_policy_candidates` and `edge_samples` to separate whole-sample
transparent OOB, bilinear partial-footprint transparency, clamp/edge extension,
and hidden transparent-border RGB behavior.

Use `quality_branch_evidence` to decide whether `0012=2` follows a distinct
sampler branch from `0012=1`; the checker and impulse primitives are the most
sensitive evidence for this.

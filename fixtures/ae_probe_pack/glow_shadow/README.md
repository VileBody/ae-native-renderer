# AE Probe Pack: Glow + Drop Shadow

This probe pack creates After Effects compositions for inspecting Glow and Drop
Shadow behavior before wiring new goldens into native conformance.

It does not contain AE goldens. The PNGs are expected to be rendered by a user in
After Effects and reviewed against the native intermediate sidecars from the
Round 4 work.

## Scope

- Glow threshold source and mask candidates.
- Glow blurred and intensity-scaled candidate buffers.
- Glow final AE outputs for the current `EFF_020` parameters and alpha/luma
  separation variants.
- Drop Shadow source alpha, no-softness raw shadow, blurred shadow-only output,
  and final composite output for the current `EFF_010` parameters.
- Drop Shadow manual raw-offset candidates for the likely direction convention.

All source imagery is generated procedurally by the JSX. No files from
`fixtures/ae_conformance_pack` are modified or required.

## How To Build In AE

1. Open After Effects.
2. Run `File > Scripts > Run Script File...`.
3. Select `jsx/build_glow_shadow_probe_project.jsx`.
4. The script creates:
   - procedural precomps under `AE_PROBE_GLOW_SHADOW/precomps`;
   - one label-free comp per probe case under `AE_PROBE_GLOW_SHADOW/cases`;
   - `master_glow_shadow_probe_reel` for quick visual review;
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
- Depth: 8 bpc is enough for matching the current conformance pack.
- Frame range: frame 0 only for every case in this pack.

The master reel is optional and is only for human preview:

```text
ae_probe_outputs/preview/master_glow_shadow_probe_reel.mov
```

## Probe Reading Notes

Glow cases use `ADBE Glo2` parameters matching `EFF_020`:

```text
Glow Threshold = 120
Glow Radius = 35
Glow Intensity = 1.25
```

The threshold-mask and blurred/scaled comps are candidate buffers, not AE
internal exports. Compare them to the actual AE Glow final variants to answer
which source participates in thresholding: luma, alpha, or a combined rule.

Drop Shadow cases use `ADBE Drop Shadow` parameters matching `EFF_010`:

```text
Color = black
Opacity = 180
Direction = 135
Distance = 28
Softness = 18
Shadow Only = off
```

The no-softness shadow-only case should reveal the raw offset. The softened
shadow-only case isolates the blurred shadow from the source composite. Manual
offset candidates are included to make the direction convention easy to inspect
without changing native effect code.

## Contract

This pack is intentionally not wired into native conformance. After AE renders
are produced, the next step is to compare these outputs to native sidecars and
only then decide whether Glow or Drop Shadow formulas need tuning.

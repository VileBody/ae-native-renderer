# Drop Shadow Composite Probe - 2026-05-05

## Scope

M10 focused probe for Drop Shadow color, opacity, Shadow Only, source-alpha
scaling, and final source-over behavior. Softness is fixed to `0` in this pass
so blur radius/kernel questions stay isolated.

## AE Run

Node:

```text
85.239.48.31
```

Command:

```bash
python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/glow_shadow \
  --entry-script jsx/build_drop_shadow_composite_probe_project.jsx \
  --node http://85.239.48.31:8000 \
  --job-id drop_shadow_composite_probe_20260505 \
  --poll-interval-s 5 \
  --timeout-s 1200
```

Outputs:

```text
target/ae_remote/drop_shadow_composite_probe_20260505/
target/ae_remote/drop_shadow_composite_probe_20260505/drop_shadow_composite_measurements.json
```

Measurement:

```bash
python3 fixtures/ae_probe_pack/glow_shadow/scripts/measure_drop_shadow_composite_probe.py \
  target/ae_remote/drop_shadow_composite_probe_20260505/extracted/drop_shadow_composite_probe_20260505/glow_shadow/ae_probe_outputs/png \
  --output target/ae_remote/drop_shadow_composite_probe_20260505/drop_shadow_composite_measurements.json
```

The AE output from this render is opaque over the black comp background, so this
pass locks visible-over-black behavior. It does not reopen the global M19 raw
alpha contract.

## Key Samples

| Case | Center RGBA | Offset RGBA | Meaning |
| --- | --- | --- | --- |
| `DSC_OPACITY_25` | `[25,0,0,255]` | `[0,0,0,255]` | opacity value maps to visible alpha contribution `25/255` |
| `DSC_OPACITY_50` | `[50,0,0,255]` | `[0,0,0,255]` | opacity is not percent |
| `DSC_OPACITY_100` | `[100,0,0,255]` | `[0,0,0,255]` | `100` means `100/255`, not full opacity |
| `DSC_OPACITY_180` | `[180,0,0,255]` | `[0,0,0,255]` | legacy template value remains raw 0..255 |
| `DSC_OPACITY_255` | `[255,0,0,255]` | `[0,0,0,255]` | full opacity |
| `DSC_SOURCE_ALPHA_50` | `[50,0,0,255]` | `[0,0,0,255]` | source alpha multiplies raw opacity |
| `DSC_COLOR_RED` | `[50,0,0,255]` | `[0,0,0,255]` | RGB color is preserved in shadow |
| `DSC_COLOR_GREEN` | `[0,50,0,255]` | `[0,0,0,255]` | RGB color is preserved in shadow |
| `DSC_COLOR_BLUE` | `[0,0,50,255]` | `[0,0,0,255]` | RGB color is preserved in shadow |
| `DSC_SHADOW_ONLY_OFFSET` | `[0,0,0,255]` | `[50,0,0,255]` | Shadow Only suppresses source |
| `DSC_FINAL_OFFSET` | `[0,0,255,255]` | `[50,0,0,255]` | final output composites source over shadow |
| `DSC_FINAL_OVERLAP_HALF` | `[153,128,128,255]` | `[0,0,0,255]` | half-alpha source over same-pixel red shadow |

## Formula Update

Before this pass native used a dual interpretation:

```text
opacity <= 100 -> opacity / 100
opacity > 100  -> opacity / 255
```

AE evidence shows the numbered Drop Shadow opacity value is raw 0..255:

```text
opacity_normalized = clamp(opacity / 255, 0, 1)
shadow_alpha = round(source_alpha * opacity_normalized)
```

The final composite remains normal source-over with the source layer over the
shadow contribution.

## Remaining M10 Work

- Fractional direction/distance sweep across quadrants.
- Softness branch/kernel edge behavior beyond the alpha-only/radius path already
  traced.
- Raw alpha export/golden if we need to tune transparent RGB, not only visible
  RGB over AE background.

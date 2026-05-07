# P1 STK_030 Stage Gate

Date: 2026-05-07

## Goal

Split the `STK_030` scenes-3rd adjustment stack into deterministic AE/native
stage gates so P1 tuning does not rely on final-pixel guessing.

`STK_030` stack:

```text
lower canvas: numbered_frames @ 70% + coordinate_field
adjustment:   Geometry2 -> Posterize Time -> Minimax -> Turbulent Displace
```

## Added Cases

| Case | Stage |
| --- | --- |
| `STK_030_S00_BASE` | lower canvas before adjustment |
| `STK_030_S01_GEOMETRY2` | after Geometry2 |
| `STK_030_S02_POSTERIZE` | after Geometry2 + Posterize Time |
| `STK_030_S03_MINIMAX` | after Geometry2 + Posterize Time + Minimax |
| `STK_030_S04_TURBULENT` | full stack, isolated as a stage probe |

Frames compared: `0`, `1`, `59`.

## Artifacts

| Kind | Path |
| --- | --- |
| AE remote output | `target/ae_remote/ae_conformance_stk030_stages_20260507_123018` |
| Native stage gate | `target/conformance_tmp/stk030_stage_gate_20260507_123018` |
| Normal `STK_030` rerun | `target/conformance_tmp/stk030_stage_gate_baseline_20260507_123018` |
| Golden PNGs | `fixtures/ae_conformance_pack/ae_goldens/png/STK_030_S0*` |

AE render queue filtering enabled only the five new stage cases. Each case
rendered 60 frames; the native gate compares the manifest-selected frames.

## Metrics

Primary tuning metric below is `rgb_straight_source_over_ae_background.mean`.
Raw RGBA is not used for effect formula tuning because the transparent
background alpha policy intentionally differs at the PNG/TIFF boundary.

| Case | Frame 0 | Frame 1 | Frame 59 | Meaning |
| --- | ---: | ---: | ---: | --- |
| `STK_030_S00_BASE` | `0.000000` | `0.000000` | `0.000000` | lower canvas matches visibly |
| `STK_030_S01_GEOMETRY2` | `0.310652` | `0.310652` | `0.310652` | small static Geometry2 residual |
| `STK_030_S02_POSTERIZE` | `0.310652` | `0.310652` | `0.310652` | Posterize adds no new diff here |
| `STK_030_S03_MINIMAX` | `0.310909` | `0.310909` | `0.310909` | Minimax adds only tiny visible drift |
| `STK_030_S04_TURBULENT` | `0.319536` | `2.488029` | `4.244668` | frame-dependent spike starts at Turbulent |

Normal `STK_030` rerun confirms the same shape:

| Frame | Primary visible mean |
| ---: | ---: |
| `0` | `0.338142` |
| `1` | `2.524728` |
| `59` | `4.283328` |

## Conclusion

The P1 blocker is now isolated to `M14 Turbulent Displace` inside the composed
adjustment stack. `M12 Geometry2`, `M15 Posterize Time`, `M13 Minimax`, and
`M16` stack routing are not the current frame-dependent blocker for `STK_030`.

The remaining Turbulent issue is not the already-fixed static pinning fade:
frame `0` is close, while frames `1` and `59` drift as evolution changes. The
next reverse pass should target Turbulent's animated field path, not open-ended
metric fitting.

## Required Reverse Questions

To move P1 from diagnostic to implementation:

1. Capture Frida state for `STK_030_S04_TURBULENT` frames `0`, `1`, and `59` on
   the CPU path.
2. Confirm the exact evolution path: fixed16 conversion, phase accumulator,
   octave phase offsets, and any temporal quantization.
3. Confirm effect-space inputs for adjustment-layer Turbulent after upstream
   effects: input world bounds, state offset, source/sample coordinates, and
   whether previous effect extents alter the kernel domain.
4. Confirm the pixel sampler callback policy used by Turbulent in this stack:
   bilinear location, edge handling, alpha/premult wrapper, and output rounding.
5. Implement only recovered behavior in Rust, then rerun:
   `STK_030_S00_BASE..S04_TURBULENT`, `EFF_060`, and normal `STK_030`.

Acceptance target for this pass: `S00..S03` must not regress, and
`S04_TURBULENT` frames `1` and `59` should fall into the same low-error band as
isolated `EFF_060` before declaring P1 closed.

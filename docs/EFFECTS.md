# Effects Module Roadmap

Initial first-party AE matchName support:

```text
ADBE Drop Shadow       approximate
ADBE Glo2              approximate
ADBE Box Blur2         approximate
ADBE Turbulent Displace approximate
ADBE Posterize Time     temporal quantization in render-core
ADBE Geometry2          approximate
ADBE Minimax            approximate
```

## Parameter contract

Effect params accept both generated payload values (`{ "0001": { "value": ... } }`) and direct JSON values (`{ "0001": ... }`). Named aliases are accepted where practical, but AE-style numbered params are the compatibility baseline.

| matchName | Typed params | Numbered params |
| --- | --- | --- |
| `ADBE Box Blur2` | `radius`, `iterations` | `0001` radius, `0002` iterations; iterations run repeated separable blur passes; `0002` is also accepted as a legacy radius fallback when no radius param is present |
| `ADBE Drop Shadow` | `color`, `opacity`, `direction_degrees`, `distance`, `softness`, `shadow_only` | `0001` color, `0002` opacity as AE raw `0..255`, `0003` direction, `0004` distance, `0005` softness, `0006` shadow only |
| `ADBE Glo2` | `based_on`, `threshold`, `radius`, `intensity` | `0001` glow based on (`1` color channels, `2` alpha channel; absent keeps combined legacy source), `0002` threshold, `0003` radius, `0004` intensity |
| `ADBE Geometry2` | `anchor`, `position`, `scale`, `rotation`, `skew`, `skew_axis`, `pixelAspect`, `sampling` | AE property-index ids confirmed by `effect_property_dump.json` and Frida CPU wrapper dumps: `0001` anchor, `0002` position, `0003` uniform-scale checkbox, `0004` scale height, `0005` scale width, `0006` skew, `0007` skew axis, `0008` rotation, `0009` effect opacity slot, `0010` use comp shutter, `0011` shutter angle, `0012` sampling (`1` bilinear, `2` bicubic); native currently implements transform controls plus sampling mode and ignores the Geometry2 opacity/shutter controls |
| `ADBE Minimax` | `operation`, `radius`, `channels`, `direction`, `dont_shrink_edges` | `0001` operation (`1` minimum, `2` maximum, `3` minimum then maximum, `4` maximum then minimum), `0002` radius, `0003` channels (`1` color, `2` alpha and color, `3` red, `4` green, `5` blue, `6` alpha), `0004` direction (`1` horizontal and vertical, `2` horizontal, `3` vertical), `0005` don't shrink edges |
| `ADBE Turbulent Displace` | `displacement`, `amount`, `size`, `offset`, `complexity`, `evolution`, `random_seed`, `pinning`, `resize_layer` | `0001` displacement type, `0002` amount, `0003` size, `0004` offset, `0005` complexity, `0006` evolution, `0010` random seed, `0012` pinning, `0013` resize layer |
| `ADBE Posterize Time` | `frame_rate` | `0001` frame rate; layer/source/effect time is quantized in `render-core`; the stateless canvas-stage effect remains pass-through |

Scalar numbered params support direct numbers, wrapped `value`, and the existing simple scalar keyframe shape where the renderer already evaluates it. Color params accept normalized `0..1` channels or byte `0..255` channels.

`ADBE Box Blur2` currently uses a clipped sample window at layer bounds for edge pixels, reported in the effect debug trace as `clip_to_layer_bounds`. AE repeat-edge behavior still needs an isolated probe before changing this policy.

`ADBE Geometry2` uses the recovered `GPUFoundation.dll` transform matrix order.
The 2026-05-06 isolated edge probe selected integer pixel centers and a
partial-footprint transparent bilinear edge policy: bilinear taps outside the
source image contribute transparent black, while taps inside the source still
contribute normally. The conformance runner writes matrix, inverse, sample
UV/RGBA, RGB/alpha split metrics, and RGB-over-background metrics so formula
tuning can ignore known background-alpha noise.
`fixtures/ae_conformance_pack/jsx/build_conformance_project.jsx` writes
`ae_goldens/metadata/effect_property_dump.json`; the 2026-05-04 remote AE run
confirmed the Geometry2 control indices above. Frida traces on the no-GPU AE85
node show the relevant path as CPU `Transform.aex+0x5f30` plus
`GPUFoundation.dll` matrix/bounds helpers, not the GPU motion/quality export
path. A follow-up Frida dump of the `Transform.aex+0x5b20` wrapper confirmed
`PF_ParamDef[12]` as `Sampling`; word offset `56` low `s32` carries `1` for
Bilinear and `2` for Bicubic. Native dispatches on that value. The 2026-05-06
Sampling=2 fit pass selected a Keys cubic kernel with `a=-0.7`, transparent
partial-footprint edges, and round quantization for the bicubic branch. The
alpha follow-up selected an effect-local sampler wrapper: accumulate
premultiplied color, then unpremultiply back to the renderer's straight-RGBA
boundary.

`ADBE Minimax` implements the enum surface recovered from the AEX strings and
CPU callbacks. The native pass model is one-dimensional horizontal/vertical
extrema composed according to Direction. The 2026-05-06 discriminator probe
confirmed AE-style positive radius rounding (`0.5 -> 1`), direction/channel
lane behavior, and `Don't Shrink Edges` polarity. With `0005 = 0`, the Minimax
window samples transparent black outside image bounds; with `0005 = 1`, the
window clips to image bounds. `minimax_debug_trace` reports the resolved edge
policy for stack telemetry.

`ADBE Turbulent Displace` is still rendered by the deterministic native
approximation, but its field telemetry now includes an AE-wrapper contract block
from the Ghidra pass: inferred internal displacement mode, kernel path
(`FracAll` vs `Frac1D`), fixed16 amount/size/offset/evolution, complexity
integer/fraction split, and H/V lookup lengths. This is instrumentation for
formula replacement/tuning, not a parity claim. The exact vector/sampler math is
blocked by hidden `TurbulentDisplaceFracAllKernel` and
`TurbulentDisplaceFrac1DKernel`; replacement work must use kernel extraction or
coordinate-field probes, not final PNG pixels.

## Production/perf manifest

`render-core` writes `manifest.json` and `render-log.jsonl` with render observability fields. Existing top-level fields remain stable.

- `renderer`: crate/version metadata, currently `{ "crate": "render-core", "version": "..." }`.
- `timing.total_ms`: total PNG sequence render duration, including frame render and PNG saves.
- `timing.frames[]`: per-frame `frame`, comp `time`, `render_ms`, `save_ms`, `total_ms`, and relative output `path`.
- `feature_counts.layers`: total layer count across top-level and nested comps, top-level count, nested composition count, and `by_type`.
- `feature_counts.effects`: total/supported/approximate/unsupported counts plus `by_match_name`.
- `asset_hashes[]`: deterministic SHA-256 hashes of each asset spec (`id`, type, path), marked with `source: "asset_spec"`. These are cheap manifest hashes, not media file content hashes.

`render-log.jsonl` mirrors the important timing fields on `render.start`, each `frame.rendered`, and `render.done`.

## Registry rules

- unknown effect fails in strict mode;
- known stub logs unsupported/placeholder;
- real implementation must have a micro-scene fixture;
- every effect must expose parameter parsing separately from pixel math.
- v0 parameter parsing accepts the generated payload shape and direct JSON values.

## Suggested implementation order

1. `ADBE Box Blur2` — separable blur.
2. `ADBE Drop Shadow` — alpha copy, color, blur, offset, under composite.
3. `ADBE Glo2` — threshold, blur, composite.
4. `ADBE Minimax` — AE enum/channel/direction/radius edge surface with remaining GPU/CPU and deep alpha probes.
5. `ADBE Posterize Time` — temporal frame quantization above canvas effects.
6. `ADBE Geometry2` — transform-like adjustment effect.
7. `ADBE Turbulent Displace` — deterministic sine/noise displacement approximation with AE control slots and AE-wrapper telemetry for the recovered two-kernel contract.

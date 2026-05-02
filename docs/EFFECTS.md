# Effects Module Roadmap

Initial first-party AE matchName support:

```text
ADBE Drop Shadow       approximate
ADBE Glo2              approximate
ADBE Box Blur2         approximate
ADBE Turbulent Displace approximate
ADBE Posterize Time     recognized no-op at canvas stage
ADBE Geometry2          approximate
ADBE Minimax            approximate
```

## Parameter contract

Effect params accept both generated payload values (`{ "0001": { "value": ... } }`) and direct JSON values (`{ "0001": ... }`). Named aliases are accepted where practical, but AE-style numbered params are the compatibility baseline.

| matchName | Typed params | Numbered params |
| --- | --- | --- |
| `ADBE Box Blur2` | `radius`, `iterations` | `0001` radius, `0002` iterations; `0002` is also accepted as legacy radius fallback |
| `ADBE Drop Shadow` | `color`, `opacity`, `direction_degrees`, `distance`, `softness`, `shadow_only` | `0001` color, `0002` opacity, `0003` direction, `0004` distance, `0005` softness, `0006` shadow only |
| `ADBE Glo2` | `threshold`, `radius`, `intensity` | `0002` threshold, `0003` radius, `0004` intensity |
| `ADBE Geometry2` | `anchor`, `position`, `scale`, `rotation` | `0001` anchor, `0002` position, `0003` uniform scale, `0004` scale width, `0008` scale height |
| `ADBE Minimax` | `operation`, `radius`, `channels` | `0001` operation, `0002` radius, `0003` channels |
| `ADBE Turbulent Displace` | `amount`, `size`, `complexity`, `evolution` | `0002` amount, `0003` size, `0005` complexity, `0006` evolution |
| `ADBE Posterize Time` | `frame_rate` | `0001` frame rate; recognized no-op at the stateless canvas stage |

Scalar numbered params support direct numbers, wrapped `value`, and the existing simple scalar keyframe shape where the renderer already evaluates it. Color params accept normalized `0..1` channels or byte `0..255` channels.

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
4. `ADBE Minimax` — alpha/RGBA dilate/erode approximation.
5. `ADBE Posterize Time` — recognized; true frame quantization belongs above canvas effects.
6. `ADBE Geometry2` — transform-like adjustment effect.
7. `ADBE Turbulent Displace` — deterministic sine/noise displacement approximation.

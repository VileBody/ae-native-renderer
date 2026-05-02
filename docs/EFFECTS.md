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

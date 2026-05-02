# Effects Module Roadmap

Initial first-party AE matchName support:

```text
ADBE Drop Shadow       approximate
ADBE Glo2              approximate
ADBE Box Blur2         approximate
ADBE Turbulent Displace
ADBE Posterize Time
ADBE Geometry2
ADBE Minimax
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
4. `ADBE Minimax` — dilate/erode/open/close.
5. `ADBE Posterize Time` — quantized source sampling time.
6. `ADBE Geometry2` — transform-like adjustment effect.
7. `ADBE Turbulent Displace` — noise/displacement approximation, calibrated later.

# Effects Module Roadmap

Initial first-party AE matchName stubs:

```text
ADBE Drop Shadow
ADBE Glo2
ADBE Turbulent Displace
ADBE Posterize Time
ADBE Geometry2
ADBE Minimax
ADBE Box Blur2
```

## Registry rules

- unknown effect fails in strict mode;
- known stub logs unsupported/placeholder;
- real implementation must have a micro-scene fixture;
- every effect must expose parameter parsing separately from pixel math.

## Suggested implementation order

1. `ADBE Box Blur2` — separable blur.
2. `ADBE Drop Shadow` — alpha copy, color, blur, offset, under composite.
3. `ADBE Glo2` — threshold, blur, composite.
4. `ADBE Minimax` — dilate/erode/open/close.
5. `ADBE Posterize Time` — quantized source sampling time.
6. `ADBE Geometry2` — transform-like adjustment effect.
7. `ADBE Turbulent Displace` — noise/displacement approximation, calibrated later.

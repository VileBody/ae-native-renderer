# Motion Blur Notes

Native-style layer motion blur is modeled as temporal supersampling.

```text
for each layer:
  if layer.motion_blur:
    render layer at subframe times inside shutter interval
    average layer contribution
  else:
    render once at frame time
```

Implemented v1:

- composition settings: `motion_blur.enabled`, `samples`, `shutter_angle`,
  `shutter_phase`;
- per-layer `transform.motion_blur` switch;
- payload `layer_meta.motionBlur` import;
- default 17-sample ladder for imported AE Transform/Geometry2-style motion
  blur, matching the observed `Transform.aex` caller constant `0x11`;
- bounded midpoint temporal samples across the shutter interval when explicit
  sample placement is not otherwise known;
- subframe transform/keyframe/expression evaluation through the existing layer
  render path;
- no optical-flow blur.

Current parity notes:

- accumulation is still straight RGBA8 and needs an AE premultiplied-alpha audit;
- static layers are not yet skipped from temporal sampling;
- exact AE endpoint placement, shutter-weight curve, and Geometry2 effect
  shutter override behavior still need probes before formula tuning;
- AE reference PNGs for the conformance fixture still need to be exported.

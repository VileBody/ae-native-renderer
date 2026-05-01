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

Initial implementation:

- fixed samples per frame;
- shutter angle;
- shutter phase;
- per-layer switch;
- no optical-flow blur.

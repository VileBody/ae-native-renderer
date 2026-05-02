# IR Schema v0

The first IR is deliberately small. It represents a composition, assets, and a layer stack.

```json
{
  "version": "0.1",
  "composition": {
    "id": "main",
    "width": 1080,
    "height": 1920,
    "fps": 30,
    "duration": 6.0,
    "background": [0, 0, 0, 0]
  },
  "assets": [
    { "id": "video_1", "type": "video", "path": "assets/video_1.mp4" },
    { "id": "font_1", "type": "font", "path": "assets/fonts/Montserrat-BoldItalic.ttf" }
  ],
  "layers": [
    {
      "id": "clip_1",
      "type": "footage",
      "start": 0,
      "duration": 2.5,
      "source": "video_1",
      "source_start": 4.2
    },
    {
      "id": "title",
      "type": "text",
      "start": 0,
      "duration": 6,
      "text": "HELLO",
      "font": "font_1",
      "fontSize": 84,
      "fill": [255, 255, 255, 255],
      "transform": {
        "anchor": [0, 0],
        "position": [100, 200],
        "scale": [100, 100],
        "rotation": 0,
        "opacity": 100,
        "animation": {
          "position": [{ "time": 0.0, "value": [100, 200] }],
          "scale": [{ "time": 0.0, "value": [100, 100] }],
          "opacity": [{ "time": 0.0, "value": 100 }],
          "reveal": [{ "time": 0.0, "value": 100 }]
        }
      }
    }
  ]
}
```

## Layer types planned

- `solid`
- `footage`
- `text`
- `precomp`
- `adjustment`

For `footage` layers, `source_start` is optional and defaults to `0`. It is the
source-media time sampled at the layer's composition `start`.

For `text` layers, `box` is interpreted in layer-local coordinates before
`transform` is applied. Imported subtitle payloads use this to center a full-width
text box around the AE layer position.

## Transform convention

Use column-vector math:

```text
M = T(position) · R(rotation) · S(scale) · T(-anchor)
```

All deviations must be validated against golden frames.

`transform.animation` is optional. v0 supports hold and linear keyframes for
`position`, `scale`, `opacity`, and a simple text `reveal` percent. Imported AE
Bezier/ease keyframes are evaluated linearly and marked as approximate.

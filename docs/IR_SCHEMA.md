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
        "opacity": 100
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

## Transform convention

Use column-vector math:

```text
M = T(position) · R(rotation) · S(scale) · T(-anchor)
```

All deviations must be validated against golden frames.

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
  "compositions": [
    {
      "composition": {
        "id": "nested",
        "width": 1080,
        "height": 1920,
        "fps": 30,
        "duration": 6.0,
        "background": [0, 0, 0, 0]
      },
      "layers": []
    }
  ],
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
          "reveal": [{ "time": 0.0, "value": 100 }],
          "expression": {
            "position": {
              "type": "edge_wobble",
              "intro": 0.63,
              "outro": 0.63,
              "amp": 22.0,
              "freq": 3.6
            }
          }
        }
      },
      "text_animators": [
        {
          "name": "Animator 1",
          "opacity": 0,
          "position": [0, 25],
          "scale": [50, 50],
          "rotation": 15,
          "selector": {
            "start": 0,
            "end": 100,
            "based_on": "words",
            "start_keyframes": [{ "time": 0.0, "value": 0 }]
          },
          "expression_selector": {
            "type": "per_character_bounce",
            "delay": 0.05,
            "freq": 2,
            "amplitude": 100,
            "decay": 8
          }
        }
      ]
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

`composition + layers` is the root comp. Optional `compositions` entries define
nested comps addressable by `precomp.composition`. Graph validation rejects
missing targets and cycles. v0 renders normal nested comps into an offscreen
canvas, then applies the precomp layer transform. When
`collapse_transformations` is requested and the target tree is text/solid-only,
the renderer flattens child layers into the parent comp and composes the parent
matrix through the precomp boundary.

### Collapse transformations support status

`precomp.collapse_transformations` has a controlled native render path for
`CollapseSupportedVectors`: solid/text layers and nested precomps that also
request collapse are rendered with the precomp parent matrix composed into the
child layer matrix. Footage, adjustment layers, effects, missing targets, cycles,
and nested non-collapsed precomps keep the boundary in `RasterizeFirst`.
`RasterizeFirst` is still allowed in permissive mode and is reported as
approximate in the manifest.

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

`transform.animation` is optional. v0 supports hold, linear, and cubic
Bezier-eased keyframes for `position`, `scale`, `opacity`, and a simple text
`reveal` percent. Imported AE Bezier/ease keyframes are still marked approximate
because AE temporal-ease tangent data is mapped into a compact cubic curve.

`text_animators` is optional on text layers. v0/v2 supports one or more
approximate Range Selectors with percent Start/End, Based On
`characters`/`words`/`lines`, selector shape/smoothness/randomized order/wiggly
metadata, animator opacity, blur, and per-character/group position, scale, and
rotation by transforming alpha-derived units. The supported expression-selector
fingerprint is the generated per-character bounce pattern used by `impulse_2nd`.

`transform.animation.expression.position` currently supports the generated
`edge_wobble` fingerprint used by `scenes_3rd`; the expression engine also
supports a deterministic subset for `value`, numeric/Vec2 literals, AE-like
context variables, `time * N`, `value + [x,y]`, and the generated text bounce
selector fingerprint. It is not a general JavaScript expression runtime.

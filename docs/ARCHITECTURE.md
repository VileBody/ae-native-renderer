# Architecture

## Boundaries

The renderer is split into portable core crates and backend crates.

```text
render-cli
  -> render-ir
  -> render-core
      -> transform-math
      -> raster-cpu
      -> text-engine
      -> effects
  -> media-gst
```

`render-core` must not depend on `media-gst`. This keeps the renderer portable for future Swift/Metal/AVFoundation backends.

## Layer order

IR layers are stored top-to-bottom, like AE's layer list.

Rendering occurs bottom-to-top:

```text
for layer in layers.reverse():
  render layer
  composite over canvas
```

## Pixel format v0

Internal v0 can start with RGBA8 for simplicity, but the target model is:

```text
RGBA f32 premultiplied alpha
```

This is necessary for correct blur/glow/compositing and future motion blur accumulation.

## Effects

Effects are modules registered by AE matchName.

```text
ADBE Drop Shadow        -> effects::drop_shadow
ADBE Glo2               -> effects::glow
ADBE Box Blur2          -> effects::box_blur
ADBE Minimax            -> effects::minimax
ADBE Posterize Time     -> effects::posterize_time
ADBE Geometry2          -> effects::geometry
ADBE Turbulent Displace -> effects::turbulent_displace
```

Unknown effects must fail in strict mode.

## Text

The text engine must expose glyph instances, not just painted text. Text Animators need per-glyph/per-word/per-line weights.

## Motion blur

Native layer motion blur is temporal supersampling:

```text
for each sample in shutter interval:
  evaluate layer at subframe time
  accumulate
average samples
```

It is not a post-process blur.

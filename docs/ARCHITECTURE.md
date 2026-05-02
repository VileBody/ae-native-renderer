# Architecture

## Core rule

Codecs and containers are platform-specific media I/O. Render math is portable.

The IR and render core must not know whether a frame came from GStreamer,
FFmpeg/libav, AVFoundation, VideoToolbox, MediaCodec, or a test mock. They should
only see raw frames, timestamps, scene data, and renderer settings.

```text
Media I/O adapters:
  server Linux:   GStreamer-first, FFmpeg/libav allowed as fallback/tooling
  iOS/macOS:      AVFoundation + VideoToolbox
  Android later:  MediaCodec / ExoPlayer / optional FFmpeg fallback

Render math core:
  Rust portable core
  scene graph
  transforms
  keyframes/easing
  effects
  text animator logic
  expression subset
  motion blur sampling
  compositing
```

## High-level topology

```text
payload.json / scene.json / job folder / tar
                 |
                 v
         render-cli / job runner
                 |
                 v
             render-ir
                 |
                 v
        +---------------------+
        | render-core (Rust)  |
        |---------------------|
        | scene graph         |
        | transforms          |
        | keyframes/easing    |
        | text animators      |
        | expression subset   |
        | effects             |
        | motion blur         |
        | compositing         |
        +---------------------+
           ^               |
           | raw frames    | rendered RGBA frames
           |               v
      VideoSource       VideoSink
           ^               |
           |               v
 +----------------+   +----------------+
 | media adapters |   | media adapters |
 +----------------+   +----------------+
```

The important boundary is `VideoSource`/`VideoSink`: media adapters own decode,
encode, containers, timestamps, caps negotiation, and platform quirks. The core
owns scene evaluation and pixels.

```rust
trait VideoSource {
    fn prepare(&mut self, plan: &SourcePlan);
    fn frame_at(&mut self, t: Time) -> Frame;
}

trait VideoSink {
    fn write_frame(&mut self, frame: &Frame, t: Time);
}
```

Expected implementations:

```text
GStreamerVideoSource
GStreamerVideoSink
FfmpegVideoSource
FfmpegVideoSink
AvFoundationVideoSource
AvFoundationVideoSink
MockVideoSource
PngSequenceSink
```

## Crate boundaries

The renderer is split into portable core crates and backend/adapter crates.

```text
render-cli
  -> render-ir
  -> render-core
      -> transform-math
      -> raster-cpu
      -> text-engine
      -> effects
  -> media-gst
  -> ae-bridge
  -> testkit
```

Dependency rules:

```text
render-core must not depend on media-gst, FFmpeg, GStreamer, AVFoundation, or OS media APIs.
render-ir must describe assets and layers, not decoded media state.
media adapters may depend on platform media stacks and convert them into raw frames.
render-cli may depend on everything because it is orchestration, not core math.
testkit may use mocks, PNGs, MP4 references, and diff tooling.
```

This keeps the renderer portable for future Swift/Metal/AVFoundation and Android
backends.

## Server media pipeline

First production server target:

```text
job runner
  -> render-cli
  -> asset resolver / probe
  -> GStreamer decode graph
       mp4/mp3 -> decodebin -> videoconvert/audioconvert -> appsink
  -> Rust render core
       raw frames + text + scene graph -> rendered RGBA frames
  -> initial output
       PNG sequence + manifest/logs
       ffmpeg CLI allowed for bootstrap MP4 mux/debug
  -> later production output
       GStreamer appsrc -> encoder -> muxer
```

GStreamer is a production media pipe, not the renderer engine. It should handle
decode graphs, caps negotiation, appsink/appsrc, stream timestamps, plugins, and
pipeline lifecycle. It should not own transforms, text animators, effects,
precomp evaluation, or compositing decisions.

FFmpeg remains useful as tooling and fallback:

```text
ffprobe / ffmpeg CLI:
  diagnostics
  compatibility checks
  bootstrap frame decode/mux
  ops/debug escape hatch

libavcodec / libavformat:
  possible fallback backend
  not the architectural center
```

## Apple/mobile pipeline

Target iOS/macOS shape:

```text
Swift app
  -> UI
  -> permissions
  -> local files
  -> progress/export/share

AVFoundation / VideoToolbox
  -> decode input assets
  -> encode final video

Rust core through FFI
  -> scene graph/math/render decisions
  -> CPU RGBA renderer first

Metal later
  -> accelerated compositing/effects when CPU becomes the bottleneck
```

The mobile port should replace media adapters, not rewrite the renderer.

## Current implementation status

`crates/media-gst` is the media-adapter crate. Today it is still a bootstrap
backend: it checks GStreamer availability for `doctor`, has a `VideoSource` trait,
`SourcePlan`/`TimelineMediaPlan` structs for scheduler input, and a `VideoSink`
contract for future encoders. It uses FFmpeg/ffprobe-backed probing plus a
persistent sequential FFmpeg pipe for active CLI frame extraction. The CLI writes
`media-plan.json` before rendering, uses it to open and decoder-start early
sources, keeps a small per-source frame cache, and maintains a bounded pool of
open decoders so it no longer spawns one FFmpeg process per requested frame.
Media reports include request/cache/decode/spawn/read timings and runtime tuning
knobs for cache size, decoder pool size, sequential decode gap, and prewarm
window. The target is still to replace the hot path with GStreamer Rust bindings
and appsink/appsrc pipelines.

`render-core` currently renders through a `FootageProvider` boundary and writes
PNG sequences plus `manifest.json` and `render-log.jsonl`. The manifest includes
aggregate layer/effect timing profiles, and the frame log includes per-frame
profile detail. The MP4 mux helper is still FFmpeg CLI based; treat it as startup
tooling that should move behind a `VideoSink`/media-backend boundary before
production.

The desired production formula:

```text
FFmpeg is not the center of the architecture.
GStreamer is the server media pipe.
AVFoundation/VideoToolbox is the Apple media pipe.
Rust is the renderer brain.
CPU/Metal are renderer backends.
```

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

## Render sequence reports

`render-core::render_sequence` writes a PNG sequence plus `manifest.json` and `render-log.jsonl`. The manifest includes renderer crate/version metadata, deterministic scene and asset-spec hashes, per-frame timing (`render_ms`, `save_ms`, `total_ms`), total render time, and basic layer/effect feature counts. These fields are observational only and must not affect pixel rendering.

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

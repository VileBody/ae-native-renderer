# Module Plan

## Built-in effects to implement first

- `ADBE Box Blur2`
- `ADBE Drop Shadow`
- `ADBE Glo2`
- `ADBE Minimax`
- `ADBE Posterize Time`
- `ADBE Geometry2`
- `ADBE Turbulent Displace`

## AE-native subsystems to model

- Transform Group
- Opacity
- TextDocument subset
- Text Animator subset
- Range Selector
- Expression Selector
- Precomp
- Adjustment layer
- Motion blur

## Expression subsystem

- `expression-engine` crate
- QuickJS/Boa backend later
- AE-like variables only for controlled subset

## Server Media Adapters

- GStreamer-first decode/probe through appsink pipelines
- GStreamer-first MP4 encode/mux through appsrc pipelines
- FFmpeg/libav fallback and CLI debug/mux tooling

## Future mobile adapters

- AVFoundation media source
- VideoToolbox encoder/decoder
- CoreText/HarfBuzz text backend
- Metal raster backend

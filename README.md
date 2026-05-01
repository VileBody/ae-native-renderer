# AE Native Renderer

Docker-first Rust skeleton for an AE-subset native renderer.

The current repository is intentionally a **boilerplate + roadmap** project. It is structured so that the first milestone can render a simple layer stack from `scene.json`, then grow into:

- footage layers;
- text layers;
- transforms/keyframes;
- effects as modules;
- precomps;
- text animators/selectors;
- expressions;
- motion blur;
- worker mode for Kubernetes.

## First mental model

```text
GStreamer / FFmpeg / AVFoundation = media I/O adapters
Rust render_core                   = AE-like scene evaluation
raster_cpu / future GPU backend     = pixels
scene.json                          = portable render IR
```

GStreamer is **not** the renderer graph. It is a codec/media adapter.

## Quick start

```bash
docker build -t ae-native-renderer:dev .

docker run --rm \
  -v "$PWD/jobs:/work/jobs" \
  -v "$PWD/fixtures:/work/fixtures" \
  ae-native-renderer:dev \
  doctor
```

Render the demo job:

```bash
docker run --rm \
  -v "$PWD/jobs:/work/jobs" \
  -v "$PWD/fixtures:/work/fixtures" \
  ae-native-renderer:dev \
  render --scene /work/jobs/demo_static/scene.json --out /work/jobs/demo_static/out
```

Resolve/probe assets from an extracted job folder or tar archive:

```bash
docker run --rm \
  -v "$PWD:/work" \
  ae-native-renderer:dev \
  resolve-assets \
  --scene /work/jobs/example/scene.json \
  --job-archive /work/jobs/example_job_folder.tar.gz \
  --strict
```

Render a scene with footage:

```bash
docker run --rm \
  -v "$PWD:/work" \
  ae-native-renderer:dev \
  render \
  --scene /work/jobs/example/scene.json \
  --out /work/jobs/example/out \
  --job-archive /work/jobs/example_job_folder.tar.gz
```

Mux PNG sequence to mp4:

```bash
./scripts/mux_png_to_mp4.sh jobs/demo_static/out/frames 30 jobs/demo_static/out/result.mp4
```

## Repository layout

```text
crates/render-cli      CLI entry point: doctor/probe/render/job/validate
crates/render-ir       scene.json schema and parsing
crates/render-core     composition/layer graph and frame evaluation
crates/raster-cpu      CPU canvas, compositing, samplers
crates/transform-math  matrices, transform conventions, interpolation helpers
crates/media-gst       GStreamer adapter skeleton
crates/text-engine     text layout/glyph skeleton
crates/effects         effect registry and effect modules
crates/ae-bridge       AE matchName mapping and future JSX/AEPX bridge
crates/testkit         golden frame tests and image diff helpers
```

## Current status

This is an early native-renderer slice with a working Docker build, payload import,
asset resolution/probing, FFmpeg frame decode, and PNG sequence rendering for static
footage timelines.
The active implementation plan is `ROADMAP_V2.md`: consume generated payload JSON
directly, translate it into render IR, then build native footage + text rendering
before adding transforms, keyframes, effects, text animators, and expressions.

`ROADMAP.md` remains as the broader original AE-subset contract.

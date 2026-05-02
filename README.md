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

Render and mux to MP4 in one pass:

```bash
docker run --rm \
  -v "$PWD:/work" \
  ae-native-renderer:dev \
  render \
  --scene /work/fixtures/animation_effects_scene.json \
  --out /work/target/smoke/animation_effects_render \
  --mp4 /work/target/smoke/animation_effects.mp4
```

Mux an existing PNG sequence to MP4:

```bash
docker run --rm \
  -v "$PWD:/work" \
  ae-native-renderer:dev \
  mux --frames /work/jobs/demo_static/out --out /work/jobs/demo_static/out/result.mp4
```

Compare native PNG frames against an AE fallback MP4:

```bash
docker run --rm \
  -v "$PWD:/work" \
  ae-native-renderer:dev \
  compare \
  --native /work/jobs/example/out/native \
  --reference /work/jobs/example/work/output.mp4 \
  --out /work/jobs/example/out/conformance \
  --threshold-mean 12 \
  --threshold-max 255
```

Run a production-style job. The runner writes `out/capabilities.json`,
`out/job-log.jsonl`, `out/job-report.json`, native frames, optional MP4, and
optional conformance reports when `work/output.mp4` is present.

```bash
docker run --rm \
  -v "$PWD:/work" \
  ae-native-renderer:dev \
  job --job-dir /work/jobs/example --payload /work/jobs/example/payload.json
```

The external helper script is still available:

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
asset resolution/probing, FFmpeg frame decode, static footage timelines, readable
text rendering, basic 2D layer transforms, PNG manifests/logs, MP4 muxing,
hold/linear transform keyframes, approximate Drop Shadow/Glow/Box Blur,
text Range Selector opacity reveals, the generated `edge_wobble` position
expression, adjustment layers, and approximate Geometry2/Minimax/Turbulent
Displace effects. Posterize Time is recognized as a canvas-stage no-op. The CLI
also has an AE conformance compare command and a production-style job runner with
native/fallback routing reports.
The active implementation plan is `ROADMAP_V2.md`: consume generated payload JSON
directly, translate it into render IR, then build native footage, text, transforms,
keyframes, effects, text animators, and expressions incrementally.

`ROADMAP.md` remains as the broader original AE-subset contract.

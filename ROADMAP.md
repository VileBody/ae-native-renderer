# Roadmap — AE Native Renderer

This roadmap is intentionally written as an implementation contract. Each milestone has a scope, checklist, acceptance criteria, and non-goals.

## North Star

Build a Docker-first Rust renderer that consumes a controlled AE-like IR and renders PNG sequences natively on Linux. The renderer starts with a minimal layer graph and evolves into a modular AE-subset engine.

The long-term architecture must allow future mobile/on-device rendering. Therefore, the core renderer must not depend on GStreamer or server-only APIs.

```text
scene.json / future JSX bridge / future editor
      -> render-ir
      -> render-core timeline + graph evaluator
      -> raster backend
      -> PNG sequence / future video writer
```

GStreamer is a media adapter, not the render engine.

---

## R0 — Repository skeleton and Docker-first toolchain

### Goal
Create a reproducible repository structure with a Docker build, Rust workspace, crates, documentation, and a CLI skeleton.

### Checklist
- [ ] Root `Cargo.toml` workspace exists.
- [ ] Crates exist:
  - [ ] `render-cli`
  - [ ] `render-ir`
  - [ ] `render-core`
  - [ ] `raster-cpu`
  - [ ] `transform-math`
  - [ ] `media-gst`
  - [ ] `text-engine`
  - [ ] `effects`
  - [ ] `ae-bridge`
  - [ ] `testkit`
- [ ] Dockerfile has build and runtime stages.
- [ ] `docker-compose.yml` can run the demo container.
- [ ] `README.md` explains quick start.
- [ ] `ROADMAP.md` exists.
- [ ] `.gitignore` is configured.

### Acceptance criteria
- [ ] `docker build -t ae-native-renderer:dev .` completes.
- [ ] `docker run --rm ae-native-renderer:dev doctor` starts the CLI.
- [ ] Repository can be opened by a developer without guessing the structure.

### Non-goals
- Full media decode.
- Real text rendering.
- AE compatibility.
- Effects.

---

## R1 — Doctor command and environment validation

### Goal
Validate that the container has the minimum runtime stack: GStreamer, FFmpeg, fontconfig, directories, and renderer version.

### Checklist
- [ ] `render-cli doctor` command exists.
- [ ] Checks renderer version.
- [ ] Checks GStreamer init.
- [ ] Checks GStreamer plugin names:
  - [ ] `decodebin`
  - [ ] `appsink`
  - [ ] `appsrc`
  - [ ] `videoconvert`
- [ ] Checks `ffmpeg -version`.
- [ ] Checks `/work/jobs` path if present.
- [ ] Emits JSON-ish or machine-readable logs.

### Acceptance criteria
- [ ] `doctor` exits with `0` when required tools exist.
- [ ] `doctor` exits non-zero if a required binary/plugin is missing in strict mode.
- [ ] Output is useful enough for CI logs.

### Non-goals
- Decode a real video.
- Render a frame.

---

## R2 — GStreamer media probe

### Goal
Read media metadata from MP4/MP3 files through the media adapter layer.

### Checklist
- [ ] `media-gst` exposes `probe(path)`.
- [ ] Returns width/height/fps/duration for video when available.
- [ ] Returns sample rate/channels/duration for audio when available.
- [ ] `render-cli probe <path>` prints metadata.
- [ ] Errors are explicit: missing file, unsupported media, decode failure.

### Acceptance criteria
- [ ] `probe demo.mp4` prints video metadata.
- [ ] `probe demo.mp3` prints audio metadata.
- [ ] No render-core dependency on GStreamer.

### Non-goals
- Frame-accurate seeking.
- Decoding frames.
- Audio mixing.

---

## R3 — Decode video frames to PNG dump

### Goal
Prove codec I/O by decoding a video and dumping frames to PNG without the scene graph.

### Checklist
- [ ] `VideoSource` trait exists.
- [ ] `GstVideoSource` implements nearest-frame decode.
- [ ] `Frame` struct uses RGBA8 or BGRA8 with explicit format.
- [ ] `render-cli dump-frames --input ... --out ... --count N` exists.
- [ ] PNG writer works.
- [ ] Frame timestamps are logged.

### Acceptance criteria
- [ ] First 10 frames from demo MP4 are saved as PNG.
- [ ] Output frame dimensions match media dimensions.
- [ ] No render graph is required to run this.

### Non-goals
- Perfect seeking.
- Variable frame rate correctness.
- Audio.

---

## R4 — Render IR v0: scene.json

### Goal
Create a portable scene schema for composition, assets, layers, transforms, and output.

### Checklist
- [ ] `render-ir` parses `scene.json`.
- [ ] Types:
  - [ ] `Scene`
  - [ ] `Composition`
  - [ ] `Asset`
  - [ ] `Layer`
  - [ ] `LayerKind`
  - [ ] `Transform2D`
  - [ ] `EffectSpec`
- [ ] Asset paths resolve relative to job directory.
- [ ] Schema examples exist in `docs/IR_SCHEMA.md`.
- [ ] `render-cli validate --scene ...` exists.

### Acceptance criteria
- [ ] Demo `scene.json` validates.
- [ ] Invalid layer type gives a helpful error.
- [ ] Missing asset gives a helpful error.

### Non-goals
- AEP/AEPX parser.
- JSX parser.
- Full AE property coverage.

---

## R5 — Render graph v0: static layer stack

### Goal
Evaluate a composition as a bottom-to-top layer stack.

### Checklist
- [ ] `CompositionGraph` model exists.
- [ ] `RenderContext` exists.
- [ ] `render_frame(scene, time)` exists.
- [ ] Layer activity by `start` and `duration` works.
- [ ] Layer order convention is documented:
  - [ ] IR order: top-to-bottom, like AE UI.
  - [ ] Render order: bottom-to-top.
- [ ] `Canvas` uses internal RGBA float premultiplied or a documented temporary format.

### Acceptance criteria
- [ ] Static solid layers composite in expected order.
- [ ] Inactive layers are skipped.
- [ ] Rendered frame is deterministic across repeated runs.

### Non-goals
- Video layers.
- Text layout.
- Transforms beyond raw placement.

---

## R6 — CPU raster backend v0

### Goal
Create the first pixel backend: canvas allocation, clear, blit, alpha compositing, and PNG output.

### Checklist
- [ ] `Canvas` can allocate transparent image.
- [ ] `composite_normal` exists.
- [ ] Premultiplied vs straight alpha convention is documented.
- [ ] Conversion to PNG RGBA8 works.
- [ ] Basic tests for alpha-over exist.

### Acceptance criteria
- [ ] Red 50% over blue gives expected alpha-over result.
- [ ] Transparent background remains transparent.
- [ ] PNG output matches canvas size.

### Non-goals
- SIMD optimization.
- Color management.
- Linear/sRGB correctness beyond documented v0 assumption.

---

## R7 — Footage layers v0

### Goal
Render decoded media frames as footage layers in the scene graph.

### Checklist
- [ ] `FootageLayer` resolves a video asset.
- [ ] `VideoSource.frame_at(time)` is called through an abstraction.
- [ ] Footage layer can be composited onto canvas.
- [ ] Source in/out offsets are represented.
- [ ] Missing media produces a hard error in strict mode.

### Acceptance criteria
- [ ] Demo scene renders one MP4 as background PNG sequence.
- [ ] A second footage layer can be placed above the first.
- [ ] Frame timestamps map to composition time consistently.

### Non-goals
- Time remapping.
- Frame blending.
- Audio.

---

## R8 — Text layer v0

### Goal
Render basic text layers while preserving enough structure for future per-glyph animation.

### Checklist
- [ ] `text-engine` loads font files.
- [ ] Text layout returns glyph instances, not just a painted bitmap.
- [ ] Glyph instance contains:
  - [ ] glyph id
  - [ ] char index
  - [ ] word index
  - [ ] line index
  - [ ] position
  - [ ] bbox
  - [ ] advance
- [ ] Text layer supports:
  - [ ] font file path
  - [ ] font size
  - [ ] fill color
  - [ ] box text rectangle
  - [ ] alignment v0
- [ ] Debug dump for glyph layout exists.

### Acceptance criteria
- [ ] Text appears on rendered PNGs.
- [ ] Multi-word text produces word indices.
- [ ] Multi-line box text produces line indices.
- [ ] Glyph debug JSON can be inspected.

### Non-goals
- AE-perfect typography.
- Ligatures/RTL/CJK/fallback fonts.
- Text animators.
- Strokes.

---

## R9 — PNG sequence renderer

### Goal
Render a full composition duration into numbered PNG frames.

### Checklist
- [ ] `render-cli render --scene ... --out ...` exists.
- [ ] Creates `out/frames/frame_%06d.png`.
- [ ] Creates `out/manifest.json`.
- [ ] Creates `out/render-log.jsonl`.
- [ ] Frame count computed from fps and duration.
- [ ] Existing output directory behavior is explicit.

### Acceptance criteria
- [ ] Demo job renders a PNG sequence.
- [ ] `ffmpeg` can mux the sequence to MP4.
- [ ] Manifest contains fps, width, height, duration, frame count.

### Non-goals
- Built-in video muxing.
- Audio mux.
- Distributed rendering.

---

## R10 — Static transforms

### Goal
Add 2D transforms for layers: anchor, position, scale, rotation, opacity.

### Checklist
- [ ] `transform-math` defines Vec2/Mat3/Transform2D.
- [ ] Transform convention is documented.
- [ ] Layer sampling supports nearest-neighbor.
- [ ] Bilinear sampler stub exists.
- [ ] Opacity multiplies source alpha correctly.
- [ ] Golden tests for anchor/position/scale/rotation.

### Acceptance criteria
- [ ] Footage layer can move, scale, rotate.
- [ ] Text layer can move, scale, rotate.
- [ ] Anchor point changes rotation center.
- [ ] Opacity visibly affects layer.

### Non-goals
- 3D transforms.
- Camera.
- Motion blur.
- AE-perfect sampling.

---

## R11 — Keyframes and interpolation v0

### Goal
Represent and evaluate animated properties.

### Checklist
- [ ] `Animated<T>` supports static and keyframed values.
- [ ] Interpolations:
  - [ ] hold
  - [ ] linear
- [ ] Types supported:
  - [ ] f32
  - [ ] Vec2
  - [ ] Vec3
  - [ ] color
- [ ] Evaluator handles time before first key and after last key.
- [ ] Keyframe schema in IR.

### Acceptance criteria
- [ ] Position keyframes move a layer over time.
- [ ] Opacity keyframes fade a layer.
- [ ] Render is deterministic.

### Non-goals
- AE temporal/spatial Bezier.
- Roving keyframes.
- Expressions.

---

## R12 — Bezier/ease interpolation v1

### Goal
Approximate AE-like ease behavior for controlled templates.

### Checklist
- [ ] Temporal ease schema is added.
- [ ] Cubic Bezier evaluator exists.
- [ ] Clamp and monotonic solving are handled.
- [ ] Golden tests for common ease-in/ease-out.

### Acceptance criteria
- [ ] Ease-in/ease-out visibly matches reference micro-scenes within threshold.
- [ ] Linear behavior remains unchanged.

### Non-goals
- Full AE graph editor fidelity.
- Spatial paths.

---

## R13 — Effect module system

### Goal
Make effects pluggable and identifiable by AE matchName.

### Checklist
- [ ] `Effect` trait exists.
- [ ] `EffectFactory` exists.
- [ ] `EffectRegistry` maps matchName to module.
- [ ] Known effects registered:
  - [ ] `ADBE Drop Shadow`
  - [ ] `ADBE Glo2`
  - [ ] `ADBE Turbulent Displace`
  - [ ] `ADBE Posterize Time`
  - [ ] `ADBE Geometry2`
  - [ ] `ADBE Minimax`
  - [ ] `ADBE Box Blur2`
- [ ] `strict-effects` mode exists.

### Acceptance criteria
- [ ] Unknown effect fails in strict mode.
- [ ] Unknown effect warns in permissive mode.
- [ ] Known stub effects can be attached without breaking scene parsing.

### Non-goals
- Real effect math.
- Effect parameter parity.

---

## R14 — Basic effects v1

### Goal
Implement first useful native effects.

### Implementation order
1. `ADBE Box Blur2`
2. `ADBE Drop Shadow`
3. `ADBE Glo2`
4. `ADBE Minimax`
5. `ADBE Posterize Time`
6. `ADBE Geometry2`
7. `ADBE Turbulent Displace`

### Checklist
- [ ] Each effect has a parameter struct.
- [ ] Each effect has unit tests.
- [ ] Each effect has a micro-scene.
- [ ] Each effect has golden PNG comparison.
- [ ] Approximation status is documented.

### Acceptance criteria
- [ ] Box blur works on alpha and color.
- [ ] Drop shadow creates alpha-derived blurred offset shadow.
- [ ] Glow extracts bright areas and composites result.
- [ ] Minimax supports at least dilate/erode.
- [ ] Posterize Time samples source at quantized time.
- [ ] Geometry2 can transform a layer/effect input.
- [ ] Turbulent Displace approximate output is stable.

### Non-goals
- Pixel-perfect AE match on first pass.
- GPU acceleration.

---

## R15 — Precomp graph v0

### Goal
Support nested compositions as precomp layers.

### Checklist
- [ ] Scene can contain multiple compositions.
- [ ] Layer can reference a nested composition.
- [ ] Precomp renders into offscreen canvas.
- [ ] Precomp frame caching exists.
- [ ] Recursive graph cycle detection exists.

### Acceptance criteria
- [ ] Nested comp renders under/over normal layers.
- [ ] Precomp transform applies after nested render.
- [ ] Cyclic precomp references fail clearly.

### Non-goals
- Collapse transformation.
- Time remapping.

---

## R16 — Collapse transformation v1

### Goal
Implement the first controlled version of AE-like collapse transformation.

### Checklist
- [ ] Collapse flag exists on precomp layer.
- [ ] Render graph can defer rasterization for supported vector/text children.
- [ ] Matrix propagation is implemented for simple child layers.
- [ ] Unsupported collapsed content falls back or fails explicitly.

### Acceptance criteria
- [ ] Simple text inside collapsed precomp remains sharp after parent scale.
- [ ] Non-collapsed precomp rasterizes before parent scale.
- [ ] Unsupported cases are detectable.

### Non-goals
- Full AE collapse behavior.
- 3D/camera collapse.

---

## R17 — Text Animator v1

### Goal
Implement the subset required by current templates: Range Selector, Percent Start/End, Opacity, Position 3D, Scale 3D, Rotation, Blur.

### Checklist
- [ ] Text animator IR exists.
- [ ] Selector weight per text object exists.
- [ ] BasedOn modes:
  - [ ] characters
  - [ ] words
  - [ ] lines
- [ ] Units:
  - [ ] percent
  - [ ] index
- [ ] Shape:
  - [ ] square
  - [ ] ramp up/down optional
- [ ] Smoothness support v0/v1.
- [ ] Animator properties:
  - [ ] opacity
  - [ ] position 3D projected to 2D
  - [ ] scale 3D projected to 2D
  - [ ] rotation Z
  - [ ] blur

### Acceptance criteria
- [ ] Character reveal works.
- [ ] Word reveal works.
- [ ] Opacity reveal works.
- [ ] Per-glyph position/scale/rotation works.
- [ ] Blur reveal works at least as per-glyph/group blur.

### Non-goals
- Full AE text animator universe.
- Wiggly selector.
- Randomize order.
- Fill/stroke animators.

---

## R18 — Expression subset

### Goal
Evaluate the small JavaScript-like expression subset used by templates.

### Checklist
- [ ] Expression engine is integrated behind a trait.
- [ ] Variables:
  - [ ] `time`
  - [ ] `inPoint`
  - [ ] `outPoint`
  - [ ] `textIndex`
  - [ ] `textTotal`
  - [ ] `value`
- [ ] Math functions:
  - [ ] `sin`
  - [ ] `cos`
  - [ ] `exp`
  - [ ] `min`
  - [ ] `max`
- [ ] Output coercion for scalar and vector values.
- [ ] Expression failure mode is explicit.

### Acceptance criteria
- [ ] Text Expression Selector can compute amount.
- [ ] Position expression can return `[x, y]`.
- [ ] Audio levels expression can return `[db, db]`.

### Non-goals
- Full ExtendScript.
- Access to arbitrary layer/property graph.

---

## R19 — Motion blur v1

### Goal
Add native layer motion blur via temporal supersampling.

### Checklist
- [ ] Composition motion blur settings exist:
  - [ ] enabled
  - [ ] samples
  - [ ] shutter angle
  - [ ] shutter phase
- [ ] Layer motion blur flag exists.
- [ ] Per-layer subframe rendering implemented.
- [ ] Static layers are not oversampled.
- [ ] Animated transforms evaluated at subframe times.

### Acceptance criteria
- [ ] Moving text layer blurs.
- [ ] Static layer stays sharp.
- [ ] `samples=1` equals no blur.
- [ ] Increasing samples smooths output.

### Non-goals
- Adaptive sample limit.
- Pixel motion blur / optical flow.
- Footage internal motion blur.

---

## R20 — Job runner and Kubernetes-ready container

### Goal
Turn renderer into a stateless job worker runnable as a container in orchestration.

### Checklist
- [ ] Job directory contract documented.
- [ ] `render-cli job --job-dir ...` exists.
- [ ] Writes logs to stdout and `render-log.jsonl`.
- [ ] Exit codes:
  - [ ] 0 success
  - [ ] 1 config/input error
  - [ ] 2 render error
  - [ ] 3 unsupported template
- [ ] Resource controls via env:
  - [ ] `RENDER_MAX_THREADS`
  - [ ] `RENDER_TMP_DIR`
  - [ ] `RENDER_STRICT`
- [ ] No hidden global mutable state.

### Acceptance criteria
- [ ] Container can process one job and exit.
- [ ] Container can be run repeatedly with no host pollution.
- [ ] Output is deterministic.
- [ ] Logs are machine-readable.

### Non-goals
- Queue integration.
- S3 upload/download.
- Autoscaling.

---

## R21 — Golden-frame conformance harness

### Goal
Compare native renderer output against AE golden references.

### Checklist
- [ ] `testkit` can load two PNGs.
- [ ] Metrics:
  - [ ] absolute pixel diff
  - [ ] alpha diff
  - [ ] bounding-box diff
  - [ ] optional SSIM placeholder
- [ ] Golden fixtures are organized by feature.
- [ ] CLI command `compare` exists.

### Acceptance criteria
- [ ] Micro-scenes can be compared to AE references.
- [ ] Diff threshold can be configured.
- [ ] Failing diff produces an artifact image.

### Non-goals
- Perfect AE parity by default.
- Full test coverage of all templates.

---

## R22 — Template acceptance gate

### Goal
Decide whether a template is supported by the native renderer.

### Checklist
- [ ] Template validator scans IR features.
- [ ] Supported/unsupported features are listed.
- [ ] Unsupported effects fail in strict mode.
- [ ] Unknown text animator modes fail in strict mode.
- [ ] Validator emits machine-readable report.

### Acceptance criteria
- [ ] Supported demo template passes.
- [ ] Template with unknown effect fails.
- [ ] Template with unsupported text feature fails.

### Non-goals
- Auto-conversion of unsupported features.

---

## R23 — Performance baseline

### Goal
Measure speed before optimizing.

### Checklist
- [ ] Per-frame timing exists.
- [ ] Per-layer timing exists.
- [ ] Per-effect timing exists.
- [ ] Peak memory logging exists.
- [ ] Benchmark scenes exist.

### Acceptance criteria
- [ ] Renderer produces timing report.
- [ ] Slowest layers/effects are visible.
- [ ] Baseline can be compared across commits.

### Non-goals
- Optimization.
- GPU.

---

## R24 — CPU optimization pass

### Goal
Improve throughput for server rendering.

### Checklist
- [ ] Parallel frame rendering optional.
- [ ] Parallel row/effect processing optional.
- [ ] Frame cache for footage/precomps.
- [ ] Reuse allocations.
- [ ] Avoid unnecessary format conversions.

### Acceptance criteria
- [ ] Demo benchmark improves without visual regression.
- [ ] Memory usage remains bounded.
- [ ] Deterministic output maintained.

### Non-goals
- GPU backend.
- Mobile port.

---

## R25 — Mobile-readiness split

### Goal
Prepare future on-device rendering without starting the mobile port yet.

### Checklist
- [ ] Core crates have no GStreamer dependency.
- [ ] Media backend trait is stable.
- [ ] Text backend trait is stable.
- [ ] Raster backend trait is stable.
- [ ] Scene IR is portable.
- [ ] Feature flags separate server-only components.

### Acceptance criteria
- [ ] `render-core` compiles without `media-gst`.
- [ ] `text-engine` backend can be swapped.
- [ ] `raster-cpu` can be replaced by future Metal backend.

### Non-goals
- Swift app.
- Metal renderer.
- AVFoundation integration.

---

# First implementation order

```text
R0  -> repo + Docker
R1  -> doctor
R2  -> probe
R3  -> dump frames
R4  -> scene.json
R5  -> graph
R6  -> canvas/composite
R7  -> footage layer
R8  -> text layer
R9  -> PNG sequence
R10 -> transforms
R11 -> keyframes
R13 -> effect registry
R14 -> first effects
R15 -> precomp
R17 -> text animators
R18 -> expressions
R19 -> motion blur
R20 -> worker mode
```

R12/R16/R21-R25 can be interleaved when the product needs them.

---

## RXXX — Production maturity / hybrid rendering platform

### Goal
Turn the native renderer from an internal R&D engine into a production routing platform that can choose the cheapest correct render path.

### Checklist
- [ ] Template capability scanner exists.
- [ ] Every template has a feature manifest.
- [ ] Every renderer feature has status: unsupported / stub / approximate / production.
- [ ] Golden-frame references exist for accepted templates.
- [ ] Renderer can route jobs by capability:
  - [ ] native cloud renderer
  - [ ] future on-device renderer
  - [ ] AE fallback if still needed
  - [ ] reject unsupported template early
- [ ] Cost model exists per template class.
- [ ] Logs include renderer version, scene hash, asset hashes, feature set, and output manifest.
- [ ] No silent degradation in strict mode.

### Acceptance criteria
- [ ] A production job can be accepted/rejected/routed before rendering starts.
- [ ] A visually unsupported feature never silently disappears.
- [ ] Same IR can be used by cloud and future device backends.
- [ ] Rendering economics are measurable per job class.

### Non-goals
- Infinite AE compatibility.
- Supporting arbitrary third-party plugins.
- One renderer backend for every platform.

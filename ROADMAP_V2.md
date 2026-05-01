# Roadmap V2 — JSON-first Native Renderer

This roadmap replaces the original milestone order for the current product reality.

The renderer does not need to interpret arbitrary JSX. The upstream pipeline already
generates structured JSON payloads and injects them into Jinja2 JSX templates for
After Effects. The native renderer should consume that generated JSON directly.

## North Star

Build a Docker-first Rust renderer that accepts a controlled generated JSON payload,
normalizes it into portable render IR, and renders vertical social-video templates
natively on Linux.

```text
generator payload JSON
      -> ae-bridge normalizer
      -> render-ir scene.json
      -> render-core timeline + graph evaluator
      -> media/text/effects/raster backends
      -> PNG sequence / MP4

legacy/debug path:
generator payload JSON -> Jinja2 JSX -> After Effects fallback
```

After Effects JSX is a compatibility/export path, not the renderer input contract.

## Product Scope

The first useful product is not full AE parity. It is:

- footage clips on a vertical timeline;
- static text layers above footage;
- basic placement, scale, opacity, and timing;
- deterministic native output;
- clear unsupported-feature reports.

Everything else comes after the native path can render real examples end-to-end.

## Current Example Payload Reality

Observed examples in local artifacts:

| Mode | Duration | Footage | Text | Key complexity |
| --- | ---: | ---: | ---: | --- |
| `template_4th` | ~13.87s | 13 footage layers | 10 text layers | Glow/Drop Shadow, reveal keyframes |
| `impulse_2nd` | 32.5s | 19 footage layers | 27 text layers | Drop Shadow, scale/opacity keyframes |
| `scenes_3rd` | ~21.8s | 9 footage layers | 17 text layers + adjustments | Geometry2, Turbulent Displace, Posterize Time, Minimax, expressions |

The payload shape is broadly:

```text
projectSpec
compsSpec
footage_layers
text_layers
```

This should become the formal input contract, with a version field added by the
generator as soon as possible.

## V2 Implementation Order

```text
V2-R0  -> repo + Docker baseline
V2-R1  -> generated payload JSON contract
V2-R2  -> payload validator and capability report
V2-R3  -> payload-to-render-ir translator
V2-R4  -> media probe and local asset resolver
V2-R5  -> video frame decode through media backend
V2-R6  -> static footage timeline render
V2-R7  -> static text render
V2-R8  -> static 2D transforms
V2-R9  -> PNG sequence + MP4 mux workflow
V2-R10 -> keyframes: hold/linear for opacity/scale/position/reveal
V2-R11 -> precomp flattening for text comps
V2-R12 -> first visual effects: Drop Shadow, Glow, Box Blur
V2-R13 -> text reveal and text animator subset
V2-R14 -> expression subset for known templates
V2-R15 -> adjustment layers and remaining effects
V2-R16 -> conformance harness against AE outputs
V2-R17 -> production job runner and routing
```

---

## V2-R0 — Baseline Repo and Container

### Goal
Keep the existing Rust workspace and Docker-first workflow as the foundation.

### Checklist
- [x] Rust workspace exists.
- [x] Dockerfile builds `render-cli`.
- [x] CLI has `doctor`, `validate`, `render`, `job`.
- [x] Demo PNG sequence render works for the current static scene.
- [x] Private GitHub repository exists.
- [ ] CI builds the Docker image or runs `cargo check`.

### Acceptance Criteria
- [x] `docker build -t ae-native-renderer:dev .` succeeds.
- [x] `docker run --rm ae-native-renderer:dev doctor` succeeds.
- [x] Demo `scene.json` renders numbered PNG frames.

### Non-goals
- Real media decode.
- Real text shaping.
- AE parity.

---

## V2-R1 — Generated Payload JSON Contract

### Goal
Define the JSON payload produced by the upstream generator as the native input.

### Checklist
- [ ] Add `docs/PAYLOAD_CONTRACT.md`.
- [ ] Add payload version field, for example `payloadVersion: "0.1"`.
- [ ] Document required top-level objects:
  - [ ] `projectSpec`
  - [ ] `compsSpec`
  - [ ] `footage_layers`
  - [ ] `text_layers`
- [ ] Document layer fields used by MVP:
  - [ ] `type`
  - [ ] `name`
  - [ ] `in_point`
  - [ ] `out_point`
  - [ ] `z_index`
  - [ ] `props`
  - [ ] `text_data`
- [ ] Document asset fields:
  - [ ] local `file_name`
  - [ ] local `file_path`
  - [ ] optional `remote_url`
- [ ] Store small representative payload fixtures without video blobs.

### Acceptance Criteria
- [ ] A generator payload can be validated without JSX.
- [ ] Missing required fields produce explicit errors.
- [ ] Unknown fields are preserved or ignored according to documented rules.

### Non-goals
- Parsing arbitrary JSX.
- Reconstructing payload from Jinja2 templates as the normal path.
- Full AE property coverage.

---

## V2-R2 — Payload Validator and Capability Report

### Goal
Tell the caller what native renderer can and cannot render before spending time.

### Checklist
- [ ] Add `render-cli validate-payload --payload ...`.
- [ ] Validate composition dimensions, fps, duration.
- [ ] Validate layer timing and layer type.
- [ ] Validate required local assets exist.
- [ ] Emit a machine-readable feature report:
  - [ ] supported
  - [ ] ignored
  - [ ] approximate
  - [ ] unsupported
- [ ] Add strict and permissive modes.

### Acceptance Criteria
- [ ] `template_4th` reports a mostly-supported MVP path with effects ignored.
- [ ] `impulse_2nd` reports keyframes/effects as pending until implemented.
- [ ] `scenes_3rd` clearly reports expressions, adjustment effects, and advanced effects as unsupported.

### Non-goals
- Auto-fixing invalid templates.
- Rendering anything.

---

## V2-R3 — Payload to Render IR Translator

### Goal
Convert generated payload JSON into the renderer's portable `scene.json`.

### Checklist
- [ ] Add translator in `ae-bridge`.
- [ ] Add `render-cli import-payload --payload ... --out scene.json`.
- [ ] Map main comp from `projectSpec.mainCompName`.
- [ ] Map dimensions/fps/duration from `compsSpec`.
- [ ] Map footage layers to IR footage layers.
- [ ] Map text layers to IR text layers.
- [ ] Preserve unsupported fields in a diagnostics sidecar.
- [ ] Sort/render layer order consistently using `z_index`.

### Acceptance Criteria
- [ ] Generated payload converts to valid `render-ir`.
- [ ] Output scene is deterministic.
- [ ] Unsupported effects/expressions are reported, not silently lost.

### Non-goals
- JSX evaluation.
- AEP/AEPX import.

---

## V2-R4 — Media Probe and Asset Resolver

### Goal
Resolve local job assets and read video/audio metadata.

### Status
Implemented with an FFprobe-backed probe and a job-folder/tar resolver. `render-core`
still receives media through a trait and has no direct backend dependency.

### Checklist
- [x] Define job input layout for native renderer.
- [x] Resolve payload `file_name` to local `media/video` and `media/audio`.
- [x] Implement real media probe using FFmpeg or GStreamer.
- [x] Return width, height, fps, duration, pixel format.
- [x] Return audio sample rate, channels, duration.
- [x] Fail clearly on missing assets in strict mode.

### Acceptance Criteria
- [x] Each example archive's local videos can be probed.
- [x] Probe result is stable JSON.
- [x] `render-core` still has no direct media backend dependency.

### Non-goals
- Frame decoding.
- Audio mixing.
- S3 download inside renderer.

---

## V2-R5 — Video Frame Decode

### Goal
Decode requested footage frames through a media backend abstraction.

### Status
Implemented with a local `FfmpegVideoSource` and `render-cli dump-frames`.

### Checklist
- [x] Implement `VideoSource` for local MP4 files.
- [x] Implement `frame_at(time)` with nearest-frame semantics.
- [x] Convert decoded frames to RGBA8.
- [x] Add `render-cli dump-frames`.
- [x] Cache decoded/probed sources per render job.

### Acceptance Criteria
- [x] First N frames of a local example video dump as PNG.
- [x] Frame dimensions match probe metadata.
- [x] Decode errors are actionable.

### Non-goals
- Perfect VFR support.
- Audio.
- GPU decode.

---

## V2-R6 — Static Footage Timeline Render

### Goal
Render the first native template with only video clips and background.

### Status
Implemented for static footage layers using activity windows, `source_start`,
z-order, cover-fit resizing, and numbered PNG output.

### Checklist
- [x] Map layer `in_point` / `out_point` to composition activity.
- [x] Map source start time from layer metadata where available.
- [x] Composite active footage layers in `z_index` order.
- [x] Add source canvas sizing and crop/fit behavior.
- [x] Add deterministic output manifest.

### Acceptance Criteria
- [x] A stripped payload with only footage renders end-to-end.
- [x] Clips appear at the expected timeline positions.
- [x] Output is a numbered PNG sequence.

### Non-goals
- Text.
- Keyframes.
- Effects.

---

## V2-R7 — Static Text Render

### Goal
Render simple text layers above footage.

### Checklist
- [ ] Load font files by payload font name or configured mapping.
- [ ] Use a real font rasterization backend.
- [ ] Support fill color and opacity.
- [ ] Support multi-line text.
- [ ] Support basic alignment used by examples.
- [ ] Ignore stroke/shadow/animator in permissive mode with diagnostics.

### Acceptance Criteria
- [ ] Text appears over footage in native output.
- [ ] Cyrillic text renders correctly for example payloads.
- [ ] Missing fonts fail clearly or use a configured fallback.

### Non-goals
- AE-perfect typography.
- Text animators.
- Per-character styling.

---

## V2-R8 — Static 2D Transforms

### Goal
Apply anchor, position, scale, rotation, and opacity to footage and text layers.

### Checklist
- [ ] Use transform matrix from `transform-math`.
- [ ] Implement transformed sampling for footage.
- [ ] Implement transformed rendering for text layer canvas.
- [ ] Support nearest-neighbor first.
- [ ] Add bilinear sampling after correctness.
- [ ] Add focused tests for anchor/position/scale/rotation.

### Acceptance Criteria
- [ ] Example footage fills the 1080x1960 frame according to payload scale/anchor.
- [ ] Text position matches payload within an acceptable v0 tolerance.
- [ ] Opacity affects final output.

### Non-goals
- 3D transforms.
- Camera.
- Motion blur.

---

## V2-R9 — Sequence Output and MP4 Workflow

### Goal
Make native output easy to inspect and compare.

### Checklist
- [ ] Write PNG frames.
- [ ] Write `manifest.json`.
- [ ] Write `render-log.jsonl`.
- [ ] Keep external FFmpeg mux script.
- [ ] Add optional `render-cli mux` only if useful.

### Acceptance Criteria
- [ ] A native render can be muxed into MP4.
- [ ] Manifest includes fps, dimensions, duration, frame count, scene hash.
- [ ] Logs include skipped/unsupported features.

### Non-goals
- Audio muxing.
- Distributed rendering.

---

## V2-R10 — Keyframes v0

### Goal
Support the simple animated properties already present in the payload.

### Checklist
- [ ] Implement `Animated<T>` hold and linear interpolation.
- [ ] Map AE interpolation codes to v0 behavior.
- [ ] Support animated opacity.
- [ ] Support animated scale.
- [ ] Support animated position.
- [ ] Support text reveal percent as data, even before visual reveal is implemented.

### Acceptance Criteria
- [ ] `impulse_2nd` scale/opacity timing visibly changes over time.
- [ ] Static behavior remains unchanged.
- [ ] Unsupported Bezier/ease reports approximate mode.

### Non-goals
- AE Bezier fidelity.
- Roving keyframes.
- Expressions.

---

## V2-R11 — Text Precomp Flattening

### Goal
Handle the common payload shape where text lives in a `Текст` precomp placed over main footage.

### Status
Implemented for direct text precomps placed in the main comp. Unsupported nested/non-text content is reported; full cycle detection remains open.

### Checklist
- [x] Resolve precomp layers by `precomp_source.comp_name`.
- [x] Flatten simple text precomp layers into the main render stack.
- [x] Apply parent precomp transform after child transform where needed.
- [x] Detect unsupported nested cases.

### Acceptance Criteria
- [x] `template_4th` and `impulse_2nd` text precomp structure maps to native layers.
- [x] Flattened output is deterministic.
- [ ] Cycles and unsupported nested content fail clearly.

### Non-goals
- Full AE precomp behavior.
- Collapse transformations.

---

## V2-R12 — First Effects

### Goal
Implement the effects that quickly improve visual match for easier templates.

### Implementation Order
1. `ADBE Drop Shadow`
2. `ADBE Glo2`
3. `ADBE Box Blur2`

### Checklist
- [ ] Parse effect params from payload.
- [ ] Implement effect parameter structs.
- [ ] Add unit tests.
- [ ] Add micro-scene examples.
- [ ] Mark effects as approximate in capability reports.

### Acceptance Criteria
- [ ] Drop Shadow works on text alpha.
- [ ] Glow produces stable approximate output.
- [ ] Box Blur works on RGBA canvas.

### Non-goals
- Pixel-perfect AE match.
- GPU acceleration.

---

## V2-R13 — Text Reveal and Animator Subset

### Goal
Support the text reveal patterns used in the generated payloads.

### Checklist
- [ ] Implement Range Selector basics.
- [ ] Support BasedOn:
  - [ ] characters
  - [ ] words
  - [ ] lines
- [ ] Support Percent Start/End.
- [ ] Support animator opacity.
- [ ] Support simple per-glyph position/scale/rotation later.

### Acceptance Criteria
- [ ] `template_4th` reveal keyframes are visible.
- [ ] `scenes_3rd` line/word reveal can be approximated.
- [ ] Unsupported animator properties are reported.

### Non-goals
- Wiggly selector.
- Randomize order.
- Full AE text animator system.

---

## V2-R14 — Expression Subset

### Goal
Evaluate only the generated expression patterns that block real templates.

### Checklist
- [ ] Catalog expressions emitted by generator.
- [ ] Add expression fingerprints or named expression modes where possible.
- [ ] Support variables:
  - [ ] `time`
  - [ ] `inPoint`
  - [ ] `outPoint`
  - [ ] `value`
  - [ ] `thisComp.frameDuration`
- [ ] Support math functions used by examples.
- [ ] Prefer structured generator parameters over raw JS where possible.

### Acceptance Criteria
- [ ] `scenes_3rd` footage position wobble expression can be evaluated or replaced by a named native mode.
- [ ] Expression failures are explicit and local to the layer/property.

### Non-goals
- Full ExtendScript.
- Arbitrary property graph access.

---

## V2-R15 — Adjustment Layers and Remaining Effects

### Goal
Support the more complex `scenes_3rd` class.

### Implementation Order
1. `ADBE Geometry2`
2. `ADBE Posterize Time`
3. `ADBE Minimax`
4. `ADBE Turbulent Displace`

### Checklist
- [ ] Implement adjustment-layer pipeline over accumulated buffer.
- [ ] Apply effect stack to layer or accumulated canvas depending on layer type.
- [ ] Add effect timing/keyframe support.
- [ ] Add approximation status to reports.

### Acceptance Criteria
- [ ] Adjustment layers no longer silently render as transparent no-ops.
- [ ] `scenes_3rd` can render with approximate native effects.

### Non-goals
- Third-party plugins.
- Perfect AE effect parity.

---

## V2-R16 — AE Conformance Harness

### Goal
Compare native output against AE fallback output from the same generated payload.

### Checklist
- [ ] Store small golden references or frame samples.
- [ ] Compare PNG frames.
- [ ] Report max diff, mean diff, changed pixel count.
- [ ] Produce diff artifact image.
- [ ] Track unsupported/ignored features alongside visual diff.

### Acceptance Criteria
- [ ] A native render can be compared against `work/output.mp4` or selected AE frames.
- [ ] Regression thresholds can be tuned per template mode.

### Non-goals
- Full video perceptual QA.
- Demanding perfect match for approximate features.

---

## V2-R17 — Production Job Runner and Routing

### Goal
Turn the renderer into a reliable native/fallback routing component.

### Checklist
- [ ] Define native job directory contract.
- [ ] Add `render-cli job --job-dir ... --payload ...`.
- [ ] Add exit codes:
  - [ ] `0` success
  - [ ] `1` config/input error
  - [ ] `2` render error
  - [ ] `3` unsupported template
- [ ] Emit machine-readable logs.
- [ ] Emit capability report before render.
- [ ] Route unsupported jobs to AE fallback outside renderer.

### Acceptance Criteria
- [ ] Supported MVP jobs render natively.
- [ ] Unsupported jobs fail early with actionable reasons.
- [ ] No silent degradation in strict mode.

### Non-goals
- Queue integration.
- S3 upload/download.
- Autoscaling.

---

## Explicit Non-Goals for V2

- No arbitrary JSX interpreter.
- No AEP parser.
- No attempt to support every AE feature.
- No S3 dependency inside `render-core`.
- No silent dropping of visual features in strict mode.

## Practical First Slice

The first implementation sprint should be:

1. Formalize payload JSON contract.
2. Add payload validator and feature report.
3. Add payload-to-IR translator for `template_4th` without effects.
4. Implement real media probe/decode.
5. Render footage-only output.
6. Add static text on top.
7. Add static transforms.

That gets us to the first native render that matters.

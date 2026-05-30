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
- [x] CI builds the Docker image or runs `cargo check`.

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

Status: implemented for the generated-payload MVP.

### Goal
Define the JSON payload produced by the upstream generator as the native input.

### Checklist
- [x] Add `docs/PAYLOAD_CONTRACT.md`.
- [x] Add payload version field, for example `payloadVersion: "0.1"`.
- [x] Document required top-level objects:
  - [x] `projectSpec`
  - [x] `compsSpec`
  - [x] `footage_layers`
  - [x] `text_layers`
- [x] Document layer fields used by MVP:
  - [x] `type`
  - [x] `name`
  - [x] `in_point`
  - [x] `out_point`
  - [x] `z_index`
  - [x] `props`
  - [x] `text_data`
- [x] Document asset fields:
  - [x] local `file_name`
  - [x] local `file_path`
  - [x] optional `remote_url`
- [x] Store small representative payload fixtures without video blobs.

### Acceptance Criteria
- [x] A generator payload can be validated without JSX.
- [x] Missing required fields produce explicit errors.
- [x] Unknown fields are preserved in diagnostics or ignored according to documented rules.

### Non-goals
- Parsing arbitrary JSX.
- Reconstructing payload from Jinja2 templates as the normal path.
- Full AE property coverage.

---

## V2-R2 — Payload Validator and Capability Report

Status: implemented with native/fallback capability categories.

### Goal
Tell the caller what native renderer can and cannot render before spending time.

### Checklist
- [x] Add `render-cli validate-payload --payload ...`.
- [x] Validate composition dimensions, fps, duration.
- [x] Validate layer timing and layer type.
- [x] Validate required local assets exist.
- [x] Emit a machine-readable feature report:
  - [x] supported
  - [x] ignored
  - [x] approximate
  - [x] unsupported
- [x] Add strict and permissive modes.

### Acceptance Criteria
- [x] `template_4th` reports a mostly-supported MVP path with approximate effects/text animators where needed.
- [x] `impulse_2nd` reports known expression-selector bounce as approximate instead of silently accepting arbitrary expressions.
- [x] `scenes_3rd` reports supported known expressions and approximate adjustment/effect coverage.

### Non-goals
- Auto-fixing invalid templates.
- Rendering anything.

---

## V2-R3 — Payload to Render IR Translator

Status: implemented through `ae-bridge` and `render-cli import-payload`.

### Goal
Convert generated payload JSON into the renderer's portable `scene.json`.

### Checklist
- [x] Add translator in `ae-bridge`.
- [x] Add `render-cli import-payload --payload ... --out scene.json`.
- [x] Map main comp from `projectSpec.mainCompName`.
- [x] Map dimensions/fps/duration from `compsSpec`.
- [x] Map footage layers to IR footage layers.
- [x] Map text layers to IR text layers.
- [x] Preserve unsupported fields in a diagnostics sidecar.
- [x] Sort/render layer order consistently using `z_index`.

### Acceptance Criteria
- [x] Generated payload converts to valid `render-ir`.
- [x] Output scene is deterministic.
- [x] Unsupported effects/expressions are reported, not silently lost.

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

### Status
Implemented with `fontdue` rasterization and fontconfig/system-font fallback. This is
not AE typography parity, but it renders readable Latin/Cyrillic static text over
footage.

### Checklist
- [x] Load font files by payload font name or configured mapping.
- [x] Use a real font rasterization backend.
- [x] Support fill color and opacity.
- [x] Support multi-line text.
- [x] Support basic alignment used by examples.
- [x] Ignore stroke/shadow/animator in permissive mode with diagnostics.

### Acceptance Criteria
- [x] Text appears over footage in native output.
- [x] Cyrillic text renders correctly for example payloads.
- [x] Missing fonts fail clearly or use a configured fallback.

### Non-goals
- AE-perfect typography.
- Text animators.
- Per-character styling.

---

## V2-R8 — Static 2D Transforms

### Goal
Apply anchor, position, scale, rotation, and opacity to footage and text layers.

### Status
Implemented with inverse sampling for static layer transforms. Bilinear sampling
is available for transformed layer canvases.

### Checklist
- [x] Use transform matrix from `transform-math`.
- [x] Implement transformed sampling for footage.
- [x] Implement transformed rendering for text layer canvas.
- [x] Support nearest-neighbor first.
- [x] Add bilinear sampling after correctness.
- [x] Add focused tests for anchor/position/scale/rotation.

### Acceptance Criteria
- [x] Example footage fills the 1080x1960 frame according to payload scale/anchor.
- [x] Text position matches payload within an acceptable v0 tolerance.
- [x] Opacity affects final output.

### Non-goals
- 3D transforms.
- Camera.
- Motion blur.

---

## V2-R9 — Sequence Output and MP4 Workflow

### Goal
Make native output easy to inspect and compare.

### Checklist
- [x] Write PNG frames.
- [x] Write `manifest.json`.
- [x] Write `render-log.jsonl`.
- [x] Keep external FFmpeg mux script.
- [x] Add optional `render-cli mux` only if useful.

### Acceptance Criteria
- [x] A native render can be muxed into MP4.
- [x] Manifest includes fps, dimensions, duration, frame count, scene hash.
- [x] Logs include skipped/unsupported features.

### Non-goals
- Audio muxing.
- Distributed rendering.

---

## V2-R10 — Keyframes v0

### Goal
Support the simple animated properties already present in the payload.

### Checklist
- [x] Implement `Animated<T>` hold and linear interpolation.
- [x] Map AE interpolation codes to v0 behavior.
- [x] Support animated opacity.
- [x] Support animated scale.
- [x] Support animated position.
- [x] Support text reveal percent as data, even before visual reveal is implemented.

### Acceptance Criteria
- [x] `impulse_2nd` scale/opacity timing visibly changes over time.
- [x] Static behavior remains unchanged.
- [x] Unsupported Bezier/ease reports approximate mode.

### Non-goals
- Pixel-perfect AE Bezier fidelity.
- Roving keyframes.
- Expressions.

---

## V2-R11 — Text Precomp Flattening

### Goal
Handle the common payload shape where text lives in a `Текст` precomp placed over main footage.

### Status
Implemented for direct payload text precomp flattening and IR-level nested
composition rendering. Unsupported nested/non-text payload content is reported;
IR graph validation rejects missing precomp targets and cycles.

### Checklist
- [x] Resolve precomp layers by `precomp_source.comp_name`.
- [x] Flatten simple text precomp layers into the main render stack.
- [x] Apply parent precomp transform after child transform where needed.
- [x] Detect unsupported nested cases.

### Acceptance Criteria
- [x] `template_4th` and `impulse_2nd` text precomp structure maps to native layers.
- [x] Flattened output is deterministic.
- [x] Cycles and unsupported nested content fail clearly.

### Non-goals
- Full AE precomp behavior.
- Pixel-perfect collapse transformations.

---

## V2-R12 — First Effects

### Goal
Implement the effects that quickly improve visual match for easier templates.

### Implementation Order
1. `ADBE Drop Shadow`
2. `ADBE Glo2`
3. `ADBE Box Blur2`

### Checklist
- [x] Parse effect params from payload.
- [x] Implement effect parameter structs.
- [x] Add unit tests.
- [x] Add micro-scene examples.
- [x] Mark effects as approximate in capability reports.

### Acceptance Criteria
- [x] Drop Shadow works on text alpha.
- [x] Glow produces stable approximate output.
- [x] Box Blur works on RGBA canvas.

### Non-goals
- Pixel-perfect AE match.
- GPU acceleration.

---

## V2-R13 — Text Reveal and Animator Subset

### Goal
Support the text reveal patterns used in the generated payloads.

### Checklist
- [x] Implement Range Selector basics.
- [x] Support BasedOn:
  - [x] characters
  - [x] words
  - [x] lines
- [x] Support Percent Start/End.
- [x] Support animator opacity.
- [x] Support simple per-glyph position/scale/rotation later.

### Acceptance Criteria
- [x] `template_4th` reveal keyframes are visible.
- [x] `scenes_3rd` line/word reveal can be approximated.
- [x] Unsupported animator properties are reported.

### Non-goals
- AE-perfect Wiggly selector.
- AE-perfect Randomize Order.
- Full AE text animator system.

---

## V2-R14 — Expression Subset

### Goal
Evaluate only the generated expression patterns that block real templates.

### Checklist
- [x] Catalog expressions emitted by generator.
- [x] Add expression fingerprints or named expression modes where possible.
- [x] Support variables:
  - [x] `time`
  - [x] `inPoint`
  - [x] `outPoint`
  - [x] `value`
  - [x] `thisComp.frameDuration`
- [x] Support generated text expression selector bounce pattern.
- [x] Support math functions used by examples.
- [x] Prefer structured generator parameters over raw JS where possible.

### Acceptance Criteria
- [x] `scenes_3rd` footage position wobble expression can be evaluated or replaced by a named native mode.
- [x] Expression failures are explicit and local to the layer/property.

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
- [x] Implement adjustment-layer pipeline over accumulated buffer.
- [x] Apply effect stack to layer or accumulated canvas depending on layer type.
- [x] Add effect timing/keyframe support.
- [x] Add approximation status to reports.

### Acceptance Criteria
- [x] Adjustment layers no longer silently render as transparent no-ops.
- [x] `scenes_3rd` can render with approximate native effects.

### Non-goals
- Third-party plugins.
- Perfect AE effect parity.

---

## V2-R16 — AE Conformance Harness

### Goal
Compare native output against AE fallback output from the same generated payload.

### Checklist
- [x] Store small golden references or frame samples.
- [x] Compare PNG frames.
- [x] Report max diff, mean diff, changed pixel count.
- [x] Produce diff artifact image.
- [x] Track unsupported/ignored features alongside visual diff.

### Acceptance Criteria
- [x] A native render can be compared against `work/output.mp4` or selected AE frames.
- [x] Regression thresholds can be tuned per template mode.

### Non-goals
- Full video perceptual QA.
- Demanding perfect match for approximate features.

---

## V2-R17 — Production Job Runner and Routing

### Goal
Turn the renderer into a reliable native/fallback routing component.

### Checklist
- [x] Define native job directory contract.
- [x] Add `render-cli job --job-dir ... --payload ...`.
- [x] Add exit codes:
  - [x] `0` success
  - [x] `1` config/input error
  - [x] `2` render error
  - [x] `3` unsupported template
- [x] Emit machine-readable logs.
- [x] Emit capability report before render.
- [x] Route unsupported jobs to AE fallback outside renderer.

### Acceptance Criteria
- [x] Supported MVP jobs render natively.
- [x] Unsupported jobs fail early with actionable reasons.
- [x] No silent degradation in strict mode.

### Non-goals
- Queue integration.
- S3 upload/download.
- Autoscaling.

---

## V2.1 — AE-like Behavior and Production Hardening

Status: first native hardening pass implemented with controlled approximations.

### True Collapse Transformations
- [x] Add collapse graph/planning API with flattened child-layer refs.
- [x] Detect supported text/solid-only collapse trees, including nested collapsed precomps.
- [x] Reject footage, adjustment layers, effects, missing targets, cycles, and non-collapsed nested precomps for collapse.
- [x] Render supported collapsed precomps by flattening child layers and composing parent matrices.
- [x] Keep unsupported collapse boundaries on rasterize-first fallback with manifest reporting.

### Text Animator v2
- [x] Add selector primitives for characters/words/lines with stable source indices.
- [x] Support selector shape, smoothness, deterministic randomize order, and deterministic wiggly modulation.
- [x] Render opacity, position, scale, rotation, blur, and generated bounce expression selector as approximate per-unit transforms.
- [x] Keep arbitrary expression selectors unsupported/explicit.

### Expression Engine v2
- [x] Add deterministic named-pattern evaluator behind `ExpressionEvaluator`.
- [x] Support `value`, numeric and Vec2 literals, AE-like context variables, `time * N`, `value + [x,y]`, and generated bounce selector fingerprint.
- [x] Keep arbitrary ExtendScript unsupported/explicit.

### Bezier/Ease Keyframes
- [x] Add cubic Bezier animation primitives for scalar and Vec2 values.
- [x] Carry compact cubic ease in IR keyframes.
- [x] Map imported AE Bezier/ease keyframes into approximate cubic curves instead of evaluating as linear.

### Effect System Hardening
- [x] Add typed parameter structs/helpers for supported first-party effects.
- [x] Accept AE numbered params and named/direct JSON params where practical.
- [x] Expand effect unit tests for parameter parsing and time-varying params.

### V2.2 — Math / AE Conformance Crosswalk

This is the current source of truth for the remaining visual-math work. The old
`ROADMAP.md` milestone numbers are preserved here as anchors; V2 marks many of
these features as implemented only in controlled or approximate form.
Detailed template-specific status and promotion criteria live in
`docs/MATH_PARITY_STATUS.md`.

| Remaining block | Old roadmap anchor | V2 anchor / current state | What is done now | What remains for AE-like parity |
| --- | --- | --- | --- | --- |
| Motion blur | R10 non-goal, R19 target | V2-R8 non-goal; V2.2 implemented v1 | Composition/layer blur flags, shutter angle/phase, bounded temporal sample times, animated transform evaluation at subframe time, payload `motionBlur` import, and motion-blur conformance micro-scene. | AE shutter/sample parity, smarter static-layer skip, premultiplied accumulation audit, AE reference PNG thresholds. |
| True glyph-level text animator | R8 text glyph instances, R17 text animator | V2-R13, Text Animator v2 controlled subset | Range selector basics, characters/words/lines, opacity, simple position/scale/rotation/blur, deterministic randomize/wiggly approximations, real fontdue glyph layout, glyph/word/line bbox units wired into render-core text animators. | HarfBuzz/AE glyph shaping, paragraph/box text parity, exact selector weighting/order, per-glyph metrics export/debug JSON, AE references. |
| Expression evaluator v2 | R18 expression subset | V2-R14, Expression Engine v2 controlled subset | Deterministic named/fingerprint evaluator, parser with precedence/parentheses/unary/sub/div, scalar/Vec2 coercion, `Math.sin/cos/exp/min/max/PI`, `thisComp.*`, `thisLayer.*`, `value`, `textIndex`, generated bounce selector. | Trait-based evaluator wired into more render properties, more AE property graph access, expression selector amount parity, audio-level expressions if templates need them. |
| Bezier/ease parity | R11 non-goal, R12 target | V2-R10, Bezier/Ease Keyframes | Hold/linear plus compact cubic ease approximation for scalar/Vec2 properties; conformance fixture scaffold for ease probes. | AE temporal ease tangent mapping, monotonic solving thresholds tuned against goldens, closer spatial/temporal behavior. Spatial paths and roving keys remain later scope. |
| Effects golden parity | R13 module system, R14 effects, R21 conformance | V2-R12, V2-R15, V2-R16, Effect System Hardening | Supported effect registry/params for Drop Shadow, Glow, Box Blur, Geometry2, Posterize Time, Minimax, Turbulent Displace; adjustment layers; approximate reports; conformance fixture scaffold for effect stacks. | AE-ish parameter mapping, checked-in AE PNGs, tighter math for blur/glow/shadow/minimax/turbulent displacement, strict thresholds per effect/template class. |
| Collapse transformations parity | R15 precomp, R16 collapse, R21 conformance | V2-R11, True Collapse Transformations controlled subset | Nested precomp graph, cycle detection, text/solid-only collapse flattening, matrix composition, scale-aware collapsed text rasterization to preserve sharpness under parent scale, raster fallback reports, collapse conformance fixture scaffold. | Defer rasterization for more vector children, expand supported nested/effect cases, AE reference tests, clear fallback/fail policy. |
| Color/compositing/sampling parity | R6 canvas/composite, R10 transforms, R21 conformance | V2-R8, V2-R16; not yet isolated as its own V2 block | Alpha-over, transforms, ROI bounds, deterministic output. | Premultiplied/straight alpha audit, AE sampling/filtering behavior, gamma/color-space assumptions, blend-mode/matte/mask inventory if templates start using them. |
| Later AE expansion | R10/R11/R12/R16/R18 non-goals | Explicit V2 non-goals | 2D controlled subset only. | 3D layers, cameras, spatial paths, roving keyframes, arbitrary ExtendScript, broad property graph access. |

Suggested implementation order:

1. Export AE PNG references for the new conformance micro-scenes and flip ready cases from `pending_ae_export`.
2. Tune motion blur accumulation and shutter semantics against AE references.
3. Tighten glyph-level text animator weighting/order after adding glyph debug export.
4. Wire the expression evaluator into more property paths only when generator payloads require them.
5. Drive Bezier/ease, collapse, and effects parity from failing conformance thresholds.
6. Run color/compositing/sampling audit as a cross-cutting parity pass.

### P05 Status Update — 2026-05-30

Detailed status report: `docs/phase_reports/P05_STATUS_ROADMAP_UPDATE_20260530.md`.

Current decision:

```text
p05_staged_candidate_green_pr_opened_pending_review_merge
```

What changed:

- P6 current-text AD68 pixel route is now part of the P05 candidate, with
  production default enabled by the prior `e86ab6e` branch commit and explicit
  disable still available through `AE_NATIVE_RENDERER_P6_AD68_TEXT_PIXELS_OPT_IN=0`.
- The P05 candidate package was committed as `77cc0b6` on
  `codex-p6-opt-in-ad68-text-route`.
- Draft PR opened: <https://github.com/VileBody/ae-native-renderer/pull/1>.
- The candidate was validated in a temporary clean worktree containing only
  `HEAD + staged patch`; local unstaged dirty context was excluded.

Validation summary:

```text
git diff --cached --check
git diff --check
cargo fmt --check -p text-engine/render-core/render-cli/ae-bridge/render-ir
python3 -m py_compile scripts/ae_trace_cooltype_text.py scripts/analyze_are_sampler_trace.py scripts/run_master_conformance_gate.py
env CARGO_BUILD_JOBS=2 cargo test --offline -p text-engine -- --test-threads=1
env CARGO_BUILD_JOBS=2 cargo test --offline -p render-core -- --test-threads=1
env CARGO_BUILD_JOBS=2 cargo test --offline -p render-cli -- --test-threads=1
env CARGO_BUILD_JOBS=2 cargo test --offline -p ae-bridge -- --test-threads=1
env CARGO_BUILD_JOBS=2 cargo test --offline -p render-ir -- --test-threads=1
```

Remaining before calling P05 fully closed:

- PR #1 review/merge and any remote CI decision.
- Decide separately what to do with unstaged `layer_eval.rs` M17 alpha-fit and
  P2 journal experiment hunks.
- Keep untracked static/TDD/probe helper files out of the P05 package unless
  they are deliberately reviewed in a later tooling package.

### V2.3 — Remaining Gaps From V1 + V2 Roadmaps

This section tracks roadmap items that are not simply "make math more AE-like"
and should not disappear just because the V2 MVP renders current examples.

| Gap | Old roadmap anchor | V2 anchor / status | Why it still matters |
| --- | --- | --- | --- |
| Doctor strict mode and runtime path checks | R1 | V2-R0 says doctor succeeds; no strict doctor mode | `doctor` reports plugins/binaries, but does not fail in strict mode or validate `/work/jobs`/runtime paths as a deployment preflight. |
| `dump-frames` backend selection | R3 | V2-R5, V2.1 media backends | Render uses selectable GStreamer/FFmpeg media backends, but `dump-frames` is still hardwired to the FFmpeg source path. |
| Real glyph-layout debug/export | R8 | V2-R7, V2-R13, V2.2 text backlog | `text-engine` now has real fontdue glyph layout used by rendering, but still lacks inspectable shaped glyph layout/debug JSON and HarfBuzz-grade shaping. |
| Vec3/color animated values | R11 | V2-R10 narrowed scope | Current runtime keyframes cover scalar and Vec2 paths needed by templates. Vec3/color animated values from V1 remain outside the implemented subset. |
| Strict/permissive effect runtime policy | R13/R22 | V2-R2/V2-R17 capability routing | Unknown effects fail during render, and payload/job validation can route unsupported work, but there is no clean render-time `strict-effects` policy switch. |
| Organized AE golden fixture suite | R14/R21/RXXX | V2-R16 plus conformance manifest scaffold | Compare CLI works and micro-scene manifests exist for ease/effects/collapse/motion blur, but AE reference PNGs and enforced thresholds still need to be exported. |
| SSIM/perceptual metric placeholder | R21 | V2-R16 non-goal-ish | Current compare reports max/mean/changed pixels and diff images; no SSIM/perceptual metric yet. |
| Audio in final jobs | R2/R18/R20 adjacent | V2-R4 probes audio; audio muxing is V2-R9 non-goal | Audio assets are recognized/probed and audio layers are ignored with diagnostics. No audio decode/mix/mux, no audio-level expressions. |
| Production resource controls | R20 | V2-R17 job runner | Job runner exists, but `RENDER_MAX_THREADS`, `RENDER_TMP_DIR`, `RENDER_STRICT` style deployment controls are not implemented as a cohesive contract. |
| Peak memory logging | R23 | V2.1 performance timing | We log frame/layer/effect/media timings, but not peak RSS/memory pressure. |
| Benchmark comparison harness | R23/R24 | Scripts exist for media backend profiling | We have media backend profile scripts, but no general renderer benchmark suite that compares commits and reports regressions. |
| Precomp frame cache | R15/R24 | V2.1 unchecked | Still missing. Expensive nested/precomp renders are not cached across frames. |
| Allocation reuse / buffer pools | R24 | V2.1 unchecked | Still missing. Canvas/effect/frame buffers are reallocated more than a production renderer should. |
| Optional parallel rendering | R24 | V2.1 unchecked | Still missing. No optional parallel frame, row, or effect processing path yet. |
| Mobile-readiness traits/feature flags | R25 | Architecture goal partially met | `render-core` stays media-backend-free, but stable text/raster backend traits and server-only feature flags are not fully split. |
| Template feature manifests and cost model | RXXX | V2-R17 capability report exists | Jobs emit capabilities, but there is no persistent per-template feature manifest, renderer-feature status catalog, or cost/economics model. |
| Native/future device/AE routing matrix | RXXX | V2-R17 native/fallback routing | Native vs AE fallback exists conceptually; future on-device routing and richer route economics remain unbuilt. |

### Performance / Production Maturity
- [x] Add render manifest/log timing: total render, per-frame render/save/total ms.
- [x] Add renderer metadata, feature counts, and deterministic asset-spec hashes.
- [x] Add per-layer/per-effect timing to render logs and manifest profile summaries.
- [x] Replace per-frame FFmpeg process spawning with a persistent sequential decoder pipe.
- [x] Add per-source media frame cache and a bounded open-decoder pool.
- [x] Add detailed media I/O timing for request/cache/decode/spawn/read and MP4 mux.
- [x] Add media profiling docs and runtime tuning knobs.
- [x] Add media runtime contract structs for frame requests, source plans, timeline media plans, and video sinks.
- [x] Add timeline media planner with `media-plan.json` output.
- [x] Add source prepare/prewarm before early render frames through `AE_RENDER_PREWARM_FRAMES`.
- [x] Add Rust GStreamer appsink decode backend selectable through `AE_RENDER_MEDIA_BACKEND`.
- [x] Add Rust GStreamer appsrc MP4 sink selectable through `AE_RENDER_MUX_BACKEND`.
- [x] Add opt-in direct render-to-`VideoSink` MP4 path through `AE_RENDER_OUTPUT_MODE=direct_mp4` / `AE_RENDER_DIRECT_MP4=1`.
- [x] Add repeatable backend profile/compare scripts for GStreamer vs fallback outputs.
- [x] Add transformed-layer ROI bounds so transforms do not scan the full comp when avoidable.
- [ ] Add precomp frame cache.
- [ ] Add allocation reuse / buffer pools.
- [ ] Add optional parallel frame rendering.

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

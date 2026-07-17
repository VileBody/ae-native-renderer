# JSON render API v1

The stable boundary is JSON in and JSON out. The request keeps the generated JSX
inputs at the top level, so production can send the same `projectSpec`,
`compsSpec`, `footage_layers`, and `text_layers` objects without an intermediate
scene format.

Schemas:

- `schemas/render-request-v1.schema.json`
- `schemas/render-response-v1.schema.json`
- `schemas/output-manifest-v1.schema.json`

## Request

```json
{
  "schema": "ae-native-renderer.render-request.v1",
  "requestId": "job-123",
  "action": "render",
  "projectSpec": {
    "mainCompName": "Comp 1",
    "subtitlesMode": "brat_5th",
    "colorManagement": {
      "workingSpace": "none",
      "linearBlending": false,
      "outputSpace": "srgb"
    }
  },
  "compsSpec": [],
  "footage_layers": [],
  "text_layers": [],
  "visualOps": [],
  "assetsSpec": {
    "root": "app"
  },
  "outputSpec": {
    "directory": "out",
    "video": "result.mp4"
  },
  "debugSpec": {
    "captureEffectStages": false
  },
  "policy": {
    "onUnsupported": "report"
  }
}
```

`visualOps` is the extension point for JSX code that creates or mutates AE
layers after the static arrays have been declared. Known v1 operation families:

- `subtitle.brat.v1`
- `subtitle.trendy.v1`
- `subtitle.bot.impulse_2nd.v1`
- `subtitle.bot.scenes_3rd.v1`
- `subtitle.bot.scenes_3rd_single_step.v1`
- `subtitle.bot.template_4th.v1`
- `subtitle.bot.legacy_blocks.v1`
- `style.semantic.v1`
- `hook.f1.sound.v1`
- `hook.f2.object.v1`
- `hook.f3.effect.v1`
- `hook.f4.motion.v1`
- `hook.f5.cognition.v1`

An operation may be represented before its native lowering exists. Such an
operation is returned under `capabilities.not_implemented`; it is never silently
dropped. Brat and Trendy subtitles lower to native text layers as `approximate`.
Trendy uses one unified fill/stroke text paint, Tracking Amount `7 -> -1`, the
native gradient/shadow approximation, and the analog stack. F3 includes
directional Motion Blur, Optics Compensation, full-radius Minimax,
shutter/snap/flash/shake, light/slow-shutter/negative-zoom hooks, and active
extras. F1/F2/F4/F5 lower to native visual operations; F1/F5 consume supplied
local audio rather than synthesizing it inside the renderer.

`projectSpec.colorManagement` is optional. Its production default is an
unmanaged working space with nonlinear blending and sRGB output. Native v1
accepts `workingSpace` values `none` and `srgb`; other ICC or wide-gamut
profiles remain explicit capability gaps. Visual operation assets use the
normalized roles `audio`, `tts_audio`, `overlay`, and `matte`.

`debugSpec` is optional. Production defaults all flags to `false`.
`captureEffectStages=true` forces the PNG/full-telemetry render path even when
`outputSpec.video` is requested, so `render/render-log.jsonl` records full effect
stage telemetry for differential debugging. The stage image switches now export
PNG snapshots under `render/stage-debug/frame_XXXXXX/<category>/` and attach the
relative paths to each `frame.rendered` event:

- `sourceLayers`: source raster after text/footage/solid/precomp acquisition.
- `preEffects`: layer raster before the local effect stack and parent transform.
- `textMasks`: text/range-selector mask raster before local effects.
- `captureEffectStages`: raster after each layer or adjustment effect.
- `adjustmentResults`: adjustment-layer result after each adjustment effect.
- `precompResults`: child comp raster inside its own raster boundary.
- `finalComposite`: final frame after all layers composite.

When any stage image flag is enabled, the renderer uses the deterministic PNG
sequence path even if `outputSpec.video` is present; MP4 muxing then happens
from those PNGs.

### Exact frame selection

Set `outputSpec.frames` to render exact source composition frame indices as PNGs:

```json
{
  "outputSpec": {
    "directory": "out",
    "frames": [41, 7, 41]
  }
}
```

The normalized request sorts and deduplicates this list to `[7, 41]`. Indices
are zero-based and must be in `0..ceil(duration * fps)`. The composition duration
and full frame count are not shortened; only the requested frame evaluations run,
and their filenames retain the source index, such as `frame_000041.png`.

`render/manifest.json` and `render/render-log.jsonl` expose `selected_frames`
and the actual rendered-frame count. `output-manifest.json` keeps the full
composition duration/frame count while its frames artifact count reflects the
number of selected PNGs.

`outputSpec.frames` must contain at least one nonnegative integer and cannot be
combined with `outputSpec.video`. Sparse frames are not a contiguous MP4 input;
an invalid index or `video` combination is a configuration error before render.

## Policies

- `report` renders the supported subset and returns `status: "partial"` plus a
  complete capability report.
- `error` returns `status: "unsupported"`, produces no render, and exits with
  code 3.

`report` is the default during parity development. Production callers can switch
to `error` when partial output must never be published.

## Commands

Send a request through stdin:

```bash
cargo run -p render-cli -- json --request - < request.json
```

Use a file and also persist the response:

```bash
cargo run -p render-cli -- json \
  --request request.json \
  --response out/response.json
```

Convert an existing generated JSX job to the v1 request shape:

```bash
cargo run -p render-cli -- extract-jsx-request \
  --jsx app/render.jsx \
  --out request.json
```

The extractor preserves the four static JSX variables and currently recognizes
Brat/Trendy word timing plus an injected F3 block as `visualOps`.

For local migration work, `render-cli adapt-bot-payload --input bot.json --out
request.json` converts the fixed Blast bot planner envelope (`blast.bot-render.v1`)
to this request. It retains the source subtitle family, scene type, focus metadata,
cut times, and semantic style IDs as native operations, rather than attempting to
recover them from generated JSX. The canonical visual corpus is under
`fixtures/bot_corpus` and is rendered with `scripts/run_bot_visual_corpus.py`.
Canonical RenderPlan/native requests can be rendered and compared to AE MP4s with
`scripts/run_render_plan_differential.py`; it writes native MP4s, optional
side-by-side control frames, optional heatmaps, contact sheets, and capability
summaries.

When `outputSpec.video` is set, every main-composition footage layer with
`layer_meta.audioEnabled=true` is mixed into the rendered MP4. Footage timing
supports `in_point`, `out_point`, `layer_meta.startTime`, static audio level,
and `audio_envelope` fields `level_db`, `fade_in_s`, `fade_out_s`, `min_db`, and
`delay_s`. Source sampling follows AE timing:
`source_time = comp_time - startTime`.

`visualOps[].assets` with role `audio` or `tts_audio` add local audio tracks.
Their operation can use `timing.start/duration/end` plus `params.trim`,
`startTime`, `levelDb`, `fadeIn`, `fadeOut`, and `delay`. F1 `params.impactAt`
places an impact track. A foreground operation may set
`params.duck={amountDb,attack,release}` to duck footage tracks marked as
background. The renderer does not synthesize or download TTS; `tts_audio.path`
must resolve through `assetsSpec` like every other media asset.

Tracks are mixed with `amix`, trimmed to composition duration, and encoded as
AAC. A track's capability is promoted from `not_implemented` to `supported`
only after its source slice and the final muxed MP4 are probed. A missing
required asset remains a capability gap and therefore follows
`policy.onUnsupported`; optional missing assets are reported as warnings.

## Response

Every successful protocol exchange prints one JSON response to stdout. Progress
and process errors go to stderr. Important fields:

- `status`: `validated`, `inspected`, `rendered`, `partial`, `unsupported`,
  `invalid`, or `failed`;
- `ok`: the requested action ran successfully;
- `complete`: no required operation is missing or unsupported;
- `capabilities`: supported, approximate, ignored, not implemented, and unknown
  features;
- `artifacts`: normalized request, imported scene, capability report, frames, and
  optional MP4;
- `request_hash`: SHA-256 of the normalized request model.

`request_hash` is SHA-256 of the normalized request model, so insignificant JSON
whitespace and object-key ordering do not change it.

Relative asset and output paths are resolved from the request file directory.
For stdin, they are resolved from the current working directory.

The deterministic `output-manifest.json` contains stable relative artifact
paths, content hashes for JSON/video artifacts, and an aggregate hash plus count
for rendered frames. MP4 output also records frame count, composition duration,
and an `ffprobe` summary including video/audio codecs, sample rate, channels, and
media duration. Runtime-only values such as elapsed time and absolute paths
remain in the response and are intentionally excluded from that manifest.

Full MP4 renders use production telemetry: stable frame/effect timings and
capability sidecars remain enabled, while expensive intermediate debug images
and full-frame hashes are reserved for PNG/conformance runs.

Run the CPU performance and RSS gate with:

```bash
scripts/benchmark_trendy_release.sh request.json target/perf/trendy_gate
```

## Exit codes

- `0`: the requested action completed, including a policy-approved partial render;
- `1`: malformed request, unsupported request schema, or invalid scene graph;
- `2`: render, asset resolution, or output failure;
- `3`: required capabilities are missing and `onUnsupported` is `error`.

Audio remains `not_implemented` when no MP4 output is requested, an asset cannot
be resolved, mux/probe fails, or the request needs time-remap, stretch, or
animated-level behavior. A silent or unverifiable MP4 therefore cannot report
`complete: true` when the input requires audio.

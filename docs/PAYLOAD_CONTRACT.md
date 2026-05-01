# Generated Payload Contract v0.1

The native renderer consumes the structured JSON payload produced by the upstream
generator. JSX/Jinja2 templates may still use the same payload for After Effects
fallback, but JSX is not the native input format.

## Top-level Shape

```json
{
  "payloadVersion": "0.1",
  "projectSpec": {
    "mainCompName": "Comp 1",
    "subtitlesMode": "template_4th"
  },
  "compsSpec": [],
  "footage_layers": [],
  "text_layers": []
}
```

Required fields:

- `projectSpec.mainCompName`: composition to render.
- `compsSpec`: composition metadata.
- `footage_layers`: footage, audio, and precomp layers currently emitted by the generator.
- `text_layers`: text and adjustment layers currently emitted by the generator.

Unknown fields are allowed. The native importer should preserve or report them
when they affect visual output.

## Composition

Each item in `compsSpec` describes one composition:

```json
{
  "name": "Comp 1",
  "w": 1080,
  "h": 1960,
  "fps": 23.9759979248047,
  "dur": 13.87,
  "pixelAspect": 1.0,
  "workAreaStart": 0.0,
  "workAreaDuration": 13.87,
  "displayStartTime": 0.0,
  "bgColor": [0, 0, 0]
}
```

`w`, `h`, `fps`, and `dur` must be positive.

## Layer

The MVP uses these common layer fields:

```json
{
  "name": "clip.mp4",
  "type": "footage",
  "in_point": 0.0,
  "out_point": 3.0,
  "z_index": 100,
  "text": "",
  "adjustment_layer": false,
  "props": {},
  "effects": {},
  "text_data": {}
}
```

Recognized layer types:

- `footage`: planned first-class native layer.
- `text`: planned first-class native layer.
- `precomp`: initially supported only by flattening simple text precomps.
- `adjustment`: recognized but unsupported until the adjustment/effects pipeline exists.

`out_point` must be greater than `in_point`.

## Properties

Layer properties are keyed by generator names such as `tf_position`, `tf_scale`,
`tf_rotation`, `tf_opacity`, `layer_opacity`, or `reveal`.

```json
{
  "match_name": "ADBE Position",
  "value": [540, 980, 0],
  "keyframes": [],
  "expression": null
}
```

MVP property plan:

- Static transform values are supported first.
- Hold/linear keyframes come after static footage and text rendering.
- Raw expressions are unsupported until the known generated expression subset is cataloged.

## Assets

Footage asset metadata usually lives under `text_data.source_footage`:

```json
{
  "file_name": "clip.mp4",
  "file_path": "",
  "remote_url": "s3://bucket/path/clip.mp4"
}
```

Native jobs should provide local assets in the job directory. S3 download and
remote asset acquisition are outside `render-core`.

## Effects

Effect keys may include an ordinal prefix, for example `0001:ADBE Drop Shadow`.
The native validator normalizes this to the AE matchName.

Initial effect priority:

1. `ADBE Drop Shadow`
2. `ADBE Glo2`
3. `ADBE Box Blur2`
4. `ADBE Geometry2`
5. `ADBE Posterize Time`
6. `ADBE Minimax`
7. `ADBE Turbulent Displace`

Until implemented, unsupported effects must be reported instead of silently
disappearing in strict mode.

## Validation

Use:

```bash
render-cli validate-payload --payload fixtures/payload/minimal_static.json
render-cli validate-payload --payload fixtures/payload/minimal_static.json --strict
```

The command prints a JSON capability report with:

- composition summary;
- layer type counts;
- effect counts;
- expression counts;
- keyframed property counts;
- supported/ignored/approximate/unsupported findings.

## Import to Render IR

Use:

```bash
render-cli import-payload \
  --payload fixtures/payload/minimal_static.json \
  --out jobs/minimal_static/scene.json \
  --diagnostics jobs/minimal_static/import-diagnostics.json
```

The importer writes `render-ir` `scene.json`.

Current import rules:

- imports video `footage` layers as IR footage layers;
- stores footage `source_start` from AE `layer_meta.startTime` when present;
- imports static `text` layers as IR text layers;
- flattens direct text children from precomps placed in `projectSpec.mainCompName`;
- applies parent precomp timing and static transform approximately to flattened text;
- skips flattened precomp placeholders after their text children are imported;
- skips audio layers with an `ignored` diagnostic;
- skips adjustment layers, non-text nested content, and unsupported nested comps with diagnostics.

## Native Job Assets

The native renderer resolves media from either an extracted job folder or a job
archive. Supported layouts:

```text
job_id/app/media/video/*.mp4
job_id/app/media/audio/*.mp3
app/media/video/*.mp4
app/media/audio/*.mp3
media/video/*.mp4
media/audio/*.mp3
```

Use:

```bash
render-cli resolve-assets \
  --scene jobs/example/scene.json \
  --assets-root jobs/example/app \
  --strict

render-cli resolve-assets \
  --scene jobs/example/scene.json \
  --job-archive jobs/example_job_folder.tar.gz \
  --strict
```

The command prints stable JSON with each asset's requested path, resolved path,
probe metadata, and errors. Video rendering uses the same resolver:

```bash
render-cli render \
  --scene jobs/example/scene.json \
  --out jobs/example/out \
  --job-archive jobs/example_job_folder.tar.gz
```

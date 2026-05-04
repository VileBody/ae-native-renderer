# Agent 5 Text, Glyphs, Expressions Native Diff

Status date: 2026-05-03.

## Scope

Agent 5 covers `M05`, `M06`, `M07`, `M08`, `M09`, and text-side `M17`.

| Case | Modules | Frames |
| --- | --- | --- |
| `TXT_010` | `M05`, `M06` | 0, 8, 16, 24, 32, 45, 59 |
| `TXT_020` | `M05`, `M06` | 0, 8, 16, 24, 32, 45, 59 |
| `TXT_030` | `M05`, `M07` | 0, 8, 16, 24, 32, 45, 59 |
| `TXT_040` | `M08`, `M09` | 0, 5, 10, 15, 20, 30, 45, 59 |
| `EXP_010` | `M09` | 0, 5, 10, 15, 20, 30, 45, 59 |
| `GPH_010` | `M17`, `M05`, `M19` | 0, 15, 30, 45 |

Native output:

```text
target/ae_agents/agent5_text_glyphs_expr/
```

## Commands

Host cargo is unavailable:

```sh
cargo --version
# zsh:1: command not found: cargo
```

Reproducible Docker command used for the native run:

```sh
docker run --rm --entrypoint sh \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'set -eu
apt-get update
apt-get install -y --no-install-recommends pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libglib2.0-dev fontconfig
groupadd -o -g "$HOST_GID" hostgroup 2>/dev/null || true
useradd -o -m -u "$HOST_UID" -g "$HOST_GID" hostuser 2>/dev/null || true
mkdir -p /tmp/cargo-home /tmp/cargo-target
chown -R "$HOST_UID:$HOST_GID" /tmp/cargo-home /tmp/cargo-target
su -s /bin/sh hostuser -c '"'"'export PATH=/usr/local/cargo/bin:$PATH CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/cargo-target; cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/agent5_text_glyphs_expr --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case EXP_010 --case GPH_010'"'"'
'
```

Result:

```text
conformance-pack.done ok=true cases=6 report=target/ae_agents/agent5_text_glyphs_expr/report.json
```

No thresholds were supplied, so each case status is `measured`; this is not a
parity pass.

Font availability probe in the same Docker family:

```sh
docker run --rm --entrypoint sh -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'fc-match -f "%{file}\n%{family}\n" "Point-Light"'
```

Observed `Point-Light` fallback:

```text
/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
DejaVu Sans
```

## Top Metrics

All six cases report `max_abs_diff=255`. Sorted by overall mean diff:

| Case | Mean | RMSE | Changed ratio | Worst frame by mean |
| --- | ---: | ---: | ---: | --- |
| `GPH_010` | 73.2563 | 134.9828 | 1.000000 | all selected frames: 73.2563 |
| `TXT_030` | 72.1700 | 132.0501 | 0.999392 | frame 0: 72.8996 |
| `TXT_040` | 69.6798 | 132.6263 | 0.999416 | frame 45: 70.5027 |
| `TXT_010` | 68.4776 | 130.8144 | 0.999864 | frame 59: 73.3961 |
| `TXT_020` | 68.3625 | 130.5238 | 0.999995 | frame 59: 73.1583 |
| `EXP_010` | 63.5228 | 127.2083 | 0.983911 | frame 10: 65.5146 |

Corner/background pixel samples explain most of the frame-wide changed ratio:

| Case/frame | Native corner RGBA | AE corner RGBA | Interpretation |
| --- | --- | --- | --- |
| `TXT_010` frame 0 | `[5, 5, 6, 255]` | `[5, 5, 6, 0]` | opaque native background vs transparent AE background |
| `EXP_010` frame 0 | `[5, 5, 6, 255]` | `[5, 5, 6, 0]` | same; text/expression signal is buried under alpha diff |
| `GPH_010` frame 0 | `[5, 5, 6, 255]` | `[5, 5, 6, 0]` | same; collapse sharpness cannot be judged yet |

This points to an `M19` alpha/background primitive before any Agent 5 formula
tuning. Agent 5 should not patch that substrate here, but downstream text diffs
need either the alpha behavior fixed by the owner or a text-focused masked
metric that excludes unchanged transparent background.

## Inspection Notes

`metrics.json` currently contains only image-level summary/frame metrics. It
does not include font resolution, glyph ids, advances, bboxes, line breaks,
selector weights, per-glyph transforms, blur radii, expression sampled values,
or collapsed raster scale.

Font resolution:

- `TXT_010`, `TXT_020`, and `GPH_010` use bundled
  `fixtures/ae_conformance_pack/assets/fonts/Montserrat-Italic[wght].ttf`.
  The AE manifest requires `Montserrat-BoldItalic`; native uses the variable
  italic TTF by path, without telemetry for selected weight/postscript.
- `TXT_030` and `TXT_040` request `Point-Light`. The native Docker environment
  resolves that request to DejaVu Sans, while the AE clean metadata reports no
  missing fonts. This is a high-risk blocker before interpreting glyph motion or
  expression-selector bounce diffs.

Glyph layout:

- `crates/text-engine/src/layout.rs` stores `glyph_id` as `ch as u32`, not the
  resolved font glyph id. Advances and bboxes come from `fontdue.metrics`.
- Layout is explicit-newline split and centered in the text box; no AE/CoreText
  shaping, kerning, or paragraph composer telemetry is emitted.

Selector weights:

- `TXT_010` word reveal, `TXT_020` character/line reveal, and `TXT_030` glyph
  animator all animate selector `start` from 0 to 100 over 2 seconds with fixed
  `end=100`, `shape=square`, and `smoothness=100`.
- Runtime selector math lives in `render-core/src/layer_eval.rs`, while helper
  selector code also exists in `crates/text-engine/src/text_animator.rs`. The
  next patch should report runtime weights, not just helper weights.

Per-glyph transform/blur:

- `TXT_030` applies position `[0,-72]`, scale `[125,125]`, rotation `18`, and
  blur `[10,10]` to character units.
- Runtime blur is a bounded splat approximation:
  `round(blur * weight * unit_scale / 8)`, clamped to 0..4. No per-unit matrix
  or blur-radius telemetry is written.

Expression sampled value:

- `EXP_010` uses generated `edge_wobble` with base position `[256,256]`,
  `intro=0.25`, `outro=0.25`, `amp=34`, and `freq=2`.
- Native sampled positions from the current formula at selected frames:

| Frame | Time | Native position |
| ---: | ---: | --- |
| 0 | 0.000000 | `[256.000, 256.000]` |
| 5 | 0.166667 | `[261.148, 255.409]` |
| 10 | 0.333333 | `[256.000, 256.000]` |
| 15 | 0.500000 | `[256.000, 256.000]` |
| 20 | 0.666667 | `[256.000, 256.000]` |
| 30 | 1.000000 | `[256.000, 256.000]` |
| 45 | 1.500000 | `[256.000, 256.000]` |
| 59 | 1.966667 | `[251.916, 257.054]` |

Expression selector:

- `TXT_040` uses native `PerCharacterBounce` with `delay=0.05`, `freq=2`,
  `amplitude=100`, and `decay=8`, then applies it as a scale animator toward
  `[0,0]`.
- No per-character `textIndex`, raw amount, clamped amount, or final scale is
  recorded in the conformance output.

Collapsed raster scale:

- `GPH_010` renders one rasterized precomp and one collapsed precomp at parent
  scale `180%`.
- The collapsed text path computes `matrix_scale_hint(matrix).clamp(1.0, 4.0)`;
  for this probe the expected effective text raster scale is about `1.8`.
- `metrics.json` does not record collapse mode, flattened layer list, parent
  matrix, local raster size, effective font size, or sharpness probe. The native
  case note only says the probe depends on the scale-aware text rasterization
  approximation.

## Suspected First Divergent Primitive

First observed divergent primitive: `M19` background alpha/composite, because
native writes an opaque `[5,5,6,255]` background while AE goldens carry the same
RGB with alpha `0` outside rendered content. This affects nearly every pixel and
dominates all six Agent 5 case metrics.

First Agent 5-owned divergent primitive after that:

1. Font resolution for `TXT_030`/`TXT_040`: exact `Point-Light` is unavailable
   in Docker and falls back to DejaVu Sans.
2. `M05` glyph layout telemetry: actual font glyph id/advance/bbox/baseline is
   not emitted, and stored `glyph_id` is currently a Unicode codepoint.
3. `M06`/`M07`/`M08` runtime selector and glyph-transform telemetry: weights,
   expression amount, final per-unit transform, opacity, and blur radius are
   not observable from `metrics.json`.
4. `M17` collapse text sharpness telemetry: effective raster scale and
   collapsed-vs-rasterized sharpness are not observable.

## Next Patch Needed

Do not tune text formulas from these PNG metrics yet. The next patch should be a
diagnostic telemetry patch, preferably emitted next to each case as
`text_telemetry.json` or included in `metrics.json` under `checkpoints`:

- `M05 font_resolution`: requested font id, resolved path, resolved family or
  postscript, fallback flag, font size, selected variable axes when known.
- `M05 glyph_layout`: char, actual font glyph id, char/word/line indices,
  advance, bbox, baseline, line width, text-box rect.
- `M06 selector_weights`: based-on mode, units, selector index/order, start/end,
  shape/smoothness, raw/final weight per selected frame.
- `M07 glyph_transform`: unit rect/glyph bbox, range weight, tx/ty, sx/sy,
  rotation, alpha scale, blur radius, and final matrix.
- `M08 expression_selector_amount`: textIndex/textTotal, delay/freq/amplitude/
  decay, raw amount, clamped amount, final glyph scale.
- `M09 property_expression_value`: input value, time, context vars, raw sampled
  value, final position for `edge_wobble`.
- `M17 collapse_text`: collapse mode, flattened layers, parent/child matrices,
  matrix scale hint, effective raster scale, local raster size, final matrix,
  and a simple sharpness metric for collapsed vs rasterized text.

Separately, route the background alpha finding to the `M19` owner or add a
temporary masked metric for text diagnosis. Without that, text-side changes will
move only a small hidden fraction of the current diff.

# Agent Round2 Text Glyph Expression

Status date: 2026-05-03.

## Scope

This pass reran the focused text/glyph/expression/collapse Round 2 baseline and
added runtime telemetry only. It did not tune glyph metrics, selector formulas,
expression formulas, or collapse formulas.

Touched runtime telemetry:

- text layout/font resolution sidecars;
- text selector unit/weight sidecars;
- position expression sample sidecars;
- collapse/precomp/text raster sidecars.

## Commands

Focused native conformance:

```sh
docker run --rm --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'set -eu
export PATH=/usr/local/cargo/bin:$PATH
export CARGO_HOME=/tmp/cargo-home
export CARGO_TARGET_DIR=/tmp/ae-native-renderer-cargo-target-round2-text
apt-get update >/dev/null
apt-get install -y --no-install-recommends fontconfig pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev >/dev/null
cargo run -p render-cli -- conformance-pack --pack fixtures/ae_conformance_pack --out target/ae_agents/round2_text --case TXT_010 --case TXT_020 --case TXT_030 --case TXT_040 --case EXP_010 --case GPH_010'
```

Telemetry renders:

```sh
cargo run -p render-cli -- render --scene target/ae_agents/round2_text/TXT_010/scene.json --out target/ae_agents/round2_text/TXT_010/telemetry_render --assets-root fixtures/ae_conformance_pack
cargo run -p render-cli -- render --scene target/ae_agents/round2_text/TXT_020/scene.json --out target/ae_agents/round2_text/TXT_020/telemetry_render --assets-root fixtures/ae_conformance_pack
cargo run -p render-cli -- render --scene target/ae_agents/round2_text/TXT_030/scene.json --out target/ae_agents/round2_text/TXT_030/telemetry_render --assets-root fixtures/ae_conformance_pack
cargo run -p render-cli -- render --scene target/ae_agents/round2_text/TXT_040/scene.json --out target/ae_agents/round2_text/TXT_040/telemetry_render --assets-root fixtures/ae_conformance_pack
```

`EXP_010` telemetry used a target-only scene copy with `alpha_square` marked as
a missing video placeholder, because `render-cli render` does not load the pack
PNG primitives as still footage. This does not affect the conformance metrics;
it only lets render-core sample the transform expression and write the
expression sidecar.

```sh
jq '(.assets[] | select(.id == "alpha_square") | .type) = "video" | del(.assets[] | select(.id == "alpha_square") | .kind)' \
  target/ae_agents/round2_text/EXP_010/scene.json \
  > target/ae_agents/round2_text/EXP_010/scene_expression_telemetry_placeholder.json

target/ae_agents/cargo-target/debug/render-cli render \
  --scene target/ae_agents/round2_text/EXP_010/scene_expression_telemetry_placeholder.json \
  --out target/ae_agents/round2_text/EXP_010/telemetry_render

target/ae_agents/cargo-target/debug/render-cli render \
  --scene target/ae_agents/round2_text/GPH_010/scene.json \
  --out target/ae_agents/round2_text/GPH_010/telemetry_render
```

## Metrics

Use `metrics.rgb` and `metrics.background_alpha_normalized`, not raw RGBA:

| Case | RGB mean | RGB max | RGB changed | BG-alpha-normalized mean | BG-alpha-normalized max |
| --- | ---: | ---: | ---: | ---: | ---: |
| `TXT_010` | 9.085755 | 250 | 114667 | 8.720305 | 255 |
| `TXT_020` | 8.660901 | 250 | 108074 | 8.367676 | 255 |
| `TXT_030` | 10.815494 | 250 | 204387 | 13.547671 | 255 |
| `TXT_040` | 6.823042 | 250 | 73322 | 6.656466 | 255 |
| `EXP_010` | 1.304101 | 250 | 12838 | 1.188759 | 255 |
| `GPH_010` | 18.658145 | 250 | 107504 | 16.043449 | 255 |

Full per-frame metrics are in:

- `target/ae_agents/round2_text/<CASE>/metrics.json`;
- aggregate report: `target/ae_agents/round2_text/report.json`.

## Exact Font Status

`TXT_030` and `TXT_040` generated scenes both use the direct repo-local path:

```text
fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf
```

Runtime text telemetry confirms `Point-Light.ttf` resolves as:

```json
{
  "source": "DirectPath",
  "fallback": false,
  "resolved_family": "Point,Point Light",
  "resolved_fullname": "Point Light",
  "resolved_postscript_name": "Point-Light"
}
```

This clears the old Point-Light fontconfig fallback blocker for native-side text
diagnosis.

## Telemetry Status

Sidecars are written under `target/ae_agents/round2_text/<CASE>/telemetry_render/`:

- `text_telemetry.jsonl`;
- `expression_telemetry.jsonl`;
- `collapse_telemetry.jsonl`;
- `render-log.jsonl` also embeds the same trace arrays in each frame profile.

Observed records:

| Case | Text records | Expression records | Collapse records |
| --- | ---: | ---: | ---: |
| `TXT_010` | 120 | 0 | 0 |
| `TXT_020` | 240 | 0 | 0 |
| `TXT_030` | 120 | 0 | 0 |
| `TXT_040` | 120 | 0 | 0 |
| `EXP_010` | 0 | 60 | 0 |
| `GPH_010` | 120 | 0 | 180 |

Text sidecars include font resolution, layout glyph ids/bboxes, selector
configuration, unit rects, range weights, expression selector raw/clamped amount
for `TXT_040`, and animator contribution matrices where a glyph transform is
applied.

Expression sidecars for `EXP_010` include base position, sampled position,
local time, frame duration, duration, expression source, envelope, and x/y
offset per rendered frame.

Collapse sidecars for `GPH_010` include rasterized and collapsed precomp modes,
flattened layer ids, parent/child/effective matrices, effective text raster
scale, raster size, and a cheap alpha-edge sharpness probe.

## Tests

All tests were run in Docker because local `cargo` was not available in PATH.

```sh
cargo test -p text-engine
cargo test -p expression-engine
cargo test -p render-core text_animator_position_moves_character_unit
cargo test -p render-core selector_weight_supports_ramp_and_random_order
cargo test -p render-core collapsed_precomp_flattens_solid_with_parent_transform
cargo test -p render-core adjustment_effect_sidecar_writes_one_record_per_effect
```

Results:

- `text-engine`: 12 passed;
- `expression-engine`: 6 passed;
- render-core targeted tests listed above: all passed.

## Next Formula Target

Tune text in this order:

1. Glyph metrics and line/text-box placement.
2. Selector segmentation and selector weights.
3. Glyph animator transform order and blur approximation.
4. Property expression samples.
5. Collapse raster scale and sharpness.

The first ready tuning submodule is glyph metrics/text-box placement: exact
Point-Light is now confirmed, glyph ids/bboxes are in sidecars, and selector and
collapse diagnostics can now be interpreted relative to concrete layout units.

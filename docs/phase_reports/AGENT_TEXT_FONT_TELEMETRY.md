# Agent Text Font Telemetry

Status date: 2026-05-03.

## Scope

This pass handled text-engine observability only:

- font resolution telemetry;
- direct-path Point-Light verification;
- glyph id and metric telemetry in text layout;
- focused text-engine tests.

No glyph formula, selector formula, expression evaluator, or collapse behavior
was tuned.

## Docker Font Probe

Command shape:

```sh
docker run --rm --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'set -eu; ls -l fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf; fc-scan -f "file=%{file}\nfamily=%{family}\nstyle=%{style}\nfullname=%{fullname}\npostscript=%{postscriptname}\n" fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf'
```

Observed result:

```text
file=fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf
family=Point,Point Light
style=Light,Regular
fullname=Point Light
postscript=Point-Light
```

The repo-local `Point-Light.ttf` is readable in Docker and identifies as the
expected Point Light face.

## Changes

`crates/text-engine/src/font_db.rs` now exposes:

- `FontResolutionTelemetry`;
- `FontResolutionSource`;
- `resolve_font_with_telemetry`;
- `load_font_with_telemetry`.

Existing `resolve_font_path` and `load_font` behavior remains compatible. Direct
existing paths still resolve first. Fontconfig resolution records the resolved
file and available family/style/fullname/postscript metadata. Unknown-family
fontconfig/common-path matches are marked as fallback.

`crates/text-engine/src/layout.rs` now exposes layout telemetry through
`TextLayoutResult.telemetry`:

- font resolution for the layout request;
- text box rect;
- line height;
- per-glyph character, actual font glyph id, char/word/line indices, advance,
  bbox, baseline, line width, and text box rect.

Real layout now stores `GlyphInstance.glyph_id` from
`fontdue::Font::lookup_glyph_index` instead of the Unicode codepoint. The layout
math itself was not rewritten.

## Tests

Docker command:

```sh
docker run --rm --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'set -eu; export PATH=/usr/local/cargo/bin:$PATH CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/cargo-target; apt-get update >/dev/null; apt-get install -y --no-install-recommends fontconfig >/dev/null; cargo test -p text-engine'
```

Result:

```text
running 12 tests
test result: ok. 12 passed; 0 failed
```

Focused coverage added:

- direct path `Point-Light.ttf` resolves with `fallback=false`;
- missing font family reports fallback or missing;
- Point-Light layout reports actual font glyph ids and direct-path font
  resolution telemetry.

## TXT_030/TXT_040 Status

`TXT_030` and `TXT_040` can now be rerun in Docker under exact repo-local
Point-Light, because conformance recipes already pass
`fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf` and text-engine
direct-path resolution reports that path as non-fallback.

The rerun should still be interpreted as diagnostic rather than final text
parity: M19 alpha/background, selector telemetry, expression-selector telemetry,
and collapse telemetry remain separate blockers.

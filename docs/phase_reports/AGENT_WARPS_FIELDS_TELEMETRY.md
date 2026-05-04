# Agent Warps/Fields Telemetry

Date: 2026-05-03

## Scope

This pass stayed on the warps/fields effect scope:

- `crates/effects/src/geometry.rs`
- `crates/effects/src/turbulent_displace.rs`
- focused unit tests in those modules

No render-core timing, text-engine, BoxBlur/Glow/DropShadow/Minimax formula work
was part of this pass.

## Geometry2

Added `geometry2_debug_data(input, params, time)` with a deterministic debug
shape:

- cloned raw params;
- resolved `anchor`, `position`, `scale`, and `rotation`;
- forward source-to-output matrix;
- inverse output-to-source matrix used by the current native sampler;
- fixed 3x3 probe samples with output pixel, float source UV, rounded sample
  pixel, and OOB flag;
- full-frame OOB count;
- explicit sampler and edge policy labels: `nearest_round` and
  `transparent_out_of_bounds`.

Parameter resolution was also patched. Numbered axis scale controls now take
priority over uniform `0003` when present:

```text
0003=82, 0004=120, 0008=72 -> scale=(120, 72)
```

If only one axis-specific control is present, the missing axis falls back to
`0003` when available. This keeps uniform-only `0003` behavior intact while
preventing the `EFF_040` width/height controls from being ignored.

No matrix composition, pixel-center, sampler, or edge-policy tuning was made
beyond routing render/debug through the same current mapping helper.

## Turbulent Displace

Added `turbulent_displace_field_telemetry(input, params, time)` with:

- cloned raw params;
- resolved/effective amount, size, rounded complexity, evolution degrees,
  amplitude, and phase radians;
- fixed 3x3 probe samples with `noise`, `dx/dy`, displaced source UV, rounded
  sample pixel, and OOB flag;
- full-frame displacement-field hash and OOB count;
- explicit sampler and edge policy labels: `nearest_round` and
  `transparent_out_of_bounds`.

The existing sine turbulence approximation was not tuned. Render and telemetry
now share the same sample helper, and zero amount normalizes displacement to
`+0.0` so the displacement-field hash stays stable across evolution when the
effect is a pass-through.

## Tests

Host `cargo` is still unavailable:

```sh
cargo --version
# zsh:1: command not found: cargo
```

Docker unit tests:

```sh
docker run --rm \
  -e CARGO_HOME=/tmp/cargo_home \
  -e CARGO_TARGET_DIR=/tmp/ae_native_renderer_cargo_target \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p effects'
```

Result:

```text
30 passed; 0 failed
```

Additional check:

```sh
git diff --check -- crates/effects/src/geometry.rs crates/effects/src/turbulent_displace.rs
```

Result: no whitespace errors.

## Rerun Recommendations

`EFF_040`: rerun frame `0` first. The expected native diagnostic change is
Geometry2 resolving the fixture scale as `(120, 72)` instead of `(82, 82)`.
Use Geometry2 debug data to compare raw `0003/0004/0008`, matrices, sample UV,
and OOB count before touching matrix order, pixel-center convention, sampler,
or edge policy.

`EFF_060`: rerun frames `0`, `15`, `30`, and `45` only with field telemetry
captured or sampled from the helper. Check resolved evolution and field hashes
before any noise formula work. Final PNG diffs are still dependent output, not
a valid tuning target for M14.

`STK_030`: keep it as a composed regression until isolated Geometry2 and
Turbulent Displace are understood. When rerunning, capture Turbulent field
telemetry per selected frame and coordinate with the temporal stack work so
frame `0`/`1` differences are attributed to time routing rather than final
pixel guessing.

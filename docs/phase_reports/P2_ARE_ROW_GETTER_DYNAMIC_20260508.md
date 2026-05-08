# P2 ARE Row Getter Dynamic Trace — 2026-05-08

## Scope

Close the next P2 text-raster question with static-first + Frida evidence:

- validate the ARE row getter and sampler pipeline on AE85;
- extract direct row coverage bytes, not inferred PNG pixels;
- identify whether the remaining `COV_W` error is in the ARE integrator or in the upstream CoolType/outline producer.

## New Tooling

- `scripts/ae_trace_cooltype_text.py`
  - added `ARE.dll` hooks for:
    - `ARE_row_getter_8230`
    - `ARE_sampler_eval_row_75d0`
    - `ARE_sampler_prepare_76dc`
    - `ARE_edge_project_78e4`
    - `ARE_edge_insert_sorted_a850`
    - `ARE_edge_bounds_accumulate_b6c0`
    - `ARE_raster_lazy_row_b7e0`
  - added focused hook profiles:
    - `are-sampler`
    - `are-row-getter`
    - `are-sampler-core`
- `scripts/analyze_are_sampler_trace.py`
  - converts large Frida JSONL into compact sampler, edge, row getter, merged ink-row, and coverage-byte summaries.

## Static Follow-Up

Bundle:

```text
target/reverse/predecoded/20260508_163607_p2_are_sampler_followups
```

Key recovered functions:

- `ARE+0x6920`: active event cursor advance.
  - The cursor points into packed `int32` event streams.
  - It advances by `+4`; when it reaches `chunk+0x18`, it follows `chunk+0x08` and resets to the next chunk begin at `chunk+0x10`.
- `ARE+0xb944`: prepares active edges for a fixed16 scan row.
- `ARE+0x430c`: converts projected edge spans into x-event streams.
  - It uses `floor(projected_min)`.
  - It closes spans with `floor(projected_max) + 1`.
- `ARE+0x75d0`: evaluates one x column by integrating active coverage over 16 subrow buckets.
- `ARE+0xb7e0`: lazy row run evaluator; writes coverage bytes and classifies row state as empty, solid, or byte-run.

## Dynamic Traces

Sampler trace:

```text
target/dynamic_tools_85/p2_are_sampler_covw_20260508_001/COV_W.jsonl
target/dynamic_tools_85/p2_are_sampler_covw_20260508_001/are_sampler_analysis.json
```

Focused sampler-core trace:

```text
target/dynamic_tools_85/p2_are_sampler_core_covw_20260508_001/COV_W.jsonl
target/dynamic_tools_85/p2_are_sampler_core_covw_20260508_001/are_sampler_core_analysis.json
```

Direct row getter trace:

```text
target/dynamic_tools_85/p2_are_row_getter_covw_20260508_002/COV_W.jsonl
target/dynamic_tools_85/p2_are_row_getter_covw_20260508_002/are_row_getter_analysis.json
```

Derived native comparison:

```text
target/ae_agents/p2_cov_w_native_scene_floor_end_20260508
target/ae_agents/p2_row_compare_covw_floor_end_20260508
target/ae_agents/p2_row_compare_covw_are_row_getter_20260508
```

## Confirmed Runtime Facts

- `ARE_row_getter_8230` fired cleanly on AE85:
  - `68` enter events and `68` leave events for `COV_W`.
  - It emitted `202` merged ink rows over `109x68`.
  - First row coverage bytes are:

```text
2440404040404040404040404040403c
```

- `ARE_sampler_core` fired cleanly:
  - `ARE_raster_lazy_row_b7e0`: `93` enter events.
  - `ARE_sampler_eval_row_75d0`: `94` enter events.
  - `ARE_sampler_prepare_76dc`: `24` enter events.
- The sampler confirms the static formula:
  - y row prepares 16 buckets;
  - x columns are evaluated in fixed16 ranges;
  - coverage accumulates into `param_1 + 0x264`;
  - `0x100` means solid, `0` means empty, otherwise byte coverage.

## Native Change

`crates/text-engine/src/rasterize.rs` now uses the recovered ARE event-boundary rule:

```text
start_fixed = floor(projected_min * 16)
end_fixed   = floor(projected_max * 16) + 1
```

This replaces the previous rounded start/end fixed coordinate path.

## Metrics

Against the older dense span trace:

```text
COV_W merged ink shape:
  before: 179/203
  after:  180/202
```

Against the direct `ARE_row_getter_8230` rows:

```text
AE ink rows:      202
native rows:      202
common topology:  180/202
first row AE:     2440404040404040404040404040403c
first row native: 18303030303030303030303030303030
```

Focused text gate after the native change:

```text
target/ae_agents/p2_text_floor_end_gate_20260508/report.json
ok=true
```

The gate remains accepted by current thresholds, with small metric drift versus the previous ARE scanline pass:

```text
TXT_010 +0.0277 mean
TXT_020 +0.0014 mean
TXT_030 +0.0059 mean
TXT_040 +0.0180 mean
GPH_010 +0.0249 mean
```

## Conclusion

The ARE integrator is no longer the main unknown. We now have:

- static formula for event-stream integration;
- dynamic sampler confirmation;
- direct row getter golden rows;
- native implementation of the recovered event boundary rule.

The remaining `0x40` vs `0x30` coverage-byte gap is upstream of the row integrator: CoolType/BIB outline production, grid-fit/hinting, or edge-coordinate construction before ARE receives projected spans.

## Next Target

Focus below the text outline producer and above ARE event streams:

- trace/inspect `TXT_ARE_PathBuilder_40580` and its indirect callsites;
- trace/inspect BIB path object factory and resolved rasterizer pointers;
- hook `ARE+0x4a04` and `ARE+0x4a80` to capture the actual inserted x-event integers;
- compare native TTF outline edge coordinates against AE edge-event coordinates before coverage integration.

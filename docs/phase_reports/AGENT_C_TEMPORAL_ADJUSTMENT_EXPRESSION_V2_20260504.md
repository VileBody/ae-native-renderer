# Agent C Temporal / Adjustment / Expression Engine v2 Boundary

Status date: 2026-05-04

## Scope

Assigned objects: `M09`, `M15`, `M16`.

Write scope used:

- `crates/expression-engine/src/property.rs`
- `crates/expression-engine/src/lib.rs`
- this report

No geometry, text, Minimax, Turbulent Displace, or effect formula tuning was
changed.

## Posterize Time / M15

Current implementation already has true temporal routing guards:

- `ADBE Posterize Time` remains a stateless canvas no-op in
  `crates/effects/src/posterize_time.rs`.
- `render-core` computes posterized layer/source time before content/effect
  evaluation for normal layers.
- `TemporalTraceRecord` logs `comp_time`, `layer_time`, `posterized_time`,
  `source_time`, `source_frame_id`, and posterize bucket metadata.

Existing tests that keep this behavior testable:

- `posterize_time_quantizes_layer_sampling_time`
- `temporal_trace_records_posterized_footage_source_time`
- `motion_blur_trace_records_phase_offset_and_posterized_sample_times`

Ghidra note retained from the prior Agent C pass: `Posterize_Time.aex` shows
floor/trunc bucket behavior in PF time units. The renderer's `1e-9` tolerance is
a seconds-space boundary guard, not recovered AE formula evidence.

## Adjustment Routing / M16

Observed and current routing order:

```text
render bottom layers normally into accumulated canvas
for an adjustment layer:
  first_pt = first ADBE Posterize Time in adjustment.effects
  lower_stack_time = first_pt.bucket_time if present else comp_time
  input_canvas = render lower stack at lower_stack_time
  for each effect in authored order:
    if effect_index <= first_pt.index:
      param_time = first_pt.bucket_time
    else:
      param_time = comp_time
    output_canvas = effect.render(input_canvas, param_time)
```

This is the key STK_030 guard: Posterize freezes the adjustment input/lower
stack bucket, but downstream effects after Posterize Time keep live parameter
time. The old routing-level over-posterize symptom is therefore covered by
`adjustment_posterize_keeps_downstream_param_time_live_inside_bucket`.

Telemetry checkpoints required before formula tuning composed STK cases:

- adjustment layer record: `comp_time`, `layer_time`, `lower_stack_time`;
- per effect index: `match_name`, `param_time`, `input_hash`, `output_hash`;
- Posterize effect record: `frame_rate`, `bucket`, `bucket_time`;
- layer/source records: `posterized_time`, `source_time`, `source_frame_id`;
- optional effect debug records: effect-local evaluated params and stage hashes.

Sidecars already exposed by sequence rendering:

- `temporal_telemetry.jsonl`
- `adjustment_effects.jsonl`
- `expression_telemetry.jsonl`

## Expression Engine v2 Boundary / M09

Added a property-expression skeleton in `expression-engine`:

- `PropertyExpressionEvaluator` trait;
- `BoundaryPropertyExpressionEvaluator`;
- `PropertyExpressionHost` trait for host-owned access;
- `PropertyValueType` and coercion helper for scalar/Vec2/Vec3 targets;
- context seeding for `time`, `value`, `thisComp.width/height/duration`,
  `thisComp.frameDuration`, `thisLayer.inPoint/outPoint/startTime/index`;
- explicit `valueAtTime(...)` boundary that requires a host property sampler;
- named/fingerprint fallback hook for template-scoped shortcuts such as
  `edge_wobble`;
- no arbitrary JavaScript execution.

The host trait is intentionally a boundary. If `valueAtTime` has no host
sampler, evaluation returns a `BLOCKER` error instead of inventing a formula.
This preserves the prior BEE/Scripting guardrail: generic Expression Engine v2
depends on AE's time/property host (`BEE_FpLongToTime`, `BEE_CompToLayerTime`,
TDB/time suites), not just a parser.

New tests make the boundary testable:

- parsed subset with `value`, `thisComp`, `thisLayer`;
- `valueAtTime(time - thisComp.frameDuration)` through a host sampler;
- explicit host-missing `BLOCKER`;
- host-scoped named fingerprint fallback;
- scalar target rejects implicit vector component picking.

## Blockers

```text
BLOCKER:
  title: Generic AE Expression Engine v2 still requires BEE/Scripting time/property host
  affected objects: M09, future generic property expressions, valueAtTime, thisLayer/thisComp property access beyond seeded numeric fields
  evidence paths:
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/09_Scripting_PTR_GetTimePalSuite_180e16918/data_target.md
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/10_Scripting_PTR_TDB_SecondsToTime_180e16958/data_target.md
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/11_Scripting_PTR_BEE_FpLongToTime_180e16cb0/data_target.md
    - target/reverse/predecoded/20260504_222748_full_predecode_round2/scripting_expression_host/14_Scripting_PTR_BEE_CompToLayerTime_180e16f60/data_target.md
  exact address/function:
    - Scripting.aex .rdata 0x180e16918 PTR_GetTimePalSuite
    - Scripting.aex .rdata 0x180e16958 PTR_TDB_SecondsToTime
    - Scripting.aex .rdata 0x180e16cb0 PTR_BEE_FpLongToTime
    - Scripting.aex .rdata 0x180e16f60 PTR_BEE_CompToLayerTime
  what assumption broke: property expressions cannot be modeled as arbitrary JS over a plain comp-time scalar
  smallest probe/test needed: AE property sample export for valueAtTime and nonzero layer start/inPoint/outPoint
  can continue on unrelated work: yes
  can continue named edge_wobble shortcut: yes
```

## Recommendation

`M15`/`M16`: no current routing blocker; keep tests and telemetry sidecars as
guards while other effect formulas tune STK_030.

`M09`: use the new property-expression boundary for native subset expansion and
named/fingerprint fallback. Do not claim arbitrary Expression Engine v2 parity
until the BEE/Scripting host contract is recovered or probed.

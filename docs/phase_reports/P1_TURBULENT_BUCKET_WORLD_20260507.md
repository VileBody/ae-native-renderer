# P1 Turbulent Bucket World Routing 2026-05-07

## Scope

This report closes the visible `STK_030` spike that remained after Geometry2,
Minimax, Posterize Time, and isolated Turbulent Displace were already low-error.

Owned modules:

- `M14` Turbulent Displace
- `M15` Posterize Time
- `M16` adjustment layer routing

## Evidence

Frida target:

```text
http://85.239.48.31:8001
TurbulentDisplace.aex+0x3410
target/dynamic_tools_85/turbulent_stk030_setup_20260507_r2/STK_030.jsonl
```

The useful trace captured 60 valid `turbulent_param_setup_cmd11_3410` leaves.
The first low-event trace installed the hook but saturated before render; the
rerun used a larger event budget and kept the hook narrow.

Recovered runtime rule:

```text
Posterize bucket boundary frames: full 512x512 world, offset [256, 256]
Inside a Posterize bucket: cropped 298x298 world, offset [149, 149]
```

For `STK_030`, the full-world frames line up with the 6fps Posterize boundary
inside a 30fps comp:

```text
0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55
```

This means the remaining spike was not the field kernel. It was the world passed
to downstream Turbulent after Posterize.

## Implementation

Changed native adjustment routing:

- `ADBE Turbulent Displace` uses full canvas when `lower_stack_time == comp_time`.
- After a preceding `ADBE Posterize Time`, when `lower_stack_time != comp_time`,
  Turbulent receives an active-alpha crop.
- The crop is expanded by the recovered AE extent grow. For `STK_030` params,
  grow is `3px`, yielding the Frida-observed `298x298` world.

Added regression coverage:

```text
adjustment_turbulent_uses_full_world_on_posterize_bucket_boundary
extent_grow_matches_stk030_frida_runtime_world
```

## Results

Focused conformance:

| Case | Primary Visible Mean | Notes |
| --- | ---: | --- |
| `EFF_060` | `0.166828` | still accepted, max diff `2` |
| `STK_030` | `0.332818` | selected frames flattened around `0.32..0.34` |
| `STK_030_S04_TURBULENT` | `0.317022` | frame `0` fixed from cropped-world regression |

Master gate:

```text
target/ae_agents/p1_turbulent_bucket_world_gate_20260507
case_count=19
accepted=2
approximate=17
regression_count=0
missing_count=0
```

## Status

`P1 / scenes_3rd` Turbulent world-routing blocker is resolved for visible RGB.
`EFF_060` is accepted; `STK_030` is now low-error approximate instead of a
frame-spiking blocker.

Remaining visible dashboard residuals are no longer owned by this blocker. Raw
RGBA/background-alpha residuals should continue to be handled under `M19` and
effect-local alpha policy, not by tuning Turbulent final pixels.

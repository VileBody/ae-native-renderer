# Agent Orchestration Status

Status date: 2026-05-04

Source TZ:

```text
docs/phase_reports/MATH_OBJECT_AGENT_TZ_20260504.md
```

Predecoded evidence root:

```text
target/reverse/predecoded/20260504_222748_full_predecode_round2
```

## Agent Roster

| Agent | Runtime id | Objects | Write scope | Status |
| --- | --- | --- | --- | --- |
| Agent A / Substrate | `019df498-3fbe-73a1-93b7-6e47b49a8d30` | `M01`, `M02`, `M19` | `docs/phase_reports/AGENT_A_SUBSTRATE_MATH_OBJECTS_20260504.md` | completed; global blocker reported |
| Agent B / Geometry Motion | `019df498-3ffd-7a60-828e-a9f499bb6a23` | `M03`, `M04`, `M12`, `M17`, `M18` | `docs/phase_reports/AGENT_B_GEOMETRY_MOTION_MATH_OBJECTS_20260504.md` | completed; needs probes |
| Agent C / Temporal Expression | `019df498-406d-7590-87b9-1bf9734173b2` | `M09`, `M15`, `M16` | `docs/phase_reports/AGENT_C_TEMPORAL_EXPRESSION_MATH_OBJECTS_20260504.md` | completed; scoped blocker reported |
| Agent D / Text Glyph | `019df498-4273-7502-aaf1-b02d2e3109a8` | `M05`, `M06`, `M07`, `M08` | `docs/phase_reports/AGENT_D_TEXT_GLYPH_MATH_OBJECTS_20260504.md` | completed; blocker reported |
| Agent E / Blur Shadow Glow | `019df498-4368-7800-a18d-1b415d78e7a3` | `M10`, `M11`, shared `M19` blur | `docs/phase_reports/AGENT_E_BLUR_SHADOW_GLOW_MATH_OBJECTS_20260504.md` | completed; blockers reported |
| Agent F / Minimax Turbulent | `019df498-4486-71c2-aded-e157f7775b29` | `M13`, `M14` | `docs/phase_reports/AGENT_F_MINIMAX_TURBULENT_MATH_OBJECTS_20260504.md` | completed; blocker reported |

## Orchestrator Rules

- Do not accept formula tuning until the object passport has evidence paths,
  exact addresses, confidence labels, and explicit cross-object dependencies.
- If an agent reports `BLOCKER`, resolve or narrow it before allowing dependent
  agents to tune formulas.
- Treat `M19` substrate, time semantics, sampling/OOB, and param remaps as
  global-risk issues.

## Current Blockers

| Blocker | Source | Affected objects | Status |
| --- | --- | --- | --- |
| CoolType-dependent glyph metrics: `Basic_Text.aex` delegates glyph IDs, widths, bboxes, baselines, feature processing, and raster warnings to CoolType text/font interfaces. Native fontdue/simple layout cannot be honestly tuned to AE glyph parity without a CoolType/CoreText-like metrics plan or stronger AE glyph probes. | Agent D report, `basic_text_aex/04_BasicText_candidate_19e10_180019e10/decompile.c`, `basic_text_aex/07_BasicText_candidate_1e630_18001e630/decompile.c` | `M05`, also `M06`, `M07`, `M17` text sharpness | Open; needs orchestrator acceptance/probe plan |
| Text animator blur depends on global alpha/premult/composite substrate. | Agent D report, `docs/phase_reports/AGENT_ROUND5_TEXT_ANIMATOR_BLUR.md` | `M07`, `M19` | Routed to Agent A substrate contract |
| Generic Expression Engine v2 needs BEE/Scripting time host contract, not only named/fingerprint subset plus comp-time scalar. | Agent C report, `scripting_expression_host/09_*`, `10_*`, `11_*`, `14_*` data targets | `M09`, future `M08`/generic expressions | Scoped blocker; current named `edge_wobble` and bounce shortcuts may continue as template-scoped approximations |
| `M19` straight `RGBA8` / normal composite contract is not locked. Shared substrate has unpremultiply, packed/unpacked alpha, source/destination alpha types, alpha-only blur, pixel-format mapping, and component-depth paths. | Agent A report, `GF::Composite`, `GF::AlphaGain`, `GF::Unpremultiply`, `GF::BlendUnpackedAlpha`, `BoxBlurOptions`, `ImageRenderer` pixel-format map | `M01`, `M07`, `M10`, `M11`, `M13`, `M17`, `M18`, final template thresholds | Global blocker; run alpha-ramp/composite probe plus blur alpha-policy probe and text blur variant before alpha-sensitive formula tuning |
| Turbulent Displace final vector/sampler math is inside hidden GPU kernels `TurbulentDisplaceFracAllKernel` and `TurbulentDisplaceFrac1DKernel`. | Agent F report, `turbulent_displace_aex/06_Turbulent_render_18000ad10/decompile.c`, `data_refs.tsv` | `M14`, plus `M19` sampler/OOB and `M15/M16` when animated | Blocked for formula tuning; use coordinate-field probes or kernel extraction before replacing field math |
| Glow `Glow Based On` enum may be reversed relative to current native assumption. AEX popup string order implies `1=Alpha Channel`, `2=Color Channels`, while current native notes say the opposite. | Agent E report, `Glow.aex` string `Alpha Channel|Color Channels`, `glow_aex/03_Glow_render_candidate_5f50_180005f50/decompile.c` | `M11` | Blocked for Glow threshold tuning; run two-pixel `0001=1/2` AE probe |
| Drop Shadow final composite goes through hidden effect kernel `DropShadow` / `CompositeShadowMask`. | Agent E report, `drop_shadow_aex/01_DropShadow_core_candidate_5bb0_180005bb0` | `M10`, `M19` | Mask/softness can be probed; final color/composite blocked until M19 or intermediate probe |

## Accepted Working Contracts

- `M15` Posterize Time: use PF-style floor/trunc bucket model with fixed16 FPS. Current `+1e-9` is native tolerance, not proven AE epsilon.
- `M16` adjustment routing: first Posterize Time in adjustment stack freezes lower-stack input at bucket time; downstream effects after Posterize evaluate params at live comp time. Old `STK_030` over-posterize issue is treated as resolved at routing level.
- `M03`/`M12` transform matrix: keep recovered matrix order and inverse-matrix sampling. Treat payload numeric keys as AE property indices: payload `"0008"` is Rotation, while matchName `ADBE Geometry2-0008` is Opacity.
- `M13` Minimax: operation/channel mapping and CPU callback shape are strong enough for an object passport; Direction enum, fractional radius, edge-shrink policy, and GPU/CPU path parity still need probes.
- Shared blur: `GF::FastBoxBlur` delegates are confirmed for Box Blur and Drop Shadow softness. Radius quantization is ceil-like in kernel evidence; dimensions/repeat-edge/alpha options still need probes.
- `M10` Drop Shadow: offset sign convention remains consistent with `dx=trunc(-cos(theta)*distance)`, `dy=trunc(sin(theta)*distance)` for the known Round 5 case; softness likely maps through `GF::FastBoxBlur` alpha-only with observed `18 -> radius 10`.

## Needs-Probe Queue

- `M03`/`M12`: Geometry2 sampler/OOB/pixel-center, Geometry2 opacity, Sampling/Bicubic UI mapping.
- `M04`: AE Bezier/ease property sample export before changing interpolation formulas.
- `M17`: deferred text/vector raster primitive contract and collapse barrier probes.
- `M18`: motion blur sample endpoints, weights, effect shutter override behavior.
- `M13`: Minimax fractional radius, Direction enum, Don't Shrink Edges, bpc/GPU path parity.
- `M14`: Turbulent coordinate-field, complexity split, evolution/cycle/seed, pinning/resize/AA probes or kernel extraction.
- `M10`: Drop Shadow direction grid, fractional offset truncation, softness scale, opacity units, raw/blurred/final mask telemetry.
- `M11`: Glow based-on enum, threshold source, `IR_GaussianBlur` radius mapping, operation/composite route.
- Shared blur: Box Blur fractional radius, dimensions H/V, repeat-edge, edge/corner impulse behavior.

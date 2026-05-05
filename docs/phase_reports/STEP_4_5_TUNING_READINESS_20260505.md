# Step 4.5 Tuning Readiness / Evidence Lock

Generated: 2026-05-05.

Step 4.5 is the bridge between broad instrumentation and real formula tuning.
It freezes the current conformance evidence, names the allowed knobs per module,
and records which substrate facts must not be guessed from final PNGs.

Source artifacts:

- `target/ae_agents/step4_math_after_current/report.json`
- `fixtures/ae_conformance_pack/manifest.json`
- `docs/reverse_engineering/MATH_CONTRACTS_GUARDRAILS.md`

## Global Locks

| Contract | Step 4.5 decision | Guardrail |
| --- | --- | --- |
| Alpha/premult | Diagnostic-only; `rgb_straight_source_over_ae_background` is the preferred effects metric until M19 is locked. | Do not tune Drop Shadow/Glow/blur from raw RGBA alone. |
| Text | SourceRect-based AE refs are valid for layout/advance/bbox tuning. | Do not claim 1:1 CoolType raster coverage until deeper glyph-id/coverage probes exist. |
| Time | TMP_010/TMP_020 are exact in current run, but boundary semantics are not parity locked. | Do not change downstream adjustment timing while tuning Posterize buckets. |
| Warps | STK_030 has native checkpoints for Geometry2/Minimax/Turbulent. | Do not tune field math from final stack pixels without coordinate/field refs. |
| Collapse | Matrix reports exist; deferred text/vector raster is not locked. | Do not hide collapsed-text blur with raster sharpness hacks. |

## Module Gateboard

| Module | State | Measured cases | Missing pack cases | Primary gate | Blocker |
| --- | --- | --- | --- | --- | --- |
| `M01` Layer activity / compositing | needs dedicated evidence | - | `PRI_010` | PRI_010/CMP_010 prove layer opacity/source-over/background behavior before effect tuning depends on it | PRI_010 was not in the Step 4 integrated run; M19 premult/straight is still diagnostic-only |
| `M02` Footage/source-time sampling | diagnostic-ready | `TMP_010` | - | source_frame.index/time/subframe must match numbered-frame AE refs | needs numbered-frame source passport on non-trivial source_start cases |
| `M03` 2D transform matrix / sampler | needs dedicated evidence | - | - | coordinate-field transform passport proves matrix/pixel-center/OOB before Geometry2 or collapse tuning | no dedicated M03 case is measured in the Step 4 integrated run; use EFF_040-style coordinate-field probes |
| `M04` Keyframes / Bezier ease | evidence-ready, not tuning-ready | `EFF_070`<br>`INT_020` | `INT_010` | INT_020 keyframe_sample records improve without worsening TMP/Motion cases | AE tangent/influence telemetry is still missing; final pixels alone are too indirect |
| `M05` Text glyph layout / sourceRect subset | partial tuning-ready | `GPH_010`<br>`TXT_010`<br>`TXT_020`<br>`TXT_030` | - | text_passport.max_abs_delta and total_mismatches decrease on TXT_010..TXT_040 | CoolType raster coverage/glyph ids are not yet probed; Montserrat instance mapping remains open |
| `M06` Range Selector reveal | evidence-ready, not tuning-ready | `TXT_010`<br>`TXT_020` | - | selector-unit boundaries/weights move independently from glyph layout drift | needs focused AE selector boundary refs after M05 layout facts are stable |
| `M07` Glyph animator transform/blur | evidence-ready, not tuning-ready | `TXT_030` | - | per-glyph matrix/opacity/blur telemetry improves before final text blur pixels | text blur and opacity are substrate-dependent on M19; exact glyph metrics still depend on M05 |
| `M08` Expression selector bounce | evidence-ready, not tuning-ready | `TXT_040` | - | bounce selector amount curve matches AE samples without perturbing static text layout | needs AE selector amount samples; current refs mostly expose layout/sourceRect fields |
| `M09` Property expression subset | evidence-ready, not tuning-ready | `EXP_010`<br>`TXT_040` | - | EXP_010 expression telemetry and final motion improve without changing timeline sampling | needs property sample refs for generated expressions and expression-selector amount curves |
| `M10` Drop Shadow / Box Blur dependency | blocked on M19 + intermediate refs | `EFF_010`<br>`EFF_030`<br>`EFF_070` | `STK_010`<br>`STK_020` | EFF_010/EFF_030/EFF_070 improve on rgb_under_alpha_policy with stable alpha stats | Drop Shadow final composite path and alpha policy are not locked |
| `M11` Glow | blocked on M19 + enum/probe refs | `EFF_020`<br>`EFF_070` | - | EFF_020/EFF_070 rgb_under_alpha_policy improves after BasedOn/source threshold is locked | Glow Based On enum and IR_GaussianBlur/composite route need direct evidence |
| `M12` Geometry2 | evidence-ready, needs isolated run | `STK_030` | `EFF_040` | coordinate-field UV/matrix sidecars match before STK_030 final pixels are tuned | EFF_040 isolated coordinate-field case was not in the Step 4 integrated run |
| `M13` Minimax | evidence-ready, needs isolated refs | `STK_030` | `EFF_050`<br>`STK_020` | EFF_050/STK_020/STK_030 direction/radius checkpoints improve before stack pixels | fractional radius and Direction enum AE refs are still missing from current run |
| `M14` Turbulent Displace | evidence-ready, not tuning-ready | `EFF_070`<br>`STK_030` | `EFF_060` | field_hash/sample grid moves toward AE coordinate-field refs before final pixels | needs field-level AE refs or kernel-derived field maps; EFF_060 was not in integrated run |
| `M15` Posterize Time | partial tuning-ready | `STK_030`<br>`TMP_010`<br>`TMP_020` | - | TMP_020 remains exact; STK_030 source/effect times show correct bucket/live split | boundary epsilon and adjustment stack ordering still need AE micro refs |
| `M16` Adjustment stack order | evidence-ready, not tuning-ready | `STK_030` | - | per-effect input/output hashes localize first divergent adjustment effect in STK_030 | needs AE-side intermediate/checkpoint refs for STK_030 |
| `M17` Collapse transformations / precomp graph | evidence-ready, not tuning-ready | `GPH_010` | - | GPH_010 matrix_report and deferred-raster checkpoints match collapsed/noncollapsed AE refs | true AE deferred text/vector rasterization refs are still missing |
| `M18` Motion blur | evidence-ready, not tuning-ready | `TMP_030` | - | TMP_030 shutter sample times/weights match AE before accumulation/composite tuning | AE shutter sample telemetry/weights are missing; premult accumulation depends on M19 |
| `M19` Color/alpha/sampling/gamma substrate | first tuning target | `CMP_010`<br>`EFF_010`<br>`EFF_020`<br>`EFF_030`<br>`EFF_070`<br>`EXP_010`<br>`GPH_010`<br>`INT_020`<br>`STK_030`<br>`TMP_010`<br>`TMP_020`<br>`TMP_030`<br>`TXT_010`<br>`TXT_020`<br>`TXT_030`<br>`TXT_040` | `PRI_010`<br>`STK_010`<br>`STK_020` | alpha_policy_diagnostics stable; rgb_under_alpha_policy is used for effect tuning until premult is locked | premult/straight contract is still diagnostic-only |

## Tuning Packets

### `M01` Layer activity / compositing

- Lane: global substrate
- State: needs dedicated evidence
- Primary gate: PRI_010/CMP_010 prove layer opacity/source-over/background behavior before effect tuning depends on it
- Allowed knobs: activity boundaries, z-order application, layer opacity scaling, normal source-over implementation
- Forbidden in this module: effect-specific alpha hacks, text/effect formula changes
- Blockers: PRI_010 was not in the Step 4 integrated run; M19 premult/straight is still diagnostic-only

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |

### `M02` Footage/source-time sampling

- Lane: temporal substrate
- State: diagnostic-ready
- Primary gate: source_frame.index/time/subframe must match numbered-frame AE refs
- Allowed knobs: source time offset, frame-index rounding, activity-window boundary handling
- Forbidden in this module: effect formula changes, global alpha/composite policy changes
- Blockers: needs numbered-frame source passport on non-trivial source_start cases

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `TMP_010` | 6 | 0 | 0 | 0 | 0 | 0 | 0 | temporal:4, text_passport |

### `M03` 2D transform matrix / sampler

- Lane: geometry substrate
- State: needs dedicated evidence
- Primary gate: coordinate-field transform passport proves matrix/pixel-center/OOB before Geometry2 or collapse tuning
- Allowed knobs: anchor/position/scale/rotation order, inverse sampling convention, pixel center, edge/OOB policy
- Forbidden in this module: Geometry2-specific parameter remaps, collapse text raster hacks
- Blockers: no dedicated M03 case is measured in the Step 4 integrated run; use EFF_040-style coordinate-field probes

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |

### `M04` Keyframes / Bezier ease

- Lane: temporal math
- State: evidence-ready, not tuning-ready
- Primary gate: INT_020 keyframe_sample records improve without worsening TMP/Motion cases
- Allowed knobs: Bezier influence/speed mapping, hold/linear/ease segment selection, scalar/vector coercion
- Forbidden in this module: changing source-frame quantization, changing transform matrix convention
- Blockers: AE tangent/influence telemetry is still missing; final pixels alone are too indirect

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `EFF_070` | 6 | 1.5218 | 1.6587 | 0.7011 | 123 | 0 | 0 | temporal:12, effects:12, text_passport |
| `INT_020` | 7 | 1.4099 | 0.4064 | 0.1932 | 250 | 0 | 0 | temporal:18, text_passport |

### `M05` Text glyph layout / sourceRect subset

- Lane: text
- State: partial tuning-ready
- Primary gate: text_passport.max_abs_delta and total_mismatches decrease on TXT_010..TXT_040
- Allowed knobs: font face mapping, advance/bbox/baseline math, multiline block placement, composer whitespace handling
- Forbidden in this module: tuning text pixels against full PNG before glyph/layout deltas improve
- Blockers: CoolType raster coverage/glyph ids are not yet probed; Montserrat instance mapping remains open

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `GPH_010` | 4 | 14.5753 | 15.0351 | 15.7899 | 255 | 0 | 0 | temporal:12, text:8, collapse:12, text_passport |
| `TXT_010` | 7 | 11.8254 | 8.8158 | 9.7589 | 255 | 2548 | 24.996 | temporal:7, text:14, text_passport |
| `TXT_020` | 7 | 9.7088 | 7.297 | 8.2232 | 255 | 4046 | 17.864 | temporal:14, text:28, text_passport |
| `TXT_030` | 7 | 14.4961 | 6.4386 | 8.1551 | 255 | 1092 | 32.7085 | temporal:7, text:14, text_passport |

### `M06` Range Selector reveal

- Lane: text selector
- State: evidence-ready, not tuning-ready
- Primary gate: selector-unit boundaries/weights move independently from glyph layout drift
- Allowed knobs: BasedOn grouping, Start/End boundary rounding, shape/smoothness/randomize/wiggly semantics
- Forbidden in this module: using glyph advance hacks to hide selector ordering bugs
- Blockers: needs focused AE selector boundary refs after M05 layout facts are stable

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `TXT_010` | 7 | 11.8254 | 8.8158 | 9.7589 | 255 | 2548 | 24.996 | temporal:7, text:14, text_passport |
| `TXT_020` | 7 | 9.7088 | 7.297 | 8.2232 | 255 | 4046 | 17.864 | temporal:14, text:28, text_passport |

### `M07` Glyph animator transform/blur

- Lane: text animator
- State: evidence-ready, not tuning-ready
- Primary gate: per-glyph matrix/opacity/blur telemetry improves before final text blur pixels
- Allowed knobs: glyph transform origin, opacity contribution, blur radius/kernel approximation, selector weighting
- Forbidden in this module: global alpha/premult changes inside text animator code
- Blockers: text blur and opacity are substrate-dependent on M19; exact glyph metrics still depend on M05

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `TXT_030` | 7 | 14.4961 | 6.4386 | 8.1551 | 255 | 1092 | 32.7085 | temporal:7, text:14, text_passport |

### `M08` Expression selector bounce

- Lane: text expression selector
- State: evidence-ready, not tuning-ready
- Primary gate: bounce selector amount curve matches AE samples without perturbing static text layout
- Allowed knobs: delay, frequency, decay, per-character phase/order
- Forbidden in this module: arbitrary JS evaluator expansion in this pass
- Blockers: needs AE selector amount samples; current refs mostly expose layout/sourceRect fields

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `TXT_040` | 8 | 4.2157 | 3.774 | 3.871 | 255 | 1560 | 22.125 | temporal:8, text:16, text_passport |

### `M09` Property expression subset

- Lane: expressions
- State: evidence-ready, not tuning-ready
- Primary gate: EXP_010 expression telemetry and final motion improve without changing timeline sampling
- Allowed knobs: named evaluator traits, scalar/vector coercion, thisLayer/thisComp access, wobble envelope parameters
- Forbidden in this module: unbounded ExtendScript execution, changing layer time globally
- Blockers: needs property sample refs for generated expressions and expression-selector amount curves
- Missing required sidecars: `{"TXT_040": ["expression"]}`

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `EXP_010` | 8 | 1.3198 | 1.2887 | 1.3416 | 255 | 0 | 0 | temporal:8, expr:8, text_passport |
| `TXT_040` | 8 | 4.2157 | 3.774 | 3.871 | 255 | 1560 | 22.125 | temporal:8, text:16, text_passport |

### `M10` Drop Shadow / Box Blur dependency

- Lane: effects
- State: blocked on M19 + intermediate refs
- Primary gate: EFF_010/EFF_030/EFF_070 improve on rgb_under_alpha_policy with stable alpha stats
- Allowed knobs: shadow offset sign/truncation, softness-to-radius mapping, box blur passes, shadow mask construction
- Forbidden in this module: raw RGBA-only tuning, global alpha policy edits inside Drop Shadow
- Blockers: Drop Shadow final composite path and alpha policy are not locked

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `EFF_010` | 1 | 0.3428 | 0.1131 | 0.8912 | 104 | 0 | 0 | temporal:1, effects:1, text_passport |
| `EFF_030` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | temporal:1, effects:1, text_passport |
| `EFF_070` | 6 | 1.5218 | 1.6587 | 0.7011 | 123 | 0 | 0 | temporal:12, effects:12, text_passport |

### `M11` Glow

- Lane: effects
- State: blocked on M19 + enum/probe refs
- Primary gate: EFF_020/EFF_070 rgb_under_alpha_policy improves after BasedOn/source threshold is locked
- Allowed knobs: Glow Based On enum, threshold source, blur radius/intensity mapping, blend/composite route
- Forbidden in this module: tuning intensity before BasedOn enum is resolved, raw RGBA-only tuning
- Blockers: Glow Based On enum and IR_GaussianBlur/composite route need direct evidence

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `EFF_020` | 1 | 9.3509 | 11.1764 | 1.745 | 137 | 0 | 0 | temporal:1, effects:1, text_passport |
| `EFF_070` | 6 | 1.5218 | 1.6587 | 0.7011 | 123 | 0 | 0 | temporal:12, effects:12, text_passport |

### `M12` Geometry2

- Lane: warps
- State: evidence-ready, needs isolated run
- Primary gate: coordinate-field UV/matrix sidecars match before STK_030 final pixels are tuned
- Allowed knobs: AE property mapping, matrix order, sampler quality, edge/OOB policy
- Forbidden in this module: tuning Geometry2 from STK_030 final PNG only, changing M03 layer transform semantics
- Blockers: EFF_040 isolated coordinate-field case was not in the Step 4 integrated run

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `STK_030` | 9 | 59.2163 | 40.0527 | 108.8 | 255 | 0 | 0 | temporal:27, adjust:36, effects:27, text_passport |

### `M13` Minimax

- Lane: morphology
- State: evidence-ready, needs isolated refs
- Primary gate: EFF_050/STK_020/STK_030 direction/radius checkpoints improve before stack pixels
- Allowed knobs: operation/channel enum, direction enum, fractional radius, Don't Shrink Edges behavior
- Forbidden in this module: Turbulent/Geometry changes in a Minimax patch
- Blockers: fractional radius and Direction enum AE refs are still missing from current run

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `STK_030` | 9 | 59.2163 | 40.0527 | 108.8 | 255 | 0 | 0 | temporal:27, adjust:36, effects:27, text_passport |

### `M14` Turbulent Displace

- Lane: procedural field
- State: evidence-ready, not tuning-ready
- Primary gate: field_hash/sample grid moves toward AE coordinate-field refs before final pixels
- Allowed knobs: FracAll/Frac1D state model, evolution/size/amount mapping, octaves/complexity, pinning/OOB
- Forbidden in this module: final PNG-only tuning, changing Posterize Time routing in Turbulent patch
- Blockers: needs field-level AE refs or kernel-derived field maps; EFF_060 was not in integrated run

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `EFF_070` | 6 | 1.5218 | 1.6587 | 0.7011 | 123 | 0 | 0 | temporal:12, effects:12, text_passport |
| `STK_030` | 9 | 59.2163 | 40.0527 | 108.8 | 255 | 0 | 0 | temporal:27, adjust:36, effects:27, text_passport |

### `M15` Posterize Time

- Lane: temporal routing
- State: partial tuning-ready
- Primary gate: TMP_020 remains exact; STK_030 source/effect times show correct bucket/live split
- Allowed knobs: bucket boundary rounding, fixed16 fps conversion, layer/source/effect time routing
- Forbidden in this module: posterizing downstream effects that AE evaluates at live comp time
- Blockers: boundary epsilon and adjustment stack ordering still need AE micro refs

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `STK_030` | 9 | 59.2163 | 40.0527 | 108.8 | 255 | 0 | 0 | temporal:27, adjust:36, effects:27, text_passport |
| `TMP_010` | 6 | 0 | 0 | 0 | 0 | 0 | 0 | temporal:4, text_passport |
| `TMP_020` | 12 | 0 | 0 | 0 | 0 | 0 | 0 | temporal:12, text_passport |

### `M16` Adjustment stack order

- Lane: render graph/effects
- State: evidence-ready, not tuning-ready
- Primary gate: per-effect input/output hashes localize first divergent adjustment effect in STK_030
- Allowed knobs: lower-stack resampling point, effect application order, adjustment input snapshot boundaries
- Forbidden in this module: changing individual effect formulas while validating stack order
- Blockers: needs AE-side intermediate/checkpoint refs for STK_030

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `STK_030` | 9 | 59.2163 | 40.0527 | 108.8 | 255 | 0 | 0 | temporal:27, adjust:36, effects:27, text_passport |

### `M17` Collapse transformations / precomp graph

- Lane: graph/collapse
- State: evidence-ready, not tuning-ready
- Primary gate: GPH_010 matrix_report and deferred-raster checkpoints match collapsed/noncollapsed AE refs
- Allowed knobs: matrix pushdown, collapse boundary selection, text/vector deferred rasterization
- Forbidden in this module: raster sharpness hacks before deferred-raster contract is known
- Blockers: true AE deferred text/vector rasterization refs are still missing

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `GPH_010` | 4 | 14.5753 | 15.0351 | 15.7899 | 255 | 0 | 0 | temporal:12, text:8, collapse:12, text_passport |

### `M18` Motion blur

- Lane: temporal sampling
- State: evidence-ready, not tuning-ready
- Primary gate: TMP_030 shutter sample times/weights match AE before accumulation/composite tuning
- Allowed knobs: sample endpoints, weight distribution, shutter phase/angle mapping, static-layer skip
- Forbidden in this module: changing transform math while tuning motion accumulation
- Blockers: AE shutter sample telemetry/weights are missing; premult accumulation depends on M19

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `TMP_030` | 6 | 0.1128 | 0.1053 | 0.1148 | 155 | 0 | 0 | temporal:102, text_passport |

### `M19` Color/alpha/sampling/gamma substrate

- Lane: global substrate
- State: first tuning target
- Primary gate: alpha_policy_diagnostics stable; rgb_under_alpha_policy is used for effect tuning until premult is locked
- Allowed knobs: straight/premult conversion, background alpha normalization, source-over math, gamma/color-space decision, sampler edge policy
- Forbidden in this module: module-specific formula hacks to compensate global substrate drift
- Blockers: premult/straight contract is still diagnostic-only

| Case | Frames | RGBA mean | RGB-under-alpha mean | Alpha mean | Max | Text mismatches | Text max delta | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `CMP_010` | 1 | 4.3968 | 5.8625 | 0 | 188 | 0 | 0 | temporal:3, text_passport |
| `EFF_010` | 1 | 0.3428 | 0.1131 | 0.8912 | 104 | 0 | 0 | temporal:1, effects:1, text_passport |
| `EFF_020` | 1 | 9.3509 | 11.1764 | 1.745 | 137 | 0 | 0 | temporal:1, effects:1, text_passport |
| `EFF_030` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | temporal:1, effects:1, text_passport |
| `EFF_070` | 6 | 1.5218 | 1.6587 | 0.7011 | 123 | 0 | 0 | temporal:12, effects:12, text_passport |
| `EXP_010` | 8 | 1.3198 | 1.2887 | 1.3416 | 255 | 0 | 0 | temporal:8, expr:8, text_passport |
| `GPH_010` | 4 | 14.5753 | 15.0351 | 15.7899 | 255 | 0 | 0 | temporal:12, text:8, collapse:12, text_passport |
| `INT_020` | 7 | 1.4099 | 0.4064 | 0.1932 | 250 | 0 | 0 | temporal:18, text_passport |
| `STK_030` | 9 | 59.2163 | 40.0527 | 108.8 | 255 | 0 | 0 | temporal:27, adjust:36, effects:27, text_passport |
| `TMP_010` | 6 | 0 | 0 | 0 | 0 | 0 | 0 | temporal:4, text_passport |
| `TMP_020` | 12 | 0 | 0 | 0 | 0 | 0 | 0 | temporal:12, text_passport |
| `TMP_030` | 6 | 0.1128 | 0.1053 | 0.1148 | 155 | 0 | 0 | temporal:102, text_passport |
| `TXT_010` | 7 | 11.8254 | 8.8158 | 9.7589 | 255 | 2548 | 24.996 | temporal:7, text:14, text_passport |
| `TXT_020` | 7 | 9.7088 | 7.297 | 8.2232 | 255 | 4046 | 17.864 | temporal:14, text:28, text_passport |
| `TXT_030` | 7 | 14.4961 | 6.4386 | 8.1551 | 255 | 1092 | 32.7085 | temporal:7, text:14, text_passport |
| `TXT_040` | 8 | 4.2157 | 3.774 | 3.871 | 255 | 1560 | 22.125 | temporal:8, text:16, text_passport |

## Step 5 Entry Criteria

Formula tuning may start for a module only when:

1. The packet names the exact cases and sidecars that will be used.
2. The patch declares which knobs it is allowed to change.
3. The patch compares against this Step 4.5 baseline and reports deltas.
4. Any substrate dependency is either locked first or explicitly excluded from the gate.
5. Final PNG improvements are accepted only when the first divergent primitive is known.

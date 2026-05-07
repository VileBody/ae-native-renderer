# M05/M06 Text Reveal: Montserrat Identity And Square Selector

Date: 2026-05-07.

## Scope

This pass followed the Glow/ImageRenderer work and moved into the text lane:

- `M05`: remove the known Montserrat named-instance blocker.
- `M06`: fix visible Range Selector opacity behavior for the Square shape.

## Evidence

The text passport diagnostics showed `TXT_010`/`TXT_020` expected
`Montserrat-BoldItalic`, while native used `Montserrat-Italic[wght].ttf` with an
unproven variable instance. A static `Montserrat-BoldItalic.ttf` from the same
OFL Montserrat family is now in the conformance pack and is preferred for those
cases.

Manual visual review of `TXT_010`/`TXT_020` showed the runtime Square selector
left a visible ghost on frame 0. Sidecar telemetry confirmed the old full-range
weights were partial:

```text
[0.25, 0.75, 0.75, 0.25]
```

AE behavior for this fixture is full selection at `Start=0`, `End=100`,
`Shape=Square`, `Smoothness=100`; with animator opacity `0`, all units should be
fully hidden.

## Changes

- Added `fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf`.
- Updated the font resolver and conformance recipes to use the exact static
  `Montserrat-BoldItalic` face for `TXT_010`/`TXT_020`.
- Changed runtime Square Range Selector weight to `1.0` inside the selected
  range and `0.0` outside, without Smoothness edge-fading the opacity path.
- Added focused tests for exact Montserrat resolution and full-range Square
  selector behavior.

## Validation

Focused tests:

```text
cargo test -p text-engine font_db -- --nocapture
cargo test -p render-core square_selector_full_range_ignores_smoothness_edge_fade -- --nocapture
cargo test -p render-cli text_passport_diagnostics_accept_exact_montserrat_bolditalic -- --nocapture
cargo test -p testkit phase5_font_assumptions_match_ae_manifest -- --nocapture
```

Focused conformance:

```text
target/conformance_tmp/text_m05_m06_square_montserrat_20260507
target/visual_review/text_m05_m06_square_montserrat_20260507
```

Master gate:

```text
target/ae_agents/p2_text_m05_m06_square_montserrat_gate_20260507
accepted=2, tuning=1, approximate=16, regression=0, missing=0
```

## Metrics

| Case | Primary visible mean before | After | Note |
| --- | ---: | ---: | --- |
| `TXT_010` | `8.815824` | `14.088211` | Visually more correct font/reveal; remaining residual is glyph metrics/space distribution. |
| `TXT_020` | `7.296998` | `5.860491` | Character/line reveal improved. |
| `TXT_030` | `6.438564` | `6.803962` | Slight regression from selector path interaction; still M05/M07-owned. |
| `TXT_040` | `3.773967` | `3.727372` | Small improvement. |
| `GPH_010` | `13.529264` | `1.701600` | Large improvement from correct full-range Square opacity. |

Text passport mismatches improved for Montserrat cases:

```text
TXT_010 mismatches: 2548 -> 2352
TXT_020 mismatches: 4046 -> 3738
```

## Remaining Work

`M05` is no longer blocked by Montserrat font identity. It is now blocked by
CoolType/sourceRect metric parity:

- per-glyph width/bbox differences versus CoolType;
- whitespace distribution where AE sourceRect refs report spaces as zero width
  and fold gaps into adjacent glyph rows;
- deeper CoolType glyph id/raster coverage rows.

`M06` still needs focused selector boundary/weight references for partial
Start/End edges and non-square shapes.

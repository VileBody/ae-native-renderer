# Agent Round 4 Minimax

Scope: M13 `ADBE Minimax`, EFF_050 only. No final-PNG-only formula tuning was
done.

## Code changes

- Added one focused unit test in `crates/effects/src/minimax.rs` proving that
  `minimax_debug_trace` reports the resolved operation/channel/radius and hashes
  the same output path as `minimax_canvas`.
- No Minimax formula, enum mapping, radius rounding, neighborhood, edge, or
  premult/alpha behavior was changed.

## Evidence

Native sidecar for EFF_050:

```text
operation=minimum
channels=alpha
radius=12.0
kernel_radius=12
input_rgba=0xe784ce0090798f71
output_rgba=0x59a327e514138a31
```

EFF_050 final PNG geometry:

| Image | RGB bbox | Alpha bbox |
| --- | --- | --- |
| AE | `(211,211)-(300,300)`, 8,100 px | `(211,211)-(300,300)`, 8,100 px |
| Native | `(223,223)-(288,288)`, 4,356 px | full frame alpha from current compositor output |
| RGB diff | `(211,211)-(300,300)`, 3,875 px | n/a |

Key samples on row `y=256`:

| x | AE RGBA | Native RGBA |
| --- | --- | --- |
| 211 | `[205,205,205,204]` | `[5,5,6,255]` |
| 222 | `[255,255,255,255]` | `[5,5,6,255]` |
| 223 | `[255,255,255,255]` | `[183,183,184,255]` |
| 224 | `[255,255,255,255]` | `[255,255,255,255]` |
| 289 | `[255,255,255,255]` | `[5,5,6,255]` |
| 300 | `[205,205,205,204]` | `[5,5,6,255]` |

Interpretation:

- Current native `Minimum + Alpha + radius 12` erodes the visible square by
  about 12 px on each side.
- AE keeps the original transformed square extent. That is not explained well
  by radius rounding, square-vs-disk neighborhood, or edge policy.
- The first unresolved mismatch is most likely enum/channel semantics, possibly
  a combined operation/channel issue. `0003=1` may not mean alpha in AE, or
  `0001=2` may not mean the operation assumed by native. The available AE
  fixture does not contain resolved enum labels or a post-effect intermediate,
  so changing the formula would be speculative.
- Premult/alpha/composite behavior remains visible in raw alpha metrics, but
  the background-alpha-normalized RGB diff still shows the Minimax edge-ring
  mismatch.

## Metrics

Before was Round3 EFF_050. After is
`target/ae_agents/round4_minimax/EFF_050/metrics.json`.

| Metric | Before | After |
| --- | ---: | ---: |
| RGB mean abs diff | 3.533292 | 3.533292 |
| RGB max abs diff | 250 | 250 |
| RGB changed pixels | 3,875 | 3,875 |
| BG-alpha-normalized mean abs diff | 2.667440 | 2.667440 |
| BG-alpha-normalized max abs diff | 250 | 250 |
| Raw RGBA mean abs diff | 64.447626 | 64.447626 |

## Verification

```text
docker run ... cargo test -p effects minimax -- --nocapture
result: 5 passed

docker run ... cargo run -p render-cli -- conformance-pack \
  --pack fixtures/ae_conformance_pack \
  --out target/ae_agents/round4_minimax \
  --case EFF_050
result: conformance-pack.done ok=true cases=1
```

## Blocker / Minimal AE Probe

Need one AE-generated Minimax enum/channel probe before formula work:

1. On the same `alpha_square` primitive and transform, render pre-effect and
   post-effect PNGs or bbox/edge samples for `ADBE Minimax`.
2. Dump AE property metadata for each Minimax property: index, display name,
   value, and resolved UI label if accessible.
3. Render a small matrix with radius `12`: operation values `1,2` and channel
   values at least `1,2` plus any additional accepted channel enum values.
4. Include radius `0` identity and radius `12` for the selected EFF_050 params
   `{ "0001": 2, "0002": 12, "0003": 1 }`.
5. Report output RGB/alpha bbox and row samples around x `211..300`, y `256`.

This will separate operation enum, channel enum, radius/neighborhood, and
premult/alpha behavior without relying on final PNG diff tuning.

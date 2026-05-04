# Math Parity OS Gateboard

Owner: parent agent / critic.

Goal: move AE-like math from `implemented approximate` toward
`instrumented/testable -> AE golden exists -> formula tuning -> parity locked`
without tuning a composed template before its primitive/operator layers are
understood.

## Global Rule

Each phase must pass this loop before it can promote:

```text
decomposition
  -> isolated result against AE test
  -> composition/stack
  -> repeat AE/native test
  -> critic gate
```

No formula tuning is accepted from a full-template diff alone. Every accepted
patch needs a smaller case that explains why it is correct.

## Workers

| Phase | Agent | Scope | Primary AE Cases | Gate |
| --- | --- | --- | --- | --- |
| 1 | Locke `019dee70-8269-7382-8946-095a94501689` | Sampling/composite + transforms/ease | `CMP_010`, `INT_010`, `INT_020`, `EFF_040` | Prep passed; parity blocked |
| 2 | Peirce `019dee70-b4e3-72b3-8e36-960083fcb3d6` | Source/layer/precomp time + Posterize Time | `TMP_010`, `TMP_020`, `STK_030` | Prep passed; parity blocked |
| 3 | Euclid `019dee70-eba3-7263-86f7-ce99076b0011` | Box Blur, Drop Shadow, Glow, Minimax | `EFF_010`, `EFF_020`, `EFF_030`, `EFF_050`, `STK_010`, `STK_020` | Prep passed; parity blocked |
| 4 | Jason `019dee71-1c8c-7451-bafa-d84dd24129d2` | Geometry2 + Turbulent Displace | `EFF_040`, `EFF_060`, `STK_030` | Readiness passed; parity blocked |
| 5 | James `019dee71-b9c9-7090-8b1f-024ef56c7f80` | Text/glyph animator + expressions + collapse | `TXT_010`, `TXT_020`, `TXT_030`, `TXT_040`, `EXP_010`, `GPH_010` | Contract passed; parity blocked |

## Gate Criteria

### Phase 1

Required:

- Confirm AE goldens are present and clean.
- Establish selected-frame diff harness for static and animated primitive cases.
- Identify whether failures belong to alpha/composite, sampling, transform matrix,
  or easing.

Pass condition:

- Static pixel substrate is stable enough to use as the base for later phases.
- Transform/ease discrepancies are quantified or isolated.

### Phase 2

Required:

- Confirm numbered-frame cases map expected buckets.
- Verify Posterize Time quantizes source/effect/property time.
- Verify adjustment-layer resampling behavior on `STK_030` enough to unblock
  phase 4 stack work.

Pass condition:

- Temporal semantics can explain selected-frame outputs, even if exact AE
  boundary tuning remains open.

### Phase 3

Required:

- Decompose each spatial effect into masks/kernels/intermediate buffers.
- Test isolated static cases before stacks.
- Treat Box Blur as a dependency of Drop Shadow and Glow.

Pass condition:

- Effects have local diffs and stack-order behavior is deterministic.
- No template-level tuning before `EFF_*` cases are characterized.

### Phase 4

Required:

- Separate Geometry2 matrix parity from sampler/edge parity.
- Separate Turbulent Displace parameter parsing from displacement-field parity.
- Use `STK_030` only after `EFF_040` and `EFF_060` are characterized.

Pass condition:

- Coordinate warp failures can be assigned to matrix, field generation, edge
  sampling, or stack order.

### Phase 5

Required:

- Confirm `Montserrat-BoldItalic` and `Point-Light` goldens rendered without font
  substitution.
- Separate glyph layout from selector weights and animator transforms.
- Separate expression value sampling from glyph rendering.
- Compare collapsed vs non-collapsed graph behavior.

Pass condition:

- Text/expression/collapse blockers are assigned to concrete submodules, not
  only final text PNG mismatch.

## Current Golden Set

Primary clean AE PNG goldens:

```text
fixtures/ae_conformance_pack/ae_goldens/png/
```

Validation:

```text
fixtures/ae_conformance_pack/ae_goldens/VALIDATION_20260503_170505_85.md
```

The older slate-contaminated render is quarantined and must not be used as
primary golden input:

```text
fixtures/ae_conformance_pack/ae_goldens/png_with_slate_20260503_160541_85/
```

## OS Gate Decisions

Status date: 2026-05-03.

The five workers completed their scoped reports and checks. The critic decision
is intentionally conservative: all phases are allowed to advance to
`instrumented/testable`, but no phase is authorized for `formula tuning` or
`parity locked` yet.

| Phase | Decision | Evidence | Blocker Before Next Stage |
| --- | --- | --- | --- |
| 1 | Prep passed | `testkit phase1` validates 4 cases and 15 selected AE PNGs | native case recipe mapping and native-vs-AE image diff runner |
| 2 | Prep passed | `testkit phase2`, `effects posterize_time`, and `render-core posterize` pass | native-vs-AE frame diffs for `TMP_010`, `TMP_020`, `STK_030` |
| 3 | Prep passed | `testkit phase3` validates effect goldens, passports, and stack coverage | per-effect native render diffs plus effect debug outputs |
| 4 | Readiness passed | `testkit phase4`, `effects geometry`, and `effects turbulent` pass | matrix/UV telemetry for Geometry2 and displacement-field telemetry for Turbulent Displace |
| 5 | Contract passed | `testkit phase5` validates text/expression/collapse cases, fonts contract, and telemetry plan | exact `Point-Light`, glyph/selector/expression/collapse telemetry, native image diffs |

Verification commands run by the OS:

```sh
docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p testkit -- --nocapture'

docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo check -p testkit'

docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p effects posterize_time -- --nocapture'

docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p effects geometry -- --nocapture'

docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p effects turbulent -- --nocapture'

docker run --rm -u $(id -u):$(id -g) --entrypoint sh \
  -v "$PWD:/work" -w /work rust:1-bookworm \
  -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test -p render-core posterize -- --nocapture'
```

Observed results:

- `testkit`: 27 passed, 0 failed, warning-clean after cleanup.
- `testkit` check: pass.
- `effects posterize_time`: 3 passed.
- `effects geometry`: 3 passed.
- `effects turbulent`: 4 passed.
- `render-core posterize`: 2 passed.

## Next Required Transition

Implemented on 2026-05-03: `render-cli conformance-pack` maps every
`fixtures/ae_conformance_pack/manifest.json` case id to a deterministic native
scene/recipe, renders the selected frames, and writes:

```text
native frame
AE golden frame
diff image
max/mean/RMSE metrics
first divergent module checkpoint when telemetry exists
```

Current output location from the first full run:

```text
target/ae_conformance_native/report.json
target/ae_conformance_native/<CASE>/metrics.json
target/ae_conformance_native/<CASE>/native/
target/ae_conformance_native/<CASE>/ae/
target/ae_conformance_native/<CASE>/diff/
```

The next transition is to add module telemetry/debug outputs so each failing
case can point to the first divergent primitive, then promote one module at a
time from `instrumented/testable` to `AE golden exists -> formula tuning`.
Composed stack cases stay regression targets until their primitive operators are
explained.

## Native Diff Round 1

The five focused agent batches were run on 2026-05-03 and summarized in:

```text
docs/phase_reports/OS_AGENT_NATIVE_DIFF_ROUND1.md
```

Critic decision: formula tuning is still blocked for composed visual modules.
The first cross-agent blocker is `M19` output alpha/background policy:

```text
native background corner: [5, 5, 6, 255]
AE golden corner:         [5, 5, 6, 0]
```

Before using raw RGBA mean/max metrics for any formula patch, the conformance
runner needs RGB-only, alpha-only, and matte/background diagnostics or
AE-compatible alpha normalization. Agent 2 did find that `TMP_010` and
`TMP_020` match AE in RGB, so isolated source/layer time and Posterize buckets
should not be tuned from the current raw RGBA failure.

M19 metrics update: `render-cli conformance-pack` now keeps the legacy raw RGBA
fields while also writing structured `metrics.rgba`, `metrics.rgb`,
`metrics.alpha`, `metrics.background_alpha_normalized`, and
`background_corner` diagnostics in each frame and case summary. Formula tuning
may use `rgb` and `background_alpha_normalized` to separate visible math errors
from the known `[5,5,6,255]` native vs `[5,5,6,0]` AE background-alpha policy.

First module-owned blockers after the `M19` substrate:

| Area | Blocker |
| --- | --- |
| Temporal stack | `STK_030` suggests Posterize Time over-quantizes downstream adjustment effects. |
| Effects | Drop Shadow offset, Glow threshold/mask, Minimax enum/channel, and BoxBlur2/Glow animated params need intermediates. |
| Warps | Geometry2 AE param mapping and Turbulent Displace field telemetry. |
| Text | Exact `Point-Light` resolution plus glyph/selector/expression/collapse telemetry. |

## Critic Notes

- Workers may propose patches, but phase promotion requires integrated review.
- A phase can be marked `diagnosed` without being allowed to tune formulas.
- If two phases touch the same underlying primitive, Phase 1 owns the final
  decision for sampling/composite/transform substrate.
- If a composed stack fails, the OS routes the failure back to the lowest
  unproven operator.

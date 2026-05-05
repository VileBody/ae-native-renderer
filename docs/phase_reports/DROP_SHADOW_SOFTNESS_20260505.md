# Drop Shadow Softness Pass

Date: 2026-05-05

## Decision

For the current 8bpc AE conformance render path, Drop Shadow softness is implemented as:

```text
radius = ceil(softness * 1.4 / 2.71)
iterations = 1
blur target = alpha channel only
```

This replaces the older coarse equivalent `ceil(softness / 2) + 1` wording with the constants recovered from `Drop_Shadow.aex` / `GPUFoundation.dll`.

## Evidence

- `Drop_Shadow.aex` core candidate `0x180005bb0` calls:
  - `GF::BoxBlurOptions::StandardOptions`
  - `GF::BoxBlurOptions::SetBlurAlphaChannelOnly`
  - `GF::FastBoxBlur`
- The call site uses constants `1.4`, `1.0`, and divisor `2.71`.
- `GPUFoundation.dll` export `StandardOptions@BoxBlurOptions` writes:
  - flags at `+0x00`
  - src alpha type at `+0x04`
  - dest alpha type at `+0x08`
  - iterations at `+0x0c`
  - horizontal/vertical radius at `+0x10/+0x14`
- `SetBlurAlphaChannelOnly` clears color-channel blur bits and sets alpha-only blur.

## Rejected Branch For This Fixture

The alternate static branch:

```text
radius = ceil(softness / 2.71)
iterations = 3
```

was tested against `EFF_010` in:

```text
target/ae_agents/m10_drop_shadow_softness_div271_i3/report.json
```

It slightly improved visible RGB mean, but worsened alpha/background coverage compared to the current branch. For the conformance fixture, `scale=1.4, iterations=1` remains the better active-mode match.

Accepted run:

```text
target/ae_agents/m10_drop_shadow_softness_scale14_div271_i1/report.json
target/ae_agents/m10_drop_shadow_softness_scale14_div271_i1/hypothesis_report.json
```

## Dynamic Trace Note

S3-backed Frida runner:

```text
scripts/ae_trace_drop_shadow_softness.py
```

The runner can upload the tracer via S3, start it on the 85 node through WinRM, render selected AE cases, and pull JSONL logs back through S3. Current trace attached to `AfterFX.exe` and installed hooks, but did not observe blur calls during `/pack-render`; likely the render work happens in another process/path or needs visual state validation on the node. Future 85-debug passes should include screenshots before/after render to catch modal/hung UI state.

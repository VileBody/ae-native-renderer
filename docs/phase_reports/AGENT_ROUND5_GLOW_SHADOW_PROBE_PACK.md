# Agent Round 5 Glow / Drop Shadow Probe Pack

Date: 2026-05-04

Scope: generated an AE-side probe pack for Glow and Drop Shadow under
`fixtures/ae_probe_pack/glow_shadow/`.

## Decision

No formula tuning was done, and no native conformance wiring was added.

This pack is for rendering new AE probe outputs by hand in After Effects. The
outputs are not AE goldens until they are produced and accepted as evidence.

## Files Added

- `fixtures/ae_probe_pack/glow_shadow/README.md`
- `fixtures/ae_probe_pack/glow_shadow/manifest.json`
- `fixtures/ae_probe_pack/glow_shadow/jsx/build_glow_shadow_probe_project.jsx`
- `docs/phase_reports/AGENT_ROUND5_GLOW_SHADOW_PROBE_PACK.md`

No files under `crates/effects`, `render-core`, `text-engine`, or the existing
`fixtures/ae_conformance_pack` were edited.

## How To Render In AE

1. Open After Effects.
2. Run `File > Scripts > Run Script File...`.
3. Select:

```text
fixtures/ae_probe_pack/glow_shadow/jsx/build_glow_shadow_probe_project.jsx
```

4. Render queued case comps as PNG sequences with alpha:

```text
fixtures/ae_probe_pack/glow_shadow/ae_probe_outputs/png/<case_id>/<case_id>_[#####].png
```

Recommended output module settings:

- PNG sequence.
- RGB + Alpha.
- Straight/unmatted when available.
- 8 bpc.
- Frame 0 for each case.

The optional preview comp is:

```text
master_glow_shadow_probe_reel
```

## Probe Coverage

Glow:

- `GLO_010`: threshold source, opaque luma ramp.
- `GLO_020`: luma threshold mask candidate.
- `GLO_030`: blurred luma mask candidate.
- `GLO_040`: intensity-scaled luma candidate.
- `GLO_050`: final AE Glow on opaque luma ramp.
- `GLO_060`: threshold source with alpha/luma split pixels.
- `GLO_070`: alpha-split luma-rule threshold candidate.
- `GLO_080`: alpha-split alpha-rule threshold candidate.
- `GLO_090`: final AE Glow on alpha/luma split source.
- `GLO_100` / `GLO_101`: final AE Glow with `Glow Based On` enum values 1 and 2, if AE accepts those property values.

Drop Shadow:

- `DSH_010`: source alpha square.
- `DSH_020`: no-softness shadow-only raw output.
- `DSH_030`: no-softness final composite.
- `DSH_040`: softened shadow-only output.
- `DSH_050`: softened final composite.
- `DSH_060`: manual raw-offset candidate `dx=-20, dy=20`.
- `DSH_061`: manual raw-offset candidate `dx=20, dy=20`.

## Questions Answered

Glow:

- Does AE threshold Glow from luma, alpha, or a combined rule?
- Does the current native assumption that the full opaque luma ramp participates
  match AE, or should thresholding reject low-luma pixels despite alpha 255?
- Does AE's visible spread look closer to the candidate blurred mask before or
  after intensity scaling?
- Which `Glow Based On` enum value corresponds to the default AE behavior, if
  the enum can be set reliably through JSX?

Drop Shadow:

- For `direction=135` and `distance=28`, does AE's raw shadow offset match
  native's current `dx=-20, dy=20` convention or another convention?
- Does `softness=0` with `Shadow Only=1` isolate the same raw mask used by the
  softened output?
- What does AE's softened shadow look like before compositing with the source?
- Does the final composite diverge before or after shadow generation?

## Validation Performed

- The pack is self-contained and uses only procedural AE layers/precomps.
- `manifest.json` is valid JSON.
- The JSX was syntax-checked with Node as plain JavaScript. Runtime validation
  still requires After Effects because `app`, `File`, `FolderItem`, and effect
  match names are AE ExtendScript APIs.

## Next Step

Render this pack in AE, then compare the generated PNGs with the Round 4 native
sidecars before changing Glow or Drop Shadow formulas.

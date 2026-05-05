# GHIDRA Analysis M12 Geometry2 Sampler

Status date: 2026-05-05.

## Status

Analysis complete for:

- `target/reverse/predecoded/20260505_153013_blocker_modules_round2/geometry_transform_gpufoundation`
- `target/reverse/predecoded/20260505_153013_blocker_modules_round2/geometry_transform_wrapper`

`ORCHESTRATOR_BLOCKER`: Geometry2's source-edge sampler policy is inside shared
`GPUFoundation.dll` transform GPU kernels. The extraction proves kernel
selection and destination prefill, but not source OOB/clamp/filter edge math.
Do not change global sampler/composite/color implementation from this pass.

## Inputs

- `geometry_transform_wrapper/index.md`
- `geometry_transform_wrapper/01_Geometry2_render_wrapper_180009a20/{decompile.c,disassembly.txt,callees.tsv}`
- `geometry_transform_wrapper/02_Geometry2_mb_candidate_33a0_1800033a0/decompile.c`
- `geometry_transform_wrapper/03_Geometry2_mb_candidate_4440_180004440/decompile.c`
- `geometry_transform_wrapper/04_Geometry2_mb_candidate_4ab0_180004ab0/decompile.c`
- `geometry_transform_gpufoundation/index.md`
- `geometry_transform_gpufoundation/01_GF_TransformOperation_ctor_1800725b0/decompile.c`
- `geometry_transform_gpufoundation/03_GF_TransformOperation_Quality_180074c40/decompile.c`
- `geometry_transform_gpufoundation/08_GF_TransformWithMotionBlur_180076ed0/{decompile.c,disassembly.txt,data_refs.tsv}`
- Local binary string scan of `target/reverse/ae_2026/{Transform.aex,GPUFoundation.dll}`.

## Findings

Pixel center convention: not derivable from this extraction. The wrapper builds
matrices and passes image/frame geometry into `GF::TransformWithMotionBlur`, but
the per-pixel coordinate convention is in the selected GPU kernels. I did not
find a visible `+0.5`/`-0.5` center adjustment in wrapper code or in the exposed
GF host dispatch. Current answer: `unknown`, not integer and not half-pixel.

OOB/sampler footprint:

- Destination is explicitly initialized with transparent black before transform:
  `Transform.aex` `18000a183`/decompile line with
  `GF::FillWithTransparentBlack`, then `GF::TransformWithMotionBlur` runs at
  `18000a74c`.
- This confirms transparent output for untouched destination pixels and
  transformed ROI misses.
- Source OOB behavior is not visible. Clamp vs transparent source samples,
  edge threshold, and partial filter footprint are inside GF GPU kernels.
- Host code computes/uses matrix radii for area paths in `GF::TransformWithMotionBlur`
  around `1800772d0..1800773b6`, uploads radius buffers around
  `180077558..1800775bd`, and toggles area/no-area kernel variants via
  `cStack_628`. The exact source-edge contribution math is still hidden.

Sampler quality / `0012`:

- `Transform.aex` strings expose Geometry2 UI strings:
  `Sampling` and `Bilinear|Bicubic`.
- Wrapper passes a two-dword `GF::SampleQuality` from effect state offsets
  `+0x60/+0x64` into `GF::TransformOperation` at `18000a63b..18000a690`.
- `GF::TransformOperation::Quality` at `180074c40` copies those 8 bytes back.
- `GF::TransformWithMotionBlur` reads quality at `180076f95` and branches on
  the first dword:
  - `0` or default/other: nearest-neighbor kernel string
    `XFormMotionBlur_kSamplingMethod_NearestNeighbor_...` (`18019e3d0`).
  - `1`: bilinear kernels, no-area/area (`18019e310`, `18019e370`).
  - `2`: bicubic Lanczos low-pass no-area kernels, sharp/smooth
    (`18019e430`, `18019e490`), with a temporary/prepass path around
    `1800775f9..180077764`.
  - `3`: bicubic area-sample kernels, sharp/smooth and no-area/area
    (`18019e180`, `18019e1e0`, `18019e240`, `18019e2b0`).
- The second quality dword is tested against `2` in the bicubic branches and
  selects sharp vs smooth variants. The extraction does not prove how AE's
  visible `0012` values map into these GF enum values.

Key confirming addresses:

- `180009a20` `Geometry2_render_wrapper`: creates GPU frames, transparent
  prefill, constructs `GF::TransformOperation`, dispatches transform.
- `18000a183`: wrapper call to `GF::FillWithTransparentBlack`.
- `18000a63b..18000a690`: wrapper packs `SampleQuality` from state `+0x60/+0x64`
  and calls GF constructor.
- `18000a74c`: wrapper call to `GF::TransformWithMotionBlur`.
- `1800725b0`: `GF::TransformOperation` ctor stores `SampleQuality` at object
  start and calls `Calculate`.
- `180074c40`: `GF::TransformOperation::Quality` returns the stored 8-byte
  quality struct.
- `180076f95`: `GF::TransformWithMotionBlur` reads quality.
- `180077221..1800773cf`: radii/area-footprint host setup.
- `1800777cd..180078845`: quality-driven kernel selection.
- GF kernel strings:
  `18019e180/1e0/240/2b0` bicubic area sample,
  `18019e310/370` bilinear,
  `18019e3d0` nearest,
  `18019e430/490` bicubic Lanczos.

## Accepted/Rejected/Unknown

Accepted:

- Wrapper pre-fills the destination with transparent black before the GF
  transform.
- Geometry2 sampling is implemented by shared GF transform GPU kernels, not by
  visible wrapper CPU loops.
- GF host code has visible quality branches for nearest, bilinear, bicubic
  Lanczos, and bicubic area-sample families.
- Area/no-area selection depends on host-computed matrix radii.

Rejected:

- This extraction does not justify changing Geometry2 to a global clamp-edge
  sampler.
- This extraction does not prove current integer pixel-center convention.
- This extraction does not prove half-pixel convention.
- This extraction does not prove that AE UI `0012` directly equals the first GF
  quality dword without a parameter-state probe.

Unknown:

- Pixel center: integer vs half-pixel.
- Source OOB: transparent vs clamp vs edge extension.
- Exact source-edge threshold: `uv < 0`, `uv < -0.5`, `uv > width - 1`,
  `uv >= width`, or footprint-specific handling.
- Exact bilinear/bicubic footprint contribution when only part of the footprint
  is outside source bounds.
- UI `0012` to GF quality mapping.

## Missing Probes

- Geometry2 identity and subpixel-translate probes over a 1-pixel impulse and
  checkerboard to distinguish integer vs half-pixel destination centers.
- Edge UV probe targeting `-0.5`, `-0.0001`, `0`, `0.5`, `width - 1`,
  `width - 0.5`, `width - 0.0001`, and `width`.
- Alpha-border probe with colored transparent pixels to distinguish transparent
  OOB from clamp-to-edge and premultiplied edge bleed.
- `0012` UI sweep for Bilinear/Bicubic while dumping wrapper state bytes at
  `+0x60/+0x64`, or equivalent sidecar telemetry, to map AE values to GF
  quality branches.
- Scale/rotation sweep that crosses the matrix-radii area/no-area threshold and
  records whether AE changes filter footprint.

## Next Implementation Candidate

No implementation change from Ghidra alone. The next safe candidate is a probe
pack plus telemetry:

- add/read Geometry2 sampler sidecar fields for `0012`, inferred quality value,
  source UV, selected neighbor/footprint, OOB hit, and sampled RGBA;
- run the edge/impulse probes above;
- only then decide whether Geometry2 needs a local sampler override or whether
  an orchestrator-owned shared sampler policy must be changed.

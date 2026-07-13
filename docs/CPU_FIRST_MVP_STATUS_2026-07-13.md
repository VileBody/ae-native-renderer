# CPU-first native MVP status

Date: 2026-07-13

## Verified result

- Trendy production render: 1080x1920, 12 s, 288 frames.
- Wall time: 82.65 s; renderer response: 82.28 s.
- Peak RSS: 998,031,360 bytes (951.8 MiB).
- Fixed CPU baseline: 288 s; measured speedup: 3.47x.
- Output: H.264 1080x1920 at 24000/1001 plus stereo AAC 44.1 kHz.
- Output duration: 12.012 s.
- Capability result: complete, 13 supported, 145 approximate, 0 not implemented.
- Brat production render: 1080x1920, 15 s, 360 frames, 58.90 s wall, 653.3 MiB
  RSS, H.264 plus stereo AAC sampled from source time 53 s.
- Direct-production Trendy MP4: 1080x1920, 12 s, 288 frames, 78.44 s wall,
  631 MiB RSS. It is written through a persistent raw-RGBA ffmpeg sink, with no
  PNG frame directory, and contains H.264 video plus stereo AAC.

## Implemented

- `EffectRuntime` owns reusable ping/pong canvases, per-worker scratch slots, and bounded
  `Arc<Canvas>` LRU caches for Posterize lower stacks, raster precomps, and static text.
- Effects expose `render_into`; production and development telemetry are separate without
  removing full conformance diagnostics.
- Minimax is an uncapped O(width*height) monotonic-deque implementation and supports radius
  165 or larger than the frame.
- Glow keeps two-effect ordering, uses recursive O(N) blur, and fuses threshold/intensity/
  composite work. Minimax, Glow, Motion Blur, Optics, and stylize passes use CPU parallelism.
- Trendy uses Tracking Amount 7 -> -1 before layout, one fill/stroke text paint, hollow stroke,
  stroke-under-fill, Cyrillic fit/sourceRect work, native gradient, and soft shadow parameters.
- Directional Motion Blur and Optics Compensation use transparent premultiplied sampling.
- Active F3 IDs lower to native deterministic primitives. Proprietary-effect pixel parity is
  still reported as approximate.
- F2 shapes and F4 gesture devices are procedural native overlays. F1/F5 accept supplied SFX
  and TTS audio, including trim, placement, fades, delay, ducking, and AAC `amix` output.
- Blend modes include Normal, Add, Screen, and Difference.
- JSON request v1 remains top-level compatible with generated JSX inputs and supports direct
  F1-F5 fields plus normalized asset roles.
- `output-manifest.json` contains artifact hashes, frame count, duration, and deterministic
  video/audio probe data.
- Glow's two-pass blur now uses a blocked transpose between horizontal and vertical passes:
  it is byte-exact at one and many worker threads, while avoiding strided per-column work.
- Fontdue glyph masks are bounded and reusable across frames; generic full-frame parallelism
  is deliberately held to 4K-class surfaces because it is memory-bandwidth bound at 1080p.
- A bounded frame scheduler, source-frame prefetch, and persistent per-lane effect runtimes
  are production-default. At 1080p under the 1.5 GiB budget it admits three lanes; the
  `AE_RENDER_FRAME_WORKERS` override remains available for controlled benchmarks.
- The corrected prefetch plan includes Posterize Time adjustment-layer bucket timestamps.
  Five frame-locked raw-PNG controls at frames 0, 24, 72, 144, and 240 are byte-identical
  between sequential and three-lane rendering.
- Trendy three-lane production render: 37.94 s wall, 34.06 s renderer time, and 1.35 GiB
  RSS, including direct H.264/AAC MP4 output. This is 2.76x faster than realtime.
- Full scheduler visual gate: sequential and three-lane render outputs are byte-identical for
  all 288 Trendy frames (`SSIM=1.0`, `MAE=0`, `changed_pixels=0`).
- The active F2/F3/F4 palette now has a contract test over five shapes, five gesture devices,
  and fourteen F3 IDs. F1/F5 audio coverage includes overlapping tracks, SFX impact timing,
  supplied TTS trim, fades, ducking, source-time seek, AAC mux, and probe validation.

## Visual checks

- Full Trendy Rust-vs-AE mean PSNR: 22.866 dB.
- Frame-locked control frame 23: SSIM 0.9292.
- Frame-locked control frame 28: SSIM 0.9375.
- Heavy shutter/snap frames 24/26: SSIM 0.6606/0.7020; these remain the main visual gap.
- Nine Brat frame-locked controls: aggregate MAE 1.749, PSNR 33.513 dB, SSIM
  0.904. The four CC Image Wipe controls remain around MAE 1.70 with per-frame
  SSIM 0.898-0.944.

## Remaining after MVP

1. Tune lane-local cache caps without changing the complete 288-frame determinism result.
   Four Trendy lanes now render in 33.49 s, but peak at 1.74 GiB and are rejected by the
   memory gate.
2. Override `render_into` in the remaining effects so every stack stage reuses storage; many
   legacy effects still use the compatibility implementation that returns a fresh canvas.
3. Finish Levels/Extract variants, generic masks/mattes, temporal trails, and explicit
   premultiplied-alpha checkpoints for every F3 combination.
4. Implement actual sRGB linear blending and broader color transforms. Unmanaged/nonlinear
   sRGB is supported; linear blending, ICC, wide gamut, ACES, and 16/32-bpc remain gaps.
5. Replace procedural F4 approximations with normalized production vector paths/assets where
   exact device silhouettes matter.
6. Tune shutter/snap/analog treatment against AE goldens. Control frames pass SSIM 0.90, but
   effect-heavy frames do not yet meet that threshold.
7. Restore the missing AE conformance PNG set and update the stale phase4 fixture selection;
   these are the six known workspace-test failures and block the complete 28-case golden battery.

## Explicitly deferred

- Legacy blocks.
- OFX and proprietary Sapphire/Boris/Red Giant binaries.
- Metal/GPU and optical flow.
- Network TTS synthesis.
- Full ICC/ACES and byte-exact proprietary-plugin parity.

# ADR: Sapphire/Boris/OFX Spike For Native Renderer

Status: accepted for P0/P1 as non-production dependency.

## Context

The public Blast flow contains looks that were originally authored in After Effects with built-in effects and, in some historical templates, third-party plugin families such as Sapphire/Boris/Red Giant. P0/P1 readiness requires that the Rust renderer does not silently drop these semantics. It does not require shipping proprietary binaries inside the production renderer.

## Spike Routes

1. Adobe `.aex` in-process loading.
   Rejected for production P0/P1. `.aex` plugins are Adobe host plugins, not stable standalone libraries. Loading them outside AE would require reimplementing host callbacks, licensing constraints, GPU/session behavior, and effect parameter UI/runtime contracts.

2. OFX host route.
   Deferred. Some plugin families historically ship OFX variants for supported hosts, but availability differs by vendor/package/version. Even when present, a safe OFX host would need a separate process boundary, license management, pixel format/color policy, deterministic parameter mapping, and failure isolation.

3. AE sidecar route.
   Deferred. This gives best parity but reintroduces AE as runtime dependency, which conflicts with the native-only production path for this slice.

4. Native approximation route.
   Accepted for P0/P1. The builder preserves stable ids, AE match names, params, timings and assets in RenderPlan/native request; the renderer reports `supported`, `approximate`, `not_implemented`, or `unsupported` before render. Proprietary matches remain `unsupported` unless mapped to an explicit native approximation.

## Decision

P0/P1 production uses native approximations only. Sapphire/Boris/Red Giant binaries are not required and are not loaded by the renderer. The spike remains documented so that a future P2/P3 can test one isolated effect such as `S_DropShadow` or `S_BlurMotion` behind a sidecar/OFX boundary.

## Consequences

- Pixel parity for proprietary plugin stacks remains `approximate`.
- Unknown plugin/effect ids must appear in capability reports and block with `policy.onUnsupported=error`.
- Native alternatives must document alpha/color assumptions and parameter fallback policy in the registry.

# AE Conformance Pack

This pack creates an After Effects project that renders primitive/operator/stack
goldens for the native renderer.

## Goal

Render AE truth data in a controlled ladder:

```text
primitive
property/interpolation
single static operator
single animated operator
effect stack
template-like slice
full template later
```

The long `master_conformance_reel` comp is for quick visual review. The important
goldens are PNG sequences rendered per case comp.

Per-case comps are intentionally label-free so their frames can be compared
pixel-for-pixel. Slate/title labels are only added in `master_conformance_reel`.

## Required Fonts

The AE machine must have these installed:

- `Montserrat-BoldItalic`
- `Point-Light`

`Point-Light` is intentionally referenced as an installed AE font. Do not swap it
for a fallback when producing goldens; glyph tests are only useful with the same
font as the templates.

## How To Use In AE

1. Open After Effects.
2. Run `File > Scripts > Run Script File...`.
3. Select `jsx/build_conformance_project.jsx`.
4. The script creates:
   - primitive footage items from `assets/primitives`;
   - one composition per test case;
   - `master_conformance_reel`;
   - render queue entries for every case and the master reel.
5. Render case comps as PNG sequences into:

```text
ae_goldens/png/<case_id>/<case_id>_[#####].png
```

6. Optionally render the master reel to:

```text
ae_goldens/preview/master_conformance_reel
```

## Notes

- Keep PNG sequences as the source of truth. MP4 is only for human preview.
- Do not change project FPS or comp size while producing goldens.
- The JSX uses explicit case names so native diff tooling can map AE frames back
  to `manifest.json`.
- Re-run `scripts/generate_primitives.py` only if primitive assets need to be
  regenerated deterministically.

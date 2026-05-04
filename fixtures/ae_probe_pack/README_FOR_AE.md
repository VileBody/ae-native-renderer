# AE Probe Packs

Take this folder to the machine with After Effects and run each JSX builder
through:

```text
File > Scripts > Run Script File...
```

## Packs

### Glow + Drop Shadow

Run:

```text
glow_shadow/jsx/build_glow_shadow_probe_project.jsx
```

Render queued PNG sequences to the paths described in:

```text
glow_shadow/README.md
```

### Minimax

Run:

```text
minimax/jsx/build_minimax_probe_project.jsx
```

Render queued PNG sequences, then measure:

```sh
python3 minimax/scripts/measure_minimax_probe.py --pack minimax
```

Details:

```text
minimax/README.md
```

### Turbulent Displace Field

The primitive PNG assets are already generated. To regenerate them:

```sh
python3 turbulent_field/scripts/generate_assets.py
```

Run:

```text
turbulent_field/jsx/build_turbulent_field_probe_project.jsx
```

Render queued PNG sequences as described in:

```text
turbulent_field/README.md
```

## Important

These are probe packs, not accepted goldens yet. After rendering in AE, bring
the output folders back into this repo so native/AE measurements can be compared
before tuning formulas.

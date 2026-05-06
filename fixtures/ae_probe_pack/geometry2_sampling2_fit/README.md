# Geometry2 Sampling=2 Fit Probe

Small isolated AE probe pack for `ADBE Geometry2` sampler tuning.

The pack renders deterministic primitive PNGs through Geometry2 with matching
`0012 = 1` and `0012 = 2` cases. The paired outputs let the native fitter
separate parameter mapping from the actual bicubic kernel, edge footprint, and
rounding policy.

Workflow:

```bash
python3 fixtures/ae_probe_pack/geometry2_sampling2_fit/scripts/measure_geometry2_sampling2_fit.py \
  --pack fixtures/ae_probe_pack/geometry2_sampling2_fit \
  --generate-assets

python3 scripts/ae_trace_drop_shadow_softness.py \
  --transport ssh \
  --ssh-host ae85 \
  --pack fixtures/ae_probe_pack/geometry2_sampling2_fit \
  --entry-script jsx/build_geometry2_sampling2_fit_project.jsx \
  --batch-cases \
  --case G2S_FIT_TEXTURE_SHIFT_X_0_125_Q1_BILINEAR \
  --case G2S_FIT_TEXTURE_SHIFT_X_0_125_Q2_BICUBIC \
  --offset-hook Transform.aex:0x5f30:geometry2_effect_proc_entry \
  --offset-hook Transform.aex:0x5b20:geometry2_wrapper_render_params_5b20
```

Then run the same script without `--generate-assets`, passing the extracted AE
PNG root as `--png-root`, to write fit reports and diff images.

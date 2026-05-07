# P2 Text Transfill Probe

Focused AE probe for the `TXT_ARE_Render_8bpc_fill_3d200` branch where fill
alpha is below full opacity and AE routes through a temp `PF_World` followed by
`PF_TransferRect`.

Run:

```sh
python3 scripts/ae_remote_pack.py fixtures/ae_probe_pack/p2_text_transfill_probe \
  --entry-script jsx/build_p2_text_transfill_probe_project.jsx \
  --node http://85.239.48.31:8001 \
  --job-id p2_transfill_YYYYMMDD_HHMMSS
```

Measure extracted PNG/TIFF output:

```sh
python3 fixtures/ae_probe_pack/p2_text_transfill_probe/scripts/measure_transfill_probe.py \
  --root target/ae_agents/p2_transfill_YYYYMMDD_HHMMSS/extracted
```

The pack intentionally uses tiny, single-glyph cases over transparent, black,
and blue backgrounds. If AE scripting ignores the fourth `fillColor` component,
the alpha cases will collapse to the opaque control; that result is still useful
and should be paired with a `TXT.dll+0x3d200`/`PF_TransferRect` hook addition.

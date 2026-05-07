# P2 Text Transfill Live Probe

Focused live AE85 probe for text fill alpha `< 255` in `TXT.dll`.

The working scripting route under test is:

```javascript
var animator = layer.property("ADBE Text Properties")
    .property("ADBE Text Animators")
    .addProperty("ADBE Text Animator");
animator.property("ADBE Text Animator Properties")
    .addProperty("ADBE Text Fill Opacity")
    .setValue(128 / 255 * 100);
animator.property("ADBE Text Selectors").addProperty("ADBE Text Selector");
```

Trace command shape:

```bash
python3 scripts/ae_trace_cooltype_text.py \
  --ssh-host ae85 \
  --node http://85.239.48.31:8001 \
  --pack fixtures/ae_probe_pack/p2_text_transfill_live_probe \
  --entry-script jsx/build_TRFLIVE_WHT_A128_FILL_OPACITY.jsx \
  --case TRFLIVE_WHT_A128_FILL_OPACITY \
  --duration 80 \
  --max-events 2600 \
  --hook-profile txt-are-spans \
  --out-dir target/dynamic_tools_85/p2_transfill_live_YYYYMMDD_HHMMSS \
  --allow-render-failure
```

The default entry script builds only the opaque control. Alpha variants have
one-case entry scripts because a previous multi-case project builder made AE
unstable before remote case filtering. The JSX writes
`ae_probe_outputs/text_fill_alpha_method.txt` with the exact match-name
attempts and readbacks from AE.

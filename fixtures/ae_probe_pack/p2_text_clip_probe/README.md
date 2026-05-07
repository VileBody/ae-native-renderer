# P2 Text Clip Probe

Tiny AE pack for P2-C clipped glyph coverage bounds.

The pack creates single-glyph text comps whose `sourceRectAtTime` edge is placed
at a fractional offset around each comp side. SourceRect is used only to position
the glyph; the useful measurement is the final rendered alpha bbox and the first
or last nonzero pixel touching the PF_World.

Cases:

- `CLP_LEFT_N049`: sourceRect left at `-0.49`.
- `CLP_TOP_N049`: sourceRect top at `-0.49`.
- `CLP_RIGHT_P049`: sourceRect right at `width + 0.49`.
- `CLP_BOTTOM_P049`: sourceRect bottom at `height + 0.49`.


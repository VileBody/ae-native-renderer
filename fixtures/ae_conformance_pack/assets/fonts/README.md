# Font Slots

Goldens must be rendered with the fonts installed in After Effects:

- `Montserrat-BoldItalic`
- `Point-Light`

This directory is only a convenient place to drop exact `.ttf`/`.otf` copies if a
future machine needs them. The JSX references the installed font names directly.

Included:

- `Montserrat-Italic[wght].ttf` from Google Fonts / SIL Open Font License.
- `Point-Light.ttf` from the local user-provided Point family archive, used by
  native conformance runs for `TXT_030` and `TXT_040`.
- `OFL-Montserrat.txt`

AE goldens should still use the installed `Montserrat-BoldItalic` face if it is
available on the render machine.

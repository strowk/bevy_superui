# Open Graph card

`card.html` is the source for `website/src/og-image.png`, the 1200×630 social
preview referenced by the `og:image` tags in `website/theme/head.hbs`. It is a
plain page styled to match `src/assets/blueprint.css`, rendered once and
committed as a PNG — link previews are scraped by bots that do not run a
browser, so the image has to be a raster file.

Regenerate after editing (any headless Chromium works; the Google Fonts link
needs network access, and `--virtual-time-budget` gives the webfonts time to
load before the shot):

```sh
cd website/tools/og-image
chrome --headless --disable-gpu --hide-scrollbars \
  --force-device-scale-factor=1 --window-size=1200,630 \
  --virtual-time-budget=8000 \
  --screenshot="$PWD/../../src/og-image.png" "$PWD/card.html"
```

`logo.svg` is loaded relative to `card.html`; keep a copy alongside it in sync
with `website/src/logo.svg`.

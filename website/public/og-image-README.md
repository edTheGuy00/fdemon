# OG Image — Regeneration Guide

`og-image.png` (1200×630) is the sitewide Open Graph / Twitter Card social preview image.
It is committed as a static asset and referenced from `<meta property="og:image">` in `index.html`.

## Quick regeneration

```bash
# From repo root — requires Google Chrome (macOS / Linux)
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  --headless=new \
  --disable-gpu \
  --no-sandbox \
  --hide-scrollbars \
  --window-size=1200,630 \
  --screenshot=website/public/og-image.png \
  "file://$(pwd)/website/public/og-image.html"
```

On Linux (CI / Docker):

```bash
google-chrome --headless=new --disable-gpu --no-sandbox \
  --hide-scrollbars --window-size=1200,630 \
  --screenshot=website/public/og-image.png \
  "file://$(pwd)/website/public/og-image.html"
```

## Source template

`og-image.html` — self-contained HTML/CSS file (no external network fetches).
All styles are inline; the only external asset it loads is `logo.png` from the same directory,
which is why the `file://` URL must point to this directory (not a temp copy).

## Design spec

| Property       | Value                                    |
|----------------|------------------------------------------|
| Dimensions     | 1200 × 630 px                            |
| Background     | `#020617` (Tailwind `slate-950`)         |
| Accent         | Blue → purple gradient (`#3b82f6` → `#a855f7`) |
| Wordmark       | "Flutter Demon" — gradient text 72px 800w |
| Tagline        | "A blazingly fast TUI for Flutter development" |
| Logo           | `logo.png` 200×200, drop-shadow          |
| Corner URL     | `flutter-demon.dev`                      |

## Editing the design

1. Edit `website/public/og-image.html` (plain HTML + CSS, no build step needed).
2. Open it in a browser to preview at 1200×630.
3. Run the regeneration command above.
4. Commit both `og-image.html` and the updated `og-image.png`.

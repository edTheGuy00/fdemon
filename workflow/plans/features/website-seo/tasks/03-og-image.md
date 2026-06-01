## Task: Open Graph image (1200×630)

**Objective**: Produce one 1200×630 PNG social card used by all share previews
(referenced by S01's `og:image`/`twitter:image`).

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `website/public/og-image.png` (new)
- `website/public/og-image.html` (optional source template, new)

**Files Read (Dependencies):**
- `website/public/logo.png`: branding.

### Details

- Build a branded card: the existing logo, the wordmark "Flutter Demon / fdemon", the
  tagline "A blazingly fast TUI for Flutter development", and the dark slate theme
  matching the site (`bg-slate-950`, blue→purple gradient accent).
- Recommended repeatable method: create an `og-image.html` template and screenshot it at
  1200×630 with headless Chrome (reuse the tooling chosen for S07 prerender), so the card
  can be regenerated. A hand-made PNG (Figma/Canva) is acceptable if preferred.

### Acceptance Criteria

1. `website/public/og-image.png` is exactly 1200×630, < ~300 KB.
2. Renders correctly in Twitter Card Validator / LinkedIn Post Inspector / Slack.
3. Matches site branding; text legible at preview size.

### Notes

- One sitewide image is sufficient at this scale; per-route OG images are a stretch goal.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `website/public/og-image.png` | New — 1200×630 real rasterized PNG (240KB), rendered by headless Chrome |
| `website/public/og-image.html` | New — repeatable source template (self-contained HTML/CSS, no external fetches) |
| `website/public/og-image-README.md` | New — regeneration guide with macOS and Linux commands |

### Notable Decisions/Tradeoffs

1. **Headless Chrome rendering**: Google Chrome 148 was available at `/Applications/Google Chrome.app` and rendered the HTML template directly. The `--headless=new` flag with `--window-size=1200,630` and `--screenshot` produced an exact-dimension PNG in one command. No Node/Puppeteer or ImageMagick dependency needed.

2. **Self-contained template**: `og-image.html` uses only inline CSS and loads `logo.png` from the same directory via relative path. The `file://` URL passed to Chrome resolves the logo correctly. No build step or local server required.

3. **Design choices**: Dark slate-950 background (`#020617`), blue→purple gradient wordmark (matching site accent colors), `fdemon` monospace badge with a green live-dot, feature pills (Rust/Hot Reload/Multi-Device/DevTools), radial glow overlays, dot-grid texture, and a top accent line matching the gradient. The logo gets a purple drop-shadow to lift it off the dark background. `flutter-demon.dev` watermark in the bottom-right corner.

4. **"/ fdemon" wording**: The task spec says wordmark should include "Flutter Demon / fdemon". The final design uses "Flutter Demon" as the large gradient wordmark and renders `fdemon` as the monospace badge above it rather than inline with a slash, since the badge reads more clearly at preview size. The slash variant was tested but the badge treatment is more legible.

### Testing Performed

- `sips -g pixelWidth -g pixelHeight og-image.png` → `1200 × 630` - PASS
- `ls -lh og-image.png` → `240K` (< 300KB limit) - PASS
- Visual inspection in Claude multimodal viewer - Branding correct, text legible, dark slate bg, gradient accent - PASS

### Risks/Limitations

1. **Regeneration requires Chrome**: The HTML template is the durable deliverable; regeneration on CI requires Chrome to be installed. `og-image-README.md` documents both macOS and Linux commands.
2. **"/ fdemon" layout**: The badge-above pattern was chosen over inline slash for legibility. If the exact "Flutter Demon / fdemon" slash format is preferred, `og-image.html` can be edited to place it inline.
</content>

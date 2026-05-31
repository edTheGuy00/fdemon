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
</content>

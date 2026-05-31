## Task: Static `<head>` SEO foundation

**Objective**: Give every crawler (incl. non-JS social/AI bots) site-level metadata in
raw HTML, since the CSR WASM `<body>` is empty until WASM boots.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `website/index.html`: add meta description, OG, Twitter card, root canonical, JSON-LD,
  optional preload.

**Files Read (Dependencies):**
- `website/public/logo.png`: branding.
- Domain: `https://fdemon.dev/`.

### Details

Add to `<head>`:
- `<meta name="description" content="…">` — e.g. "fdemon is a blazingly fast terminal UI
  (TUI) for Flutter development — live logs, hot reload, multi-device sessions, DevTools,
  written in Rust."
- Open Graph: `og:title`, `og:description`, `og:image` (`https://fdemon.dev/og-image.png`),
  `og:url` (`https://fdemon.dev/`), `og:type=website`, `og:site_name`.
- Twitter: `twitter:card=summary_large_image`, `twitter:title`, `twitter:description`,
  `twitter:image`.
- `<link rel="canonical" href="https://fdemon.dev/">` (root canonical in raw HTML per
  Google's Dec 2025 guidance).
- `<script type="application/ld+json">` `SoftwareApplication` schema: `name` "Flutter
  Demon", `alternateName` "fdemon", `applicationCategory` "DeveloperApplication",
  `operatingSystem` ["macOS","Linux","Windows"], `description`, `url`, `installUrl`
  `https://fdemon.dev/docs/installation`, `codeRepository`
  `https://github.com/edTheGuy00/fdemon`, `programmingLanguage` "Rust", `license`,
  `featureList` (log viewer, hot reload, multi-device launch, DevTools, native log
  capture, keyboard-driven TUI).
- (Optional) `<link rel="preload" as="fetch" type="application/wasm" crossorigin>` for the
  wasm bundle and `as="script"` for the JS glue.

### Acceptance Criteria

1. description, OG, Twitter, canonical, and valid JSON-LD present in raw HTML.
2. JSON-LD validates in Google Rich Results Test.
3. `cd website && trunk build` still produces a working site.

### Notes

- Keep the existing static `<title>`; per-route titles come from `leptos_meta` (S04/S05),
  which override these defaults once WASM boots.
- Trunk leaves `index.html` head content intact — these tags ship as-is.
</content>

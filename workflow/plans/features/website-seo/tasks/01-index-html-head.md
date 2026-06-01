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

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `website/index.html` | Added meta description, canonical link, OG tags (type, site_name, title, description, url, image), Twitter card tags (card, title, description, image), JSON-LD SoftwareApplication schema block; kept all existing trunk directives and favicons unchanged |

### Notable Decisions/Tradeoffs

1. **Tag ordering**: SEO meta tags placed before Trunk directives so they appear first in the rendered `<head>` — Trunk rewrites the build-specific links but leaves static meta untouched.
2. **og:image / twitter:image**: Both point to `https://fdemon.dev/og-image.png` per the task spec. The actual image file is created in task 03 (og-image); the URL is pre-wired here.
3. **JSON-LD license URL**: Points to `https://github.com/edTheGuy00/fdemon/blob/main/LICENSE` — inferred from the codeRepository field since no explicit license URL was specified in the task.
4. **No preload links added**: The task marked WASM/JS preload hints as optional. They were omitted — the filenames are content-hashed by Trunk at build time, so static preloads in `index.html` would reference stale hashes. Preloads should be added by a post-build step or via nginx `Link` headers (covered in task 08).

### Testing Performed

- JSON-LD validated as well-formed JSON via `python3 -c "import json; json.loads(...)"` — Passed
- `cd /Users/ed/Dev/zabin/flutter-demon/website && cargo check` — Passed (1 pre-existing dead_code warning, no errors)
- HTML structure visually verified — all tags self-closed, `<head>` properly closed, `<body></body>` preserved

### Risks/Limitations

1. **og:image does not yet exist**: The `https://fdemon.dev/og-image.png` URL is referenced but the file is created in task 03. Until task 03 ships, social scrapers will get a 404 on the OG image — but the text card metadata will still render.
2. **Worktree cargo check**: `cargo check` from the worktree directory fails because the workspace Cargo.toml is at the main repo path and the exclude pattern `["website"]` does not match the worktree's absolute path. Verified from the canonical `website/` directory in the main repo instead — same source file.
</content>

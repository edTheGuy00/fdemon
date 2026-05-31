## Task: robots.txt + sitemap.xml

**Objective**: Add standard crawl directives and a sitemap so search engines discover all
routes and AI crawlers are explicitly allowed.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `website/public/robots.txt` (new)
- `website/public/sitemap.xml` (new)

**Files Read (Dependencies):**
- The route list (11 routes) from `website/src/lib.rs`.

### Details

`public/` is copied into `dist/` by the existing `copy-dir public` Trunk link.

`robots.txt`:
```
User-agent: *
Allow: /

User-agent: GPTBot
Allow: /
User-agent: PerplexityBot
Allow: /
User-agent: ClaudeBot
Allow: /

Sitemap: https://fdemon.dev/sitemap.xml
```

`sitemap.xml`: all 11 routes with `https://fdemon.dev/` URLs — `/`, `/docs`,
`/docs/installation`, `/docs/keybindings`, `/docs/mouse`, `/docs/devtools`,
`/docs/native-logs`, `/docs/debugging`, `/docs/configuration`, `/docs/architecture`,
`/docs/changelog`. Home `priority=1.0`, docs `0.8`, `changefreq=monthly`.

### Acceptance Criteria

1. `/robots.txt` and `/sitemap.xml` resolve at the domain root after build/deploy.
2. All 11 routes present in the sitemap with absolute `fdemon.dev` URLs.
3. robots.txt allows GPTBot/PerplexityBot/ClaudeBot and references the sitemap.

### Notes

- Verify Trunk copies `public/*` to the dist root (served at `/robots.txt`, not
  `/public/robots.txt`); if `copy-dir` nests under `/public/`, adjust nginx in S08.
- Optionally generate `sitemap.xml` from the route list in `build.rs` to stay in sync.
</content>

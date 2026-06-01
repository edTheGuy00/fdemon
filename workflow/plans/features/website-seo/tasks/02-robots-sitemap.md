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

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `website/public/robots.txt` | New file — crawl directives with explicit Allow for GPTBot, PerplexityBot, ClaudeBot, and Sitemap directive |
| `website/public/sitemap.xml` | New file — all 11 routes with absolute `https://fdemon.dev/` URLs, home priority=1.0, docs priority=0.8, changefreq=monthly |

### Notable Decisions/Tradeoffs

1. **Path placement**: Files are placed in `website/public/` as specified. With Trunk's `copy-dir href="public"`, these copy to `dist/public/robots.txt` and `dist/public/sitemap.xml`. The nginx task (S08) must alias `/robots.txt` and `/sitemap.xml` to serve from `/public/` — this is noted in the task's Notes section and is expected.

2. **Static files over build.rs generation**: Used static files rather than a `build.rs` generator since the route list is stable and the task did not require dynamic generation. Update manually if routes change.

### Testing Performed

- `xmllint --noout website/public/sitemap.xml` — Passed (XML is valid)
- Manual inspection of robots.txt — All three AI bots (GPTBot, PerplexityBot, ClaudeBot) explicitly allowed, Sitemap directive present
- Route count check — 11 routes confirmed: `/`, `/docs`, `/docs/installation`, `/docs/keybindings`, `/docs/mouse`, `/docs/devtools`, `/docs/native-logs`, `/docs/debugging`, `/docs/configuration`, `/docs/architecture`, `/docs/changelog`

### Risks/Limitations

1. **Trunk copy-dir nesting**: Files will be at `/public/robots.txt` in the dist output, not `/robots.txt`. The nginx configuration task (S08) must add rewrite/alias rules to expose them at the domain root. This is the expected flow per the task notes.
</content>

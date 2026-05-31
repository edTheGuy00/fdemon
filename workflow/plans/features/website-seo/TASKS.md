# Website SEO Improvements (Tier 1 + Tier 2) - Task Index

## Overview

Add SEO infrastructure to the CSR/WASM Leptos site: static `<head>` meta + OG + JSON-LD,
robots.txt, sitemap.xml, OG image, per-route `leptos_meta`, WASM compression (Tier 1),
then build-time headless-Chrome prerendering + keyword landing copy (Tier 2). SSR is a
documented deferral (Tier 3).

**Total Tasks:** 10
**Estimated Hours:** 18-30 hours (S07 prerender dominates at ~1-2 days)

Plan: [PLAN.md](./PLAN.md) · Domain: `https://fdemon.dev/`

## Task Dependency Graph

```
Wave 1 (parallel):
┌────────────────┐ ┌────────────────┐ ┌────────────────┐
│ 01-index-html  │ │ 02-robots-site │ │ 03-og-image    │
└────────────────┘ └────────────────┘ └────────────────┘
┌────────────────┐ ┌────────────────┐ ┌────────────────┐
│ 04-meta-context│ │ 06-wasm-perf   │ │ 10-ssr-decision│
└───────┬────────┘ └────────────────┘ └────────────────┘
        │
        ▼  (+ after the website-content-accuracy plan)
┌────────────────┐
│ 05-per-route-… │  Wave 2
└───────┬────────┘
        ├───────────────────────────┐
        ▼                           ▼
┌────────────────┐         ┌────────────────┐
│ 07-prerender   │ Wave 3  │ 09-landing-copy│ Wave 3 (+ content T01)
└───────┬────────┘         └────────────────┘
        ▼
┌────────────────┐
│ 08-nginx       │  Wave 4
└────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Modules |
|---|------|--------|------------|------------|-------|---------|
| 1 | [01-index-html-head](tasks/01-index-html-head.md) | Not Started | - | 1h | implementor | `website/index.html` |
| 2 | [02-robots-sitemap](tasks/02-robots-sitemap.md) | Not Started | - | 0.5h | implementor | `website/public/{robots.txt,sitemap.xml}` |
| 3 | [03-og-image](tasks/03-og-image.md) | Not Started | - | 1-2h | implementor | `website/public/og-image.png` |
| 4 | [04-meta-context](tasks/04-meta-context.md) | Not Started | - | 0.5-1h | implementor | `website/src/lib.rs` |
| 5 | [05-per-route-meta](tasks/05-per-route-meta.md) | Not Started | 4, content-plan T01/T05/T06 | 2-3h | implementor | `website/src/pages/home.rs`, `pages/docs/*.rs` |
| 6 | [06-wasm-perf](tasks/06-wasm-perf.md) | Not Started | - | 1h | implementor | `website/Cargo.toml`, `website/Trunk.toml` |
| 7 | [07-prerender](tasks/07-prerender.md) | Not Started | 5 | 8-16h | implementor | `website/prerender/*` (new), CI workflow |
| 8 | [08-nginx](tasks/08-nginx.md) | Not Started | 7 | 1-2h | implementor | `website/nginx.conf` |
| 9 | [09-landing-copy](tasks/09-landing-copy.md) | Not Started | 5, content-plan T01 | 2-4h | implementor | `website/src/pages/home.rs`, `website/src/data.rs` |
| 10 | [10-ssr-decision](tasks/10-ssr-decision.md) | Not Started | - | 0.5h | implementor | `workflow/plans/features/website-seo/DECISION-ssr.md` (new) |

## File Overlap Analysis

<!-- The orchestrator uses this section to determine isolation strategy per wave -->

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|----------------------------|
| 01-index-html-head | `website/index.html` | logo, domain |
| 02-robots-sitemap | `website/public/robots.txt`, `website/public/sitemap.xml` | route list |
| 03-og-image | `website/public/og-image.png` (+ optional `og-image.html`) | `website/public/logo.png` |
| 04-meta-context | `website/src/lib.rs` | `leptos_meta` docs |
| 05-per-route-meta | `website/src/pages/home.rs`, `website/src/pages/docs/*.rs` (10) | task 04, content-plan page edits |
| 06-wasm-perf | `website/Cargo.toml`, `website/Trunk.toml` | Leptos binary-size guide |
| 07-prerender | `website/prerender/*` (new), CI workflow | built `dist/` from task 05 |
| 08-nginx | `website/nginx.conf` | task 07 output layout |
| 09-landing-copy | `website/src/pages/home.rs`, `website/src/data.rs` | task 05, content-plan T01 |
| 10-ssr-decision | `DECISION-ssr.md` (new) | - |

### Overlap Matrix

<!-- Read-only overlap is fine — only write overlap forces sequential execution -->

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 / 02 / 03 / 04 / 06 / 10 (Wave 1) | None | Parallel (worktree) |
| 05 ↔ content-plan T02–T06 + home.rs | docs `*.rs` pages, `home.rs` | Sequential (after content plan) |
| 09 ↔ 05 | `home.rs` | Sequential (09 after 05) |
| 09 ↔ content-plan T01 | `data.rs` | Sequential (after content plan) |
| 08 ↔ 07 | None (08 sole writer of `nginx.conf`) | Sequential (08 depends on 07) |
| 07 ↔ all | None (new dir) | Parallel-safe, but depends on 05 |

## Success Criteria

This feature is complete when:

### Tier 1
- [ ] `index.html` has static `<title>`, description, full OG + Twitter card, root
      canonical, and valid `SoftwareApplication` JSON-LD.
- [ ] `robots.txt` (allowing major + AI crawlers, with `Sitemap:`) and `sitemap.xml`
      (all 11 routes) serve at the site root.
- [ ] A 1200×630 OG image renders in social-share validators.
- [ ] Every route sets a unique `<title>`, description, and canonical via `leptos_meta`.
- [ ] WASM is served gzip/brotli-compressed; Lighthouse SEO ≥ 95 on home + a docs page.

### Tier 2
- [ ] All 11 routes return fully-rendered static HTML to a non-JS user agent
      (`curl -A Twitterbot`).
- [ ] Home page has keyword-targeted `<h1>`/`<h2>` copy and descriptive internal links.

### Tier 3
- [ ] A decision record documents why SSR/SSG is deferred and the trigger to revisit.

## Notes

- All `leptos_meta` tags inject client-side; static `index.html` tags (task 01) are the
  fallback for non-JS crawlers, and the prerender (task 07) makes the injected tags
  visible to all crawlers.
- Domain `https://fdemon.dev/` is baked into tasks 01, 02, 05, 08.
- Cross-plan ordering: tasks 05 and 09 must run after the website-content-accuracy plan.
</content>

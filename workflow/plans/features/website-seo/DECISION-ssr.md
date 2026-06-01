# Decision Record: SSR/SSG Migration Deferred

**Date:** 2026-06-01
**Status:** Decided — deferred
**Deciders:** fdemon project

---

## Decision

The fdemon website will **not** migrate to Leptos SSR or Leptos SSG at this time.

The chosen approach is:

- **Tier 1:** CSR/WASM with static `<head>` SEO metadata in `index.html` plus per-route
  `leptos_meta` tags.
- **Tier 2:** Build-time **headless-Chrome prerendering** of all ~11 routes (writing
  static per-route HTML into `dist/`) after `trunk build`, served by the existing
  nginx-static deployment.

Full SSR/SSG migration remains documented as a future option under the trigger conditions
listed below.

---

## Context

The fdemon website is a **Leptos 0.8 CSR/WASM SPA** built with Trunk and served as static
files via nginx (see `website/nginx.conf`). It has approximately 11 routes, all
mostly-static documentation pages and a landing page. The current `index.html` ships an
empty `<body>`, which breaks social-share previews and non-JS crawlers entirely.

Three rendering strategies were evaluated:

| Strategy | Deployment | Complexity | Leptos stability |
|---|---|---|---|
| CSR + static `<head>` + prerender (chosen) | nginx static files | Low | Stable (CSR is mature) |
| Leptos SSG | nginx static files | Medium | Buggy in 0.7/0.8 |
| Leptos SSR | Running Rust server | High | Stable but infra change |

---

## Rationale for Deferring SSR/SSG

### 1. Leptos SSG has active blocking bugs

Leptos SSG (static site generation) in versions 0.7 and 0.8 has three open issues that
make it unreliable for production use:

- **#3226** — SSG generates fallback-only output; individual route HTML files are not
  emitted correctly.
- **#3822** — Static route context is broken; components that rely on `use_context` during
  SSG rendering fail or produce incorrect output.
- **#3871** — Related breakage in the static route collection machinery.

Until these are fixed and a one-command static export works end-to-end, SSG would require
patching or workarounds that add maintenance burden with no clear payoff date.

### 2. SSR requires abandoning nginx-static deployment

Leptos SSR requires a **running Rust HTTP server** (e.g. Axum or Actix-web) to execute
server-side rendering at request time. The current deployment is deliberately simple:
nginx serves files from `dist/` with no application server. Migrating to SSR would
require:

- A persistent Rust process (deployment, health-checks, restarts, memory limits).
- Dual Cargo feature flags (`ssr` vs `csr/hydrate`), two build targets, and a CI pipeline
  that builds and tests both.
- Hydration testing to ensure client-side WASM picks up the server-rendered DOM without
  mismatches.
- Infrastructure changes (container or VPS) rather than the current static-file host.

This is a significant complexity increase for a documentation site.

### 3. Build-time prerendering already closes the crawlability gap

The Tier 2 prerender approach — running headless Chrome over the built `dist/` and saving
rendered HTML per route — achieves the same outcome as SSG without the Leptos SSG bugs or
an SSR server:

- All crawlers (Bing, social scrapers, AI bots such as GPTBot, ClaudeBot, PerplexityBot)
  receive complete HTML on the first response.
- `nginx.conf` uses `try_files $uri $uri/index.html /index.html;` so prerendered files
  are served automatically.
- The step is wired once into the deploy pipeline and regenerated from the live build on
  every deploy — no hand-edited snapshots.

This removes the WASM crawlability ceiling (the only material SEO gap beyond the static
`<head>` metadata work in Tier 1) without any server-side infrastructure.

### 4. Small site; search is a secondary channel

The fdemon website currently has ~11 mostly-static routes. The target audience — terminal-
using Flutter developers — discovers tools primarily via **GitHub, Reddit, and Hacker
News**, not via search. Search is a secondary acquisition channel at the current scale of
this niche tool. The marginal SEO improvement from SSR over prerendering does not justify
the cost difference.

---

## Chosen Alternative: CSR/WASM + Tier-2 Prerender

```
trunk build
  └─> dist/ (WASM SPA — CSR)
        └─> prerender step (headless Chrome, all 11 routes)
              └─> dist/<route>/index.html  (static HTML for crawlers)
                    └─> served by nginx (try_files)
```

Crawlers get full HTML. WASM boots for interactive users. No server required. nginx-static
deployment unchanged.

---

## Trigger Conditions to Revisit

Revisit full Leptos SSR or SSG migration when **any** of the following apply:

1. **Leptos SSG bugs resolved:** Issues #3226, #3822, and #3871 are closed and a
   `cargo leptos build --release` (or equivalent) reliably emits per-route static HTML
   in a single command with no workarounds. At that point SSG provides the same outcome
   as prerendering with tighter Rust integration.

2. **Dynamic routes introduced:** The site gains routes whose content depends on runtime
   data (e.g. search, user-specific pages, server-fetched release notes). Headless-Chrome
   prerendering becomes impractical for large or unbounded route sets; SSR becomes
   the natural fit.

3. **Search becomes a primary growth channel:** Analytics show organic search driving a
   material share of signups or downloads. At that point the investment in SSR
   infrastructure pays off more clearly.

4. **Deployment infrastructure changes anyway:** If the project moves from nginx-static to
   a containerized or VPS deployment for unrelated reasons, the barrier to adding an SSR
   process drops significantly.

---

## References

- `workflow/plans/features/website-seo/PLAN.md` — full research findings, affected
  modules, and phased implementation plan.
- Leptos SSG bug tracker: https://github.com/leptos-rs/leptos/issues/3226,
  https://github.com/leptos-rs/leptos/issues/3822,
  https://github.com/leptos-rs/leptos/issues/3871
- WASM crawlability ceiling:
  https://www.nikouusitalo.com/blog/client-webassembly-websites-are-held-back-by-web-crawlers/
- Google JS SEO basics:
  https://developers.google.com/search/docs/crawling-indexing/javascript/javascript-seo-basics
- nginx prerender pattern: https://gist.github.com/thoop/8165802
- presite (headless prerender tool): https://github.com/egoist/presite

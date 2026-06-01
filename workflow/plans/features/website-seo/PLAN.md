# Plan: Website SEO Improvements

## TL;DR

The fdemon website is a Leptos 0.8 **client-side-rendered (CSR) WASM SPA** served as
static files via nginx. Its raw `index.html` ships an **empty `<body>`** with no
`<title>` beyond a single static line, **no meta description, no Open Graph/Twitter
cards, no canonical, no JSON-LD, no sitemap.xml, no robots.txt**, and `leptos_meta`
(already a dependency) is **never used**. The result: social-share previews are broken,
non-JS crawlers (Bing, AI bots, social scrapers) see nothing, and Google only indexes
the content on its delayed second rendering pass.

This plan delivers SEO in three tiers by ROI:

- **Tier 1 (quick wins, no architecture change):** static `<head>` meta + OG + JSON-LD
  in `index.html`, `robots.txt`, `sitemap.xml`, an OG image, per-route `leptos_meta`
  tags, and WASM bundle/compression tuning.
- **Tier 2 (medium, high value):** build-time **prerendering** of the ~11 routes via
  headless Chrome so *all* crawlers see complete HTML — without migrating to SSR. Plus
  keyword-optimized landing copy.
- **Tier 3 (documented, deferred):** full Leptos SSR/SSG migration — **not recommended**
  for this niche tool at current scale.

Research basis: see Research Findings; sources cited at the end.

---

## Decisions (confirmed)

- **Production domain:** `https://fdemon.dev/` — use for all canonical/OG/sitemap URLs.
- **Scope:** Tier 1 **and** Tier 2 (build-time prerender + landing copy). Tier 3
  (SSR/SSG) is a documented deferral only.
- **Sequencing:** the per-route `leptos_meta` work (Phase 2) and landing-copy work
  (Phase 5) edit the same files as the content-accuracy plan, so they run **after** it.

---

## Background

- Stack: `leptos 0.8 features=["csr"]`, `leptos_meta`/`leptos_router` deps present,
  built with **Trunk** (`Trunk.toml`, `data-wasm-opt="z"` already set on the rust link),
  served by **nginx** (`website/nginx.conf`) as static files in `dist/`.
- Routes (11): `/`, `/docs`, `/docs/installation`, `/docs/keybindings`, `/docs/mouse`,
  `/docs/devtools`, `/docs/native-logs`, `/docs/debugging`, `/docs/configuration`,
  `/docs/architecture`, `/docs/changelog`.
- Current `index.html` head: charset, viewport, one static `<title>`, tailwind+rust+copy
  trunk links, favicons. **Nothing else.**
- No production domain is referenced anywhere in the repo (only the GitHub URL
  `github.com/edTheGuy00/fdemon`). The canonical/OG/sitemap URLs require the real domain
  — see Open Questions.

---

## Research Findings (2025–2026)

**The CSR/WASM crawlability ceiling.** Googlebot (evergreen Chromium/V8) *can* execute
WASM and index CSR content, but only on its delayed **second rendering wave** — the first
wave sees the empty `<body>`. Non-Google crawlers are the hard limit: Bing/Yandex/Baidu
render JS weakly, and **social scrapers (Twitter/Slack/Discord/LinkedIn) and AI crawlers
(GPTBot, ClaudeBot, PerplexityBot) do not execute JS/WASM at all** — they get the empty
skeleton. So OG previews are guaranteed-broken today, and AI-search visibility is ~zero.

**`leptos_meta` caveat.** Per-route `<Title>`/`<Meta>`/`<Link rel=canonical>` via
`leptos_meta` are injected *after* WASM boots — Google sees them in wave 2, but non-JS
crawlers never do. Therefore static `<head>` tags in `index.html` (site-level defaults)
are required *in addition to* `leptos_meta`. Google's Dec 2025 guidance also recommends
the root **canonical be in raw HTML**, not JS-injected.

**Prerendering beats SSR here.** Leptos SSR is the gold standard but costs a running
server (can't stay nginx-static), dual feature flags, two build targets, and hydration
testing. Leptos **SSG has active bugs in 0.7/0.8** (issues #3226, #3822, #3871). For ~11
mostly-static routes, a **build-time headless-Chrome prerender** (e.g. `presite` /
Puppeteer over the built `dist/`, saving rendered HTML per route) gives SSG-like output
with no SSR migration and no runtime server. This is the single highest-value larger
investment.

**Honest ROI.** fdemon's audience (terminal-using Flutter devs) discovers tools via
GitHub, Reddit, HN — search is secondary. Tier 1 captures nearly all available SEO value
cheaply; Tier 2 prerender permanently removes the WASM ceiling and unlocks social + AI
visibility; Tier 3 SSR is poor ROI at this scale.

---

## Affected Modules / Files

**Tier 1**
- `website/index.html` — static `<head>`: `<title>`, `description`, OG (`og:title`,
  `og:description`, `og:image`, `og:url`, `og:type`), Twitter card, root `<link
  rel="canonical">`, and a `<script type="application/ld+json">` `SoftwareApplication`
  block. Add `<link rel="preload">` for the wasm/js and (optional) `manifest.json`.
- `website/public/robots.txt` — **NEW** (allow all incl. GPTBot/PerplexityBot; `Sitemap:`
  directive). Copied to `dist/` via the existing `copy-dir public` trunk link.
- `website/public/sitemap.xml` — **NEW** (11 routes, hand-written or build.rs-generated).
- `website/public/og-image.png` — **NEW** 1200×630 social image.
- `website/public/site.webmanifest` — **NEW/optional** (PWA manifest; icons already
  exist in `public/`).
- `website/src/lib.rs` — add `provide_meta_context()` + a global `<Title formatter=…>`
  in `App`.
- `website/src/pages/home.rs` and each `website/src/pages/docs/*.rs` — add `<Title
  text=…>`, `<Meta name="description" …>`, `<Link rel="canonical" …>`, and per-route OG
  overrides via `leptos_meta`.
- `website/Cargo.toml` — `[profile.wasm-release]` (or confirm release profile);
  `website/Trunk.toml` — confirm `wasm-opt`.
- `website/nginx.conf` — `gzip on; gzip_types application/wasm …` (+ brotli if module
  available); long cache headers on hashed assets.

**Tier 2**
- New build tooling: a prerender step (Node `presite`/Puppeteer script or a Rust
  headless-chrome crate) wired after `trunk build`, writing `dist/<route>/index.html`.
- `website/nginx.conf` — `try_files $uri $uri/index.html /index.html;` so prerendered
  per-route HTML is served to crawlers.
- `website/src/pages/home.rs` (+ `data.rs` copy) — keyword-optimized `<h1>`/`<h2>` and
  intro paragraphs; descriptive internal links.
- CI: prerender step integrated into the deploy pipeline.

**Tier 3 (deferred, documented only)** — SSR/SSG migration touchpoints recorded for
future reference; no files changed now.

---

## Development Phases

### Phase 1 — Static `<head>` SEO foundation (Tier 1a)
`index.html` meta/OG/Twitter/canonical + JSON-LD; `robots.txt`; `sitemap.xml`; OG image.
**No Rust changes.** Fixes social previews and gives every crawler site-level context in
wave 1.

**Milestone:** Pasting the site URL into Slack/Twitter/Discord shows a rich card;
`/robots.txt` and `/sitemap.xml` resolve; Google Search Console accepts the sitemap.

### Phase 2 — `leptos_meta` per-route metadata (Tier 1b)
`provide_meta_context()` + global title formatter in `lib.rs`; per-route `<Title>`,
description, canonical, OG overrides in all 11 route components. Covers Googlebot wave 2
with accurate per-page metadata.

**Milestone:** Each route, when rendered, has a unique `<title>`/description/canonical;
verified in browser devtools and via Google's URL Inspection / Rich Results test.

### Phase 3 — Performance / Core Web Vitals (Tier 1c)
Confirm `wasm-opt=z`, add a size-optimized release profile, enable nginx gzip/brotli for
`application/wasm`, add `preload`/`preconnect`, long-cache hashed assets.

**Milestone:** WASM served compressed; Lighthouse SEO + performance scores improve;
measured before/after bundle size recorded.

### Phase 4 — Build-time prerendering (Tier 2) — gated on approval
Add a headless-Chrome prerender of all 11 routes after `trunk build`, emit static
per-route HTML into `dist/`, adjust `nginx.conf` `try_files`, wire into CI.

**Milestone:** `curl -A "Twitterbot" https://<domain>/docs/installation` returns fully
rendered HTML (not the empty shell); AI/social crawlers see content.

### Phase 5 — Landing copy & content SEO (Tier 2)
Rewrite home `<h1>`/`<h2>` and add 3–4 genuine paragraphs targeting "flutter tui",
"flutter terminal ui", "flutter hot reload cli", "flutter development without IDE";
descriptive internal links to Installation/Configuration; ensure one `<h1>` per page and
correct heading hierarchy; alt text on images.

**Milestone:** Home page reads as keyword-relevant prose with a single clear `<h1>`;
internal links use descriptive anchors.

### Phase 6 (Tier 3) — SSR/SSG evaluation — DEFERRED
Documented decision record only. Revisit if the site grows to many dynamic routes or
search becomes a primary growth channel. Not implemented in this plan.

---

## Edge Cases & Risks

- **Risk:** `leptos_meta` tags overriding static `index.html` tags cause inconsistency
  for JS-capable crawlers. **Mitigation:** keep static tags as sensible site-level
  defaults; per-route `leptos_meta` overrides are strict supersets (same property names).
- **Risk:** No confirmed production domain → wrong canonical/OG/sitemap URLs.
  **Mitigation:** parameterize the domain (build-time env or a single constant); block
  Phase 1 finalization on the domain decision (Open Questions).
- **Risk:** Prerender step adds CI complexity / flakiness (headless Chrome in CI).
  **Mitigation:** make it a separate, cache-friendly CI stage; keep CSR `dist/` as the
  fallback if prerender fails; pin the prerender tool version.
- **Risk:** Prerendered HTML drifting from the live WASM app. **Mitigation:** regenerate
  on every deploy from the same build; never hand-edit snapshots.
- **Risk:** robots.txt accidentally blocking AI crawlers. **Mitigation:** explicitly
  `Allow` GPTBot/PerplexityBot/ClaudeBot; review before deploy.
- **Risk:** File overlap with the Content-Accuracy plan (both edit `pages/docs/*.rs` and
  `home.rs`/`data.rs`). **Mitigation:** sequence content-accuracy first, then add
  `leptos_meta`; or coordinate in the TASKS File Overlap Analysis.

---

## Success Criteria

### Tier 1 (must-have)
- [ ] `index.html` contains static `<title>`, meta description, full OG + Twitter card,
      root canonical, and a valid `SoftwareApplication` JSON-LD block.
- [ ] `robots.txt` (allowing major + AI crawlers, with `Sitemap:`) and `sitemap.xml`
      (all 11 routes) are served at the site root.
- [ ] A 1200×630 OG image exists and renders in social-share validators
      (Twitter Card Validator / Slack / LinkedIn Post Inspector).
- [ ] Every route sets a unique `<title>`, description, and canonical via `leptos_meta`.
- [ ] WASM is served gzip/brotli-compressed; Lighthouse SEO ≥ 95 on home + a docs page.

### Tier 2 (high value)
- [ ] All 11 routes return fully-rendered static HTML to a non-JS user agent
      (verified via `curl -A Twitterbot`).
- [ ] Home page has keyword-targeted `<h1>`/`<h2>` copy and descriptive internal links.

### Tier 3
- [ ] A short decision record documents why SSR/SSG is deferred and the trigger to
      revisit.

---

## Open Questions

1. **What is the production domain?** (Needed for canonical/OG/sitemap URLs.) Is the site
   already deployed somewhere public?
2. **Prerendering (Tier 2): in scope now, or Tier 1 only?** It's the biggest unlock for
   social/AI visibility but adds a headless-Chrome CI step.
3. **OG image:** do you have brand assets (the logo exists) to base a 1200×630 card on,
   or should the plan include designing one?
4. **Prerender tooling preference:** Node (`presite`/Puppeteer) vs a Rust
   `headless_chrome`/`chromiumoxide` build step — any preference given the repo is
   otherwise Rust-only?

---

## References

- `website/index.html`, `website/nginx.conf`, `website/Trunk.toml`, `website/Cargo.toml`
- Onely — Google CAN index WASM: https://www.onely.com/blog/webassembly-experiment/
- WASM crawlability limits:
  https://www.nikouusitalo.com/blog/client-webassembly-websites-are-held-back-by-web-crawlers/
- Google JS SEO basics:
  https://developers.google.com/search/docs/crawling-indexing/javascript/javascript-seo-basics
- `leptos_meta`: https://docs.rs/leptos_meta/ ; Leptos book metadata:
  https://book.leptos.dev/metadata.html
- Leptos SSG bug status: issues #3226, #3822, #3871 (github.com/leptos-rs/leptos)
- nginx prerender pattern: https://gist.github.com/thoop/8165802 ;
  presite: https://github.com/egoist/presite
- Schema.org SoftwareApplication: https://schema.org/SoftwareApplication
- Leptos binary-size guide: https://book.leptos.dev/deployment/binary_size.html
</content>

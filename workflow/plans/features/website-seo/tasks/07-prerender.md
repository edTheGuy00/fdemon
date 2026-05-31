## Task: Build-time headless-Chrome prerender (Tier 2)

**Objective**: Generate static, fully-rendered HTML per route at build time so all
crawlers (Bing, GPTBot, PerplexityBot, social scrapers) see complete content — without an
SSR migration.

**Depends on**: 05-per-route-meta (so snapshots include per-route meta)

**Agent:** implementor

**Estimated Time**: 8-16 hours (~1-2 days)

### Scope

**Files Modified (Write):**
- `website/prerender/*` (new tooling: script + config)
- CI workflow (new or edited): post-build prerender stage.

**Files Read (Dependencies):**
- Built `dist/` from `trunk build` (which includes S05's meta).

### Details

1. `trunk build --release` → CSR WASM output in `dist/`.
2. Serve `dist/` locally and run a headless-Chrome prerenderer over all 11 routes, waiting
   for WASM hydration (known DOM node / network idle), then write the rendered DOM to
   `dist/<route>/index.html`.
3. Tooling: Node `presite`/Puppeteer **or** a Rust `headless_chrome`/`chromiumoxide` build
   step (per Open Question #4; default Puppeteer for maturity). Pin the tool version.
4. Wire into CI as a post-build stage; if prerender fails, fall back to the plain CSR
   `dist/` (don't block deploy on a flaky headless run).

### Acceptance Criteria

1. After build, `dist/docs/installation/index.html` (and all routes) contains fully
   rendered content + correct `<title>`/meta in `<head>`.
2. Prerender runs in CI and is reproducible; tool version pinned.
3. CSR fallback path documented if prerender is skipped.

### Notes

- Snapshots must be regenerated every build from the same artifacts — never hand-edited.
- nginx routing to serve these snapshots is S08.
- The same headless-Chrome tooling can render the OG image (S03).
</content>

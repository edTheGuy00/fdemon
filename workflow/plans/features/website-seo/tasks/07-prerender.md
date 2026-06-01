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

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a01dff02bea08ddf7

### Files Modified

| File | Changes |
|------|---------|
| `website/prerender/package.json` | New — pinned Puppeteer 22.15.0 + serve 14.2.4 |
| `website/prerender/package-lock.json` | New — reproducible lock file (npm lockfileVersion 3) |
| `website/prerender/prerender.js` | New — headless Chrome driver: serves dist/ locally, renders 11 routes, waits for network-idle + DOM marker, writes snapshots |
| `website/prerender/.npmrc` | New — npm config comment |
| `website/prerender/.gitignore` | New — excludes node_modules/ |
| `website/prerender/README.md` | New — documents tooling rationale, local usage, CI integration, CSR fallback path |
| `website/Dockerfile` | Updated — added `prerender` stage (Node 22 + Puppeteer); fallback sentinel logic; `SKIP_PRERENDER` build arg; nginx stage now copies from prerender stage |
| `.github/workflows/publish-site.yml` | Updated — added `skip_prerender` workflow_dispatch input; passes `SKIP_PRERENDER` build arg to Docker |
| `.github/workflows/release.yml` | Updated — passes `SKIP_PRERENDER=0` build arg to Docker (explicit default) |

### Notable Decisions/Tradeoffs

1. **Node + Puppeteer over Rust headless_chrome**: Puppeteer 22 has a stable `waitForNetworkIdle()` API that maps directly to WASM hydration detection. Rust `headless_chrome`/`chromiumoxide` require manual CDP polling. The prerender step is a build tool (never shipped), so Node is acceptable in an otherwise Rust repo.

2. **Fallback via Docker sentinel files**: If `prerender.js` exits non-zero, a `.prerender-failed` sentinel is written but `dist/` is left intact. The nginx stage still copies from the prerender stage regardless — it gets the plain CSR dist/ as fallback. This means headless Chrome flakiness never breaks the Docker build or deploy.

3. **`SKIP_PRERENDER=1` build arg**: Both the manual workflow and release workflow support skipping prerender for emergency deploys. The `publish-site.yml` exposes this as a `workflow_dispatch` boolean input.

4. **Hydration detection strategy**: `waitForNetworkIdle` (≤2 in-flight requests for 500 ms) is the primary signal; a DOM text-content check (`HYDRATION_SELECTOR`) with 5 s timeout is secondary. If both time out we still capture whatever HTML is present — partial render beats the empty shell.

5. **`--single-process` Chrome flag**: Used in Docker/CI environments where `/dev/shm` may be limited and the zygote process model can hang. Accepted tradeoff for build-time rendering.

### Testing Performed

- `node --check prerender.js` — syntax OK (no parse errors)
- Dry-run against a static HTML fixture using local Chrome 148:
  - File server started on port 3738
  - Puppeteer launched Chrome (`Chrome/148.0.7778.181`)
  - Page rendered, `<h1>` and `<title>` confirmed in snapshot
  - `DRY-RUN PASSED`
- Full end-to-end (`trunk build --release` → prerender) **not run in sandbox** — requires `wasm32-unknown-unknown` target + `trunk` CLI, which are not installed locally. Will execute in the Docker CI builder stage.

### Risks/Limitations

1. **WASM hydration timing**: The `networkidle2` heuristic works for the current Leptos CSR setup but may need tuning if the app adds background WebSocket connections or polling (would prevent idle). Mitigation: `WASM_TIMEOUT_MS` env var is configurable.
2. **Chrome version drift**: Puppeteer 22.15.0 pins a Chrome for Testing revision; the `node:22-slim` base image's system Chrome (if any) is not used — Puppeteer downloads its own. This keeps the version deterministic across CI runs.
3. **Docker layer cache**: The `PUPPETEER_CACHE_DIR=/root/.cache/puppeteer` layer will be large (~300 MB for Chrome). The `cache-from: type=gha` Docker Buildx cache in CI should keep rebuild times acceptable.
4. **nginx snapshot routing**: Writing `dist/<route>/index.html` is complete. Serving them to crawlers via `try_files` is task 08 — not done here.
</content>

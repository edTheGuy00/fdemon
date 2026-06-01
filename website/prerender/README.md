# fdemon.dev — Build-time Prerenderer

Generates fully-rendered static HTML for all 11 routes of the fdemon.dev SPA so
non-JS crawlers (Bing, GPTBot, PerplexityBot, social card scrapers) receive
complete content without an SSR migration.

---

## Why headless-Chrome prerender, not SSR?

| Option | Status |
|---|---|
| Leptos SSR (axum server) | Not viable without a running server; breaks the nginx-static deploy |
| Leptos SSG (0.8) | Active bugs (#3226, #3822, #3871) as of 2026 |
| Build-time headless Chrome | **Chosen**: runs after `trunk build`, writes static HTML, zero runtime changes |

---

## Tooling choice: Node + Puppeteer

Puppeteer 22 was chosen over `headless_chrome`/`chromiumoxide` (Rust crates)
because:

- **Maturity**: Puppeteer's Chrome DevTools Protocol surface is stable and
  comprehensively documented.
- **Network-idle API**: `page.waitForNetworkIdle()` is battle-tested for SPA
  hydration detection; the Rust alternatives require manual CDP polling.
- **CI ecosystem**: `ubuntu-latest` runners ship Node 20+; the Docker builder
  installs it in one layer.
- **Version pinning**: exact Puppeteer version (`22.15.0`) in `package.json`
  ensures reproducible builds.

The repository is otherwise Rust-only, but the prerender step is an isolated
build tool (not shipped to end users), so the Node dependency is acceptable.

---

## How it works

```
trunk build --release
       │
       ▼
  dist/  (CSR WASM SPA — empty <body> shell)
       │
       ▼
  prerender.js
    1. Starts a local static-file server on port 3737 (serves dist/)
    2. Launches Puppeteer with headless Chrome
    3. For each of the 11 routes:
         a. page.goto(http://localhost:3737/<route>)
         b. waitForNetworkIdle (≤2 in-flight requests for 500 ms)
         c. Wait for a rendered DOM node with non-empty text content
         d. Serialize document.documentElement.outerHTML
         e. Write to dist/<route>/index.html
    4. Closes browser + server
```

The resulting `dist/` has both the original root `index.html` (for JS-capable
browsers via nginx SPA routing) AND per-route static snapshots (for crawlers via
`try_files $uri $uri/index.html /index.html` — configured by task 08).

---

## Running locally

```bash
# Requires: trunk + wasm32-unknown-unknown target, Node >= 20
cd website
trunk build --release          # produces dist/

cd prerender
npm ci                         # install pinned deps
DIST_DIR=../dist node prerender.js
```

Environment variables:

| Variable | Default | Description |
|---|---|---|
| `DIST_DIR` | `../dist` | Path to built dist/ |
| `PRERENDER_PORT` | `3737` | Local server port |
| `WASM_TIMEOUT_MS` | `30000` | Max wait per route (ms) |
| `CHROME_PATH` | _(Puppeteer default)_ | Override Chrome binary path |

---

## CI integration

The prerender step runs **inside the Docker build** (see `website/Dockerfile`).

The build has two stages:

1. **`builder`** — Rust + Trunk builds the WASM SPA into `/app/dist`.
2. **`prerender`** — Node + Puppeteer runs `prerender.js` over `dist/`.
3. **`serve`** (nginx) — copies `dist/` from whichever stage succeeded.

The key CI property: **if prerender fails, the Docker build falls back to the
CSR `dist/`** and the deploy still succeeds. Headless Chrome flakiness never
blocks a release.

```
builder (Rust/Trunk)
    │
    ├─ success ─→ prerender (Node/Puppeteer) ─→ serve (nginx) ← rendered snapshots
    │                  │ failure
    │                  └─────────────────────→ serve (nginx) ← plain CSR dist/
    │
    └─ failure → Docker build fails (expected — Rust build must pass)
```

---

## CSR fallback path

If prerender is **skipped or fails**:

- The nginx container still starts and serves the CSR SPA correctly for all
  JS-capable browsers (which is 99%+ of real users).
- Non-JS crawlers will receive the empty `<body>` shell — the same behavior as
  before this feature.
- Googlebot (JS-capable) will still index via its delayed second-render wave.
- Social/AI scrapers will get the static `index.html` OG tags (added in task 01)
  but not per-route rendered content.

To force a pure-CSR deploy (e.g. for emergency hotfixes), set the Docker build
arg `SKIP_PRERENDER=1`:

```bash
docker build --build-arg SKIP_PRERENDER=1 -t fdemon-site ./website
```

---

## Snapshot hygiene

- Snapshots are **never hand-edited** — they are always regenerated from the
  same `trunk build` artifacts.
- The `.gitignore` under `website/` should exclude `dist/` to prevent snapshots
  from being committed (they are build artifacts).
- Per-route HTML in `dist/` is overwritten on every build.

---

## What was validated in the sandbox (2026-06-01)

- `node prerender.js --help` → syntax parsed cleanly (no JS errors).
- Script logic dry-run against a trivial static HTML fixture (see validation
  notes in the task completion summary).
- The Chrome executable at `/Applications/Google Chrome.app` responded to
  `--version` (Chrome 148).
- A full end-to-end run (`trunk build --release` → prerender) requires the
  `wasm32-unknown-unknown` target and `trunk` CLI, which were **not available**
  in this sandbox. That run will happen in CI (Docker builder stage).

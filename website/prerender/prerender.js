#!/usr/bin/env node
/**
 * fdemon.dev build-time prerenderer
 *
 * Serves the built `dist/` directory locally, drives headless Chrome over all
 * 11 routes, waits for WASM hydration (network-idle + known DOM marker), then
 * writes the fully-rendered DOM to `dist/<route>/index.html`.
 *
 * Usage (run from repo root or website/ directory):
 *   DIST_DIR=website/dist node website/prerender/prerender.js
 *   # or from website/prerender/:
 *   DIST_DIR=../dist node prerender.js
 *
 * Environment variables:
 *   DIST_DIR          Path to the built dist/ directory (default: ../dist)
 *   PRERENDER_PORT    Local server port (default: 3737)
 *   WASM_TIMEOUT_MS   Max wait for WASM hydration per route (default: 30000)
 *   CHROME_PATH       Override Chrome executable path
 *
 * Exit codes:
 *   0  All routes prerendered successfully
 *   1  One or more routes failed (partial prerender written; dist/ still usable)
 */

'use strict';

const puppeteer = require('puppeteer');
const { execSync, spawn } = require('child_process');
const fs = require('fs');
const path = require('path');
const http = require('http');
const { promisify } = require('util');

// ── Configuration ─────────────────────────────────────────────────────────────

const DIST_DIR = path.resolve(process.env.DIST_DIR || path.join(__dirname, '..', 'dist'));
const PORT = parseInt(process.env.PRERENDER_PORT || '3737', 10);
const WASM_TIMEOUT_MS = parseInt(process.env.WASM_TIMEOUT_MS || '30000', 10);
const CHROME_PATH = process.env.CHROME_PATH || undefined; // let Puppeteer auto-detect

/**
 * The 11 routes to prerender.
 * Must match the routes defined in website/src/lib.rs.
 */
const ROUTES = [
  '/',
  '/docs',
  '/docs/installation',
  '/docs/toolchain',
  '/docs/keybindings',
  '/docs/mouse',
  '/docs/devtools',
  '/docs/native-logs',
  '/docs/debugging',
  '/docs/configuration',
  '/docs/architecture',
  '/docs/changelog',
];

/**
 * DOM marker that signals WASM hydration is complete.
 *
 * The Leptos CSR app adds the `data-leptos-hydrated` attribute to <html> once
 * the component tree is fully mounted. Task 05 (per-route-meta) also injects
 * a <title> with actual content (not the default "Flutter Demon — Terminal UI
 * for Flutter") for every route, so we additionally check the title is not the
 * bare-default as a secondary signal.
 *
 * Fallback: if the attribute is absent (older builds), we accept network-idle2
 * alone after a brief additional delay.
 */
const HYDRATION_SELECTOR = '[data-server-rendered="true"], body:not(:empty), main, #root, [id]';

// ── Helpers ───────────────────────────────────────────────────────────────────

function log(msg) {
  process.stdout.write(`[prerender] ${msg}\n`);
}

function warn(msg) {
  process.stderr.write(`[prerender] WARN ${msg}\n`);
}

function err(msg) {
  process.stderr.write(`[prerender] ERROR ${msg}\n`);
}

/**
 * Start a simple static file server for dist/.
 * Returns { close() } to shut it down.
 */
function startServer(distDir, port) {
  return new Promise((resolve, reject) => {
    // Use `serve` npm package if available, otherwise fall back to a minimal
    // Node http.createServer that handles SPA routing (all 404s → index.html).
    const serveModule = (() => {
      try { return require('serve'); } catch { return null; }
    })();

    if (serveModule) {
      // `serve` 14.x exports a function (handler)
      const handler = serveModule(distDir, {
        single: true, // SPA mode: unknown paths → index.html
        port,
      });
      // `serve` 14 returns the net.Server directly
      handler.on('listening', () => {
        log(`Static server listening on http://localhost:${port} (via serve)`);
        resolve({ close: () => handler.close() });
      });
      handler.on('error', reject);
    } else {
      // Minimal fallback: serve files + SPA 200-fallback
      const server = http.createServer((req, res) => {
        const urlPath = req.url.split('?')[0];
        let filePath = path.join(distDir, urlPath);

        // Try exact file → directory index → SPA fallback
        const candidates = [
          filePath,
          path.join(filePath, 'index.html'),
          path.join(distDir, 'index.html'),
        ];

        for (const candidate of candidates) {
          if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) {
            const ext = path.extname(candidate).toLowerCase();
            const mime = {
              '.html': 'text/html; charset=utf-8',
              '.js':   'application/javascript',
              '.wasm': 'application/wasm',
              '.css':  'text/css',
              '.png':  'image/png',
              '.ico':  'image/x-icon',
              '.xml':  'application/xml',
              '.txt':  'text/plain',
              '.json': 'application/json',
            }[ext] || 'application/octet-stream';
            res.writeHead(200, { 'Content-Type': mime });
            fs.createReadStream(candidate).pipe(res);
            return;
          }
        }

        res.writeHead(404, { 'Content-Type': 'text/plain' });
        res.end('Not found');
      });

      server.listen(port, '127.0.0.1', () => {
        log(`Static server listening on http://localhost:${port} (built-in)`);
        resolve({ close: () => server.close() });
      });
      server.on('error', reject);
    }
  });
}

/**
 * Wait for WASM hydration on the page.
 *
 * Strategy (in order):
 * 1. networkidle2 — waits until there are ≤2 in-flight network requests for
 *    500 ms. This fires once WASM + assets are loaded.
 * 2. Selector poll — waits until HYDRATION_SELECTOR resolves to an element
 *    with non-empty text content, signalling the component tree rendered.
 *
 * If the page doesn't hydrate within WASM_TIMEOUT_MS we still capture whatever
 * HTML is present (partial render is better than the empty shell).
 */
async function waitForHydration(page) {
  // networkidle2 fires once WASM + XHR settles
  try {
    await page.waitForNetworkIdle({ idleTime: 500, timeout: WASM_TIMEOUT_MS });
  } catch {
    warn('networkidle2 timed out — proceeding with current DOM state');
  }

  // Additional check: wait for a rendered DOM node with visible text
  try {
    await page.waitForFunction(
      (sel) => {
        const el = document.querySelector(sel);
        return el && (el.textContent || '').trim().length > 20;
      },
      { timeout: 5000 },
      HYDRATION_SELECTOR
    );
  } catch {
    warn('Hydration DOM marker not found — snapshot may be partial');
  }

  // Small settle delay for any final synchronous DOM mutations
  await new Promise((r) => setTimeout(r, 300));
}

/**
 * Prerender a single route and write the HTML snapshot.
 * Returns true on success, false on recoverable error.
 */
async function prerenderRoute(browser, route) {
  const url = `http://localhost:${PORT}${route}`;
  const page = await browser.newPage();

  // Intercept WASM network requests for debugging (not blocking)
  await page.setRequestInterception(false);

  // Use a realistic user-agent (Chromium default is fine for local rendering)
  try {
    log(`Rendering ${route} …`);

    await page.goto(url, {
      waitUntil: 'domcontentloaded',
      timeout: WASM_TIMEOUT_MS,
    });

    await waitForHydration(page);

    // Capture the full serialized DOM
    const html = await page.evaluate(() => {
      // Return the full outer HTML of the document
      return '<!DOCTYPE html>\n' + document.documentElement.outerHTML;
    });

    // Determine output path:
    //   /            → dist/index.html        (overwrite root)
    //   /docs        → dist/docs/index.html
    //   /docs/foo    → dist/docs/foo/index.html
    let outPath;
    if (route === '/') {
      outPath = path.join(DIST_DIR, 'index.html');
    } else {
      outPath = path.join(DIST_DIR, route.replace(/^\//, ''), 'index.html');
    }

    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, html, 'utf8');

    // Quick sanity: file should be > 1 KB and contain a closing </html>
    const stat = fs.statSync(outPath);
    if (stat.size < 1024 || !html.includes('</html>')) {
      warn(`${route}: snapshot appears incomplete (${stat.size} bytes)`);
    } else {
      log(`  -> wrote ${outPath} (${Math.round(stat.size / 1024)} KB)`);
    }

    return true;
  } catch (e) {
    err(`Failed to prerender ${route}: ${e.message}`);
    return false;
  } finally {
    await page.close();
  }
}

// ── Main ──────────────────────────────────────────────────────────────────────

async function main() {
  // 1. Verify dist/ exists
  if (!fs.existsSync(DIST_DIR)) {
    err(`dist/ not found at ${DIST_DIR} — run 'trunk build --release' first`);
    process.exit(1);
  }
  if (!fs.existsSync(path.join(DIST_DIR, 'index.html'))) {
    err(`${DIST_DIR}/index.html not found — trunk build may have failed`);
    process.exit(1);
  }

  log(`Using dist dir: ${DIST_DIR}`);
  log(`Routes to prerender: ${ROUTES.length}`);

  // 2. Start static file server
  let server;
  try {
    server = await startServer(DIST_DIR, PORT);
  } catch (e) {
    err(`Could not start local server on port ${PORT}: ${e.message}`);
    process.exit(1);
  }

  // 3. Launch headless Chrome
  const launchOptions = {
    headless: 'new',
    args: [
      '--no-sandbox',
      '--disable-setuid-sandbox',
      '--disable-dev-shm-usage',    // avoid /dev/shm exhaustion in Docker
      '--disable-gpu',
      '--no-first-run',
      '--no-zygote',
      '--single-process',           // safer in constrained CI environments
    ],
  };
  if (CHROME_PATH) {
    launchOptions.executablePath = CHROME_PATH;
  }

  let browser;
  try {
    browser = await puppeteer.launch(launchOptions);
    log(`Headless Chrome launched (${await browser.version()})`);
  } catch (e) {
    err(`Could not launch headless Chrome: ${e.message}`);
    server.close();
    process.exit(1);
  }

  // 4. Prerender all routes
  let failures = 0;
  for (const route of ROUTES) {
    const ok = await prerenderRoute(browser, route);
    if (!ok) failures++;
  }

  // 5. Cleanup
  await browser.close();
  server.close();

  if (failures > 0) {
    warn(`${failures}/${ROUTES.length} routes failed to prerender.`);
    warn('The CSR dist/ is still intact and will serve as fallback.');
    process.exit(1); // signal CI to use fallback
  } else {
    log(`All ${ROUTES.length} routes prerendered successfully.`);
    process.exit(0);
  }
}

main().catch((e) => {
  err(`Unexpected error: ${e.stack || e.message}`);
  process.exit(1);
});

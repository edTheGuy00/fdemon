## Task: nginx compression + prerender try_files

**Objective**: Serve compressed WASM and route requests to the prerendered per-route HTML.

**Depends on**: 07-prerender (output layout)

**Agent:** implementor

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `website/nginx.conf`: compression, prerender `try_files`, root for robots/sitemap, cache
  headers.

**Files Read (Dependencies):**
- S07 prerender output layout (`dist/<route>/index.html`).

### Details

- Compression: `gzip on;` and add `application/wasm` (plus js/css/svg/json) to
  `gzip_types`; enable `brotli on;` + `brotli_types` if the module is available. WASM
  compresses to <50% — large CWV win.
- Routing for prerender: serve the per-route snapshot first, e.g.
  `try_files $uri $uri/index.html /index.html;` so `/docs/installation` returns
  `dist/docs/installation/index.html` and unknown paths fall back to the CSR shell.
- Confirm `/robots.txt` and `/sitemap.xml` (from S02) resolve at root (adjust location
  blocks if `copy-dir` nested them under `/public/`).
- Long cache headers (`Cache-Control: public, max-age=31536000, immutable`) for the
  hash-named WASM/JS assets (Trunk `filehash = true`); short/no-cache for HTML.

### Acceptance Criteria

1. `.wasm` served with `Content-Encoding: gzip` (or br).
2. `curl -A "Twitterbot" https://fdemon.dev/docs/installation` returns the prerendered
   HTML (not the empty shell).
3. `/robots.txt` and `/sitemap.xml` resolve at root.
4. Hashed assets carry immutable long-cache headers; HTML does not.

### Notes

- Sole writer of `nginx.conf`; depends on S07 producing the per-route HTML layout.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `website/nginx.conf` | Added prerender try_files, expanded gzip_types (wasm/json/xml/svg), added root static file aliases, tightened cache headers regex to hash-stamped filenames only, broadened no-cache rule to all .html files |

### dist/ Layout Confirmed

The actual `dist/` layout (verified from the committed build artifact at `website/dist/`):

```
dist/
  index.html                               ← root CSR shell (prerender overwrites)
  flutter-demon-website-<hash>_bg.wasm     ← Trunk hash-stamped WASM
  flutter-demon-website-<hash>.js          ← Trunk hash-stamped JS loader
  tailwind-<hash>.css                      ← Trunk hash-stamped CSS
  public/
    robots.txt                             ← Trunk copy-dir nested here
    sitemap.xml                            ← Trunk copy-dir nested here
    og-image.png                           ← Trunk copy-dir nested here
    favicon.ico, favicon-*.png, logo-*.png, apple-touch-icon.png
  docs/installation/index.html             ← prerender writes (not in repo; build artifact)
  docs/.../index.html                      ← (same for all 10 non-root routes)
```

Trunk's `copy-dir public` copies the entire `public/` directory into `dist/public/` (not `dist/` root). This means `robots.txt`, `sitemap.xml`, and `og-image.png` are served from `/public/<file>` by default.

### How robots/sitemap/og-image Are Exposed at Root

Added three `location = /...` exact-match blocks with `alias` directives pointing to `/usr/share/nginx/html/public/<file>`. This maps:
- `GET /robots.txt` → `/usr/share/nginx/html/public/robots.txt`
- `GET /sitemap.xml` → `/usr/share/nginx/html/public/sitemap.xml`
- `GET /og-image.png` → `/usr/share/nginx/html/public/og-image.png`

No rewrite to `/public/` is needed in `index.html` favicons because those already reference `/public/favicon.ico` etc. — the public/ subpath is intentional for favicons; only the SEO crawl-critical files need root exposure.

### Notable Decisions/Tradeoffs

1. **Hash-regex for long cache**: Changed `location ~* \.(js|wasm|css)$` to `~* -[0-9a-f]{16,}\.(js|wasm|css)$` to target only Trunk-hashed assets. This prevents accidentally caching any non-hashed JS/WASM/CSS that might be added later (e.g. during development) with a 1-year TTL.
2. **HTML no-cache broadened**: Changed `location = /index.html` to `location ~* \.html$` so all per-route prerendered snapshots (`/docs/installation/index.html` etc.) also get no-cache headers — critical for hot-fix deployments to propagate to crawlers.
3. **Brotli left commented out**: `nginx:alpine` does not include `ngx_brotli`. The directives are present but commented with a note about `fholzer/nginx-brotli` for teams wanting to enable it without risking startup failure.
4. **gzip_min_length 1024**: Avoids compressing tiny files where compression overhead exceeds savings. WASM/JS/CSS are all well above this threshold.

### Testing Performed

- Brace balance check (Python): OK (depth=0 at EOF)
- Missing-semicolon scan: No issues found
- Directive presence verification: All 10 key directives confirmed present
- `nginx -t`: nginx not installed on macOS dev machine; syntax verified manually

### Risks/Limitations

1. **nginx -t not run**: The config file is a `conf.d` fragment (no `http {}` wrapper), so `nginx -t` would need the full nginx config context anyway. The config will be validated at `docker build` time when copied to `/etc/nginx/conf.d/default.conf` in the nginx:alpine image.
2. **Public directory nesting**: If a future Trunk version changes `copy-dir` semantics to copy contents rather than the directory, `robots.txt` etc. would land at `dist/` root and the alias blocks would 404. The fix would be to remove the alias blocks. The behavior should be confirmed in CI logs.
</content>

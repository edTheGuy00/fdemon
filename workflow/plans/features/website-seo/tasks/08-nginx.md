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
</content>

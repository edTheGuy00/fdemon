# ────────────────────────────────────────────────────────────────────────────
# Stage 1: Build the WASM app with Trunk
# ────────────────────────────────────────────────────────────────────────────
FROM rust:slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev wget ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN rustup toolchain install nightly \
    && rustup default nightly \
    && rustup target add wasm32-unknown-unknown

# Prebuilt trunk binary: `cargo install trunk` compiles trunk with the floating
# nightly toolchain above, which breaks whenever nightly regresses one of
# trunk's pinned deps (e.g. E0119 in `time` with trunk 0.21.14).
ARG TRUNK_VERSION=v0.21.14
RUN wget -qO- "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
    | tar xz -C /usr/local/bin trunk

WORKDIR /app
COPY . .
RUN trunk build --release

# ────────────────────────────────────────────────────────────────────────────
# Stage 2: Prerender all 11 routes with headless Chrome
#
# This stage runs AFTER `trunk build --release` has produced dist/.
# If prerendering succeeds the rendered HTML snapshots are written into dist/.
# If it fails (flaky Chrome, timeout, etc.) we emit a sentinel file and the
# final nginx stage falls back to the plain CSR dist/ without blocking deploy.
#
# To skip prerendering entirely (e.g. emergency hotfix) pass:
#   docker build --build-arg SKIP_PRERENDER=1 ...
# ────────────────────────────────────────────────────────────────────────────
FROM node:22-slim AS prerender

ARG SKIP_PRERENDER=0

# Install Chrome dependencies + Chrome for Testing via Puppeteer
RUN apt-get update && apt-get install -y --no-install-recommends \
    wget \
    ca-certificates \
    fonts-liberation \
    libappindicator3-1 \
    libasound2 \
    libatk-bridge2.0-0 \
    libatk1.0-0 \
    libc6 \
    libcairo2 \
    libcups2 \
    libdbus-1-3 \
    libexpat1 \
    libfontconfig1 \
    libgbm1 \
    libgcc1 \
    libglib2.0-0 \
    libgtk-3-0 \
    libnspr4 \
    libnss3 \
    libpango-1.0-0 \
    libpangocairo-1.0-0 \
    libstdc++6 \
    libx11-6 \
    libx11-xcb1 \
    libxcb1 \
    libxcomposite1 \
    libxcursor1 \
    libxdamage1 \
    libxext6 \
    libxfixes3 \
    libxi6 \
    libxrandr2 \
    libxrender1 \
    libxss1 \
    libxtst6 \
    lsb-release \
    xdg-utils \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /prerender

# Copy pinned package.json and install dependencies
COPY prerender/package.json ./
COPY prerender/.npmrc ./
COPY prerender/package-lock.json ./

# Cache Puppeteer's Chrome download in a Docker layer
ENV PUPPETEER_CACHE_DIR=/root/.cache/puppeteer
RUN npm ci --omit=dev

# Copy the prerender script
COPY prerender/prerender.js ./

# Copy the built dist/ from the builder stage
COPY --from=builder /app/dist /app/dist

# Run prerender with graceful fallback:
#   - Exit 0 (success): snapshots written to /app/dist
#   - Exit non-zero: write sentinel, leave /app/dist untouched (CSR fallback)
RUN if [ "$SKIP_PRERENDER" = "1" ]; then \
      echo "[prerender] SKIP_PRERENDER=1 — skipping headless Chrome step" && \
      touch /app/dist/.prerender-skipped; \
    else \
      DIST_DIR=/app/dist node prerender.js && \
      echo "[prerender] Prerender succeeded" && \
      touch /app/dist/.prerender-ok || \
      (echo "[prerender] Prerender failed — CSR fallback will be used" && \
       touch /app/dist/.prerender-failed); \
    fi

# ────────────────────────────────────────────────────────────────────────────
# Stage 3: Serve with nginx
#
# Serves the CLEAN CSR build from the `builder` stage. Prerendering (Stage 2) is
# currently DISABLED and intentionally NOT referenced here:
#
#   The app mounts via Leptos `mount_to_body` (CSR), which APPENDS the rendered
#   tree to <body>. Serving a prerendered snapshot therefore makes the WASM
#   render a SECOND, duplicate copy — the static snapshot sits inert on top
#   (nav does nothing) while the live app renders below (content "at the bottom"),
#   and navigation jumps back to the stale snapshot. See PR discussion.
#
#   Re-enabling prerender requires a CSR-safe mount (clear <body> before mounting)
#   or an SSR/hydration build — tracked as follow-up. Static <head> meta + OG +
#   JSON-LD (index.html), robots.txt, sitemap.xml, and per-route leptos_meta still
#   provide the bulk of the SEO value without the prerender snapshots.
#
# Because the final image no longer copies from the `prerender` stage, buildx
# skips building it entirely (no headless Chrome at build time).
# ────────────────────────────────────────────────────────────────────────────
FROM nginx:alpine

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

# Validate the nginx config at BUILD time. Without this, a config syntax error
# (e.g. an unquoted regex containing {N,} braces) only surfaces when nginx tries
# to start the container — producing a 502 in production while the image still
# builds and publishes "successfully". `nginx -t` fails the build instead.
RUN nginx -t

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]

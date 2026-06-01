# ────────────────────────────────────────────────────────────────────────────
# Stage 1: Build the WASM app with Trunk
# ────────────────────────────────────────────────────────────────────────────
FROM rust:slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN rustup toolchain install nightly \
    && rustup default nightly \
    && rustup target add wasm32-unknown-unknown
RUN cargo install trunk

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
# Copies dist/ from the prerender stage (which may have rendered snapshots OR
# may be the plain CSR dist/ if prerender failed — either is valid).
# nginx routing for crawler snapshot serving is configured in task 08.
# ────────────────────────────────────────────────────────────────────────────
FROM nginx:alpine

COPY --from=prerender /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

# Validate the nginx config at BUILD time. Without this, a config syntax error
# (e.g. an unquoted regex containing {N,} braces) only surfaces when nginx tries
# to start the container — producing a 502 in production while the image still
# builds and publishes "successfully". `nginx -t` fails the build instead.
RUN nginx -t

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]

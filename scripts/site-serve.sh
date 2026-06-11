#!/usr/bin/env bash
# scripts/site-serve.sh — serve the website (trunk serve) bound to BIND_ADDR.
#
# trunk's default bind is loopback only. Like the VM beds
# (tests/docker/*/docker-compose.yml), this binds to ${BIND_ADDR:-127.0.0.1} —
# loopback by default, or the host's tailnet address when BIND_ADDR is set, so
# other tailnet devices can preview the site without exposing it to the
# LAN/WiFi.
#
# Usage:
#   scripts/site-serve.sh                 # http://$BIND_ADDR:8080 (or 127.0.0.1)
#   scripts/site-serve.sh --release ...   # extra args pass through to `trunk serve`
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
site_dir="$repo_root/website"

[ -d "$site_dir" ] || { echo "error: missing $site_dir (is this the fdemon repo?)" >&2; exit 1; }
command -v trunk >/dev/null || { echo "error: trunk not found (cargo install trunk --locked)" >&2; exit 1; }

bind="${BIND_ADDR:-127.0.0.1}"
echo "==> serving website at http://$bind:8080 (bound to $bind; default trunk port unless overridden)"
cd "$site_dir"
exec trunk serve --address "$bind" "$@"

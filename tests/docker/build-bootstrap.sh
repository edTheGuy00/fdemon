#!/usr/bin/env bash
# tests/docker/build-bootstrap.sh
#
# Build the toolchain-bootstrap install-wizard test images, one per login shell.
# The Rust build layer is compiled once and reused across all three (identical
# build context), so only the first image pays the full compile cost.
#
# Usage (from anywhere — the script cds to the repo root itself):
#   tests/docker/build-bootstrap.sh            # build all: zsh, bash, fish
#   tests/docker/build-bootstrap.sh zsh fish   # build a subset
#
# Resulting images: fdemon-bootstrap-zsh, fdemon-bootstrap-bash, fdemon-bootstrap-fish
set -euo pipefail

# Repo root = two levels up from this script (tests/docker/ -> repo root).
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
dockerfile="tests/docker/toolchain-bootstrap.Dockerfile"

shells=("$@")
if [ "${#shells[@]}" -eq 0 ]; then
  shells=(zsh bash fish)
fi

cd "$repo_root"
for sh in "${shells[@]}"; do
  case "$sh" in
    zsh|bash|fish) ;;
    *) echo "unknown shell '$sh' (use bash|zsh|fish)" >&2; exit 1 ;;
  esac
  echo "==> building fdemon-bootstrap-$sh"
  docker build --build-arg "TEST_SHELL=$sh" -t "fdemon-bootstrap-$sh" -f "$dockerfile" .
done

echo "==> done: ${shells[*]/#/fdemon-bootstrap-}"

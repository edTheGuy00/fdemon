#!/usr/bin/env bash
# tests/docker/build-bootstrap.sh
#
# Build the toolchain-bootstrap install-wizard test images, one per login shell,
# for a chosen distro. The Rust build layer is compiled once and reused across
# the shells of a distro (identical build context), so only the first image of
# each distro pays the full compile cost.
#
# Usage (from anywhere — the script cds to the repo root itself):
#   tests/docker/build-bootstrap.sh                       # debian: zsh bash fish
#   tests/docker/build-bootstrap.sh zsh fish              # debian, subset
#   tests/docker/build-bootstrap.sh --distro fedora       # fedora: zsh bash fish
#   tests/docker/build-bootstrap.sh --distro fedora bash  # fedora, subset
#
# Resulting images:
#   debian:  fdemon-bootstrap-debian-{zsh,bash,fish}
#   fedora:  fdemon-bootstrap-fedora-{zsh,bash,fish}
set -euo pipefail

# Repo root = two levels up from this script (tests/docker/ -> repo root).
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

distro="debian"
shells=()
while [ $# -gt 0 ]; do
  case "$1" in
    --distro) shift; distro="${1:-}"; shift || true ;;
    --distro=*) distro="${1#*=}"; shift ;;
    -h|--help) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) shells+=("$1"); shift ;;
  esac
done

case "$distro" in
  debian) dockerfile="tests/docker/toolchain-bootstrap.Dockerfile";        prefix="fdemon-bootstrap-debian" ;;
  fedora) dockerfile="tests/docker/toolchain-bootstrap-fedora.Dockerfile"; prefix="fdemon-bootstrap-fedora" ;;
  *) echo "unknown distro '$distro' (use debian|fedora)" >&2; exit 1 ;;
esac

if [ "${#shells[@]}" -eq 0 ]; then
  shells=(zsh bash fish)
fi

cd "$repo_root"
for sh in "${shells[@]}"; do
  case "$sh" in
    zsh|bash|fish) ;;
    *) echo "unknown shell '$sh' (use bash|zsh|fish)" >&2; exit 1 ;;
  esac
  echo "==> building $prefix-$sh ($distro)"
  docker build --build-arg "TEST_SHELL=$sh" -t "$prefix-$sh" -f "$dockerfile" .
done

echo "==> done: ${shells[*]/#/$prefix-}"

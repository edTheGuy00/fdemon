#!/usr/bin/env bash
# scripts/win-vm.sh — control the Windows 11 E2E test VM (dockur/windows).
#
# The VM bed lives in tests/docker/windows/ (see its README). This wraps the
# common lifecycle actions so you don't have to remember the btrfs no-CoW
# mitigation, the oem/ binary refresh, or the storage wipe.
#
# Usage:
#   scripts/win-vm.sh up         # start the VM (docker compose up -d)
#   scripts/win-vm.sh down       # stop the VM, KEEP the installed disk (fast restart)
#   scripts/win-vm.sh status     # container status + access URLs
#   scripts/win-vm.sh rebuild    # cross-compile fdemon.exe → stage into shared/ + oem/
#   scripts/win-vm.sh fresh      # FROM SCRATCH: wipe Windows + reinstall (~20-30 min)
#   scripts/win-vm.sh teardown   # stop + delete the VM disk (reclaim disk, no re-up)
#
# Notes:
# - `fresh` re-applies `chattr +C` to the new storage dir (btrfs no-CoW) and
#   refreshes oem/fdemon.exe from shared/ so the clean install auto-stages the
#   latest binary. First boot re-downloads the Windows ISO.
# - To reset just the TOOLCHAIN (no Windows reinstall), run the in-guest
#   PowerShell: C:\fdemon\reset-toolchain.ps1 (shipped via tests/docker/windows/oem/).
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
win_dir="$repo_root/tests/docker/windows"
dockerfile="tests/docker/windows-wine.Dockerfile"
container="fdemon-windows"
image_tag="fdemon-win-builder"

die()  { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

[ -d "$win_dir" ] || die "missing $win_dir (is this the fdemon repo?)"
command -v docker >/dev/null || die "docker not found"

show_access() {
  cat <<EOF

  Windows VM access:
    noVNC (browser, any host):  http://localhost:8006   (or http://<this-host>:8006)
    RDP:                        <host>:3389   user: Docker   pass: admin
    First install takes ~20-30 min (ISO download + setup); watch it via noVNC.
EOF
}

build_and_stage_exe() {
  note "cross-compiling fdemon.exe (x86_64-pc-windows-gnu)…"
  ( cd "$repo_root" && docker build --target builder -t "$image_tag" -f "$dockerfile" . )
  local cid
  cid="$(docker create "$image_tag")"
  docker cp "$cid":/build/target/x86_64-pc-windows-gnu/release/fdemon.exe "$win_dir/shared/fdemon.exe"
  docker cp "$cid":/build/target/x86_64-pc-windows-gnu/release/fdemon.exe "$win_dir/oem/fdemon.exe"
  docker rm "$cid" >/dev/null
  note "staged fdemon.exe → shared/ (live, for the running VM) and oem/ (auto-stage on fresh install)"
}

cmd_up() {
  ( cd "$win_dir" && docker compose up -d )
  show_access
}

cmd_down() {
  ( cd "$win_dir" && docker compose down )
  note "VM stopped; installed disk kept in storage/ (next 'up' boots in ~30-60s)"
}

cmd_status() {
  docker ps --filter "name=$container" --format '{{.Names}}  {{.Status}}  {{.Ports}}' || true
  [ -d "$win_dir/storage" ] && echo "storage/: $(du -sh "$win_dir/storage" 2>/dev/null | cut -f1) (installed VM disk)"
  show_access
}

cmd_rebuild() {
  build_and_stage_exe
  note "for the RUNNING VM, copy it in-guest from the shared folder to C:\\fdemon\\fdemon.exe"
}

cmd_fresh() {
  note "FROM SCRATCH — this deletes the installed Windows VM and reinstalls (~20-30 min)."
  ( cd "$win_dir" && docker compose down ) || true
  note "wiping storage/ …"
  rm -rf "$win_dir/storage"
  mkdir -p "$win_dir/storage"
  if chattr +C "$win_dir/storage" 2>/dev/null; then
    note "applied btrfs no-CoW (chattr +C) to storage/"
  else
    note "chattr +C not applied (non-btrfs or unsupported) — fine on ext4/xfs/zfs/APFS"
  fi
  if [ -f "$win_dir/shared/fdemon.exe" ]; then
    cp -f "$win_dir/shared/fdemon.exe" "$win_dir/oem/fdemon.exe"
    note "refreshed oem/fdemon.exe from shared/ (clean install will auto-stage it)"
  elif [ ! -f "$win_dir/oem/fdemon.exe" ]; then
    note "no fdemon.exe staged yet — run 'scripts/win-vm.sh rebuild' first, or the install.bat smoke will be skipped"
  fi
  ( cd "$win_dir" && docker compose up -d )
  show_access
}

cmd_teardown() {
  ( cd "$win_dir" && docker compose down ) || true
  rm -rf "$win_dir/storage"
  note "VM removed and storage/ deleted (disk reclaimed). 'up'/'fresh' will reinstall."
}

case "${1:-}" in
  up)        cmd_up ;;
  down)      cmd_down ;;
  status)    cmd_status ;;
  rebuild)   cmd_rebuild ;;
  fresh)     cmd_fresh ;;
  teardown)  cmd_teardown ;;
  ""|-h|--help|help)
    sed -n '2,28p' "$0" | sed 's/^# \{0,1\}//'
    ;;
  *) die "unknown command '$1' (try: up | down | status | rebuild | fresh | teardown)" ;;
esac

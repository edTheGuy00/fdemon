#!/usr/bin/env bash
# scripts/macos-vm.sh — control the macOS E2E test VM (dockur/macos).
#
# ⚠️  EULA: Apple's macOS EULA permits macOS only on Apple hardware. dockur/macos
#     says "Only run this container on Apple hardware." This wraps the lifecycle
#     for local/internal testing on a non-Apple host AT YOUR OWN RISK — prefer a
#     real Mac or GitHub Actions `macos` runners. See tests/docker/macos/README.md.
#
# The VM bed lives in tests/docker/macos/. Unlike the Windows bed there is NO
# cross-compile/auto-stage step (macOS binaries can't be built in a Linux
# container without the Apple SDK) and first boot is MANUAL (no oem hook).
#
# Usage:
#   scripts/macos-vm.sh up         # start the VM (docker compose up -d)
#   scripts/macos-vm.sh down       # stop, KEEP the installed disk (fast restart)
#   scripts/macos-vm.sh status     # container status + access URLs
#   scripts/macos-vm.sh fresh      # FROM SCRATCH: wipe macOS + reinstall (manual first boot)
#   scripts/macos-vm.sh teardown   # stop + delete the disk (reclaim space, no re-up)
#
# Get fdemon into the guest (see tests/docker/macos/README.md):
#   - install.sh (released build), or build on a real Mac and drop into shared/,
#     or build in-guest via rustup + Xcode CLT.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mac_dir="$repo_root/tests/docker/macos"
container="fdemon-macos"

die()  { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

[ -d "$mac_dir" ] || die "missing $mac_dir (is this the fdemon repo?)"
command -v docker >/dev/null || die "docker not found"

show_access() {
  cat <<EOF

  macOS VM access:
    noVNC (browser):  http://localhost:8006   (or http://<this-host>:8006)
    VNC:              <host>:5900
    SSH:              <host>:2222  (only after enabling Remote Login in macOS)
    First install is MANUAL via noVNC (Disk Utility erase → Reinstall macOS →
    create a user); ~30-90 min. Then install fdemon (see the README).
EOF
}

cmd_up()     { ( cd "$mac_dir" && docker compose up -d ); show_access; }
cmd_down()   { ( cd "$mac_dir" && docker compose down ); note "stopped; disk kept in storage/ (next 'up' boots in ~1-3 min)"; }

cmd_status() {
  docker ps --filter "name=$container" --format '{{.Names}}  {{.Status}}  {{.Ports}}' || true
  [ -d "$mac_dir/storage" ] && echo "storage/: $(du -sh "$mac_dir/storage" 2>/dev/null | cut -f1) (installed macOS disk)"
  show_access
}

cmd_fresh() {
  note "FROM SCRATCH — deletes the installed macOS VM; first boot is MANUAL (~30-90 min)."
  ( cd "$mac_dir" && docker compose down ) || true
  note "wiping storage/ …"
  rm -rf "$mac_dir/storage"; mkdir -p "$mac_dir/storage"
  if chattr +C "$mac_dir/storage" 2>/dev/null; then
    note "applied btrfs no-CoW (chattr +C) to storage/"
  else
    note "chattr +C not applied (non-btrfs or unsupported) — fine on ext4/xfs/zfs/APFS"
  fi
  ( cd "$mac_dir" && docker compose up -d )
  show_access
}

cmd_teardown() {
  ( cd "$mac_dir" && docker compose down ) || true
  rm -rf "$mac_dir/storage"
  note "VM removed and storage/ deleted (disk reclaimed)."
}

case "${1:-}" in
  up)        cmd_up ;;
  down)      cmd_down ;;
  status)    cmd_status ;;
  fresh)     cmd_fresh ;;
  teardown)  cmd_teardown ;;
  ""|-h|--help|help) sed -n '2,33p' "$0" | sed 's/^# \{0,1\}//' ;;
  *) die "unknown command '$1' (try: up | down | status | fresh | teardown)" ;;
esac

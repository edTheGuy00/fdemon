#!/usr/bin/env bash
# tests/docker/macos/shared/reset-toolchain.sh
#
# Reset the Flutter/Android toolchain state INSIDE the macOS guest so the install
# wizard can be re-tested from a clean toolchain WITHOUT reinstalling macOS.
# (A full macOS reinstall is `scripts/macos-vm.sh fresh` from the host.)
#
# This is shipped via the /shared 9p mount (dockur/macos has no oem hook). Inside
# the guest:  sudo mount_9p shared   then   bash /Volumes/shared/reset-toolchain.sh
# Run it, then open a NEW terminal so the cleaned PATH/env is picked up.
#
# Removes: managed Flutter (~/fvm, ~/flutter), the Android SDK ($ANDROID_HOME /
# ~/Library/Android/sdk), and the installed fdemon binary; strips fdemon's
# marker-fenced PATH/ANDROID_HOME blocks from the shell rc files (zsh/bash/fish).
set -uo pipefail

step() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }

FORCE="${1:-}"
if [ "$FORCE" != "--force" ] && [ "$FORCE" != "-f" ]; then
  printf 'This removes the installed Flutter/Android SDKs + fdemon and cleans shell rc files in THIS macOS guest.\n'
  printf 'Proceed? (y/N) '
  read -r ans
  case "$ans" in [Yy]*) ;; *) echo "Aborted."; exit 1 ;; esac
fi

# 1. Remove SDK directories + the installed binary.
for d in "$HOME/fvm" "$HOME/flutter" "${ANDROID_HOME:-}" "$HOME/Library/Android/sdk" "$HOME/.android/sdk"; do
  if [ -n "$d" ] && [ -e "$d" ]; then step "removing $d"; rm -rf "$d"; fi
done
for b in "$HOME/.local/bin/fdemon" "/usr/local/bin/fdemon"; do
  if [ -e "$b" ]; then step "removing $b"; rm -f "$b"; fi
done

# 2. Strip fdemon's marker-fenced blocks from shell rc files.
#    fdemon writes blocks fenced by these begin/end markers (POSIX + fish).
strip_fence() {
  local file="$1" open="$2" close="$3"
  [ -f "$file" ] || return 0
  if grep -qF "$open" "$file" 2>/dev/null; then
    step "cleaning $open block from $file"
    # Delete the inclusive range from the open marker line to the close marker line.
    sed -i.fdemon-bak "/$(printf '%s' "$open" | sed 's/[][\/.*^$]/\\&/g')/,/$(printf '%s' "$close" | sed 's/[][\/.*^$]/\\&/g')/d" "$file"
  fi
}
for f in "$HOME/.zshenv" "$HOME/.zprofile" "$HOME/.zshrc" "$HOME/.bash_profile" "$HOME/.bashrc" "$HOME/.profile" "$HOME/.config/fish/config.fish"; do
  strip_fence "$f" '# >>> fdemon flutter path >>>' '# <<< fdemon flutter path <<<'
  strip_fence "$f" '# >>> fdemon android env >>>'  '# <<< fdemon android env <<<'
done

# 3. (Optional) uninstall a Homebrew JDK so the JDK prerequisite is missing again.
if command -v brew >/dev/null 2>&1; then
  step "brew uninstall openjdk (best-effort)"
  brew uninstall --ignore-dependencies openjdk 2>/dev/null || true
fi

printf '\n\033[1;32mToolchain reset complete.\033[0m Open a NEW terminal, then relaunch fdemon:\n'
printf '    fdemon ~/test-project\n'

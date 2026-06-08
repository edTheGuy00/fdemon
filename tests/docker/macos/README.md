# macOS E2E test bed (dockur/macos)

A clean macOS VM for cross-OS testing of fdemon — the install wizard and general
behavior — complementing the Windows bed in `../windows/`.

> **⚠️ Legal / EULA.** Apple's macOS EULA permits macOS **only on Apple-branded
> hardware**. dockur/macos itself says *"Only run this container on Apple
> hardware."* Running it on a non-Apple Linux host violates that EULA — local /
> internal developer testing only, at your own risk, never in shared/public CI.
> **Prefer your physical Mac** (and GitHub Actions `macos` runners) for anything
> that matters; this VM is the "clean machine" fallback when you can't reach one.

## When to use what

| Goal | Best vehicle |
|---|---|
| Interactive wizard / TUI walk-through on macOS | **Your physical Mac** (native, full TUI, legal) |
| Automated CI of macOS-specific logic | **GitHub Actions `macos-*` runners** (real Apple HW, free for public repos) |
| A throwaway clean macOS without a spare Mac | this dockur/macos bed (EULA caveat above) |

## Requirements

- `/dev/kvm` on the host (this host has it). x86_64 (Intel) guest only → the
  fdemon binary under test must be **`x86_64-apple-darwin`**.
- ~8 GB RAM for the guest, ~30 GB free disk, and bandwidth for the macOS recovery
  download (~1 GB base; the installer then pulls ~12-13 GB).

## 1. Boot + install macOS (one-time, manual)

dockur/macos has **no unattended/oem hook** — first boot is interactive.

```bash
cd tests/docker/macos
docker compose up -d          # or: scripts/macos-vm.sh up
```
Open <http://localhost:8006> (noVNC) and in the macOS installer:
1. **Disk Utility** → select the largest QEMU HARDDISK → **Erase** (APFS) → quit.
2. **Reinstall macOS** → install onto that disk (~30-90 min, several reboots).
3. Create a user account (remember the username/password).

The `./storage` volume persists this — later boots take ~1-3 min.

## 2. Get fdemon into the guest

**Option A — install the released version (simplest, no build):**
```bash
curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash
# installs the latest GitHub release for x86_64-apple-darwin → ~/.local/bin/fdemon
export PATH="$HOME/.local/bin:$PATH"
```
This installs the **latest published release** — great for general cross-OS
testing, but it does **not** include unreleased branch fixes.

**Option B — test an unreleased branch (e.g. `feat/toolchain-bootstrap`):**
Build on a real Mac and copy the binary in via the share — cross-compiling macOS
binaries from Linux needs the Apple SDK (osxcross), which is impractical and has
its own licensing issues, so build natively:
```bash
# on your physical Mac (Intel, or add --target for a universal/x86_64 build):
cargo build --release --target x86_64-apple-darwin
cp target/x86_64-apple-darwin/release/fdemon  <repo>/tests/docker/macos/shared/
```
Then inside the guest mount the share and install it (next section).

**Option C — build in-guest:** install Xcode CLT (`xcode-select --install`) + rustup
(`curl … sh.rustup.rs | sh`), clone, `cargo build`. Works but slow under QEMU.

## 3. The shared folder (host ↔ guest)

`./shared` on the host is a 9p mount. Inside the guest, after each boot:
```bash
sudo mount_9p shared          # appears at /Volumes/shared
ls /Volumes/shared            # reset-toolchain.sh + any binary you dropped in (Option B)
# install a copied binary:
mkdir -p ~/.local/bin && cp /Volumes/shared/fdemon ~/.local/bin/ && chmod +x ~/.local/bin/fdemon
```

## 4. (Optional) SSH for scripted access

dockur/macos doesn't expose SSH by default. In the guest: System Settings →
General → Sharing → enable **Remote Login**. The compose already maps `2222:22`,
so then: `ssh -p 2222 <user>@localhost`.

## 5. A test project + run the wizard

```bash
mkdir -p ~/test-project/macos
cat > ~/test-project/pubspec.yaml <<'EOF'
name: test_project
description: Toolchain bootstrap E2E test project
dependencies:
  flutter:
    sdk: flutter
environment:
  sdk: ">=3.0.0 <4.0.0"
EOF
fdemon ~/test-project          # wizard opens if Flutter is absent
# verify rc files afterward (zsh is the macOS default):
cat ~/.zprofile ~/.zshenv 2>/dev/null
```

## 6. Reset / teardown

- **Reset just the toolchain** (no macOS reinstall): mount the share and run
  `bash /Volumes/shared/reset-toolchain.sh` (removes SDKs + fdemon, cleans the
  fdemon PATH/ANDROID_HOME fence blocks from the shell rc files).
- **From scratch** (new macOS): `scripts/macos-vm.sh fresh` (wipes `storage/`).
- **Stop / remove:** `scripts/macos-vm.sh down` (keep disk) / `teardown` (delete disk).

## Notes / caveats

- First boot is manual and **not automatable** with dockur/macos (no oem hook, no
  default SSH) — unlike the Windows bed's `install.bat`.
- `storage/` and any binary dropped in `shared/` are gitignored; the compose,
  README, and `shared/reset-toolchain.sh` are tracked.
- Apple-Silicon (arm64) macOS guests are **not** supported by dockur/macos (KVM,
  x86_64 only). For arm64 macOS, use a real M-series Mac or `macos-14`/`-15` CI.

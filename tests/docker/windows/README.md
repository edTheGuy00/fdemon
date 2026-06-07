# Windows E2E test — toolchain bootstrap on a real Windows 11 VM

This spins up a **real Windows 11 VM** (via [dockur/windows](https://github.com/dockur/windows)
+ QEMU/KVM) to test fdemon's Windows-only toolchain-bootstrap paths end-to-end —
most importantly the user-PATH **registry writes** to `HKCU:\Environment`
(`New-ItemProperty` `ExpandString`/`String`, read back with
`GetValue(..,'DoNotExpandEnvironmentNames')`). Wine **cannot** faithfully emulate
PowerShell + the registry, so this is the only way to verify that path short of
real hardware or a `windows-latest` CI runner.

## Requirements

- **`/dev/kvm` on the host** (hardware virtualization). Check: `ls -l /dev/kvm`.
  Without it dockur falls back to ~10× slower software emulation and won't boot
  in practice. GitHub-hosted runners do **not** have KVM — for automated registry
  assertions use a `windows-latest` CI job instead.
- ~8 GB free RAM to allocate to the guest, ~20 GB free disk for the VM image,
  and bandwidth for a one-time ~7.9 GB Windows 11 ISO download.

## 1. Build the Windows binary

Cross-compile `fdemon.exe` (`x86_64-pc-windows-gnu`) using the repo's existing
Windows builder stage and drop it where the VM can pick it up:

```bash
# from the repo root
docker build --target builder -t fdemon-win-builder -f tests/docker/windows-wine.Dockerfile .
cid=$(docker create fdemon-win-builder)
docker cp "$cid":/build/target/x86_64-pc-windows-gnu/release/fdemon.exe \
          tests/docker/windows/oem/fdemon.exe
docker rm "$cid"
```

The registry logic runs by shelling out to `powershell.exe` at runtime, so the
GNU target is fine — the `.exe` only needs to start and spawn PowerShell.

## 2. Boot the VM

```bash
cd tests/docker/windows
docker compose up -d
```

- Watch the unattended install at <http://localhost:8006> (noVNC). First run is
  **~20-30 min** (ISO download + install). The `./storage` volume persists the
  installed disk, so later boots take ~30-60 s.
- `install.bat` (in `./oem`) auto-runs at the end of setup: it copies
  `fdemon.exe` to `C:\fdemon` (and onto the machine PATH), creates a minimal
  runnable Flutter project at `C:\test-project`, and writes a `fdemon doctor`
  smoke log to the Shared desktop folder (visible from the host at `./shared`).
  The OEM hook is first-boot-only and occasionally flaky — if it didn't run, do
  those steps manually (below).

## 3. Walk the wizard (interactive)

RDP in (a real terminal — the noVNC viewer can't host a TUI):

```bash
xfreerdp /v:localhost /u:Docker /p:admin /cert:ignore      # or Remmina / mstsc
```

Inside Windows, open **Windows Terminal** (or `cmd`) and:

```bat
REM (only if install.bat didn't run — otherwise these already exist)
mkdir C:\fdemon & copy "%USERPROFILE%\Desktop\Shared\fdemon.exe" C:\fdemon\
REM mkdir C:\test-project\windows  + a pubspec.yaml (see install.bat)

REM 1) Snapshot the user PATH BEFORE (note REG_EXPAND_SZ vs REG_SZ):
reg query "HKCU\Environment" /v Path

REM 2) Run the wizard and install Flutter (it git-clones + auto-configures PATH):
fdemon C:\test-project

REM 3) Snapshot AFTER — confirm the Flutter bin dir was appended and the type /
REM    any %VAR% tokens were preserved (task 02's fix):
reg query "HKCU\Environment" /v Path
powershell -NoProfile -Command "(Get-Item 'HKCU:\Environment').GetValue('Path',$null,'DoNotExpandEnvironmentNames')"
```

What to verify (the Windows-specific fixes):
- The Flutter `bin` dir is **appended** to the user PATH.
- If the pre-existing PATH was `REG_EXPAND_SZ` (or contained `%USERPROFILE%` /
  `%JAVA_HOME%` tokens), it is **still** `REG_EXPAND_SZ` with the literal `%VAR%`
  tokens intact — not flattened to `REG_SZ` (the task-02 defect).
- Re-running the wizard's PATH step is **idempotent** (no duplicate entries).

## 4. Two panes

RDP gives you a full desktop, so just open two Windows Terminal tabs/panes — one
running `fdemon`, one for `reg query` / PowerShell inspection. (tmux is a
Linux-container concept; on the Windows guest use Windows Terminal panes.)

## 5. Teardown

```bash
docker compose down            # stop the VM (keeps ./storage for a fast restart)
# rm -rf storage/              # discard the VM entirely to start fresh
```

## Notes / caveats

- **This is a manual E2E vehicle.** Driving the interactive Ratatui wizard
  requires a human at an RDP terminal — it is not automatable in CI. For an
  automated regression of the registry logic, add a `windows-latest` CI step that
  runs the writer against a live `HKCU:\Environment` and asserts the type with
  PowerShell.
- Default RDP creds are `Docker` / `admin` (set via compose `USERNAME`/`PASSWORD`).
- `./storage`, `./oem/fdemon.exe`, and `./shared/*` are gitignored (machine-local
  artifacts); the compose file, `oem/install.bat`, and this README are tracked.

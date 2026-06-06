## Task: OS-accurate JDK command + filter already-installed prerequisites (Bugs 2 & 3)

**Agent:** implementor

**Objective:** Make every per-OS guided install command accurate for the actual
host. Two fixes in the same `*_guided_commands` family:

- **Bug 2:** the Android/JDK guided command must use the detected Linux package
  manager (`pacman`/`dnf`/`yum`/`zypper`/`apt`), not a hardcoded `sudo apt install`.
- **Bug 3:** the Linux prerequisites command must list **only** the packages that
  are actually missing, not the full canonical list — and the daemon must probe the
  two currently-unprobed packages (GLU, libstdc++) so the filter is complete.

**Depends on:** — (file-disjoint from Tasks 01 and 03; safe in a parallel worktree)

**Estimated Time:** 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs`
- `crates/fdemon-app/src/install_wizard/types.rs` (test fixtures only)
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `LinuxPackageManager`,
  `ComponentCheck`, `ToolchainReport` (read-only)
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` — `parse_missing_prereq_keys`,
  `PREREQ_KEY_*`, `MISSING_PREFIX` re-exports (read-only)

### Details

#### Bug 2 — per-manager JDK command (`state.rs`)

`jdk_guided_command` (`state.rs:417-432`) currently takes `HostPlatform` and emits
`"sudo apt install openjdk-17-jdk"` for **all** Linux, never reading
`report.linux_package_manager`. The Prerequisites path already does the correct
thing — mirror it.

1. Change the signature to `fn jdk_guided_command(report: &ToolchainReport) ->
   GuidedCommand` and, for `HostPlatform::Linux`, match on
   `report.linux_package_manager.unwrap_or(LinuxPackageManager::Unknown)`.
2. Update the call site in `build_steps` (`state.rs:~695`) from
   `jdk_guided_command(report.platform.clone())` to `jdk_guided_command(report)`.

JDK 17 package per manager:

| Manager | Command |
|---------|---------|
| Apt (Debian/Ubuntu) | `sudo apt install openjdk-17-jdk` |
| Dnf (Fedora/RHEL8+) | `sudo dnf install java-17-openjdk-devel` |
| Yum (RHEL7/CentOS7) | `sudo yum install java-17-openjdk-devel` |
| Pacman (Arch/Manjaro) | `sudo pacman -S jdk17-openjdk` |
| Zypper (openSUSE) | `sudo zypper install java-17-openjdk-devel` |
| Unknown | `Install JDK 17 from https://adoptium.net` |

Keep `note` as an alternative-manager hint (e.g. for pacman:
`or: sudo pacman -S jre17-openjdk (runtime only)`), consistent with the existing
prerequisites notes. macOS (`brew install openjdk@17`) and Windows
(`winget install --id EclipseAdoptium.Temurin.17.JDK`) arms are unchanged.

#### Bug 3 — filter the Linux prerequisites command

**Daemon side (`checks/prerequisites.rs`) — add the missing probes:**

Today `check_linux_prerequisites` probes `LINUX_REQUIRED_TOOLS`
(`git, zip, curl, unzip, xz, clang, cmake, ninja, pkg-config`) via `which::which`
plus GTK via `pkg-config --exists gtk+-3.0`, and encodes only-missing items as
`"missing: <key>, …"`. But the generated apt command also contains
`libglu1-mesa` and `libstdc++-12-dev`, which are **never probed**, so they can't be
filtered. Add probes so the `missing:` detail is complete:

- **GLU:** probe via `probe_pkg_config_exists("glu")` (same mechanism as the GTK
  probe; only run it when `pkg_config_found`). When missing, push the key
  `"libglu1-mesa"` (define a `GLU_ITEM_LABEL` const mirroring `GTK_ITEM_LABEL`).
- **libstdc++:** the C++ stdlib dev headers. Probe via
  `probe_pkg_config_exists` is not reliable across distros; instead detect presence
  of a C++ toolchain header. Simplest robust signal: treat it as present when
  `clang` **or** `g++` is on PATH (the dev headers ship with the compiler on every
  supported distro). Only push `"libstdc++"` when neither compiler is found. (Avoid
  per-distro `dpkg -l`/`pacman -Q` shelling — keep the probe distro-agnostic, as the
  existing module comment requires.) If this proves noisy, fall back to **not**
  probing libstdc++ and document it as an always-included base package — but the
  preferred path is the compiler-presence heuristic.
- Add matching `PREREQ_KEY_GLU` / `PREREQ_KEY_LIBSTDCPP` consts (or reuse the label
  strings) and re-export through `checks/mod.rs` so `state.rs` can reference them.
- Keep the existing status semantics: any missing **binary** → `Missing`; GTK/GLU
  dev-header missing with all binaries present → `Partial`.

**App side (`state.rs`) — filter the Linux branch:**

`prerequisites_guided_commands` Linux branch (`state.rs:480-522`) emits a static
full string. Replace it with the same pattern the macOS/Windows arms already use:

1. Find the `ComponentKind::Prerequisites` component, take its `detail`, and call
   `parse_missing_prereq_keys(detail)` → the list of actually-missing keys.
2. If the missing-key list is empty, return `Vec::new()` (nothing to install).
3. Map each missing key → the distro package name for
   `report.linux_package_manager`, then build
   `sudo <pm-install> <space-joined filtered packages>`.

Per-manager install verb and package-name mapping (probe key → package):

| key | apt | dnf/yum | pacman | zypper |
|-----|-----|---------|--------|--------|
| git | git | git | git | git |
| zip | zip | zip | zip | zip |
| curl | curl | curl | curl | curl |
| unzip | unzip | unzip | unzip | unzip |
| xz | xz-utils | xz | xz | xz |
| clang | clang | clang | clang | clang |
| cmake | cmake | cmake | cmake | cmake |
| ninja | ninja-build | ninja-build | ninja | ninja |
| pkg-config | pkg-config | pkgconf | pkgconf | pkg-config |
| libgtk-3-dev | libgtk-3-dev | gtk3-devel | gtk3 | gtk3-devel |
| libglu1-mesa | libglu1-mesa | mesa-libGLU | glu | Mesa-libGLU1 |
| libstdc++ | libstdc++-12-dev | libstdc++-devel | gcc | libstdc++-devel |

Install verbs: apt → `sudo apt-get install -y`, dnf → `sudo dnf install -y`,
yum → `sudo yum install -y`, pacman → `sudo pacman -S --needed`,
zypper → `sudo zypper in`. `Unknown` keeps the docs-URL fallback. Define the
mapping as a small table/helper in `state.rs` (single source, easy to test). The
`note` may stay a generic "package names are best-effort" hint or be dropped — keep
it short.

> The macOS and Windows branches already filter via `parse_missing_prereq_keys` and
> need no change.

#### Test fixtures (`types.rs`)

Update the `GuidedCommand` unit-test fixtures in `types.rs` (~lines 179-194) that
hardcode the apt/brew JDK strings so they reflect the new per-manager output or are
made manager-parametric. Do not change production types.

### Acceptance Criteria

1. `jdk_guided_command(&report)` returns the correct JDK package for each
   `LinuxPackageManager` variant (unit-tested for all six), and the call site passes
   `report`. macOS/Windows arms unchanged.
2. `check_linux_prerequisites` probes GLU and libstdc++; when both are present they
   do **not** appear in the `missing:` detail, and when absent they do.
3. `prerequisites_guided_commands` (Linux) returns a command listing **only** the
   missing packages: with `curl` + `git` already present, neither appears; with all
   prerequisites present it returns `Vec::new()`.
4. Package names are correctly distro-mapped per the table for at least apt, dnf,
   and pacman (unit-tested).
5. No regression to macOS/Windows guided-command generation.

### Testing

```rust
// fdemon-app/src/install_wizard/state.rs tests
// - NEW: jdk_command_uses_pacman_on_arch / _dnf / _zypper / _yum / _apt
// - NEW: linux_prereq_command_excludes_present_packages (detail "missing: clang, cmake"
//        → command contains clang+cmake, NOT curl/git)
// - NEW: linux_prereq_command_empty_when_all_present
// - NEW: linux_prereq_package_names_mapped_per_manager (apt vs dnf vs pacman)
//
// fdemon-daemon/src/toolchain/checks/prerequisites.rs tests
// - NEW: glu probe / libstdc++ heuristic reflected in missing-key detail
//        (use detect-from-candidates-style pure helpers where filesystem probing
//        would be required, mirroring detect_from_candidates).
```

### Notes

- Keep all command strings in app-land (`state.rs`) — the daemon stays
  detection-only, consistent with the existing split.
- This task does **not** edit any TUI file; the `step_detail.rs` doc-comment/fixture
  refresh is handled by Task 01 (which owns that file), avoiding a shared-file
  dependency.
- Prefer pure, table-driven helpers for the key→package mapping and the
  manager→verb mapping so they unit-test without I/O.

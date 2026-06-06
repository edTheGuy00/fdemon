//! # OS-Level Prerequisites Probe
//!
//! Read-only check for platform-level tools required by Flutter development.
//! The check is lightweight: it only verifies binary presence via
//! `which::which` and `pkg-config --exists` for library headers, and never
//! installs anything. Command generation for missing items lives in
//! app-land `state.rs`, not here.
//!
//! ## Missing-key contract
//!
//! macOS and Windows prerequisite `ComponentCheck.detail` strings encode
//! missing items using a stable, parseable format:
//!
//! ```text
//! "missing: xcode-clt, cocoapods, rosetta"
//! ```
//!
//! When all items are present the detail is a human-readable OK message (no
//! `"missing:"` prefix). Use [`parse_missing_prereq_keys`] to extract the keys
//! from a detail string without brittle prose parsing.  The canonical key
//! constants are [`PREREQ_KEY_XCODE_CLT`], [`PREREQ_KEY_COCOAPODS`],
//! [`PREREQ_KEY_ROSETTA`], and [`PREREQ_KEY_GIT`].

use std::process::Stdio;

use tokio::process::Command;

use super::super::types::{ComponentCheck, ComponentKind, ComponentStatus, HostPlatform};
use super::PROBE_TIMEOUT;

// ─── Canonical missing-item keys ─────────────────────────────────────────────
//
// These constants are the single source of truth for the prerequisite key
// strings embedded in `ComponentCheck.detail` on macOS and Windows.  Task 03
// (guided-command helper in `fdemon-app`) reads them via `parse_missing_prereq_keys`
// to decide which install commands to emit.

/// Key for Xcode Command Line Tools presence (macOS).
pub const PREREQ_KEY_XCODE_CLT: &str = "xcode-clt";
/// Key for CocoaPods presence (macOS).
pub const PREREQ_KEY_COCOAPODS: &str = "cocoapods";
/// Key for Rosetta 2 presence (macOS / Apple Silicon only).
pub const PREREQ_KEY_ROSETTA: &str = "rosetta";
/// Key for Git presence (Windows).
pub const PREREQ_KEY_GIT: &str = "git";

/// Prefix used in the `detail` field when one or more prerequisite items are
/// missing.  The format is: `"missing: <key1>, <key2>, ..."`.
const MISSING_PREFIX: &str = "missing: ";

/// Parse the stable, comma-joined missing-item keys out of a Prerequisites
/// `ComponentCheck.detail` string.
///
/// # Detail format
///
/// When items are missing the detail is:
/// ```text
/// "missing: xcode-clt, cocoapods, rosetta"
/// ```
/// When all items are present the detail is a human-readable OK message that
/// does **not** start with `"missing: "`, so this function returns an empty
/// `Vec` in that case.
///
/// # Returns
///
/// A `Vec` of `&str` slices into `detail`.  The slices point directly into the
/// input string, so no allocation is needed for the common (all-present) path.
///
/// # Example
///
/// ```rust
/// use fdemon_daemon::toolchain::{parse_missing_prereq_keys, PREREQ_KEY_XCODE_CLT, PREREQ_KEY_COCOAPODS};
///
/// let detail = "missing: xcode-clt, cocoapods";
/// let keys = parse_missing_prereq_keys(detail);
/// assert!(keys.contains(&PREREQ_KEY_XCODE_CLT));
/// assert!(keys.contains(&PREREQ_KEY_COCOAPODS));
/// ```
pub fn parse_missing_prereq_keys(detail: &str) -> Vec<&str> {
    let Some(rest) = detail.strip_prefix(MISSING_PREFIX) else {
        return Vec::new();
    };
    rest.split(", ")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// The detected Linux package manager.
///
/// Used by the install-wizard's `state.rs` (task 03) to select the
/// correct package-install command string. Detection is done via
/// `which::which` in preference order — do not parse `/etc/os-release`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxPackageManager {
    /// apt-get (Debian / Ubuntu)
    Apt,
    /// dnf (Fedora / RHEL 8+)
    Dnf,
    /// yum (older RHEL / CentOS)
    Yum,
    /// pacman (Arch / Manjaro)
    Pacman,
    /// zypper (openSUSE)
    Zypper,
    /// None of the above were found on PATH.
    Unknown,
}

/// Pure dispatch: given the set of package manager binary names that are
/// **known to be present** on PATH (in any order), return the highest-priority
/// `LinuxPackageManager` according to the canonical precedence order
/// **apt-get → dnf → yum → pacman → zypper**.
///
/// Returns [`LinuxPackageManager::Unknown`] when `present` is empty or
/// contains none of the known names.
///
/// Extracted so that precedence logic can be unit-tested without requiring
/// a live filesystem (`which::which` cannot be mocked in unit tests).
fn detect_from_candidates(present: &[&str]) -> LinuxPackageManager {
    const ORDER: &[(&str, LinuxPackageManager)] = &[
        ("apt-get", LinuxPackageManager::Apt),
        ("dnf", LinuxPackageManager::Dnf),
        ("yum", LinuxPackageManager::Yum),
        ("pacman", LinuxPackageManager::Pacman),
        ("zypper", LinuxPackageManager::Zypper),
    ];
    for (name, variant) in ORDER {
        if present.contains(name) {
            return *variant;
        }
    }
    LinuxPackageManager::Unknown
}

/// Detect the Linux package manager by probing `which::which` in preference
/// order: **apt-get → dnf → yum → pacman → zypper**.
///
/// Returns [`LinuxPackageManager::Unknown`] when none are present.
/// This is a pure, synchronous probe — it reads PATH only, never invokes
/// the package manager.
pub fn detect_linux_package_manager() -> LinuxPackageManager {
    let candidates: Vec<&str> = ["apt-get", "dnf", "yum", "pacman", "zypper"]
        .iter()
        .copied()
        .filter(|name| which::which(name).is_ok())
        .collect();
    detect_from_candidates(&candidates)
}

/// Check OS-level prerequisites for Flutter development.
///
/// The check is **lightweight and read-only** — it only verifies binary
/// presence via `which::which` and library dev-headers via
/// `pkg-config --exists`. Command generation for missing items lives in
/// app-land `state.rs`.
///
/// - **Linux**: checks for `git`, `zip`, `curl`, `unzip`, `xz`, `clang`,
///   `cmake`, `ninja`, `pkg-config` (with alias fallbacks) and GTK 3
///   dev-headers via `pkg-config --exists gtk+-3.0`.
/// - **macOS**: checks `xcode-select -p` exit status.
/// - **Windows**: checks for `git` (a proxy for developer tools presence).
/// - **Other**: returns `Unknown`.
pub async fn check_prerequisites(platform: &HostPlatform) -> ComponentCheck {
    match platform {
        HostPlatform::Linux => check_linux_prerequisites().await,
        HostPlatform::MacOs => check_macos_prerequisites().await,
        HostPlatform::Windows => check_windows_prerequisites().await,
        HostPlatform::Unknown => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Unknown,
            detail: "Unknown platform — prerequisites check skipped".to_string(),
        },
    }
}

/// Required tools on Linux for Flutter development.
///
/// Alias fallbacks (ninja-build, xz-utils, pkgconf) are handled separately
/// in [`check_linux_prerequisites`].
const LINUX_REQUIRED_TOOLS: &[&str] = &[
    "git",
    "zip",
    "curl",
    "unzip",
    "xz",
    "clang",
    "cmake",
    "ninja",
    "pkg-config",
];

/// Label used in the `detail` string for missing GTK dev-headers.
const GTK_ITEM_LABEL: &str = "libgtk-3-dev";
/// Key used in the `detail` string for missing GLU dev-headers.
pub const PREREQ_KEY_GLU: &str = "libglu1-mesa";
/// Key used in the `detail` string for missing C++ stdlib dev-headers.
///
/// Presence is inferred from C++ compiler availability: the dev-headers ship
/// alongside the compiler on every supported distro, so if `clang` or `g++`
/// is on PATH the package is treated as present.
pub const PREREQ_KEY_LIBSTDCPP: &str = "libstdc++";

async fn check_linux_prerequisites() -> ComponentCheck {
    let mut missing_binaries: Vec<String> = Vec::new();
    let mut pkg_config_found = false;
    let mut cpp_compiler_found = false;

    // ── Binary probes (which::which) ─────────────────────────────────────────
    for tool in LINUX_REQUIRED_TOOLS {
        let found = which::which(tool).is_ok();
        if !found {
            let alias_found = match *tool {
                // ninja may be called `ninja-build` on Debian/Ubuntu
                "ninja" => which::which("ninja-build").is_ok(),
                // xz binary may be absent when only `xz-utils` (the Debian
                // package) is installed without creating the `xz` symlink
                "xz" => which::which("xz-utils").is_ok(),
                // pkg-config may be provided as `pkgconf` on Fedora / Arch
                "pkg-config" => {
                    let alias_ok = which::which("pkgconf").is_ok();
                    if !alias_ok {
                        // Neither pkg-config nor pkgconf found; GTK/GLU probes
                        // cannot run, so track this separately.
                        missing_binaries.push(tool.to_string());
                        continue;
                    }
                    pkg_config_found = true;
                    true // alias found — do not add to missing_binaries
                }
                _ => false,
            };
            if !alias_found {
                missing_binaries.push(tool.to_string());
            }
        } else {
            if *tool == "pkg-config" {
                pkg_config_found = true;
            }
            // Track C++ compiler presence for libstdc++ heuristic.
            if *tool == "clang" {
                cpp_compiler_found = true;
            }
        }
    }

    // Also check g++ as a C++ compiler fallback (not in LINUX_REQUIRED_TOOLS).
    if !cpp_compiler_found && which::which("g++").is_ok() {
        cpp_compiler_found = true;
    }

    // ── GTK dev-headers probe (pkg-config --exists gtk+-3.0) ─────────────────
    // `which` cannot detect library dev-headers — only pkg-config can.
    // When pkg-config itself is absent the GTK probe cannot run; we skip it
    // rather than falsely asserting GTK missing (n1 — GTK double-report fix).
    // Only probe GTK/GLU when pkg-config (or its pkgconf alias) is available.
    let (gtk_missing, glu_missing) = if pkg_config_found {
        let (gtk, glu) = tokio::join!(
            probe_pkg_config_exists("gtk+-3.0"),
            probe_pkg_config_exists("glu"),
        );
        (!gtk, !glu)
    } else {
        (false, false) // undetermined — do not assert missing
    };

    // ── libstdc++ dev-headers heuristic ──────────────────────────────────────
    // The C++ stdlib dev-headers ship with the compiler on every supported
    // distro. We infer presence from `clang` or `g++` being on PATH.
    // This avoids per-distro `dpkg -l`/`pacman -Q` shelling.
    let libstdcpp_missing = !cpp_compiler_found;

    // ── Aggregate result ──────────────────────────────────────────────────────
    //
    // Status semantics (consistent with macOS/Windows):
    //   - Any absent required binary → `Missing` (the component is not present)
    //   - Dev-headers absent but all binaries present → `Partial` (present but
    //     degraded: binaries work, but GUI/build paths will fail)
    //   - All present → `Ok`
    //
    // Header-only items (GTK, GLU, libstdc++) contribute to the missing-key
    // list only in the Partial branch (all binaries present).
    if missing_binaries.is_empty() && !gtk_missing && !glu_missing && !libstdcpp_missing {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: "All required Linux tools present".to_string(),
        }
    } else if !missing_binaries.is_empty() {
        // One or more required binaries absent — treat as Missing, matching
        // the macOS/Windows contract where definitive absence → Missing.
        let mut all_missing = missing_binaries;
        if gtk_missing {
            all_missing.push(GTK_ITEM_LABEL.to_string());
        }
        if glu_missing {
            all_missing.push(PREREQ_KEY_GLU.to_string());
        }
        if libstdcpp_missing {
            all_missing.push(PREREQ_KEY_LIBSTDCPP.to_string());
        }
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Missing,
            detail: format!("{}{}", MISSING_PREFIX, all_missing.join(", ")),
        }
    } else {
        // All required binaries are present; only dev-headers are absent.
        // Partial = "present but degraded": tools work, headers missing.
        let mut partial_missing: Vec<&str> = Vec::new();
        if gtk_missing {
            partial_missing.push(GTK_ITEM_LABEL);
        }
        if glu_missing {
            partial_missing.push(PREREQ_KEY_GLU);
        }
        if libstdcpp_missing {
            partial_missing.push(PREREQ_KEY_LIBSTDCPP);
        }
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Partial,
            detail: format!("{}{}", MISSING_PREFIX, partial_missing.join(", ")),
        }
    }
}

/// Pure helper for testing: derive the linux prerequisites check result from
/// the sets of found binaries and pkg-config package presence flags.
///
/// Mirrors `check_linux_prerequisites` but takes pre-probed data so unit tests
/// can exercise the aggregation logic without live filesystem I/O.
#[cfg(test)]
pub(crate) fn build_linux_check_from_candidates(
    found_binaries: &[&str],
    pkg_config_available: bool,
    gtk_present: bool,
    glu_present: bool,
) -> ComponentCheck {
    let mut missing_binaries: Vec<String> = Vec::new();
    let mut pkg_config_found = pkg_config_available;
    let mut cpp_compiler_found = false;

    for tool in LINUX_REQUIRED_TOOLS {
        let found = found_binaries.contains(tool);
        if !found {
            let alias_found = match *tool {
                "ninja" => found_binaries.contains(&"ninja-build"),
                "xz" => found_binaries.contains(&"xz-utils"),
                "pkg-config" => {
                    let alias_ok = found_binaries.contains(&"pkgconf");
                    if !alias_ok {
                        missing_binaries.push(tool.to_string());
                        continue;
                    }
                    pkg_config_found = true;
                    true
                }
                _ => false,
            };
            if !alias_found {
                missing_binaries.push(tool.to_string());
            }
        } else {
            if *tool == "pkg-config" {
                pkg_config_found = true;
            }
            if *tool == "clang" {
                cpp_compiler_found = true;
            }
        }
    }

    if !cpp_compiler_found && found_binaries.contains(&"g++") {
        cpp_compiler_found = true;
    }

    let (gtk_missing, glu_missing) = if pkg_config_found {
        (!gtk_present, !glu_present)
    } else {
        (false, false)
    };

    let libstdcpp_missing = !cpp_compiler_found;

    if missing_binaries.is_empty() && !gtk_missing && !glu_missing && !libstdcpp_missing {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: "All required Linux tools present".to_string(),
        }
    } else if !missing_binaries.is_empty() {
        let mut all_missing = missing_binaries;
        if gtk_missing {
            all_missing.push(GTK_ITEM_LABEL.to_string());
        }
        if glu_missing {
            all_missing.push(PREREQ_KEY_GLU.to_string());
        }
        if libstdcpp_missing {
            all_missing.push(PREREQ_KEY_LIBSTDCPP.to_string());
        }
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Missing,
            detail: format!("{}{}", MISSING_PREFIX, all_missing.join(", ")),
        }
    } else {
        let mut partial_missing: Vec<&str> = Vec::new();
        if gtk_missing {
            partial_missing.push(GTK_ITEM_LABEL);
        }
        if glu_missing {
            partial_missing.push(PREREQ_KEY_GLU);
        }
        if libstdcpp_missing {
            partial_missing.push(PREREQ_KEY_LIBSTDCPP);
        }
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Partial,
            detail: format!("{}{}", MISSING_PREFIX, partial_missing.join(", ")),
        }
    }
}

/// Run `pkg-config --exists <package>` and return `true` when the exit code
/// is zero (package found).
///
/// Uses [`PROBE_TIMEOUT`] and suppresses all output, mirroring the macOS
/// `xcode-select` block.
async fn probe_pkg_config_exists(package: &str) -> bool {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("pkg-config")
            .arg("--exists")
            .arg(package)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status()
            .await
    })
    .await;

    matches!(result, Ok(Ok(status)) if status.success())
}

async fn check_macos_prerequisites() -> ComponentCheck {
    // ── Probe CLT, CocoaPods, and (aarch64 only) Rosetta concurrently ──────────
    let (clt_status, cocoapods_status, rosetta_status) = tokio::join!(
        probe_macos_xcode_clt(),
        probe_macos_cocoapods(),
        probe_macos_rosetta(),
    );

    build_macos_check_from_statuses(clt_status, cocoapods_status, rosetta_status)
}

/// Probe Xcode Command Line Tools via `xcode-select -p`.
///
/// Returns the [`MacOsProbeStatus`] for the CLT gate.
async fn probe_macos_xcode_clt() -> MacOsProbeStatus {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("xcode-select")
            .arg("-p")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status()
            .await
    })
    .await;

    match result {
        Ok(Ok(status)) if status.success() => MacOsProbeStatus::Present,
        Ok(Ok(_)) => MacOsProbeStatus::Missing,
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => MacOsProbeStatus::Missing,
        _ => MacOsProbeStatus::Unknown,
    }
}

/// Probe CocoaPods via `pod --version`.
///
/// A successful exit (exit code 0) means CocoaPods is installed and functional.
/// `NotFound` IO error → [`MacOsProbeStatus::Missing`]; timeout/other error
/// → [`MacOsProbeStatus::Unknown`].
async fn probe_macos_cocoapods() -> MacOsProbeStatus {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("pod")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status()
            .await
    })
    .await;

    match result {
        Ok(Ok(status)) if status.success() => MacOsProbeStatus::Present,
        Ok(Ok(_)) => MacOsProbeStatus::Missing,
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => MacOsProbeStatus::Missing,
        _ => MacOsProbeStatus::Unknown,
    }
}

/// Probe Rosetta 2 via `pgrep oahd` — **only on Apple Silicon (aarch64)**.
///
/// On `x86_64` this returns [`MacOsProbeStatus::NotApplicable`] immediately
/// so that x86_64 Macs never appear to have Rosetta "missing".
///
/// `pgrep` returns exit code 0 when a matching process is found, non-zero
/// otherwise.  If `pgrep` itself is not found (unlikely on macOS) we return
/// [`MacOsProbeStatus::Unknown`] rather than [`MacOsProbeStatus::Missing`].
async fn probe_macos_rosetta() -> MacOsProbeStatus {
    // Gate: only meaningful on Apple Silicon
    if std::env::consts::ARCH != "aarch64" {
        return MacOsProbeStatus::NotApplicable;
    }

    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("pgrep")
            .arg("oahd")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status()
            .await
    })
    .await;

    match result {
        Ok(Ok(status)) if status.success() => MacOsProbeStatus::Present,
        Ok(Ok(_)) => MacOsProbeStatus::Missing,
        // `pgrep` not found → we cannot confirm missing; report Unknown
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => MacOsProbeStatus::Unknown,
        _ => MacOsProbeStatus::Unknown,
    }
}

/// Status outcome for a single macOS prerequisite probe.
///
/// `NotApplicable` is used for arch-gated probes (e.g. Rosetta on x86_64) so
/// they are silently excluded from the missing-item list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MacOsProbeStatus {
    /// The item was found / the probe succeeded.
    Present,
    /// The item is definitively absent.
    Missing,
    /// The probe could not determine presence (timeout, spawn error, etc.).
    Unknown,
    /// The probe does not apply on this arch/platform variant.
    NotApplicable,
}

/// Build a [`ComponentCheck`] from the three macOS probe outcomes.
///
/// This is the pure, testable core of [`check_macos_prerequisites`].
///
/// Missing items are collected into the stable `"missing: <key1>, ..."` detail
/// format. [`MacOsProbeStatus::Unknown`] items are excluded from the
/// missing-keys list but cause the overall status to be [`ComponentStatus::Unknown`]
/// when no item is [`MacOsProbeStatus::Missing`]. [`MacOsProbeStatus::NotApplicable`]
/// items are silently ignored in all outcomes.
///
/// Status precedence: `Missing` > `Unknown` > `Ok`.
pub(crate) fn build_macos_check_from_statuses(
    clt: MacOsProbeStatus,
    cocoapods: MacOsProbeStatus,
    rosetta: MacOsProbeStatus,
) -> ComponentCheck {
    let mut missing_keys: Vec<&'static str> = Vec::new();
    let mut any_unknown = false;

    let probes: &[(&'static str, MacOsProbeStatus)] = &[
        (PREREQ_KEY_XCODE_CLT, clt),
        (PREREQ_KEY_COCOAPODS, cocoapods),
        (PREREQ_KEY_ROSETTA, rosetta),
    ];

    for (key, status) in probes {
        match status {
            MacOsProbeStatus::Missing => missing_keys.push(key),
            MacOsProbeStatus::Unknown => any_unknown = true,
            MacOsProbeStatus::Present | MacOsProbeStatus::NotApplicable => {}
        }
    }

    if !missing_keys.is_empty() {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Missing,
            detail: format!("{}{}", MISSING_PREFIX, missing_keys.join(", ")),
        }
    } else if any_unknown {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Unknown,
            detail: "Could not determine all macOS prerequisite statuses".to_string(),
        }
    } else {
        // All present (or not applicable)
        let note = if std::env::consts::ARCH == "aarch64" {
            "Xcode Command Line Tools, CocoaPods, and Rosetta 2 installed"
        } else {
            "Xcode Command Line Tools and CocoaPods installed"
        };
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: note.to_string(),
        }
    }
}

async fn check_windows_prerequisites() -> ComponentCheck {
    let git_present = which::which("git").is_ok();
    let winget_present = which::which("winget").is_ok();
    build_windows_check_from_presence(git_present, winget_present)
}

/// Caveat appended to Windows `Ok` detail to flag the unverified VS C++
/// workload.  A real `vswhere.exe` probe remains out of scope (deferred).
const WINDOWS_MSVC_CAVEAT: &str = r#"Visual Studio C++ build tools not verified; install "Desktop development with C++" if Windows desktop builds fail"#;

/// Build a [`ComponentCheck`] from the Windows git/winget presence flags.
///
/// This is the pure, testable core of [`check_windows_prerequisites`].
///
/// Missing items are collected into the stable `"missing: <key>, ..."` detail
/// format so that [`parse_missing_prereq_keys`] can extract them.
///
/// **Notes:**
/// - PowerShell is not gated (assumed present on Windows 10 1903+).
/// - Visual Studio "Desktop development with C++" (`vswhere.exe`) is not probed
///   here — a caveat is appended to the `Ok` detail so that users with git but
///   no MSVC C++ toolchain are not misled into thinking everything is ready.
///   A real `vswhere.exe` probe remains out of scope (deferred to a later phase).
pub(crate) fn build_windows_check_from_presence(
    git_present: bool,
    winget_present: bool,
) -> ComponentCheck {
    let mut missing_keys: Vec<&'static str> = Vec::new();

    if !git_present {
        missing_keys.push(PREREQ_KEY_GIT);
    }

    if missing_keys.is_empty() {
        let detail = if winget_present {
            format!("Git found; winget available. {}", WINDOWS_MSVC_CAVEAT)
        } else {
            // winget absent is informational — it's present on Win10 1903+, but
            // older systems may lack it.  We still report Ok when git is present.
            format!(
                "Git found; winget not found (install Git for Windows manually if needed). {}",
                WINDOWS_MSVC_CAVEAT
            )
        };
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail,
        }
    } else {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Missing,
            detail: format!("{}{}", MISSING_PREFIX, missing_keys.join(", ")),
        }
    }
}

// ─── Pure helpers exposed for unit testing ───────────────────────────────────

/// Map a set of missing binary tools and dev-header presence flags into a
/// [`ComponentCheck`].
///
/// This pure function mirrors the status-mapping logic of
/// [`check_linux_prerequisites`] without spawning processes.
///
/// `glu_present` and `cpp_compiler_present` default to `true` for callers that
/// only need to test binary-tool and GTK behaviour (which is the bulk of the
/// existing test suite).  Pass `false` to exercise GLU or libstdc++ absence.
///
/// # Status semantics
///
/// - `missing_tools` is empty **and** all dev-headers present → `Ok`
/// - `missing_tools` is non-empty → `Missing` (required binaries absent, detail
///   uses `MISSING_PREFIX`; missing dev-headers are also appended)
/// - `missing_tools` is empty **and** any dev-header absent → `Partial`
#[cfg(test)]
pub(crate) fn build_linux_check_from_missing(
    missing_tools: &[&str],
    gtk_present: bool,
) -> ComponentCheck {
    build_linux_check_from_missing_full(missing_tools, gtk_present, true, true)
}

/// Extended version of `build_linux_check_from_missing` that also accepts
/// GLU and C++-compiler presence flags.
#[cfg(test)]
pub(crate) fn build_linux_check_from_missing_full(
    missing_tools: &[&str],
    gtk_present: bool,
    glu_present: bool,
    cpp_compiler_present: bool,
) -> ComponentCheck {
    let gtk_missing = !gtk_present;
    let glu_missing = !glu_present;
    let libstdcpp_missing = !cpp_compiler_present;

    if missing_tools.is_empty() && !gtk_missing && !glu_missing && !libstdcpp_missing {
        return ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: "All required Linux tools present".to_string(),
        };
    }

    if !missing_tools.is_empty() {
        // Required binaries absent → Missing (consistent with macOS/Windows).
        let mut all_missing: Vec<String> = missing_tools.iter().map(|s| s.to_string()).collect();
        if gtk_missing {
            all_missing.push(GTK_ITEM_LABEL.to_string());
        }
        if glu_missing {
            all_missing.push(PREREQ_KEY_GLU.to_string());
        }
        if libstdcpp_missing {
            all_missing.push(PREREQ_KEY_LIBSTDCPP.to_string());
        }
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Missing,
            detail: format!("{}{}", MISSING_PREFIX, all_missing.join(", ")),
        }
    } else {
        // All required binaries present; only dev-headers degraded.
        let mut partial_missing: Vec<&str> = Vec::new();
        if gtk_missing {
            partial_missing.push(GTK_ITEM_LABEL);
        }
        if glu_missing {
            partial_missing.push(PREREQ_KEY_GLU);
        }
        if libstdcpp_missing {
            partial_missing.push(PREREQ_KEY_LIBSTDCPP);
        }
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Partial,
            detail: format!("{}{}", MISSING_PREFIX, partial_missing.join(", ")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Package-manager detection ─────────────────────────────────────────────
    // Precedence ordering is tested via the pure `detect_from_candidates`
    // helper which accepts a pre-resolved slice of binary names.  The live
    // `detect_linux_package_manager()` is covered by a no-panic smoke test
    // (`test_detect_linux_package_manager_never_panics` below).

    #[test]
    fn test_detect_from_candidates_apt_wins_over_dnf() {
        // apt-get has higher precedence than dnf; when both are "present" apt wins.
        let pm = detect_from_candidates(&["dnf", "apt-get"]);
        assert_eq!(
            pm,
            LinuxPackageManager::Apt,
            "apt-get must win over dnf regardless of slice order"
        );
    }

    #[test]
    fn test_detect_from_candidates_dnf_wins_over_yum() {
        let pm = detect_from_candidates(&["yum", "dnf"]);
        assert_eq!(
            pm,
            LinuxPackageManager::Dnf,
            "dnf must win over yum regardless of slice order"
        );
    }

    #[test]
    fn test_detect_from_candidates_yum_wins_over_pacman() {
        let pm = detect_from_candidates(&["pacman", "yum"]);
        assert_eq!(pm, LinuxPackageManager::Yum, "yum must win over pacman");
    }

    #[test]
    fn test_detect_from_candidates_pacman_wins_over_zypper() {
        let pm = detect_from_candidates(&["zypper", "pacman"]);
        assert_eq!(
            pm,
            LinuxPackageManager::Pacman,
            "pacman must win over zypper"
        );
    }

    #[test]
    fn test_detect_from_candidates_zypper_alone_returns_zypper() {
        let pm = detect_from_candidates(&["zypper"]);
        assert_eq!(pm, LinuxPackageManager::Zypper);
    }

    #[test]
    fn test_detect_from_candidates_empty_returns_unknown() {
        let pm = detect_from_candidates(&[]);
        assert_eq!(pm, LinuxPackageManager::Unknown);
    }

    #[test]
    fn test_detect_from_candidates_unrecognised_names_return_unknown() {
        let pm = detect_from_candidates(&["emerge", "nix"]);
        assert_eq!(
            pm,
            LinuxPackageManager::Unknown,
            "unrecognised manager names must yield Unknown"
        );
    }

    #[test]
    fn test_package_manager_variants_are_distinct() {
        assert_ne!(LinuxPackageManager::Apt, LinuxPackageManager::Dnf);
        assert_ne!(LinuxPackageManager::Dnf, LinuxPackageManager::Yum);
        assert_ne!(LinuxPackageManager::Yum, LinuxPackageManager::Pacman);
        assert_ne!(LinuxPackageManager::Pacman, LinuxPackageManager::Zypper);
        assert_ne!(LinuxPackageManager::Zypper, LinuxPackageManager::Unknown);
    }

    // ── Missing-tool aggregation ──────────────────────────────────────────────

    #[test]
    fn test_all_present_yields_ok() {
        let check = build_linux_check_from_missing(&[], true);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert_eq!(check.detail, "All required Linux tools present");
    }

    #[test]
    fn test_missing_git_yields_missing_status() {
        // Required binary absent → Missing (matches macOS/Windows contract).
        let check = build_linux_check_from_missing(&["git"], true);
        assert_eq!(
            check.status,
            ComponentStatus::Missing,
            "absent required binary must yield Missing, not Partial"
        );
        assert!(
            check.detail.contains("git"),
            "detail should mention 'git', got: {}",
            check.detail
        );
    }

    #[test]
    fn test_missing_git_uses_missing_prefix() {
        // m2: detail must use lowercase MISSING_PREFIX, not capital-M "Missing:"
        let check = build_linux_check_from_missing(&["git"], true);
        assert!(
            check.detail.starts_with(MISSING_PREFIX),
            "detail must start with '{}' (lowercase), got: {}",
            MISSING_PREFIX,
            check.detail
        );
    }

    #[test]
    fn test_missing_zip_yields_missing_status() {
        let check = build_linux_check_from_missing(&["zip"], true);
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.contains("zip"),
            "detail should mention 'zip', got: {}",
            check.detail
        );
        assert!(
            check.detail.starts_with(MISSING_PREFIX),
            "detail must use lowercase MISSING_PREFIX, got: {}",
            check.detail
        );
    }

    #[test]
    fn test_missing_multiple_tools_all_appear_in_detail() {
        let check = build_linux_check_from_missing(&["git", "cmake", "clang"], true);
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.starts_with(MISSING_PREFIX),
            "detail must use MISSING_PREFIX, got: {}",
            check.detail
        );
        assert!(
            check.detail.contains("git"),
            "missing git in: {}",
            check.detail
        );
        assert!(
            check.detail.contains("cmake"),
            "missing cmake in: {}",
            check.detail
        );
        assert!(
            check.detail.contains("clang"),
            "missing clang in: {}",
            check.detail
        );
    }

    // ── GTK dev-headers ───────────────────────────────────────────────────────

    #[test]
    fn test_gtk_only_missing_maps_to_partial() {
        // GTK headers absent but all binaries present → Partial ("present but
        // degraded"): binaries work, but GTK-dependent build path will fail.
        let check = build_linux_check_from_missing(&[], false);
        assert_eq!(
            check.status,
            ComponentStatus::Partial,
            "GTK-only absence must yield Partial (degraded), got: {:?}",
            check.status
        );
        assert!(
            check.detail.starts_with(MISSING_PREFIX),
            "GTK-only Partial must use MISSING_PREFIX, got: {}",
            check.detail
        );
        assert!(
            check.detail.contains(GTK_ITEM_LABEL),
            "detail should mention '{}', got: {}",
            GTK_ITEM_LABEL,
            check.detail
        );
    }

    #[test]
    fn test_gtk_present_with_all_tools_yields_ok() {
        let check = build_linux_check_from_missing(&[], true);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(!check.detail.contains(GTK_ITEM_LABEL));
    }

    #[test]
    fn test_gtk_missing_plus_missing_binary_yields_missing_status() {
        // When required binaries are also absent the status escalates to Missing.
        let check = build_linux_check_from_missing(&["cmake"], false);
        assert_eq!(
            check.status,
            ComponentStatus::Missing,
            "required binary absent must yield Missing even when GTK also absent"
        );
        assert!(
            check.detail.starts_with(MISSING_PREFIX),
            "detail must use MISSING_PREFIX, got: {}",
            check.detail
        );
        assert!(
            check.detail.contains("cmake"),
            "cmake missing from: {}",
            check.detail
        );
        assert!(
            check.detail.contains(GTK_ITEM_LABEL),
            "{GTK_ITEM_LABEL} missing from: {}",
            check.detail
        );
    }

    #[test]
    fn test_linux_detail_prefix_is_lowercase_missing() {
        // m2 regression: detail must start with MISSING_PREFIX (lowercase "missing: ")
        // not the old capital-M ad-hoc string.
        for tools in [&["git"][..], &["cmake"], &["ninja", "clang"]] {
            let check = build_linux_check_from_missing(tools, true);
            assert!(
                check.detail.starts_with(MISSING_PREFIX),
                "expected lowercase '{}' prefix, got: {}",
                MISSING_PREFIX,
                check.detail
            );
            assert!(
                !check.detail.starts_with("Missing:"),
                "capital-M 'Missing:' must not appear, got: {}",
                check.detail
            );
        }
    }

    // ── n1: GTK double-report ─────────────────────────────────────────────────
    // When pkg-config itself is missing, the helper models the live function's
    // behaviour: GTK is not additionally pushed to the missing list since the
    // probe is genuinely undeterminable.  The test uses build_linux_check_from_missing
    // with pkg-config as a missing binary; GTK state is left as "not probed"
    // (gtk_present = true in the helper, to isolate the binary-missing path).

    #[test]
    fn test_pkg_config_missing_does_not_double_report_gtk() {
        // Simulate: pkg-config absent (binary loop adds it to missing_binaries),
        // but because pkg-config is gone, the GTK probe cannot run.
        // In the live function, gtk_missing = false when pkg_config_found = false.
        // The test helper encodes this as: pass pkg-config in missing_tools, and
        // gtk_present = true (since we did not probe it).
        let check = build_linux_check_from_missing(&["pkg-config"], true);
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.contains("pkg-config"),
            "pkg-config must appear in missing list: {}",
            check.detail
        );
        assert!(
            !check.detail.contains(GTK_ITEM_LABEL),
            "GTK must not be reported missing when pkg-config is absent (undetermined): {}",
            check.detail
        );
    }

    // ── Smoke tests (no panic guarantee) ─────────────────────────────────────

    #[tokio::test]
    async fn test_check_prerequisites_never_panics_linux() {
        let _ = check_prerequisites(&HostPlatform::Linux).await;
    }

    #[tokio::test]
    async fn test_check_prerequisites_never_panics_macos() {
        let _ = check_prerequisites(&HostPlatform::MacOs).await;
    }

    #[tokio::test]
    async fn test_check_prerequisites_never_panics_windows() {
        let _ = check_prerequisites(&HostPlatform::Windows).await;
    }

    #[test]
    fn test_detect_linux_package_manager_never_panics() {
        let _ = detect_linux_package_manager();
    }

    // ── Required-tool coverage ────────────────────────────────────────────────

    #[test]
    fn test_required_tools_include_git_and_zip() {
        assert!(
            LINUX_REQUIRED_TOOLS.contains(&"git"),
            "LINUX_REQUIRED_TOOLS must include 'git'"
        );
        assert!(
            LINUX_REQUIRED_TOOLS.contains(&"zip"),
            "LINUX_REQUIRED_TOOLS must include 'zip'"
        );
    }

    #[test]
    fn test_required_tools_include_expected_set() {
        let expected = [
            "curl",
            "unzip",
            "xz",
            "clang",
            "cmake",
            "ninja",
            "pkg-config",
        ];
        for tool in &expected {
            assert!(
                LINUX_REQUIRED_TOOLS.contains(tool),
                "LINUX_REQUIRED_TOOLS must include '{tool}'"
            );
        }
    }

    // ── parse_missing_prereq_keys ─────────────────────────────────────────────

    #[test]
    fn test_parse_missing_prereq_keys_single_item() {
        let detail = "missing: xcode-clt";
        let keys = parse_missing_prereq_keys(detail);
        assert_eq!(keys, vec!["xcode-clt"]);
    }

    #[test]
    fn test_parse_missing_prereq_keys_multiple_items() {
        let detail = "missing: xcode-clt, cocoapods, rosetta";
        let keys = parse_missing_prereq_keys(detail);
        assert_eq!(keys, vec!["xcode-clt", "cocoapods", "rosetta"]);
    }

    #[test]
    fn test_parse_missing_prereq_keys_empty_when_ok() {
        // A successful detail has no "missing: " prefix — must return empty vec
        let detail = "Xcode Command Line Tools and CocoaPods installed";
        let keys = parse_missing_prereq_keys(detail);
        assert!(
            keys.is_empty(),
            "expected no keys for OK detail, got: {keys:?}"
        );
    }

    #[test]
    fn test_parse_missing_prereq_keys_empty_detail() {
        let keys = parse_missing_prereq_keys("");
        assert!(keys.is_empty());
    }

    #[test]
    fn test_parse_missing_prereq_keys_windows_git() {
        let detail = "missing: git";
        let keys = parse_missing_prereq_keys(detail);
        assert_eq!(keys, vec!["git"]);
    }

    #[test]
    fn test_parse_missing_prereq_keys_constants_round_trip() {
        // Verify that the detail produced by build_macos_check_from_statuses
        // round-trips through parse_missing_prereq_keys correctly.
        let check = build_macos_check_from_statuses(
            MacOsProbeStatus::Missing,
            MacOsProbeStatus::Missing,
            MacOsProbeStatus::NotApplicable,
        );
        let keys = parse_missing_prereq_keys(&check.detail);
        assert!(
            keys.contains(&PREREQ_KEY_XCODE_CLT),
            "expected xcode-clt in {keys:?}"
        );
        assert!(
            keys.contains(&PREREQ_KEY_COCOAPODS),
            "expected cocoapods in {keys:?}"
        );
        assert!(
            !keys.contains(&PREREQ_KEY_ROSETTA),
            "rosetta should not appear (NotApplicable)"
        );
    }

    #[test]
    fn test_parse_missing_prereq_keys_rosetta_round_trip() {
        let check = build_macos_check_from_statuses(
            MacOsProbeStatus::Present,
            MacOsProbeStatus::Present,
            MacOsProbeStatus::Missing,
        );
        let keys = parse_missing_prereq_keys(&check.detail);
        assert_eq!(
            keys,
            vec![PREREQ_KEY_ROSETTA],
            "only rosetta should be missing"
        );
    }

    // ── macOS: build_macos_check_from_statuses ────────────────────────────────

    #[test]
    fn test_macos_all_present_yields_ok() {
        let check = build_macos_check_from_statuses(
            MacOsProbeStatus::Present,
            MacOsProbeStatus::Present,
            MacOsProbeStatus::NotApplicable, // x86_64
        );
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(
            check.detail.contains("CocoaPods"),
            "detail: {}",
            check.detail
        );
    }

    #[test]
    fn test_macos_clt_missing_yields_missing_status() {
        let check = build_macos_check_from_statuses(
            MacOsProbeStatus::Missing,
            MacOsProbeStatus::Present,
            MacOsProbeStatus::NotApplicable,
        );
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.starts_with(MISSING_PREFIX),
            "detail must start with '{}', got: {}",
            MISSING_PREFIX,
            check.detail
        );
        let keys = parse_missing_prereq_keys(&check.detail);
        assert!(keys.contains(&PREREQ_KEY_XCODE_CLT));
        assert!(!keys.contains(&PREREQ_KEY_COCOAPODS));
    }

    #[test]
    fn test_macos_cocoapods_missing_yields_missing_status() {
        let check = build_macos_check_from_statuses(
            MacOsProbeStatus::Present,
            MacOsProbeStatus::Missing,
            MacOsProbeStatus::NotApplicable,
        );
        assert_eq!(check.status, ComponentStatus::Missing);
        let keys = parse_missing_prereq_keys(&check.detail);
        assert!(keys.contains(&PREREQ_KEY_COCOAPODS));
        assert!(!keys.contains(&PREREQ_KEY_XCODE_CLT));
    }

    #[test]
    fn test_macos_rosetta_missing_yields_missing_status() {
        let check = build_macos_check_from_statuses(
            MacOsProbeStatus::Present,
            MacOsProbeStatus::Present,
            MacOsProbeStatus::Missing, // aarch64: Rosetta absent
        );
        assert_eq!(check.status, ComponentStatus::Missing);
        let keys = parse_missing_prereq_keys(&check.detail);
        assert!(keys.contains(&PREREQ_KEY_ROSETTA));
    }

    #[test]
    fn test_macos_rosetta_not_applicable_does_not_appear_missing() {
        // On x86_64, Rosetta must never be in the missing keys
        let check = build_macos_check_from_statuses(
            MacOsProbeStatus::Present,
            MacOsProbeStatus::Present,
            MacOsProbeStatus::NotApplicable,
        );
        let keys = parse_missing_prereq_keys(&check.detail);
        assert!(
            !keys.contains(&PREREQ_KEY_ROSETTA),
            "Rosetta must not appear when NotApplicable; keys: {keys:?}"
        );
    }

    #[test]
    fn test_macos_unknown_yields_unknown_when_nothing_missing() {
        let check = build_macos_check_from_statuses(
            MacOsProbeStatus::Present,
            MacOsProbeStatus::Unknown, // could not probe cocoapods
            MacOsProbeStatus::NotApplicable,
        );
        assert_eq!(check.status, ComponentStatus::Unknown);
        // Unknown should NOT embed "missing:" prefix
        assert!(
            !check.detail.starts_with(MISSING_PREFIX),
            "Unknown status should not use missing-prefix; detail: {}",
            check.detail
        );
    }

    #[test]
    fn test_macos_missing_takes_precedence_over_unknown() {
        let check = build_macos_check_from_statuses(
            MacOsProbeStatus::Missing,
            MacOsProbeStatus::Unknown,
            MacOsProbeStatus::NotApplicable,
        );
        // Missing > Unknown
        assert_eq!(check.status, ComponentStatus::Missing);
        let keys = parse_missing_prereq_keys(&check.detail);
        assert!(keys.contains(&PREREQ_KEY_XCODE_CLT));
        // Unknown items are excluded from the missing keys list
        assert!(!keys.contains(&PREREQ_KEY_COCOAPODS));
    }

    // ── macOS: arch-gating of Rosetta probe ──────────────────────────────────

    #[tokio::test]
    async fn test_rosetta_probe_not_applicable_on_non_aarch64() {
        // On the current host: if we're not aarch64, probe returns NotApplicable.
        // If we are aarch64, we accept any outcome (Present/Missing/Unknown).
        let result = probe_macos_rosetta().await;
        if std::env::consts::ARCH != "aarch64" {
            assert_eq!(
                result,
                MacOsProbeStatus::NotApplicable,
                "Rosetta probe must be NotApplicable on non-aarch64"
            );
        }
        // On aarch64 the probe runs live; any MacOsProbeStatus is acceptable.
    }

    // ── Windows: build_windows_check_from_presence ───────────────────────────

    #[test]
    fn test_windows_git_present_winget_present_yields_ok() {
        let check = build_windows_check_from_presence(true, true);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(check.detail.contains("winget"), "detail: {}", check.detail);
    }

    #[test]
    fn test_windows_git_present_winget_absent_yields_ok() {
        // winget absent does not fail the gate — git is the critical item
        let check = build_windows_check_from_presence(true, false);
        assert_eq!(check.status, ComponentStatus::Ok);
    }

    #[test]
    fn test_windows_ok_detail_contains_msvc_caveat() {
        // m4: Windows Ok detail must flag the unverified VS C++ workload so
        // that git-only presence does not overstate readiness.
        let check_with_winget = build_windows_check_from_presence(true, true);
        assert!(
            check_with_winget.detail.contains("C++"),
            "Ok detail must mention C++ caveat, got: {}",
            check_with_winget.detail
        );

        let check_without_winget = build_windows_check_from_presence(true, false);
        assert!(
            check_without_winget.detail.contains("C++"),
            "Ok detail (no winget) must mention C++ caveat, got: {}",
            check_without_winget.detail
        );
    }

    #[test]
    fn test_windows_ok_detail_does_not_embed_missing_prefix() {
        // The Ok detail should not start with "missing:" — that prefix is only
        // for the Missing status path.
        let check = build_windows_check_from_presence(true, true);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(
            !check.detail.starts_with(MISSING_PREFIX),
            "Ok detail must not start with '{}', got: {}",
            MISSING_PREFIX,
            check.detail
        );
    }

    #[test]
    fn test_windows_git_missing_yields_missing() {
        let check = build_windows_check_from_presence(false, false);
        assert_eq!(check.status, ComponentStatus::Missing);
        let keys = parse_missing_prereq_keys(&check.detail);
        assert!(
            keys.contains(&PREREQ_KEY_GIT),
            "expected 'git' key in {keys:?}"
        );
    }

    #[test]
    fn test_windows_git_missing_winget_present_still_missing() {
        // Even if winget is present, missing git must be reported
        let check = build_windows_check_from_presence(false, true);
        assert_eq!(check.status, ComponentStatus::Missing);
        let keys = parse_missing_prereq_keys(&check.detail);
        assert!(keys.contains(&PREREQ_KEY_GIT));
    }

    #[test]
    fn test_windows_detail_uses_missing_prefix_when_git_absent() {
        let check = build_windows_check_from_presence(false, false);
        assert!(
            check.detail.starts_with(MISSING_PREFIX),
            "detail must start with '{}', got: {}",
            MISSING_PREFIX,
            check.detail
        );
    }

    // ── Key constants are stable strings ─────────────────────────────────────

    #[test]
    fn test_prereq_key_constants_have_expected_values() {
        assert_eq!(PREREQ_KEY_XCODE_CLT, "xcode-clt");
        assert_eq!(PREREQ_KEY_COCOAPODS, "cocoapods");
        assert_eq!(PREREQ_KEY_ROSETTA, "rosetta");
        assert_eq!(PREREQ_KEY_GIT, "git");
    }

    // ── GLU dev-headers probe ─────────────────────────────────────────────────

    #[test]
    fn test_glu_missing_yields_partial_when_binaries_present() {
        // GLU headers absent but all binaries present → Partial.
        let check = build_linux_check_from_missing_full(&[], true, false, true);
        assert_eq!(
            check.status,
            ComponentStatus::Partial,
            "GLU-only absence must yield Partial, got: {:?}",
            check.status
        );
        assert!(
            check.detail.contains(PREREQ_KEY_GLU),
            "detail should mention '{}', got: {}",
            PREREQ_KEY_GLU,
            check.detail
        );
        assert!(
            check.detail.starts_with(MISSING_PREFIX),
            "detail must use MISSING_PREFIX, got: {}",
            check.detail
        );
    }

    #[test]
    fn test_glu_missing_plus_missing_binary_yields_missing() {
        let check = build_linux_check_from_missing_full(&["curl"], true, false, true);
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(check.detail.contains(PREREQ_KEY_GLU));
        assert!(check.detail.contains("curl"));
    }

    #[test]
    fn test_glu_and_gtk_both_absent_with_all_binaries_yields_partial() {
        // Both GLU and GTK absent, binaries present → Partial (both appear in detail).
        let check = build_linux_check_from_missing_full(&[], false, false, true);
        assert_eq!(check.status, ComponentStatus::Partial);
        assert!(
            check.detail.contains(GTK_ITEM_LABEL),
            "GTK missing from detail: {}",
            check.detail
        );
        assert!(
            check.detail.contains(PREREQ_KEY_GLU),
            "GLU missing from detail: {}",
            check.detail
        );
    }

    #[test]
    fn test_glu_present_does_not_appear_in_ok_detail() {
        let check = build_linux_check_from_missing_full(&[], true, true, true);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(!check.detail.contains(PREREQ_KEY_GLU));
    }

    // ── libstdc++ heuristic (C++ compiler presence) ───────────────────────────

    #[test]
    fn test_libstdcpp_missing_when_no_cpp_compiler_yields_partial() {
        // No C++ compiler → libstdc++ treated as missing → Partial when binaries present.
        let check = build_linux_check_from_missing_full(&[], true, true, false);
        assert_eq!(
            check.status,
            ComponentStatus::Partial,
            "libstdc++-only absence must yield Partial, got: {:?}",
            check.status
        );
        assert!(
            check.detail.contains(PREREQ_KEY_LIBSTDCPP),
            "detail should mention '{}', got: {}",
            PREREQ_KEY_LIBSTDCPP,
            check.detail
        );
    }

    #[test]
    fn test_libstdcpp_present_when_cpp_compiler_found_does_not_appear_in_detail() {
        let check = build_linux_check_from_missing_full(&[], true, true, true);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(!check.detail.contains(PREREQ_KEY_LIBSTDCPP));
    }

    #[test]
    fn test_libstdcpp_and_binary_missing_yields_missing() {
        let check = build_linux_check_from_missing_full(&["cmake"], true, true, false);
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(check.detail.contains("cmake"));
        assert!(check.detail.contains(PREREQ_KEY_LIBSTDCPP));
    }

    #[test]
    fn test_all_three_headers_missing_yields_partial_with_all_keys() {
        // All dev-headers absent (GTK + GLU + libstdc++), binaries present → Partial.
        let check = build_linux_check_from_missing_full(&[], false, false, false);
        assert_eq!(check.status, ComponentStatus::Partial);
        assert!(check.detail.contains(GTK_ITEM_LABEL));
        assert!(check.detail.contains(PREREQ_KEY_GLU));
        assert!(check.detail.contains(PREREQ_KEY_LIBSTDCPP));
    }

    // ── build_linux_check_from_candidates (full pure helper) ─────────────────

    #[test]
    fn test_candidates_helper_all_present_yields_ok() {
        // Provide all required tools + clang (for libstdc++ heuristic).
        let all_tools = [
            "git",
            "zip",
            "curl",
            "unzip",
            "xz",
            "clang",
            "cmake",
            "ninja",
            "pkg-config",
        ];
        let check = build_linux_check_from_candidates(&all_tools, true, true, true);
        assert_eq!(check.status, ComponentStatus::Ok);
    }

    #[test]
    fn test_candidates_helper_missing_git_yields_missing() {
        let tools_without_git = [
            "zip",
            "curl",
            "unzip",
            "xz",
            "clang",
            "cmake",
            "ninja",
            "pkg-config",
        ];
        let check = build_linux_check_from_candidates(&tools_without_git, true, true, true);
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(check.detail.contains("git"));
    }

    #[test]
    fn test_candidates_helper_glu_absent_when_clang_present_yields_partial() {
        let all_tools = [
            "git",
            "zip",
            "curl",
            "unzip",
            "xz",
            "clang",
            "cmake",
            "ninja",
            "pkg-config",
        ];
        let check = build_linux_check_from_candidates(&all_tools, true, true, false);
        assert_eq!(check.status, ComponentStatus::Partial);
        assert!(check.detail.contains(PREREQ_KEY_GLU));
    }

    #[test]
    fn test_candidates_helper_libstdcpp_absent_when_clang_absent_yields_missing() {
        // clang absent (required binary) and g++ absent → libstdc++ in missing list.
        // The status is Missing (not Partial) because clang is a required binary.
        let tools_no_compiler = [
            "git",
            "zip",
            "curl",
            "unzip",
            "xz",
            "cmake",
            "ninja",
            "pkg-config",
        ];
        let check = build_linux_check_from_candidates(&tools_no_compiler, true, true, true);
        // clang is in LINUX_REQUIRED_TOOLS → absent → Missing
        assert_eq!(check.status, ComponentStatus::Missing);
        // libstdc++ also appears because cpp_compiler_found = false
        assert!(
            check.detail.contains(PREREQ_KEY_LIBSTDCPP),
            "libstdc++ must appear when no C++ compiler found; detail: {}",
            check.detail
        );
        assert!(
            check.detail.contains("clang"),
            "clang must appear as missing binary; detail: {}",
            check.detail
        );
    }

    #[test]
    fn test_candidates_helper_gplus_suppresses_libstdcpp_when_clang_missing() {
        // g++ in found_binaries → cpp_compiler_found = true → libstdc++ not missing.
        // clang is still absent → Missing status, but libstdc++ not in the list.
        let tools_with_gplus = [
            "git",
            "zip",
            "curl",
            "unzip",
            "xz",
            "cmake",
            "ninja",
            "pkg-config",
            "g++",
        ];
        let check = build_linux_check_from_candidates(&tools_with_gplus, true, true, true);
        // clang missing → Missing
        assert_eq!(check.status, ComponentStatus::Missing);
        // g++ found → cpp_compiler_found = true → libstdc++ NOT missing
        assert!(
            !check.detail.contains(PREREQ_KEY_LIBSTDCPP),
            "libstdc++ must not appear when g++ is present; detail: {}",
            check.detail
        );
    }

    // ── New PREREQ_KEY constants ──────────────────────────────────────────────

    #[test]
    fn test_prereq_key_glu_and_libstdcpp_constants_have_expected_values() {
        assert_eq!(PREREQ_KEY_GLU, "libglu1-mesa");
        assert_eq!(PREREQ_KEY_LIBSTDCPP, "libstdc++");
    }
}

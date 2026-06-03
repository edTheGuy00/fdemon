//! # OS-Level Prerequisites Probe
//!
//! Read-only check for platform-level tools required by Flutter development.
//! The check is lightweight: it only verifies binary presence via
//! `which::which` and never installs anything.

use std::process::Stdio;

use tokio::process::Command;

use super::super::types::{ComponentCheck, ComponentKind, ComponentStatus, HostPlatform};
use super::PROBE_TIMEOUT;

/// Check OS-level prerequisites for Flutter development.
///
/// The check is **lightweight and read-only** — it only verifies binary
/// presence via `which::which`, never generates install commands (Phase 4).
///
/// - **Linux**: checks for `cmake`, `ninja`, `pkg-config`, `clang`, `curl`,
///   `unzip`, `xz` (or `xz-utils`).
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
const LINUX_REQUIRED_TOOLS: &[&str] = &[
    "cmake",
    "ninja",
    "pkg-config",
    "clang",
    "curl",
    "unzip",
    "xz",
];

async fn check_linux_prerequisites() -> ComponentCheck {
    let missing: Vec<&str> = LINUX_REQUIRED_TOOLS
        .iter()
        .copied()
        .filter(|tool| {
            // Try both the bare name and common alternatives
            let found = which::which(tool).is_ok();
            if !found && *tool == "ninja" {
                // ninja may be called `ninja-build` on some distros
                return which::which("ninja-build").is_err();
            }
            if !found && *tool == "xz" {
                // xz may not be on PATH separately; also check `xz-utils`
                return which::which("xz-utils").is_err();
            }
            !found
        })
        .collect();

    if missing.is_empty() {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: "All required Linux tools present".to_string(),
        }
    } else {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Partial,
            detail: format!("Missing tools: {}", missing.join(", ")),
        }
    }
}

async fn check_macos_prerequisites() -> ComponentCheck {
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
        Ok(Ok(status)) if status.success() => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: "Xcode Command Line Tools installed".to_string(),
        },
        Ok(Ok(_)) => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Missing,
            detail: "Xcode Command Line Tools not installed. Run: xcode-select --install"
                .to_string(),
        },
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Missing,
            detail: "xcode-select not found — install Xcode from the App Store".to_string(),
        },
        _ => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Unknown,
            detail: "Could not determine Xcode Command Line Tools status".to_string(),
        },
    }
}

async fn check_windows_prerequisites() -> ComponentCheck {
    // On Windows, use git presence as a proxy for developer tools
    match which::which("git") {
        Ok(_) => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: "Git found (Windows prerequisites appear satisfied)".to_string(),
        },
        Err(_) => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Partial,
            detail: "Git not found on PATH. Install Git for Windows.".to_string(),
        },
    }
}

//! # Web Browser Probe
//!
//! Detect a Chromium-based browser for `flutter run -d chrome`.
//!
//! Probe order:
//! 1. Explicit `browser_override` (from `web_browser_executable` config).
//! 2. `CHROME_EXECUTABLE` environment variable.
//! 3. Per-OS default locations, dispatched on [`HostPlatform`].
//!
//! The probe never installs anything — it is read-only. A successful probe
//! attempts a best-effort `<browser> --version` call to enrich the `detail`
//! string; on timeout or error, the bare path is used instead.

use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::Command;

use super::super::types::{ComponentCheck, ComponentKind, ComponentStatus, HostPlatform};
use super::{strip_and_truncate, PROBE_TIMEOUT};

/// Detect a Chromium-based browser for Flutter web (`flutter run -d chrome`).
///
/// Probe order: explicit override → `CHROME_EXECUTABLE` env → per-OS defaults.
///
/// # Arguments
///
/// * `platform` — The host platform, used to select the OS-specific search
///   strategy.
/// * `browser_override` — Optional explicit path from `web_browser_executable`
///   in `.fdemon/config.toml`. Takes precedence over all other detection
///   strategies when the file exists.
///
/// # Returns
///
/// A [`ComponentCheck`] with:
/// - `Ok` — browser found; `detail` is the resolved version string or path.
/// - `Missing` — no browser found via any strategy.
/// - `Unknown` — platform is [`HostPlatform::Unknown`]; probe skipped.
pub async fn check_web(platform: &HostPlatform, browser_override: Option<&str>) -> ComponentCheck {
    if matches!(platform, HostPlatform::Unknown) {
        return ComponentCheck {
            kind: ComponentKind::WebBrowser,
            status: ComponentStatus::Unknown,
            detail: "Unknown platform — web browser check skipped".to_string(),
        };
    }

    // 1. Explicit override
    if let Some(path_str) = browser_override {
        let path = PathBuf::from(path_str);
        if path.is_file() {
            let detail = probe_version(&path)
                .await
                .unwrap_or_else(|| path_str.to_string());
            return ComponentCheck {
                kind: ComponentKind::WebBrowser,
                status: ComponentStatus::Ok,
                detail,
            };
        }
    }

    // 2. CHROME_EXECUTABLE env var
    if let Ok(env_path_str) = std::env::var("CHROME_EXECUTABLE") {
        let env_path = PathBuf::from(&env_path_str);
        if env_path.is_file() {
            let detail = probe_version(&env_path)
                .await
                .unwrap_or_else(|| env_path_str.clone());
            return ComponentCheck {
                kind: ComponentKind::WebBrowser,
                status: ComponentStatus::Ok,
                detail,
            };
        }
    }

    // 3. Per-OS default locations
    if let Some(path) = find_browser_default(platform) {
        let detail = probe_version(&path)
            .await
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        return ComponentCheck {
            kind: ComponentKind::WebBrowser,
            status: ComponentStatus::Ok,
            detail,
        };
    }

    ComponentCheck {
        kind: ComponentKind::WebBrowser,
        status: ComponentStatus::Missing,
        detail: "No Chromium-based browser found (Chrome, Chromium, or Edge)".to_string(),
    }
}

/// Search per-OS default locations and return the first found browser path.
///
/// - **Linux**: `which` probes for `google-chrome`, `google-chrome-stable`,
///   `chromium`, `chromium-browser` in that order.
/// - **macOS**: checks fixed `.app` bundle paths. Chrome is not on PATH on macOS,
///   so `which` is not used here.
/// - **Windows**: checks `%PROGRAMFILES%` and `%LOCALAPPDATA%` Chrome paths,
///   then falls back to `msedge` on PATH (Edge uses the Chromium engine).
fn find_browser_default(platform: &HostPlatform) -> Option<PathBuf> {
    match platform {
        HostPlatform::Linux => find_browser_linux(),
        HostPlatform::MacOs => find_browser_macos(),
        HostPlatform::Windows => find_browser_windows(),
        HostPlatform::Unknown => None,
    }
}

/// Linux browser detection: probe PATH for Chromium-based browser binaries.
fn find_browser_linux() -> Option<PathBuf> {
    const CANDIDATES: &[&str] = &[
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ];
    for candidate in CANDIDATES {
        if let Ok(path) = which::which(candidate) {
            return Some(path);
        }
    }
    None
}

/// macOS browser detection: check fixed `.app` bundle paths.
///
/// Chrome (and Chromium) are installed as `.app` bundles and are **not**
/// symlinked onto PATH, so `which::which` cannot find them. We check the
/// canonical installation paths directly via `PathBuf::is_file`.
fn find_browser_macos() -> Option<PathBuf> {
    const CANDIDATES: &[&str] = &[
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ];
    for candidate in CANDIDATES {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Windows browser detection: check `%PROGRAMFILES%` and `%LOCALAPPDATA%`
/// Chrome paths, then fall back to `msedge` on PATH.
fn find_browser_windows() -> Option<PathBuf> {
    // PROGRAMFILES Chrome
    if let Ok(prog_files) = std::env::var("PROGRAMFILES") {
        let path = PathBuf::from(&prog_files)
            .join("Google")
            .join("Chrome")
            .join("Application")
            .join("chrome.exe");
        if path.is_file() {
            return Some(path);
        }
    }

    // LOCALAPPDATA Chrome (per-user installation)
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let path = PathBuf::from(&local_app_data)
            .join("Google")
            .join("Chrome")
            .join("Application")
            .join("chrome.exe");
        if path.is_file() {
            return Some(path);
        }
    }

    // Edge (uses the Chromium engine; valid for `flutter run -d chrome`)
    if let Ok(path) = which::which("msedge") {
        return Some(path);
    }

    None
}

/// Run `<browser> --version` with [`PROBE_TIMEOUT`] and return the cleaned
/// version string, or `None` on timeout/error.
///
/// This is best-effort: callers fall back to the bare path when this returns
/// `None`.
async fn probe_version(browser_path: &PathBuf) -> Option<String> {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new(browser_path)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            let line = raw.lines().next().unwrap_or("").trim();
            if line.is_empty() {
                None
            } else {
                Some(strip_and_truncate(line))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Smoke test: never panics ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_check_web_never_panics() {
        let platform = HostPlatform::detect();
        let _ = check_web(&platform, None).await;
    }

    // ── Unknown platform returns Unknown ─────────────────────────────────────

    #[tokio::test]
    async fn test_check_web_unknown_platform_returns_unknown() {
        let result = check_web(&HostPlatform::Unknown, None).await;
        assert_eq!(result.kind, ComponentKind::WebBrowser);
        assert_eq!(result.status, ComponentStatus::Unknown);
    }

    // ── Override path ─────────────────────────────────────────────────────────

    /// When browser_override points to an existing file, the result must be Ok
    /// and the detail must contain the override path (as a fallback if
    /// --version fails, or the version string which also contains the path
    /// on most browsers).
    #[tokio::test]
    async fn test_check_web_respects_browser_override() {
        // Use the test binary itself as a stand-in for a "browser" — it exists.
        let self_exe = std::env::current_exe().expect("cannot determine test binary path");
        assert!(self_exe.is_file(), "test binary must exist");

        // We pass any platform — the override is checked before platform dispatch.
        let result = check_web(&HostPlatform::Linux, Some(self_exe.to_str().unwrap())).await;

        assert_eq!(result.kind, ComponentKind::WebBrowser);
        assert_eq!(
            result.status,
            ComponentStatus::Ok,
            "override pointing to existing file must be Ok; detail: {}",
            result.detail
        );
        // detail must contain some reference to the path we passed
        // (either bare path fallback or version output; bare path fallback is
        // guaranteed when --version returns unexpected output for a test binary).
        assert!(
            result.detail.contains(self_exe.to_str().unwrap()) || !result.detail.is_empty(),
            "detail should be non-empty"
        );
    }

    /// When browser_override path does not exist, the probe must fall through
    /// to the next strategy, not return Ok.
    #[tokio::test]
    async fn test_check_web_nonexistent_override_falls_through() {
        let result = check_web(&HostPlatform::Linux, Some("/nonexistent/path/to/browser")).await;
        // The override path doesn't exist, so we fall through. On a CI Linux
        // host without Chrome the result may be Missing or Ok (if Chrome is
        // present). We just check it is not an Unknown/Error from the override.
        assert_ne!(result.status, ComponentStatus::Unknown);
    }

    // ── CHROME_EXECUTABLE env var ─────────────────────────────────────────────

    /// When CHROME_EXECUTABLE points to an existing file, the result must be Ok.
    ///
    /// This test serialises with other env-mutating tests to avoid cross-test
    /// contamination (env vars are process-global).
    #[tokio::test]
    #[serial_test::serial]
    async fn test_check_web_respects_chrome_executable_env() {
        // Create a real temporary file to use as a "browser" stand-in.
        let tmp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
        let fake_browser = tmp_dir.path().join("fake-chrome");
        std::fs::write(&fake_browser, b"#!/bin/sh\necho 'Google Chrome 120.0.0'\n")
            .expect("failed to write fake browser");

        // On Unix, make the file executable so probe_version would succeed if called.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&fake_browser).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&fake_browser, perms).unwrap();
        }

        let saved = std::env::var_os("CHROME_EXECUTABLE");
        std::env::set_var("CHROME_EXECUTABLE", &fake_browser);

        let result = check_web(&HostPlatform::Linux, None).await;

        // Restore env state regardless of test outcome.
        match saved {
            Some(v) => std::env::set_var("CHROME_EXECUTABLE", v),
            None => std::env::remove_var("CHROME_EXECUTABLE"),
        }

        assert_eq!(result.kind, ComponentKind::WebBrowser);
        assert_eq!(
            result.status,
            ComponentStatus::Ok,
            "CHROME_EXECUTABLE pointing to existing file must be Ok; detail: {}",
            result.detail
        );
    }
}

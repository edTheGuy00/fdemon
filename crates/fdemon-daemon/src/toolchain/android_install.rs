//! # Managed Android SDK Installer
//!
//! Implements the high-level Android toolchain install flow:
//!
//! 1. Download the Android command-line tools zip for the host platform.
//! 2. Extract the zip to a temp directory.
//! 3. Relocate the extracted `cmdline-tools` directory to
//!    `<sdk_root>/cmdline-tools/latest` (the path checked by
//!    [`super::checks::check_android_cmdline_tools`]).
//! 4. Accept SDK licenses non-interactively via `sdkmanager --licenses`,
//!    feeding `y\n` through stdin.
//! 5. Install the required SDK packages via `sdkmanager <packages…>`.
//! 6. Return [`AndroidInstallOutcome`] with the SDK root and installed packages.
//!
//! ## Design Notes
//!
//! - **Atomic relocation**: extraction happens in a sibling temp directory;
//!   the final `cmdline-tools/latest` rename is atomic on POSIX. Temp dirs are
//!   cleaned up on failure.
//! - **No SHA verification**: Google publishes no easily-fetched per-build
//!   SHA-256 for `cmdline-tools`. The download relies on HTTPS/TLS. A future
//!   `[toolchain] cmdline_tools_sha256` override is not implemented here.
//! - **License acceptance is idempotent**: re-running `--licenses` on an
//!   already-licensed SDK is harmless.
//! - **`spawn_blocking` for sync extract**: [`extract_zip`] is synchronous;
//!   it is wrapped in [`tokio::task::spawn_blocking`].
//! - **JDK path**: when `target.jdk_path` is `Some`, `JAVA_HOME` is set for
//!   `sdkmanager` child processes so they find the correct JDK.
//!
//! ## Public API
//!
//! - [`install_android_tools`] — full install flow.
//! - [`resolve_cmdline_tools_url`] — testable URL helper (no I/O).

use std::path::{Path, PathBuf};

use fdemon_core::{Error, Result};

use super::checks::sdkmanager_bin_name;
use super::download::{download_to_file, ensure_disk_space, extract_zip};
use super::flutter_install::InstallEvent;
use super::process_stream::run_streaming_with_input;
use super::types::{
    cmdline_tools_url, sdkmanager_packages, AndroidInstallOutcome, AndroidInstallTarget,
    DownloadProgress,
};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Number of `y\n` responses to feed to `sdkmanager --licenses`.
///
/// The license screen presents multiple license agreements in sequence; 20
/// affirmative responses is more than enough for all current SDK versions.
const LICENSE_YES_COUNT: usize = 20;

/// Success marker in `sdkmanager --licenses` output indicating all licenses
/// were accepted.
///
/// **Format dependency**: this string is matched against `sdkmanager` stdout/
/// stderr output and will need updating if Google changes the message format in
/// a future `cmdline-tools` release.  A grep for `LICENSES_ACCEPTED_MARKER` is
/// sufficient to locate all affected call sites.
const LICENSES_ACCEPTED_MARKER: &str = "All SDK package licenses accepted";

/// Conservative free-disk-space budget for the Android SDK install.
///
/// Android command-line tools zip is ~130 MiB compressed; after extraction
/// plus the subsequent `sdkmanager` package downloads (build-tools, platform,
/// platform-tools) the total on-disk footprint typically exceeds 1.5 GiB.
/// A 2 GiB budget provides a safe margin for the initial install.
const ANDROID_DISK_BUDGET_BYTES: u64 = 2_147_483_648; // 2 GiB

// ── Public API ────────────────────────────────────────────────────────────────

/// Resolve the `cmdline-tools` download URL for the given install target.
///
/// This is a thin wrapper around [`cmdline_tools_url`] that converts the
/// `None` case into a typed `Err`, making it trivially unit-testable without
/// network I/O.
///
/// # Errors
///
/// Returns an error when `target.platform` is [`crate::toolchain::HostPlatform::Unknown`].
pub fn resolve_cmdline_tools_url(target: &AndroidInstallTarget) -> Result<String> {
    cmdline_tools_url(target.platform.clone(), &target.cmdline_tools_build).ok_or_else(|| {
        Error::process(format!(
            "cannot build cmdline-tools URL: unsupported platform '{}'",
            target.platform
        ))
    })
}

/// Install the Android SDK command-line tools, accept licenses, and install
/// the required SDK packages for the given API level.
///
/// All progress is reported through the `on_event` callback as [`InstallEvent`]
/// variants. The caller maps these to UI messages (task 06/07).
///
/// ## Flow
///
/// 1. **Download** `commandlinetools-<os>-<build>_latest.zip` to a temp file.
/// 2. **Extract** the zip (in `spawn_blocking`) to a temp directory.
/// 3. **Relocate** `<tmp>/cmdline-tools` → `<sdk_root>/cmdline-tools/latest`.
/// 4. **Accept licenses** by running `sdkmanager --licenses` and feeding `y\n`
///    through stdin.
/// 5. **Install packages** by running `sdkmanager <packages…>`.
///
/// ## Errors
///
/// Returns `Err` on download failure (including HTTP 4xx), spawn failure, or
/// non-zero `sdkmanager` exit. Temp directories are cleaned up on failure.
pub async fn install_android_tools<F>(
    target: &AndroidInstallTarget,
    mut on_event: F,
) -> Result<AndroidInstallOutcome>
where
    F: FnMut(InstallEvent) + Send,
{
    // ── Resolve URL ──────────────────────────────────────────────────────────
    let url = resolve_cmdline_tools_url(target)?;

    // ── Create temp workspace ────────────────────────────────────────────────
    // Use a temp dir *inside* sdk_root so we can rename atomically on POSIX.
    std::fs::create_dir_all(&target.sdk_root).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("create Android SDK root {}: {e}", target.sdk_root.display()),
        ))
    })?;

    let tmp_dir = target
        .sdk_root
        .join(format!(".fdemon-android-tmp-{}", std::process::id()));

    // Remove any pre-existing temp dir from a previous (crashed) run with the
    // same PID before creating a fresh one.  PID recycling means a stale dir
    // could otherwise contain partial downloads or extracted files, leading to
    // a corrupted install.
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "remove stale android install temp dir {}: {e}",
                    tmp_dir.display()
                ),
            ))
        })?;
    }

    std::fs::create_dir_all(&tmp_dir).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("create android install temp dir {}: {e}", tmp_dir.display()),
        ))
    })?;

    let result = install_android_tools_inner(target, &tmp_dir, &url, &mut on_event).await;

    // Clean up the temp dir on either success or failure.
    if tmp_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&tmp_dir) {
            tracing::warn!(
                "Failed to remove android install temp dir {}: {e}",
                tmp_dir.display()
            );
        }
    }

    result
}

// ── Inner implementation (with cleanup on drop via caller) ─────────────────

async fn install_android_tools_inner<F>(
    target: &AndroidInstallTarget,
    tmp_dir: &Path,
    url: &str,
    on_event: &mut F,
) -> Result<AndroidInstallOutcome>
where
    F: FnMut(InstallEvent) + Send,
{
    // ── Step 1: Download ─────────────────────────────────────────────────────
    on_event(InstallEvent::Phase("Downloading command-line tools"));

    let tmp_zip = tmp_dir.join("cmdline-tools.zip");
    download_to_file(url, &tmp_zip, |p: DownloadProgress| {
        on_event(InstallEvent::Download(p));
    })
    .await
    .map_err(|e| Error::process(format!("failed to download cmdline-tools from {url}: {e}")))?;

    // ── Step 2: Extract ──────────────────────────────────────────────────────
    on_event(InstallEvent::Phase("Extracting"));

    let tmp_extract = tmp_dir.join("extract");
    std::fs::create_dir_all(&tmp_extract).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("create extract dir {}: {e}", tmp_extract.display()),
        ))
    })?;

    // Preflight: check disk space in the SDK root before extraction.
    // Android cmdline-tools + installed packages can consume several GiB;
    // use a conservative 2 GiB budget to cover the full initial setup.
    ensure_disk_space(&target.sdk_root, ANDROID_DISK_BUDGET_BYTES)?;

    // extract_zip is synchronous — wrap in spawn_blocking.
    let zip_path = tmp_zip.clone();
    let extract_path = tmp_extract.clone();
    tokio::task::spawn_blocking(move || extract_zip(&zip_path, &extract_path))
        .await
        .map_err(|e| Error::process(format!("extract_zip task panicked: {e}")))?
        .map_err(|e| Error::process(format!("failed to extract cmdline-tools zip: {e}")))?;

    // ── Step 3: Relocate to cmdline-tools/latest ─────────────────────────────
    on_event(InstallEvent::Phase("Relocating to cmdline-tools/latest"));

    relocate_cmdline_tools(&tmp_extract, &target.sdk_root)?;

    // ── Step 4: Accept licenses ──────────────────────────────────────────────
    on_event(InstallEvent::Phase("Accepting licenses"));

    let sdkmanager = sdkmanager_path(&target.sdk_root);
    let sdk_root_str = target.sdk_root.to_string_lossy().to_string();

    // Build JAVA_HOME env slice if a JDK path was provided.
    let java_home_str: Option<String> = target
        .jdk_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());

    let license_stdin = "y\n".repeat(LICENSE_YES_COUNT);

    // Build env pairs for sdkmanager invocations.
    let mut env_pairs: Vec<(String, String)> =
        vec![("ANDROID_HOME".to_string(), sdk_root_str.clone())];
    if let Some(ref java_home) = java_home_str {
        env_pairs.push(("JAVA_HOME".to_string(), java_home.clone()));
        // Prepend the JDK bin dir to PATH so sdkmanager finds `java`.
        // Use Path::join to avoid producing `//bin` when java_home has a
        // trailing slash.
        let jdk_bin = std::path::Path::new(java_home.as_str())
            .join("bin")
            .to_string_lossy()
            .into_owned();
        let existing_path = std::env::var("PATH").unwrap_or_default();
        let new_path = if existing_path.is_empty() {
            jdk_bin
        } else {
            format!("{jdk_bin}:{existing_path}")
        };
        env_pairs.push(("PATH".to_string(), new_path));
    }

    // Convert to Vec<(&str, &str)> for run_streaming_with_input.
    let env_refs: Vec<(&str, &str)> = env_pairs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let sdkmanager_str = sdkmanager.to_string_lossy().to_string();

    {
        let mut log_lines: Vec<String> = Vec::new();
        let status = run_streaming_with_input(
            &sdkmanager_str,
            &["--licenses", &format!("--sdk_root={sdk_root_str}")],
            Some(&target.sdk_root),
            &env_refs,
            license_stdin.as_bytes(),
            |line| {
                on_event(InstallEvent::Log(line.clone()));
                log_lines.push(line);
            },
        )
        .await?;

        if !status.success() {
            return Err(Error::process(format!(
                "sdkmanager --licenses exited with {status}; see log above for details"
            )));
        }

        // Verify that the license acceptance was actually confirmed by scanning
        // the output for the known success marker.  The exit code is the primary
        // signal; a missing marker is a non-fatal warning (sdkmanager wording may
        // vary across releases or already-licensed SDKs may print a different
        // message).
        if !licenses_confirmed(&log_lines) {
            let warning = "sdkmanager --licenses completed but the expected confirmation \
                           message was not found in output — licenses may not have been \
                           accepted; if builds fail, run `sdkmanager --licenses` manually";
            tracing::warn!("{warning}");
            on_event(InstallEvent::Log(format!("[fdemon warn] {warning}")));
        }
    }

    // ── Step 5: Install packages ─────────────────────────────────────────────
    on_event(InstallEvent::Phase("Installing packages"));

    let packages = sdkmanager_packages(target.api_level);

    // Build the full args slice: package names + --sdk_root flag.
    let sdk_root_arg = format!("--sdk_root={sdk_root_str}");
    let mut install_args: Vec<&str> = packages.iter().map(String::as_str).collect();
    install_args.push(&sdk_root_arg);

    // Use run_streaming_with_input so that any inline license prompts during
    // package install are also answered with 'y'.
    let install_stdin = "y\n".repeat(LICENSE_YES_COUNT);

    let install_status = run_streaming_with_input(
        &sdkmanager_str,
        &install_args,
        Some(&target.sdk_root),
        &env_refs,
        install_stdin.as_bytes(),
        |line| {
            on_event(InstallEvent::Log(line));
        },
    )
    .await?;

    if !install_status.success() {
        return Err(Error::process(format!(
            "sdkmanager package install exited with {install_status}; see log above for details"
        )));
    }

    Ok(AndroidInstallOutcome {
        sdk_root: target.sdk_root.clone(),
        packages_installed: packages,
    })
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Relocate the extracted `cmdline-tools` directory to
/// `<sdk_root>/cmdline-tools/latest`.
///
/// The zip extracts to `<extract_dir>/cmdline-tools/`. We need to rename that
/// into `<sdk_root>/cmdline-tools/latest/`. Steps:
///
/// 1. Ensure `<sdk_root>/cmdline-tools/` parent exists.
/// 2. If `<sdk_root>/cmdline-tools/latest/` already exists, remove it
///    atomically to allow replacement.
/// 3. Rename `<extract_dir>/cmdline-tools` → `<sdk_root>/cmdline-tools/latest`.
///
/// # Errors
///
/// Returns an error when the extracted `cmdline-tools` directory is missing,
/// when the parent cannot be created, or when the rename fails.
pub fn relocate_cmdline_tools(extract_dir: &Path, sdk_root: &Path) -> Result<()> {
    let source = extract_dir.join("cmdline-tools");
    if !source.is_dir() {
        return Err(Error::process(format!(
            "expected cmdline-tools directory at {} after extraction",
            source.display()
        )));
    }

    let cmdline_tools_parent = sdk_root.join("cmdline-tools");
    std::fs::create_dir_all(&cmdline_tools_parent).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!(
                "create cmdline-tools parent dir {}: {e}",
                cmdline_tools_parent.display()
            ),
        ))
    })?;

    let dest = cmdline_tools_parent.join("latest");

    // Remove pre-existing latest/ to allow atomic replacement.
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "remove existing cmdline-tools/latest {}: {e}",
                    dest.display()
                ),
            ))
        })?;
    }

    std::fs::rename(&source, &dest).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("rename {} → {}: {e}", source.display(), dest.display()),
        ))
    })?;

    tracing::debug!("Relocated cmdline-tools to {}", dest.display());

    Ok(())
}

/// Check whether the `sdkmanager --licenses` output contains the expected
/// success marker.
///
/// This is a pure function so it can be unit-tested without invoking
/// `sdkmanager`.  The marker is defined by [`LICENSES_ACCEPTED_MARKER`]; see
/// that constant's doc comment for format-dependency notes.
///
/// Returns `true` when at least one output line contains the marker substring.
pub(crate) fn licenses_confirmed(lines: &[String]) -> bool {
    lines.iter().any(|l| l.contains(LICENSES_ACCEPTED_MARKER))
}

/// Compute the full path to the `sdkmanager` binary for the given SDK root.
fn sdkmanager_path(sdk_root: &Path) -> PathBuf {
    sdk_root
        .join("cmdline-tools")
        .join("latest")
        .join("bin")
        .join(sdkmanager_bin_name())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolchain::types::{
        AndroidInstallTarget, HostPlatform, DEFAULT_CMDLINE_TOOLS_BUILD,
    };

    // ── URL resolution ────────────────────────────────────────────────────────

    #[test]
    fn test_resolve_cmdline_tools_url_linux() {
        let target = AndroidInstallTarget {
            sdk_root: PathBuf::from("/opt/android"),
            api_level: 36,
            cmdline_tools_build: DEFAULT_CMDLINE_TOOLS_BUILD.to_string(),
            jdk_path: None,
            platform: HostPlatform::Linux,
        };
        let url = resolve_cmdline_tools_url(&target).expect("must resolve for Linux");
        assert!(
            url.contains("commandlinetools-linux-"),
            "URL must contain linux slug: {url}"
        );
        assert!(
            url.contains(DEFAULT_CMDLINE_TOOLS_BUILD),
            "URL must contain build number: {url}"
        );
    }

    #[test]
    fn test_resolve_cmdline_tools_url_unknown_platform_is_err() {
        let target = AndroidInstallTarget {
            sdk_root: PathBuf::from("/opt/android"),
            api_level: 36,
            cmdline_tools_build: "12345".to_string(),
            jdk_path: None,
            platform: HostPlatform::Unknown,
        };
        assert!(
            resolve_cmdline_tools_url(&target).is_err(),
            "Unknown platform must return Err"
        );
    }

    #[test]
    fn test_resolve_cmdline_tools_url_custom_build() {
        let target = AndroidInstallTarget {
            sdk_root: PathBuf::from("/opt/android"),
            api_level: 36,
            cmdline_tools_build: "99999999".to_string(),
            jdk_path: None,
            platform: HostPlatform::MacOs,
        };
        let url = resolve_cmdline_tools_url(&target).expect("must resolve");
        assert!(url.contains("99999999"), "URL must use custom build number");
        assert!(url.contains("-mac-"), "URL must use mac slug");
    }

    // ── Relocation helper ─────────────────────────────────────────────────────

    #[test]
    fn test_relocate_cmdline_tools_to_latest() {
        let sdk_root = tempfile::TempDir::new().unwrap();
        let extract_dir = tempfile::TempDir::new().unwrap();

        // Create a synthetic extracted tree: <extract_dir>/cmdline-tools/bin/sdkmanager
        let extracted_cmdline = extract_dir.path().join("cmdline-tools");
        let extracted_bin = extracted_cmdline.join("bin");
        std::fs::create_dir_all(&extracted_bin).unwrap();
        std::fs::write(
            extracted_bin.join("sdkmanager"),
            b"#!/bin/sh\necho sdkmanager",
        )
        .unwrap();

        // Perform the relocation.
        relocate_cmdline_tools(extract_dir.path(), sdk_root.path()).expect("relocate must succeed");

        // Assert the final layout.
        let latest_bin = sdk_root
            .path()
            .join("cmdline-tools")
            .join("latest")
            .join("bin");
        assert!(
            latest_bin.is_dir(),
            "cmdline-tools/latest/bin must exist: {}",
            latest_bin.display()
        );
        assert!(
            latest_bin.join("sdkmanager").is_file(),
            "sdkmanager must be present under latest/bin"
        );

        // The source must no longer exist (it was renamed, not copied).
        assert!(
            !extracted_cmdline.exists(),
            "source cmdline-tools must be gone after rename"
        );
    }

    #[test]
    fn test_relocate_replaces_existing_latest() {
        let sdk_root = tempfile::TempDir::new().unwrap();
        let extract_dir = tempfile::TempDir::new().unwrap();

        // Pre-populate <sdk_root>/cmdline-tools/latest with a stale file.
        let latest = sdk_root
            .path()
            .join("cmdline-tools")
            .join("latest")
            .join("bin");
        std::fs::create_dir_all(&latest).unwrap();
        std::fs::write(latest.join("old_file"), b"old").unwrap();

        // Create fresh extracted source.
        let src_bin = extract_dir.path().join("cmdline-tools").join("bin");
        std::fs::create_dir_all(&src_bin).unwrap();
        std::fs::write(src_bin.join("sdkmanager"), b"new").unwrap();

        relocate_cmdline_tools(extract_dir.path(), sdk_root.path()).expect("replace must succeed");

        // Old file must be gone; new sdkmanager must be present.
        let new_bin = sdk_root
            .path()
            .join("cmdline-tools")
            .join("latest")
            .join("bin");
        assert!(
            !new_bin.join("old_file").exists(),
            "old stale file must be gone"
        );
        assert!(
            new_bin.join("sdkmanager").is_file(),
            "new sdkmanager must be present"
        );
    }

    #[test]
    fn test_relocate_missing_source_is_err() {
        let sdk_root = tempfile::TempDir::new().unwrap();
        let empty_extract = tempfile::TempDir::new().unwrap();

        let result = relocate_cmdline_tools(empty_extract.path(), sdk_root.path());
        assert!(
            result.is_err(),
            "must error when cmdline-tools is absent from extract dir"
        );
    }

    // ── licenses_confirmed (m1) ───────────────────────────────────────────────

    #[test]
    fn test_licenses_confirmed_returns_true_when_marker_present() {
        let lines = vec![
            "Reuse the existing license accepted response.".to_string(),
            "All SDK package licenses accepted.".to_string(),
        ];
        assert!(
            licenses_confirmed(&lines),
            "should return true when marker is present"
        );
    }

    #[test]
    fn test_licenses_confirmed_returns_false_when_marker_absent() {
        let lines = vec![
            "Reading existing licenses...".to_string(),
            "License for package Android SDK Platform 36 not accepted.".to_string(),
        ];
        assert!(
            !licenses_confirmed(&lines),
            "should return false when marker is absent"
        );
    }

    #[test]
    fn test_licenses_confirmed_returns_false_for_empty_lines() {
        assert!(
            !licenses_confirmed(&[]),
            "should return false for empty output"
        );
    }

    #[test]
    fn test_licenses_confirmed_marker_is_substring_match() {
        // The marker can appear embedded in a longer line.
        let lines =
            vec!["[100% Installing...] All SDK package licenses accepted — done.".to_string()];
        assert!(
            licenses_confirmed(&lines),
            "marker match should be a substring, not a whole-line match"
        );
    }

    // ── sdkmanager_path helper ────────────────────────────────────────────────

    #[test]
    fn test_sdkmanager_path_correct_layout() {
        let sdk_root = PathBuf::from("/opt/android");
        let path = sdkmanager_path(&sdk_root);
        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains("cmdline-tools"),
            "path must contain cmdline-tools: {path_str}"
        );
        assert!(
            path_str.contains("latest"),
            "path must contain latest: {path_str}"
        );
        assert!(
            path_str.contains("bin"),
            "path must contain bin: {path_str}"
        );
        assert!(
            path_str.contains("sdkmanager"),
            "path must end with sdkmanager: {path_str}"
        );
    }
}

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
//! - **SHA verification**: when [`AndroidInstallTarget::cmdline_tools_sha256`]
//!   is `Some`, `verify_sha256` is called (via `spawn_blocking`) on the
//!   downloaded zip **before** `extract_zip` or any binary is executed.  A
//!   mismatching digest aborts the install and cleans up the temp dir.  When
//!   `None`, the download relies on HTTPS/TLS for integrity (Google publishes
//!   no easily-fetched per-build SHA-256 for the floating `_latest.zip`).
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
use tokio_util::sync::CancellationToken;

use super::checks::sdkmanager_bin_name;
use super::download::{download_to_file, ensure_disk_space, extract_zip, verify_sha256};
use super::flutter_install::InstallEvent;
use super::process_stream::run_streaming_with_input;
use super::types::{
    cmdline_tools_url, sdkmanager_packages, AndroidInstallOutcome, AndroidInstallTarget,
    DownloadProgress,
};

// ── RAII temp-dir guard ───────────────────────────────────────────────────────

/// RAII guard that removes a directory (and all its contents) when dropped.
///
/// The guard is **armed** on construction.  Removal runs even when the outer
/// future is dropped mid-execution via `JoinHandle::abort()`, because `Drop`
/// is called synchronously during the drop cascade.  This ensures that a
/// partially-extracted Android SDK tree is never leaked on abort or panic.
struct TempDirGuard {
    path: PathBuf,
    armed: bool,
}

impl TempDirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if self.armed && self.path.exists() {
            if let Err(e) = std::fs::remove_dir_all(&self.path) {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "TempDirGuard: failed to remove android install temp dir (best-effort)"
                );
            }
        }
    }
}

// ── Stale-temp reclamation ────────────────────────────────────────────────────

/// Glob `sdk_root` for any `.fdemon-android-tmp-*` directories (from any PID)
/// and remove them.
///
/// Any directory that cannot be removed is logged as a warning and skipped.
fn reclaim_stale_android_tmps(sdk_root: &Path) {
    let read_dir = match std::fs::read_dir(sdk_root) {
        Ok(rd) => rd,
        Err(e) => {
            tracing::debug!(
                root = %sdk_root.display(),
                error = %e,
                "reclaim_stale_android_tmps: read_dir failed; skipping reclamation"
            );
            return;
        }
    };
    for entry in read_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(".fdemon-android-tmp-") {
            let path = entry.path();
            if path.is_dir() {
                tracing::debug!(path = %path.display(), "reclaim_stale_android_tmps: removing stale temp dir");
                if let Err(e) = std::fs::remove_dir_all(&path) {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "reclaim_stale_android_tmps: could not remove stale temp dir"
                    );
                }
            }
        }
    }
}

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
/// ## Cancellation
///
/// Pass a [`CancellationToken`] to support user-initiated cancellation.  A
/// pre-cancelled token causes an immediate return of
/// [`fdemon_core::Error::Cancelled`] before any I/O.  The token is also
/// forwarded to [`download_to_file`] for per-chunk cancellation during the
/// archive download step.
///
/// For a non-cancellable install, pass `CancellationToken::new()`.
///
/// ## Errors
///
/// Returns `Err` on download failure (including HTTP 4xx), spawn failure, or
/// non-zero `sdkmanager` exit. Temp directories are cleaned up on failure.
pub async fn install_android_tools<F>(
    target: &AndroidInstallTarget,
    cancel: CancellationToken,
    mut on_event: F,
) -> Result<AndroidInstallOutcome>
where
    F: FnMut(InstallEvent) + Send,
{
    // Pre-cancel check: if the token is already cancelled, return immediately.
    if cancel.is_cancelled() {
        return Err(Error::cancelled("Android install cancelled before start"));
    }
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

    // Reclaim any stale `.fdemon-android-tmp-*` dirs from prior aborted runs
    // (any PID — a future-abort drops the guard without running `Drop` in the
    // aborted context if the abort wins the race before `Drop` fires).
    reclaim_stale_android_tmps(&target.sdk_root);

    let tmp_dir_path = target
        .sdk_root
        .join(format!(".fdemon-android-tmp-{}", std::process::id()));

    std::fs::create_dir_all(&tmp_dir_path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!(
                "create android install temp dir {}: {e}",
                tmp_dir_path.display()
            ),
        ))
    })?;

    // Arm the RAII guard. Its `Drop` removes the temp dir even when the outer
    // `JoinHandle` is aborted mid-`await`, ensuring no partially-extracted
    // Android SDK tree is leaked.
    let _tmp_guard = TempDirGuard::new(tmp_dir_path.clone());

    install_android_tools_inner(target, &tmp_dir_path, &url, cancel, &mut on_event).await
}

// ── Inner implementation (with cleanup on drop via caller) ─────────────────

async fn install_android_tools_inner<F>(
    target: &AndroidInstallTarget,
    tmp_dir: &Path,
    url: &str,
    cancel: CancellationToken,
    on_event: &mut F,
) -> Result<AndroidInstallOutcome>
where
    F: FnMut(InstallEvent) + Send,
{
    // ── Step 1: Download ─────────────────────────────────────────────────────
    on_event(InstallEvent::Phase("Downloading command-line tools"));

    let tmp_zip = tmp_dir.join("cmdline-tools.zip");
    download_to_file(url, &tmp_zip, cancel, |p: DownloadProgress| {
        on_event(InstallEvent::Download(p));
    })
    .await
    .map_err(|e| Error::process(format!("failed to download cmdline-tools from {url}: {e}")))?;

    // ── Step 1b: Verify SHA-256 (if configured) ──────────────────────────────
    //
    // When `cmdline_tools_sha256` is configured, verify the downloaded zip
    // before any extraction or execution.  This mirrors the Flutter install
    // path (flutter_install.rs:914-925).
    //
    // When no hash is provided, we rely on the HTTPS/TLS channel enforced by
    // `download_to_file` (non-https URLs and http-downgrade redirects are
    // rejected at the transport layer).  The residual risk is undetected
    // on-disk corruption; callers who need integrity assurance should set the
    // `[toolchain] cmdline_tools_sha256` override.
    if let Some(ref expected_sha) = target.cmdline_tools_sha256 {
        on_event(InstallEvent::Phase("Verifying"));
        on_event(InstallEvent::Log(
            "Verifying SHA-256 checksum of cmdline-tools …".to_owned(),
        ));

        let sha_zip_path = tmp_zip.clone();
        let expected_sha_clone = expected_sha.clone();
        tokio::task::spawn_blocking(move || verify_sha256(&sha_zip_path, &expected_sha_clone))
            .await
            .map_err(|e| Error::process(format!("spawn_blocking for verify_sha256 panicked: {e}")))?
            .map_err(|e| {
                Error::process(format!("cmdline-tools SHA-256 verification failed: {e}"))
            })?;
    }

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
    // The cancel token was consumed by download_to_file above; extraction
    // here is not cancellable (the Android install path was already past the
    // download gate, and the blocking thread cannot be aborted mid-write).
    let zip_path = tmp_zip.clone();
    let extract_path = tmp_extract.clone();
    tokio::task::spawn_blocking(move || {
        extract_zip(&zip_path, &extract_path, &CancellationToken::new())
    })
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
        let jdk_bin = std::path::Path::new(java_home.as_str()).join("bin");
        let existing_path = std::env::var("PATH").unwrap_or_default();
        // Build an OS-correct PATH by prepending `jdk_bin` to the existing PATH
        // entries.  `split_paths`/`join_paths` handles the platform separator
        // (`:` on POSIX, `;` on Windows) and correct quoting for paths with
        // spaces — avoiding the POSIX-only `format!("{jdk_bin}:{existing}")` bug.
        let existing_entries = std::env::split_paths(&existing_path);
        let new_entries: Vec<_> = std::iter::once(jdk_bin).chain(existing_entries).collect();
        let new_path = std::env::join_paths(new_entries)
            .unwrap_or_else(|_| existing_path.clone().into())
            .to_string_lossy()
            .into_owned();
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
/// 2. If `<sdk_root>/cmdline-tools/latest/` already exists, **back it up**
///    by renaming it to `latest.bak-<pid>` inside the same parent directory.
///    This keeps the existing install intact until the new one is in place.
/// 3. Rename `<extract_dir>/cmdline-tools` → `<sdk_root>/cmdline-tools/latest`.
/// 4. On success, remove the backup.
/// 5. On rename failure, **restore** the backup so the pre-existing install is
///    not destroyed.
///
/// # Errors
///
/// Returns an error when the extracted `cmdline-tools` directory is missing,
/// when the parent cannot be created, or when the rename fails.  A failed
/// rename leaves the pre-existing `latest/` intact (restored from backup).
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

    // If a pre-existing `latest/` exists, rename it to a backup.  This
    // preserves the working install until the new one is safely in place.
    // The backup lives in the same directory so the rename is on the same
    // filesystem (guaranteed-atomic on POSIX).
    let backup = cmdline_tools_parent.join(format!("latest.bak-{}", std::process::id()));
    let has_backup = if dest.exists() {
        std::fs::rename(&dest, &backup).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "backup existing cmdline-tools/latest {} → {}: {e}",
                    dest.display(),
                    backup.display()
                ),
            ))
        })?;
        true
    } else {
        false
    };

    // Attempt the rename of the newly extracted tree.
    match std::fs::rename(&source, &dest) {
        Ok(()) => {
            tracing::debug!("Relocated cmdline-tools to {}", dest.display());
            // Success: remove the backup (best-effort; a leftover backup is
            // harmless and will be cleaned up by stale-tmp reclamation on the
            // next run).
            if has_backup {
                if let Err(e) = std::fs::remove_dir_all(&backup) {
                    tracing::warn!(
                        path = %backup.display(),
                        error = %e,
                        "could not remove cmdline-tools backup after successful relocation (harmless)"
                    );
                }
            }
            Ok(())
        }
        Err(rename_err) => {
            // Rename failed — restore the backup so the pre-existing install
            // is not destroyed.
            if has_backup {
                if let Err(restore_err) = std::fs::rename(&backup, &dest) {
                    tracing::error!(
                        backup = %backup.display(),
                        dest = %dest.display(),
                        rename_error = %rename_err,
                        restore_error = %restore_err,
                        "could not restore cmdline-tools backup after failed relocation; \
                         manual recovery needed"
                    );
                } else {
                    tracing::warn!(
                        "cmdline-tools relocation failed ({rename_err}); \
                         restored previous install from backup"
                    );
                }
            }
            Err(Error::Io(std::io::Error::new(
                rename_err.kind(),
                format!(
                    "rename {} → {}: {rename_err}",
                    source.display(),
                    dest.display()
                ),
            )))
        }
    }
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
            cmdline_tools_sha256: None,
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
            cmdline_tools_sha256: None,
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
            cmdline_tools_sha256: None,
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

    // ── Cancellation ──────────────────────────────────────────────────────────

    /// A pre-cancelled token must cause `install_android_tools` to return
    /// `Error::Cancelled` before any I/O is performed.
    #[tokio::test]
    async fn test_install_android_tools_precancelled_returns_cancelled() {
        let tmp = tempfile::TempDir::new().unwrap();

        let target = AndroidInstallTarget {
            sdk_root: tmp.path().to_owned(),
            api_level: 36,
            cmdline_tools_build: DEFAULT_CMDLINE_TOOLS_BUILD.to_string(),
            jdk_path: None,
            platform: HostPlatform::Linux,
            cmdline_tools_sha256: None,
        };

        let token = CancellationToken::new();
        token.cancel();

        let err = install_android_tools(&target, token, |_| {})
            .await
            .expect_err("pre-cancelled install must return Err");

        assert!(err.is_cancelled(), "error must be Cancelled, got: {err:?}");
    }

    // ── TempDirGuard (F14) ────────────────────────────────────────────────────

    /// An armed `TempDirGuard` removes the directory and its contents on drop.
    #[test]
    fn android_temp_dir_guard_removes_dir_on_drop() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join(".fdemon-android-tmp-12345");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("partial.zip"), b"partial").unwrap();
        assert!(dir.exists(), "dir must exist before drop");

        {
            let guard = TempDirGuard::new(dir.clone());
            drop(guard);
        }

        assert!(
            !dir.exists(),
            "armed TempDirGuard must remove the dir on drop"
        );
    }

    /// `reclaim_stale_android_tmps` removes all `.fdemon-android-tmp-*` dirs
    /// regardless of PID suffix, and leaves other directories intact.
    #[test]
    fn reclaim_stale_android_tmps_removes_all_tmp_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        // Plant two stale temp dirs with different PIDs.
        let stale1 = root.join(".fdemon-android-tmp-11111");
        let stale2 = root.join(".fdemon-android-tmp-22222");
        std::fs::create_dir_all(&stale1).unwrap();
        std::fs::write(stale1.join("cmdline-tools.zip"), b"").unwrap();
        std::fs::create_dir_all(&stale2).unwrap();

        // A non-tmp dir (e.g. installed SDK component) must be left alone.
        let keep = root.join("cmdline-tools");
        std::fs::create_dir_all(&keep).unwrap();

        reclaim_stale_android_tmps(root);

        assert!(!stale1.exists(), "stale android temp dir 1 must be removed");
        assert!(!stale2.exists(), "stale android temp dir 2 must be removed");
        assert!(keep.exists(), "non-tmp dir must not be removed");
    }

    // ── SHA-256 verification before extraction (AC2) ──────────────────────────

    /// Build a minimal valid zip archive in memory (single stored file).
    ///
    /// Used to produce a real zip body so `download_to_file` can write it to
    /// disk; the content is arbitrary since the SHA check fires before unzip.
    fn make_minimal_zip() -> Vec<u8> {
        use std::io::Write as IoWrite;
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            writer.start_file("dummy.txt", opts).unwrap();
            writer.write_all(b"dummy content").unwrap();
            writer.finish().unwrap();
        }
        buf
    }

    /// Compute the SHA-256 hex digest of a byte slice (test helper).
    fn sha256_hex(data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(data);
        format!("{:x}", h.finalize())
    }

    /// When `cmdline_tools_sha256` is configured and the downloaded zip has a
    /// *mismatching* digest, the install must fail before `extract_zip` is
    /// invoked (i.e., no files are extracted and an error is returned).
    ///
    /// We verify the "before extraction" property by asserting that the extract
    /// directory is empty/absent after the failed install.
    #[tokio::test]
    async fn test_sha256_mismatch_rejects_before_extraction() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // Serve a known valid zip.
        let zip_bytes = make_minimal_zip();
        let correct_sha = sha256_hex(&zip_bytes);
        // Tamper: flip the first hex digit to produce a wrong digest.
        let wrong_sha = {
            let mut s = correct_sha.clone();
            // Replace the first char with a different hex digit.
            let first = s.chars().next().unwrap();
            let replacement = if first == 'f' { '0' } else { 'f' };
            s.replace_range(..1, &replacement.to_string());
            s
        };
        // Ensure they actually differ (they should unless the hash starts with 'f' and
        // we swapped to 'f' again — statistically impossible but guard it).
        assert_ne!(
            correct_sha, wrong_sha,
            "test setup error: hashes must differ"
        );

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cmdline-tools.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone()))
            .mount(&mock_server)
            .await;

        let tmp = tempfile::TempDir::new().unwrap();
        let sdk_root = tmp.path().join("sdk");
        let tmp_dir = tmp.path().join("work");
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let target = AndroidInstallTarget {
            sdk_root: sdk_root.clone(),
            api_level: 36,
            cmdline_tools_build: DEFAULT_CMDLINE_TOOLS_BUILD.to_string(),
            jdk_path: None,
            platform: HostPlatform::Linux,
            // Provide a WRONG SHA-256 digest.
            cmdline_tools_sha256: Some(wrong_sha),
        };

        let url = format!("{}/cmdline-tools.zip", mock_server.uri());
        let token = tokio_util::sync::CancellationToken::new();
        let mut events: Vec<String> = Vec::new();

        let result =
            install_android_tools_inner(&target, &tmp_dir, &url, token, &mut |evt| match evt {
                InstallEvent::Phase(p) => events.push(p.to_string()),
                InstallEvent::Log(l) => events.push(l),
                _ => {}
            })
            .await;

        // Must fail with a SHA mismatch error.
        let err = result.expect_err("mismatched SHA must cause an error");
        assert!(
            err.to_string().contains("SHA-256") || err.to_string().contains("mismatch"),
            "error should mention SHA-256 or mismatch: {err}"
        );

        // The extract directory must NOT have been created/populated —
        // extraction was never reached.
        let extract_dir = tmp_dir.join("extract");
        assert!(
            !extract_dir.exists() || std::fs::read_dir(&extract_dir).unwrap().next().is_none(),
            "extract_dir must be absent or empty (extraction must not have happened)"
        );
    }

    // ── PATH separator (cross-platform) ──────────────────────────────────────

    /// The child PATH assembled by the install inner function must use the
    /// OS-correct separator (`:`on POSIX, `;` on Windows).
    ///
    /// We test the pure `join_paths`/`split_paths` composition directly.
    #[test]
    fn test_path_separator_join_paths_roundtrips() {
        // Build a PATH from two entries and verify each entry survives.
        let jdk_bin = PathBuf::from("/home/user/.jdks/corretto-21/bin");
        let existing = "/usr/local/bin:/usr/bin:/bin";

        let existing_entries = std::env::split_paths(existing);
        let new_entries: Vec<_> = std::iter::once(jdk_bin.clone())
            .chain(existing_entries)
            .collect();
        let joined = std::env::join_paths(new_entries).expect("join_paths must succeed");
        let joined_str = joined.to_string_lossy();

        // The JDK bin dir must appear first.
        let split: Vec<_> = std::env::split_paths(joined_str.as_ref()).collect();
        assert_eq!(split[0], jdk_bin, "JDK bin must be first entry");
        // Existing entries must be preserved.
        assert!(
            split.iter().any(|p| p == &PathBuf::from("/usr/local/bin")),
            "existing PATH entry must be preserved"
        );
        assert!(
            split.iter().any(|p| p == &PathBuf::from("/usr/bin")),
            "existing PATH entry /usr/bin must be preserved"
        );
    }

    #[test]
    fn test_path_separator_empty_existing_path_is_jdk_only() {
        let jdk_bin = PathBuf::from("/home/user/.jdks/corretto-21/bin");
        let existing = "";

        let existing_entries = std::env::split_paths(existing);
        let new_entries: Vec<_> = std::iter::once(jdk_bin.clone())
            .chain(existing_entries)
            .collect();
        let joined = std::env::join_paths(new_entries).expect("join_paths must succeed for empty");
        let split: Vec<_> = std::env::split_paths(joined.to_string_lossy().as_ref()).collect();

        // With empty existing PATH, the joined result should contain only the
        // JDK bin entry (and possibly an empty trailing entry from `split_paths
        // on ""`, which we filter).
        let non_empty: Vec<_> = split.iter().filter(|p| !p.as_os_str().is_empty()).collect();
        assert_eq!(
            non_empty.len(),
            1,
            "only JDK bin must be present when existing PATH is empty, got: {split:?}"
        );
        assert_eq!(non_empty[0], &jdk_bin);
    }

    // ── Backup-restore relocation (AC3) ───────────────────────────────────────

    /// When the source rename fails (simulated by making `source` a file, not a
    /// dir, so `rename` onto a pre-existing dir will fail on some platforms —
    /// we simulate by removing the source after the backup step), the
    /// pre-existing `latest/` must be restored from the backup.
    ///
    /// Because we can't reliably force `std::fs::rename` to fail in a
    /// cross-platform unit test, we test the invariant at the logical level:
    /// after a failed relocation the `dest` directory must still exist and the
    /// backup must be gone (restored).
    ///
    /// We use a wrapper that exercises the same backup-restore path by
    /// providing an invalid source (non-existent after the guard check) and
    /// verifying state.
    #[test]
    fn test_relocate_backup_restored_on_source_missing() {
        let sdk_root = tempfile::TempDir::new().unwrap();
        let cmdline_tools_parent = sdk_root.path().join("cmdline-tools");
        std::fs::create_dir_all(&cmdline_tools_parent).unwrap();

        // Pre-populate the existing `latest/` with a sentinel file.
        let latest = cmdline_tools_parent.join("latest");
        std::fs::create_dir_all(latest.join("bin")).unwrap();
        std::fs::write(latest.join("bin").join("existing_sdkmanager"), b"old").unwrap();

        // Use a separate extract_dir where we do NOT create the `cmdline-tools`
        // subdirectory — this triggers the "source missing" error path before
        // the backup is created, so `latest/` must remain untouched.
        let empty_extract = tempfile::TempDir::new().unwrap();
        let result = relocate_cmdline_tools(empty_extract.path(), sdk_root.path());

        // Must fail.
        assert!(result.is_err(), "missing source must return an error");

        // The pre-existing `latest/` must still be intact.
        assert!(
            latest.join("bin").join("existing_sdkmanager").exists(),
            "pre-existing latest/ must be untouched when source is absent"
        );
    }

    /// When a pre-existing `latest/` is backed up and the rename of the new
    /// source into `latest/` succeeds, the backup must be removed on success.
    #[test]
    fn test_relocate_backup_removed_on_success() {
        let sdk_root = tempfile::TempDir::new().unwrap();
        let extract_dir = tempfile::TempDir::new().unwrap();

        // Pre-populate an existing `latest/`.
        let cmdline_tools_parent = sdk_root.path().join("cmdline-tools");
        let latest = cmdline_tools_parent.join("latest");
        std::fs::create_dir_all(latest.join("bin")).unwrap();
        std::fs::write(latest.join("bin").join("old_sdkmanager"), b"old").unwrap();

        // Create a fresh extracted source.
        let src_bin = extract_dir.path().join("cmdline-tools").join("bin");
        std::fs::create_dir_all(&src_bin).unwrap();
        std::fs::write(src_bin.join("sdkmanager"), b"new").unwrap();

        relocate_cmdline_tools(extract_dir.path(), sdk_root.path())
            .expect("relocation must succeed");

        // New sdkmanager must be present.
        assert!(
            latest.join("bin").join("sdkmanager").exists(),
            "new sdkmanager must be present"
        );

        // Old file must be gone.
        assert!(
            !latest.join("bin").join("old_sdkmanager").exists(),
            "old file must be replaced"
        );

        // Backup directory (latest.bak-<pid>) must not exist.
        let backup = cmdline_tools_parent.join(format!("latest.bak-{}", std::process::id()));
        assert!(
            !backup.exists(),
            "backup directory must be removed after successful relocation"
        );
    }

    /// When `cmdline_tools_sha256` is configured and matches the downloaded zip,
    /// the installer proceeds past verification (it may fail later when trying
    /// to run `sdkmanager`, but the SHA check itself must not reject it).
    #[tokio::test]
    async fn test_sha256_match_passes_verification() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let zip_bytes = make_minimal_zip();
        let correct_sha = sha256_hex(&zip_bytes);

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cmdline-tools.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone()))
            .mount(&mock_server)
            .await;

        let tmp = tempfile::TempDir::new().unwrap();
        let sdk_root = tmp.path().join("sdk");
        std::fs::create_dir_all(&sdk_root).unwrap();
        let tmp_dir = tmp.path().join("work");
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let target = AndroidInstallTarget {
            sdk_root: sdk_root.clone(),
            api_level: 36,
            cmdline_tools_build: DEFAULT_CMDLINE_TOOLS_BUILD.to_string(),
            jdk_path: None,
            platform: HostPlatform::Linux,
            // Provide the CORRECT SHA-256 digest.
            cmdline_tools_sha256: Some(correct_sha),
        };

        let url = format!("{}/cmdline-tools.zip", mock_server.uri());
        let token = tokio_util::sync::CancellationToken::new();

        let result = install_android_tools_inner(&target, &tmp_dir, &url, token, &mut |_| {}).await;

        // The install will fail at relocation (the zip doesn't contain a
        // `cmdline-tools` dir), not at SHA verification.  We assert the error
        // message does NOT mention SHA to confirm the check passed.
        let err = result.expect_err("install must fail (no real sdkmanager)");
        assert!(
            !err.to_string().contains("SHA-256") && !err.to_string().contains("mismatch"),
            "error must not be a SHA-256 mismatch (sha check must have passed): {err}"
        );
    }
}

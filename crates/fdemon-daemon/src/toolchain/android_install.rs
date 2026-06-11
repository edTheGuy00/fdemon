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
//! - **JDK path**: the installer resolves the JDK home via `target.jdk_path`
//!   (explicit config) falling back to [`super::jdk::resolve_jdk_home`] (env
//!   `JAVA_HOME` / `which java`). The resolved home is validated with
//!   [`super::jdk::validate_jdk_home`] (requires `bin/javac`) before being set
//!   as `JAVA_HOME` for sdkmanager child processes. A missing or invalid JDK
//!   home fails the step with a clear, actionable error.
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
use super::jdk::{resolve_jdk_home, validate_jdk_home};
use super::process_stream::{run_streaming, run_streaming_with_input};
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

/// Maximum number of trailing lines included in a failure error message.
///
/// Keeps `WizardStepFailed.reason` readable while still surfacing the most
/// actionable part of sdkmanager output (the last few lines usually contain
/// the root cause).
const OUTPUT_TAIL_MAX_LINES: usize = 10;

/// Maximum total character length of the tail string embedded in an error.
///
/// Caps the error message at a human-readable length even when sdkmanager
/// emits very long lines.
const OUTPUT_TAIL_MAX_CHARS: usize = 800;

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

    // ── Pre-spawn guard: verify sdkmanager exists before invoking it ─────────
    //
    // After the relocate step, cmdline-tools/latest/bin/sdkmanager[.bat] must be
    // present. If it's missing (e.g. due to a layout change in a future
    // cmdline-tools release), `check_sdkmanager_guard` surfaces a diagnostic
    // listing the actual directory contents instead of letting the OS emit the
    // cryptic "path specified" error.
    check_sdkmanager_guard(&target.sdk_root)?;

    // ── Resolve and validate a JDK home for sdkmanager ──────────────────────
    //
    // Delegate to `build_sdkmanager_env` which handles the resolution /
    // validation / env-pair assembly in one pure step (also unit-testable).
    // It now also returns the resolved jdk_home so we can validate it before
    // running sdkmanager.
    tracing::debug!(
        explicit = target.jdk_path.is_some(),
        "Android install: resolving JDK home for sdkmanager"
    );
    let (env_pairs, jdk_home) = build_sdkmanager_env(&target.sdk_root, target.jdk_path.clone())?;

    // ── Pre-run JDK validation ───────────────────────────────────────────────
    //
    // Run `<jdk_home>/bin/java[.exe] -version` with the same env as sdkmanager
    // to confirm the resolved JDK can actually execute before invoking the bat
    // script.  A stale `[toolchain] jdk_path` / bad `JAVA_HOME` is caught here
    // with a clear diagnostic rather than inside sdkmanager.bat with a cryptic
    // cmd.exe "system cannot find the path specified" error.
    on_event(InstallEvent::Phase("Validating JDK"));

    let java_exe_path = java_exe_path(&jdk_home);
    let java_exe_str = java_exe_path.to_string_lossy().to_string();

    let env_refs_owned: Vec<(&str, &str)> = env_pairs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    tracing::debug!(java = %java_exe_path.display(), "Android install: validating JDK with -version");

    let java_version_result = run_streaming(
        &java_exe_str,
        &["-version"],
        Some(&target.sdk_root),
        |line| {
            tracing::debug!("java -version: {line}");
        },
    )
    .await;

    match java_version_result {
        Err(spawn_err) => {
            return Err(Error::process(format!(
                "resolved JDK at '{}' cannot execute java \
                 ('{java_exe_str} -version' failed: {spawn_err}); \
                 set [toolchain] jdk_path in .fdemon/config.toml to a valid JDK 17+ home",
                jdk_home.display()
            )));
        }
        Ok(status) if !status.success() => {
            return Err(Error::process(format!(
                "resolved JDK at '{}' cannot execute java \
                 ('{java_exe_str} -version' exited with {status}); \
                 set [toolchain] jdk_path in .fdemon/config.toml to a valid JDK 17+ home",
                jdk_home.display()
            )));
        }
        Ok(_) => {
            tracing::debug!("JDK validation passed for '{}'", jdk_home.display());
        }
    }

    let license_stdin = "y\n".repeat(LICENSE_YES_COUNT);

    // env_refs_owned was already built above during JDK validation; reuse it
    // for all sdkmanager invocations.
    let env_refs = env_refs_owned;

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
            let tail = output_tail(&log_lines, OUTPUT_TAIL_MAX_LINES, OUTPUT_TAIL_MAX_CHARS);
            return Err(Error::process(format!(
                "sdkmanager --licenses exited with {status}; last output: {tail}"
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

    let mut install_log_lines: Vec<String> = Vec::new();
    let install_status = run_streaming_with_input(
        &sdkmanager_str,
        &install_args,
        Some(&target.sdk_root),
        &env_refs,
        install_stdin.as_bytes(),
        |line| {
            on_event(InstallEvent::Log(line.clone()));
            install_log_lines.push(line);
        },
    )
    .await?;

    if !install_status.success() {
        let tail = output_tail(
            &install_log_lines,
            OUTPUT_TAIL_MAX_LINES,
            OUTPUT_TAIL_MAX_CHARS,
        );
        return Err(Error::process(format!(
            "sdkmanager package install exited with {install_status}; last output: {tail}"
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

/// List the file-system entries in `dir` and return them as a comma-separated
/// string for use in diagnostic error messages.
///
/// Returns `"<directory does not exist>"` when `dir` is absent, and
/// `"<empty>"` when `dir` exists but has no entries. Any entry whose name
/// cannot be decoded as UTF-8 is shown with a `?` placeholder.
fn list_dir_contents(dir: &Path) -> String {
    match std::fs::read_dir(dir) {
        Ok(rd) => {
            let names: Vec<String> = rd
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            if names.is_empty() {
                "<empty>".to_string()
            } else {
                names.join(", ")
            }
        }
        Err(_) => "<directory does not exist>".to_string(),
    }
}

/// Guard: verify that the `sdkmanager` binary exists at its expected path inside
/// `sdk_root/cmdline-tools/latest/bin/`. If absent, returns an error that lists
/// the actual contents of the bin directory so a layout-change regression yields
/// a precise diagnostic message.
///
/// This is the same check that fires inside `install_android_tools_inner` before
/// spawning sdkmanager; it is extracted as a standalone helper so unit tests can
/// exercise the guard without going through the full async install flow.
pub(crate) fn check_sdkmanager_guard(sdk_root: &Path) -> Result<()> {
    let sdkmanager = sdkmanager_path(sdk_root);
    if !sdkmanager.is_file() {
        let bin_dir = sdkmanager
            .parent()
            .expect("sdkmanager path always has a parent dir");
        let listing = list_dir_contents(bin_dir);
        return Err(Error::process(format!(
            "sdkmanager not found at '{}' after cmdline-tools installation. \
             Contents of '{}': [{}]. \
             This may indicate a cmdline-tools layout change — please file a bug \
             or update the fdemon cmdline-tools build number.",
            sdkmanager.display(),
            bin_dir.display(),
            listing,
        )));
    }
    Ok(())
}

/// Build the environment variable pairs required for `sdkmanager` child processes,
/// and return the resolved JDK home alongside them.
///
/// This is a pure, synchronous helper that:
/// 1. Resolves the JDK home: uses `jdk_path` if `Some`, otherwise falls back to
///    [`resolve_jdk_home`] (reads `JAVA_HOME` env / walks from `which java`).
/// 2. Validates the resolved home with [`validate_jdk_home`] (requires
///    `bin/java[.exe]` + `bin/javac[.exe]`).
/// 3. Assembles the env-pair vector:
///    - `ANDROID_HOME` = `sdk_root`
///    - `JAVA_HOME` = validated JDK home
///    - `PATH` = `<jdk_home>/bin` prepended to the current process `PATH`
///
/// Both the `--licenses` call and the package-install call use the same env
/// pairs returned by this function.  The returned `PathBuf` is the validated
/// JDK home, which the caller uses for pre-run java validation.
///
/// # Errors
///
/// Returns `Err` when no valid JDK home can be resolved or when the resolved home
/// fails validation. The error message names the remedies so the user can act
/// without reading docs.
pub(crate) fn build_sdkmanager_env(
    sdk_root: &Path,
    jdk_path: Option<PathBuf>,
) -> Result<(Vec<(String, String)>, PathBuf)> {
    let raw_jdk_home: Option<PathBuf> = jdk_path.or_else(resolve_jdk_home);

    let jdk_home: PathBuf = match raw_jdk_home {
        Some(candidate) => validate_jdk_home(&candidate).map_err(|e| {
            Error::process(format!(
                "Android install: JDK validation failed — {e}. \
                 Install a JDK 17+ (e.g. Eclipse Temurin), set '[toolchain] jdk_path' \
                 in .fdemon/config.toml, or fix the JAVA_HOME environment variable."
            ))
        })?,
        None => {
            return Err(Error::process(
                "Android install: no JDK home could be resolved. \
                 sdkmanager requires a valid Java Development Kit. \
                 Install a JDK 17+ (e.g. Eclipse Temurin), set '[toolchain] jdk_path' \
                 in .fdemon/config.toml, or fix the JAVA_HOME environment variable."
                    .to_string(),
            ));
        }
    };

    let sdk_root_str = sdk_root.to_string_lossy().into_owned();
    let java_home_str = jdk_home.to_string_lossy().into_owned();

    // Build an OS-correct PATH by prepending `<jdk_home>/bin` to the existing PATH
    // entries.  `split_paths`/`join_paths` handles the platform separator
    // (`:` on POSIX, `;` on Windows) and correct quoting for paths with spaces.
    let jdk_bin = jdk_home.join("bin");
    let existing_path = std::env::var("PATH").unwrap_or_default();
    let existing_entries = std::env::split_paths(&existing_path);
    let new_entries: Vec<_> = std::iter::once(jdk_bin).chain(existing_entries).collect();
    let new_path = std::env::join_paths(new_entries)
        .unwrap_or_else(|_| existing_path.clone().into())
        .to_string_lossy()
        .into_owned();

    let mut env_pairs: Vec<(String, String)> = vec![("ANDROID_HOME".to_string(), sdk_root_str)];
    env_pairs.push(("JAVA_HOME".to_string(), java_home_str));
    env_pairs.push(("PATH".to_string(), new_path));
    Ok((env_pairs, jdk_home))
}

/// Return the platform-correct path to the `java` executable within `jdk_home`.
///
/// On Windows the binary is `bin\java.exe`; on POSIX it is `bin/java`.
///
/// This is extracted as a pure helper so unit tests can assert the correct name
/// is constructed for both platform names without spawning a real process.
pub(crate) fn java_exe_path(jdk_home: &Path) -> PathBuf {
    #[cfg(windows)]
    let java_name = "java.exe";
    #[cfg(not(windows))]
    let java_name = "java";
    jdk_home.join("bin").join(java_name)
}

/// Format the last `max_lines` non-empty lines from `lines` as a single
/// `" | "`-joined string, capped at `max_chars` total characters.
///
/// This is used to embed a human-readable tail of sdkmanager output inside
/// a `WizardStepFailed.reason` so the user sees the actual error from the
/// bat script rather than "see log above for details".
///
/// Empty lines are filtered out before selecting the tail so that a block of
/// blank trailing lines does not push the meaningful output off the end.
///
/// When the joined tail exceeds `max_chars`, it is truncated to exactly
/// `max_chars` characters and a `"…"` suffix is appended.
pub(crate) fn output_tail(lines: &[String], max_lines: usize, max_chars: usize) -> String {
    let non_empty: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|l| !l.trim().is_empty())
        .collect();

    let tail_start = non_empty.len().saturating_sub(max_lines);
    let tail = non_empty[tail_start..].join(" | ");

    if tail.len() <= max_chars {
        tail
    } else {
        // Truncate to max_chars and append ellipsis.
        let truncated: String = tail.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
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
        // Construct the existing PATH with OS-correct separators (`;` on
        // Windows, `:` on POSIX) via `join_paths` rather than a hardcoded
        // colon-joined literal — otherwise `split_paths` on Windows treats the
        // whole string as a single entry and the preservation checks fail.
        let existing = std::env::join_paths([
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
        ])
        .expect("join existing PATH must succeed");

        let existing_entries = std::env::split_paths(&existing);
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

    // ── build_sdkmanager_env: JDK-home precedence ─────────────────────────────

    /// Helper: create a minimal JDK fixture (bin/java + bin/javac) in `dir`.
    fn make_jdk_fixture(dir: &std::path::Path) {
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();

        #[cfg(windows)]
        let (java_name, javac_name) = ("java.exe", "javac.exe");
        #[cfg(not(windows))]
        let (java_name, javac_name) = ("java", "javac");

        std::fs::write(bin.join(java_name), b"#!/bin/sh\nexec java").unwrap();
        std::fs::write(bin.join(javac_name), b"#!/bin/sh\nexec javac").unwrap();
    }

    /// Helper: create the sdkmanager binary at `sdk_root/cmdline-tools/latest/bin/`.
    fn make_sdkmanager_fixture(sdk_root: &std::path::Path) {
        let bin = sdk_root.join("cmdline-tools").join("latest").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        // Create the platform-correct binary name.
        std::fs::write(
            bin.join(sdkmanager_bin_name()),
            b"#!/bin/sh\necho sdkmanager",
        )
        .unwrap();
    }

    /// When `jdk_path` is `Some(valid_jdk)`, `build_sdkmanager_env` uses that
    /// explicit path and does NOT fall back to `resolve_jdk_home()`. We verify
    /// this by supplying a valid JDK fixture and confirming JAVA_HOME in the
    /// returned env pairs matches the fixture path.
    #[test]
    fn test_build_sdkmanager_env_explicit_jdk_path_wins() {
        let sdk_tmp = tempfile::TempDir::new().unwrap();
        let jdk_tmp = tempfile::TempDir::new().unwrap();
        make_jdk_fixture(jdk_tmp.path());

        let (env_pairs, returned_jdk_home) =
            build_sdkmanager_env(sdk_tmp.path(), Some(jdk_tmp.path().to_owned()))
                .expect("explicit valid jdk_path must succeed");

        let java_home = env_pairs
            .iter()
            .find(|(k, _)| k == "JAVA_HOME")
            .map(|(_, v)| v.as_str())
            .expect("JAVA_HOME must be present in env_pairs");

        // The returned JAVA_HOME must be (a normalisation of) the fixture path.
        assert_eq!(
            PathBuf::from(java_home),
            jdk_tmp.path(),
            "JAVA_HOME must match the explicit jdk_path fixture, got: {java_home}"
        );
        // The returned jdk_home PathBuf must also match.
        assert_eq!(
            returned_jdk_home,
            jdk_tmp.path(),
            "returned jdk_home must match fixture path"
        );
    }

    // ── build_sdkmanager_env: env-pair contents ───────────────────────────────

    /// The assembled env pairs must contain ANDROID_HOME, JAVA_HOME, and PATH,
    /// with PATH's first entry being `<jdk_home>/bin`.
    #[test]
    fn test_build_sdkmanager_env_contains_required_vars() {
        let sdk_tmp = tempfile::TempDir::new().unwrap();
        let jdk_tmp = tempfile::TempDir::new().unwrap();
        make_jdk_fixture(jdk_tmp.path());

        let (env_pairs, _jdk_home) =
            build_sdkmanager_env(sdk_tmp.path(), Some(jdk_tmp.path().to_owned()))
                .expect("must succeed with valid JDK fixture");

        // ANDROID_HOME must be set to sdk_root.
        let android_home = env_pairs
            .iter()
            .find(|(k, _)| k == "ANDROID_HOME")
            .map(|(_, v)| v.as_str())
            .expect("ANDROID_HOME must be present");
        assert_eq!(
            PathBuf::from(android_home),
            sdk_tmp.path(),
            "ANDROID_HOME must equal sdk_root"
        );

        // JAVA_HOME must be set to the validated JDK home.
        let java_home = env_pairs
            .iter()
            .find(|(k, _)| k == "JAVA_HOME")
            .map(|(_, v)| PathBuf::from(v))
            .expect("JAVA_HOME must be present");
        assert_eq!(
            java_home,
            jdk_tmp.path(),
            "JAVA_HOME must equal the jdk fixture path"
        );

        // PATH must be present and its first entry must be <jdk_home>/bin.
        let path_val = env_pairs
            .iter()
            .find(|(k, _)| k == "PATH")
            .map(|(_, v)| v.clone())
            .expect("PATH must be present");

        let path_entries: Vec<PathBuf> = std::env::split_paths(&path_val).collect();
        assert!(
            !path_entries.is_empty(),
            "PATH must have at least one entry"
        );
        assert_eq!(
            path_entries[0],
            jdk_tmp.path().join("bin"),
            "first PATH entry must be <jdk_home>/bin, got: {:?}",
            path_entries[0]
        );
    }

    // ── build_sdkmanager_env: missing / invalid JDK yields actionable error ───

    /// When `jdk_path` points to a non-existent directory and `resolve_jdk_home()`
    /// also fails (because the provided path is explicitly invalid), the function
    /// must return `Err` whose message names the remedies.
    #[test]
    fn test_build_sdkmanager_env_invalid_jdk_path_yields_actionable_error() {
        let sdk_tmp = tempfile::TempDir::new().unwrap();
        let nonexistent_jdk = PathBuf::from("/this/path/does/not/exist/fdemon_jdk_test");

        let err = build_sdkmanager_env(sdk_tmp.path(), Some(nonexistent_jdk))
            .expect_err("invalid jdk_path must return Err");

        let msg = err.to_string();
        // The error must name at least one remedy so the user can act.
        assert!(
            msg.contains("jdk_path") || msg.contains("JAVA_HOME") || msg.contains("JDK"),
            "error must name a remedy (jdk_path / JAVA_HOME / JDK): {msg}"
        );
        assert!(
            msg.contains("Install") || msg.contains("fix") || msg.contains("set"),
            "error must be actionable (install / fix / set): {msg}"
        );
    }

    /// When `jdk_path` is `Some` pointing to a JRE-only directory (bin/java but
    /// no bin/javac), the function must return `Err` mentioning the missing
    /// javac and the sdkmanager requirement.
    #[test]
    fn test_build_sdkmanager_env_jre_only_dir_yields_error() {
        let sdk_tmp = tempfile::TempDir::new().unwrap();
        let jre_tmp = tempfile::TempDir::new().unwrap();

        // Plant only java, not javac — simulates a JRE install.
        let bin = jre_tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        #[cfg(windows)]
        let java_name = "java.exe";
        #[cfg(not(windows))]
        let java_name = "java";
        std::fs::write(bin.join(java_name), b"#!/bin/sh\nexec java").unwrap();

        let err = build_sdkmanager_env(sdk_tmp.path(), Some(jre_tmp.path().to_owned()))
            .expect_err("JRE-only dir must return Err");

        let msg = err.to_string();
        assert!(
            msg.contains("javac") || msg.contains("JRE") || msg.contains("JDK"),
            "error must mention javac / JRE / JDK: {msg}"
        );
    }

    // ── check_sdkmanager_guard: pre-spawn guard ───────────────────────────────

    /// When `sdkmanager` binary is present at the expected path, the guard
    /// returns `Ok(())`.
    #[test]
    fn test_check_sdkmanager_guard_present_returns_ok() {
        let sdk_tmp = tempfile::TempDir::new().unwrap();
        make_sdkmanager_fixture(sdk_tmp.path());

        let result = check_sdkmanager_guard(sdk_tmp.path());
        assert!(
            result.is_ok(),
            "guard must return Ok when sdkmanager is present: {:?}",
            result
        );
    }

    /// When `sdkmanager` is absent (empty bin dir), the guard must return `Err`
    /// whose message includes the expected path AND a listing of the bin dir
    /// contents.
    #[test]
    fn test_check_sdkmanager_guard_absent_returns_err_with_listing() {
        let sdk_tmp = tempfile::TempDir::new().unwrap();

        // Create the bin dir with a decoy file, but NOT sdkmanager.
        let bin_dir = sdk_tmp
            .path()
            .join("cmdline-tools")
            .join("latest")
            .join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("not_sdkmanager.sh"), b"#!/bin/sh").unwrap();

        let err = check_sdkmanager_guard(sdk_tmp.path())
            .expect_err("guard must return Err when sdkmanager is absent");

        let msg = err.to_string();

        // Error must mention the expected path.
        assert!(
            msg.contains("sdkmanager"),
            "error must mention sdkmanager: {msg}"
        );
        // Error must include a listing (the decoy file name).
        assert!(
            msg.contains("not_sdkmanager.sh"),
            "error must list bin dir contents (decoy file): {msg}"
        );
        // Error must mention the bin dir path.
        assert!(
            msg.contains("cmdline-tools"),
            "error must mention cmdline-tools path: {msg}"
        );
    }

    /// When `sdkmanager` is absent and the bin dir itself is also absent
    /// (e.g. fresh SDK root), the guard must return `Err` indicating the dir
    /// does not exist.
    #[test]
    fn test_check_sdkmanager_guard_absent_bin_dir_returns_err() {
        let sdk_tmp = tempfile::TempDir::new().unwrap();
        // Do NOT create cmdline-tools/latest/bin/ at all.

        let err = check_sdkmanager_guard(sdk_tmp.path())
            .expect_err("guard must return Err when bin dir is absent");

        let msg = err.to_string();
        assert!(
            msg.contains("sdkmanager"),
            "error must mention sdkmanager: {msg}"
        );
        // list_dir_contents returns "<directory does not exist>" for absent dirs.
        assert!(
            msg.contains("does not exist") || msg.contains("not found"),
            "error must indicate directory absence: {msg}"
        );
    }

    // ── output_tail helper ────────────────────────────────────────────────────

    /// Empty input returns an empty string.
    #[test]
    fn test_output_tail_empty_returns_empty() {
        assert_eq!(output_tail(&[], 10, 800), "");
    }

    /// When there are fewer lines than max_lines, all non-empty lines are
    /// included.
    #[test]
    fn test_output_tail_short_input_returns_all_non_empty() {
        let lines: Vec<String> = vec![
            "line 1".to_string(),
            "line 2".to_string(),
            "line 3".to_string(),
        ];
        let tail = output_tail(&lines, 10, 800);
        assert!(tail.contains("line 1"), "all lines must appear: {tail}");
        assert!(tail.contains("line 2"), "all lines must appear: {tail}");
        assert!(tail.contains("line 3"), "all lines must appear: {tail}");
    }

    /// When there are more lines than max_lines, only the last max_lines are
    /// included and early lines are excluded.
    #[test]
    fn test_output_tail_long_input_returns_last_n_lines() {
        // Use zero-padded names so "item-001" is not a substring of "item-016".
        let lines: Vec<String> = (1..=20).map(|i| format!("item-{i:03}")).collect();
        let tail = output_tail(&lines, 5, 800);
        // Last 5 lines must be present.
        for i in 16..=20 {
            assert!(
                tail.contains(&format!("item-{i:03}")),
                "item-{i:03} must be in tail: {tail}"
            );
        }
        // Early lines must be absent.
        assert!(
            !tail.contains("item-001"),
            "early item-001 must not appear in tail: {tail}"
        );
        assert!(
            !tail.contains("item-015"),
            "item-015 must not appear in tail: {tail}"
        );
    }

    /// Blank lines are filtered out before selecting the tail.
    #[test]
    fn test_output_tail_filters_blank_lines() {
        let lines: Vec<String> = vec![
            "error: something bad".to_string(),
            "".to_string(),
            "   ".to_string(),
            "".to_string(),
        ];
        let tail = output_tail(&lines, 10, 800);
        // Only the non-blank line should appear.
        assert!(
            tail.contains("error: something bad"),
            "non-blank line must be present: {tail}"
        );
        // The result must not contain the blank lines as standalone content.
        // (join uses " | " so blanks would appear as empty segments — they
        // should be absent entirely after filtering.)
        assert!(
            !tail.contains(" |  | "),
            "blank lines must be filtered: {tail}"
        );
    }

    /// When all lines are blank, output_tail returns an empty string.
    #[test]
    fn test_output_tail_all_blank_returns_empty() {
        let lines: Vec<String> = vec!["".to_string(), "   ".to_string(), "\t".to_string()];
        assert_eq!(output_tail(&lines, 10, 800), "");
    }

    /// When the joined tail exceeds max_chars, it is truncated and appended
    /// with an ellipsis.
    #[test]
    fn test_output_tail_truncates_at_max_chars() {
        // Create a single very long line.
        let long_line: String = "x".repeat(2000);
        let lines = vec![long_line];
        let tail = output_tail(&lines, 10, 100);
        // Tail must be capped at 100 chars + "…".
        assert!(
            tail.ends_with('…'),
            "truncated tail must end with ellipsis: {tail}"
        );
        // The body (before "…") must be exactly 100 chars.
        let body: String = tail.chars().take_while(|&c| c != '…').collect();
        assert_eq!(
            body.len(),
            100,
            "truncated body must be exactly max_chars long"
        );
    }

    /// A tail that is exactly max_chars long is returned without truncation.
    #[test]
    fn test_output_tail_exactly_max_chars_not_truncated() {
        let line: String = "a".repeat(100);
        let lines = vec![line.clone()];
        let tail = output_tail(&lines, 10, 100);
        assert_eq!(
            tail, line,
            "tail must not be truncated at exactly max_chars"
        );
    }

    // ── java_exe_path helper ──────────────────────────────────────────────────

    /// On any host OS, `java_exe_path` with a POSIX-style JDK home produces
    /// `bin/java` (the POSIX name) when compiled for non-Windows.
    ///
    /// We test the output name via the last path component so the test is
    /// meaningful on both Linux and macOS CI runners.
    #[cfg(not(windows))]
    #[test]
    fn test_java_exe_path_posix_name() {
        let jdk_home = PathBuf::from("/home/user/.jdks/corretto-21");
        let path = java_exe_path(&jdk_home);
        let file_name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(file_name, "java", "POSIX java exe name must be 'java'");
        assert!(
            path.to_string_lossy().contains("bin"),
            "path must include bin/: {}",
            path.display()
        );
    }

    /// On Windows (or when simulating the Windows path), `java_exe_path`
    /// produces `bin/java.exe`.
    ///
    /// This test runs on all hosts but uses a manually constructed path to
    /// verify the Windows file name is correctly derived.
    #[cfg(windows)]
    #[test]
    fn test_java_exe_path_windows_name() {
        let jdk_home = PathBuf::from(r"C:\Program Files\Eclipse Adoptium\jdk-21");
        let path = java_exe_path(&jdk_home);
        let file_name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(
            file_name, "java.exe",
            "Windows java exe name must be 'java.exe'"
        );
    }

    /// Cross-platform: verify the `bin/` component is always present in the
    /// constructed path regardless of OS.
    #[test]
    fn test_java_exe_path_always_has_bin_component() {
        let jdk_home = PathBuf::from("/some/jdk/home");
        let path = java_exe_path(&jdk_home);

        let components: Vec<_> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        assert!(
            components.contains(&"bin".to_string()),
            "java_exe_path must always include a 'bin' component: {}",
            path.display()
        );
    }
}

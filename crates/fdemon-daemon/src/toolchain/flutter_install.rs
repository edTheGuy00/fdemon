//! # Managed Flutter SDK Installer
//!
//! Implements the high-level Flutter SDK install flow:
//!
//! 1. Resolve the install root (`~/fvm/versions` by default, `$FVM_CACHE_PATH`
//!    if set, or an explicit caller override).
//! 2. Fetch the Flutter releases manifest from the Google CDN and select the
//!    best release for the current OS + CPU architecture for the configured channel.
//! 3. Install via `git clone` (default) or archive download+verify+extract
//!    (fallback when `git` is absent or the caller forces `Archive` mode).
//! 4. Atomically rename the temp dir into the final install location, reclaiming
//!    any incomplete prior install in `final_dir`.
//! 5. Run `flutter precache` (non-fatal on failure — the SDK is usable; the
//!    caller may retry precache separately).
//!
//! ## Design Notes
//!
//! - **Atomic install**: all work happens inside `.fdemon-install-tmp-<pid>`;
//!   the final rename is atomic on POSIX. On failure the temp dir is removed.
//! - **Concurrent-install guard**: an advisory lockfile (`.fdemon-install.lock`)
//!   in the install root prevents two processes from colliding on the same
//!   `final_dir`. A RAII `LockGuard` releases the lock on drop (including panics
//!   and early returns).
//! - **Partial-dir reclamation**: if `final_dir` exists but `bin/flutter` is
//!   absent (incomplete prior install), the partial dir is removed before the
//!   rename so the install can proceed cleanly.
//! - **Precache non-fatal**: `flutter precache` may fail on network-restricted
//!   hosts or in CI. The installed SDK is still functional for most workflows.
//!   The caller receives a `Log` event describing the failure.
//! - **UI-agnostic**: `InstallEvent` carries only data; mapping to app messages
//!   is the caller's responsibility (task 08).
//!
//! ## Public API
//!
//! - [`InstallEvent`] — progress events streamed to the caller.
//! - [`resolve_install_dir`] — determine where to install.
//! - [`fetch_release_manifest`] — download and parse the releases JSON.
//! - [`install_flutter`] — orchestrate the full install flow.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use fdemon_core::{Error, Result};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::download::{
    check_network_connectivity, download_to_file, extract_archive, verify_sha256,
};
use super::process_stream::run_streaming;
use super::types::{
    DownloadProgress, FlutterInstallOutcome, FlutterInstallTarget, FlutterRelease,
    FlutterReleaseManifest, HostArch, HostPlatform, InstallMethod,
};

// ── Timeout constants ─────────────────────────────────────────────────────────

/// TCP connection timeout for manifest HTTP requests.
const MANIFEST_CONNECT_TIMEOUT_SECS: u64 = 15;

/// Total request timeout for manifest HTTP requests (includes body transfer).
const MANIFEST_REQUEST_TIMEOUT_SECS: u64 = 60;

// ── InstallEvent ──────────────────────────────────────────────────────────────

/// Progress events emitted during a managed Flutter SDK install.
///
/// Callers receive these through the `on_event` callback and are responsible
/// for mapping them to UI messages (see task 08).
#[derive(Debug, Clone)]
pub enum InstallEvent {
    /// A single log line from the install process (e.g., `git clone` progress,
    /// `flutter precache` output).
    Log(String),
    /// Archive download progress (bytes received / total).
    Download(DownloadProgress),
    /// High-level phase transition: `"Cloning"`, `"Downloading"`,
    /// `"Verifying"`, `"Extracting"`, `"Precaching"`.
    Phase(&'static str),
}

// ── Advisory lock guard ───────────────────────────────────────────────────────

/// RAII guard that holds an advisory install lockfile.
///
/// The lock is acquired by creating `<install_root>/.fdemon-install.lock` with
/// `O_CREAT | O_EXCL`. The file is removed when this guard is dropped —
/// including on panics and early returns — so the lock is always released.
#[derive(Debug)]
struct LockGuard {
    lock_path: PathBuf,
}

impl LockGuard {
    /// Acquire the lock. Returns `Err` if the lock already exists (another
    /// install is in progress or a stale lock was not cleaned up).
    fn acquire(install_root: &Path) -> Result<Self> {
        let lock_path = install_root.join(".fdemon-install.lock");

        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    Error::process(format!(
                        "another install is in progress (or a stale lock exists at {}); \
                         retry shortly — if no install is running, remove the lock file manually",
                        lock_path.display()
                    ))
                } else {
                    Error::Io(std::io::Error::new(
                        e.kind(),
                        format!("create install lock {}: {e}", lock_path.display()),
                    ))
                }
            })?;

        Ok(Self { lock_path })
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.lock_path) {
            tracing::warn!(
                "Failed to remove install lockfile {}: {e}",
                self.lock_path.display()
            );
        }
    }
}

// ── RAII temp-dir guard ───────────────────────────────────────────────────────

/// RAII guard that removes a directory (and all its contents) when dropped.
///
/// The guard is **armed** on construction and **disarmed** only on success via
/// [`TempDirGuard::disarm`].  Removal runs even when the outer future is
/// dropped mid-execution via `JoinHandle::abort()`, because `Drop` is called
/// synchronously during the drop cascade.
///
/// This ensures no partially-extracted SDK tree is leaked on cancellation,
/// abort, or any other early-exit path.
struct TempDirGuard {
    path: PathBuf,
    armed: bool,
}

impl TempDirGuard {
    /// Create a new armed guard for `path`.
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    /// Disarm the guard so the directory is **not** removed on drop.
    ///
    /// Call this after a successful atomic rename so the guard does not attempt
    /// to remove the now-renamed (and relocated) directory.
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if self.armed && self.path.exists() {
            if let Err(e) = std::fs::remove_dir_all(&self.path) {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "TempDirGuard: failed to remove install temp dir (best-effort)"
                );
            }
        }
    }
}

// ── Stale-temp reclamation ────────────────────────────────────────────────────

/// Glob `install_root` for any `.fdemon-install-tmp-*` directories (from any
/// PID) and remove them.
///
/// Must be called **under the `LockGuard`** so there is no race with a
/// concurrent install that also holds a live temp dir.
///
/// Any directory that cannot be removed is logged as a warning and skipped;
/// the function does **not** propagate removal errors.
fn reclaim_stale_flutter_tmps(install_root: &Path) {
    let read_dir = match std::fs::read_dir(install_root) {
        Ok(rd) => rd,
        Err(e) => {
            tracing::debug!(
                root = %install_root.display(),
                error = %e,
                "reclaim_stale_flutter_tmps: read_dir failed; skipping reclamation"
            );
            return;
        }
    };
    for entry in read_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(".fdemon-install-tmp-") {
            let path = entry.path();
            if path.is_dir() {
                tracing::debug!(path = %path.display(), "reclaim_stale_flutter_tmps: removing stale temp dir");
                if let Err(e) = std::fs::remove_dir_all(&path) {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "reclaim_stale_flutter_tmps: could not remove stale temp dir"
                    );
                }
            }
        }
    }
}

// ── Channel validation ────────────────────────────────────────────────────────

/// Validate a Flutter channel name for use in git and archive operations.
///
/// Accepted characters: `[A-Za-z0-9._-]`. A leading `-` is rejected because
/// it would be interpreted as a command-line flag by git. Empty strings are
/// also rejected.
///
/// This guard prevents argument-injection attacks when the channel value comes
/// from user-controlled config (e.g. `.fdemon/config.toml`'s `[toolchain]`
/// block). A value like `--upload-pack=…` or `--config core.askpass=…` would
/// otherwise be passed directly to `git clone -b <channel>` and interpreted as
/// a git option, enabling remote code execution.
fn validate_channel(channel: &str) -> Result<()> {
    if channel.is_empty() {
        return Err(Error::process(
            "toolchain channel must not be empty; valid values are e.g. 'stable', 'beta'",
        ));
    }
    if channel.starts_with('-') {
        return Err(Error::process(format!(
            "toolchain channel '{channel}' starts with '-', which is not a valid channel name \
             (would be interpreted as a git option)"
        )));
    }
    if !channel
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(Error::process(format!(
            "toolchain channel '{channel}' contains invalid characters; \
             only [A-Za-z0-9._-] are allowed"
        )));
    }
    Ok(())
}

// ── resolve_install_dir ───────────────────────────────────────────────────────

/// Determine the directory under which the Flutter SDK will be installed.
///
/// Resolution order:
/// 1. `explicit_root` — caller-supplied override (e.g. from `.fdemon/config.toml`).
/// 2. `$FVM_CACHE_PATH` — environment variable honoured by fvm.
///    Must be an absolute path; relative values are ignored with a warning.
/// 3. `~/fvm/versions` — fvm default; created if absent.
///
/// The resolved directory is created with `create_dir_all` when it does not
/// exist. The `FlutterInstallTarget::version_dir_name` sub-directory is
/// **not** created here; that happens inside [`install_flutter`].
///
/// # Errors
///
/// Returns an error when no home directory can be determined or when the
/// directory cannot be created.
pub fn resolve_install_dir(explicit_root: Option<&Path>) -> Result<PathBuf> {
    // 1. Explicit override.
    if let Some(root) = explicit_root {
        std::fs::create_dir_all(root).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("create install root {root:?}: {e}"),
            ))
        })?;
        return Ok(root.to_owned());
    }

    // 2. $FVM_CACHE_PATH env var (must be an absolute path).
    if let Ok(env_path) = std::env::var("FVM_CACHE_PATH") {
        let path = PathBuf::from(&env_path);
        if path.is_absolute() {
            std::fs::create_dir_all(&path).map_err(|e| {
                Error::Io(std::io::Error::new(
                    e.kind(),
                    format!("create $FVM_CACHE_PATH directory {path:?}: {e}"),
                ))
            })?;
            return Ok(path);
        } else {
            tracing::warn!(
                "$FVM_CACHE_PATH is a relative path ({env_path:?}); \
                 ignoring it and falling back to ~/fvm/versions"
            );
        }
    }

    // 3. ~/fvm/versions default.
    let home = dirs::home_dir().ok_or_else(|| {
        Error::process("cannot determine home directory for Flutter install root")
    })?;
    let default_root = home.join("fvm").join("versions");
    std::fs::create_dir_all(&default_root).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("create default install root {default_root:?}: {e}"),
        ))
    })?;
    Ok(default_root)
}

// ── Manifest fetch ────────────────────────────────────────────────────────────

/// OS identifier used in the Flutter releases manifest URL.
fn platform_os_label(platform: &HostPlatform) -> &'static str {
    match platform {
        HostPlatform::Linux => "linux",
        HostPlatform::MacOs => "macos",
        HostPlatform::Windows => "windows",
        HostPlatform::Unknown => "linux", // Best-effort fallback.
    }
}

/// Construct the manifest URL for the given platform.
fn manifest_url(platform: &HostPlatform) -> String {
    let os = platform_os_label(platform);
    format!("https://storage.googleapis.com/flutter_infra_release/releases/releases_{os}.json")
}

/// Archive file extension for the given platform (`.tar.xz` on Linux; `.zip` elsewhere).
fn archive_extension(platform: &HostPlatform) -> &'static str {
    match platform {
        HostPlatform::Linux => ".tar.xz",
        _ => ".zip",
    }
}

/// Resolve the best release for `channel` + `arch` from a manifest.
///
/// Selection order:
/// 1. First release whose `channel` matches and whose `dart_sdk_arch` matches `arch`.
/// 2. First release whose `channel` matches (any arch — covers older manifests).
///
/// Returns `None` when no release for the requested channel exists.
fn resolve_channel_release<'m>(
    manifest: &'m FlutterReleaseManifest,
    channel: &str,
    arch: HostArch,
) -> Option<&'m FlutterRelease> {
    let arch_str = arch.as_manifest_str();

    // Pass 1: prefer exact arch match within the channel.
    if let Some(label) = arch_str {
        if let Some(r) = manifest
            .releases
            .iter()
            .find(|r| r.channel == channel && r.dart_sdk_arch.as_deref() == Some(label))
        {
            return Some(r);
        }
    }

    // Pass 2: fall back to any release in the channel.
    manifest.releases.iter().find(|r| r.channel == channel)
}

// ── Wire JSON types (serde deserialization) ───────────────────────────────────

/// Internal JSON shape for a single releases manifest entry.
#[derive(Debug, Deserialize)]
struct RawRelease {
    version: String,
    channel: String,
    archive: String,
    sha256: String,
    #[serde(rename = "dart_sdk_arch")]
    dart_sdk_arch: Option<String>,
}

/// Internal JSON shape for the `current_release` object.
#[derive(Debug, Deserialize)]
struct RawCurrentRelease {
    stable: Option<String>,
}

/// Internal JSON shape for the top-level manifest.
#[derive(Debug, Deserialize)]
struct RawManifest {
    base_url: String,
    current_release: Option<RawCurrentRelease>,
    releases: Vec<RawRelease>,
}

/// Fetch and parse the Flutter releases manifest for the given platform.
///
/// The manifest is downloaded from the Google Flutter infrastructure CDN.
/// URL format: `https://storage.googleapis.com/flutter_infra_release/releases/releases_<os>.json`
///
/// This is a thin wrapper around [`fetch_release_manifest_from`] that
/// constructs the CDN URL from `platform` and delegates all HTTP work there.
///
/// # Errors
///
/// Returns an error on network failure (including the HEAD probe), non-2xx
/// HTTP status, or JSON parse error.
pub async fn fetch_release_manifest(platform: HostPlatform) -> Result<FlutterReleaseManifest> {
    let url = manifest_url(&platform);
    fetch_release_manifest_from(&url).await
}

/// Fetch and parse the Flutter releases manifest from an explicit URL.
///
/// The HTTP client applies a [`MANIFEST_CONNECT_TIMEOUT_SECS`] TCP connect
/// timeout and a [`MANIFEST_REQUEST_TIMEOUT_SECS`] total request timeout.
///
/// A ≤5s HEAD probe is performed against `url` before the full GET.  This
/// bounds the offline stall to 5 seconds instead of
/// `MANIFEST_REQUEST_TIMEOUT_SECS` (60 s) for users without network access.
///
/// Extracted from [`fetch_release_manifest`] so that tests can point it at a
/// local mock server instead of the production CDN URL, exercising the real
/// HEAD→GET→parse sequencing end-to-end.
///
/// # Errors
///
/// Returns an error on network failure (including the HEAD probe), non-2xx
/// HTTP status, or JSON parse error.
pub(crate) async fn fetch_release_manifest_from(url: &str) -> Result<FlutterReleaseManifest> {
    tracing::debug!("Fetching Flutter releases manifest from {url}");

    let client = reqwest::Client::builder()
        .user_agent(concat!("fdemon/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(
            MANIFEST_CONNECT_TIMEOUT_SECS,
        ))
        .timeout(std::time::Duration::from_secs(
            MANIFEST_REQUEST_TIMEOUT_SECS,
        ))
        .build()
        .map_err(|e| Error::process(format!("failed to build HTTP client: {e}")))?;

    // Network preflight: fast HEAD probe bounds the offline stall to ≤5 s.
    // If the probe fails the full GET would also fail, so surface the error
    // immediately with a clear "no network connectivity" message.
    check_network_connectivity(&client, url).await?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| Error::process(format!("manifest request failed for {url}: {e}")))?;

    if !response.status().is_success() {
        return Err(Error::process(format!(
            "manifest HTTP {} for {url}",
            response.status()
        )));
    }

    let raw: RawManifest = response
        .json()
        .await
        .map_err(|e| Error::process(format!("failed to parse manifest from {url}: {e}")))?;

    let current_stable_hash = raw.current_release.and_then(|cr| cr.stable);

    let releases: Vec<FlutterRelease> = raw
        .releases
        .into_iter()
        .map(|r| FlutterRelease {
            version: r.version,
            channel: r.channel,
            archive: r.archive,
            sha256: r.sha256,
            dart_sdk_arch: r.dart_sdk_arch,
        })
        .collect();

    Ok(FlutterReleaseManifest {
        base_url: raw.base_url,
        current_stable_hash,
        releases,
    })
}

// ── install_flutter ───────────────────────────────────────────────────────────

/// Check whether `git` is resolvable on `PATH`.
fn git_is_available() -> bool {
    which::which("git").is_ok()
}

/// Compute the full archive download URL from manifest fields.
///
/// `base_url` from the manifest typically ends without a `/`; `archive` is a
/// relative path like `stable/linux/flutter_linux_3.24.0-stable.tar.xz`.
pub fn archive_download_url(base_url: &str, archive: &str) -> String {
    format!("{base_url}/{archive}")
}

/// Best-effort: read `<sdk_root>/version` to determine the installed version.
///
/// Falls back to reading the `VERSION` file (older SDK layout), then to the
/// supplied `fallback` string (e.g. the channel name).
fn read_installed_version(sdk_root: &Path, fallback: &str) -> String {
    // Try the `version` file (lowercase, Flutter 3.x+).
    if let Ok(v) = std::fs::read_to_string(sdk_root.join("version")) {
        let v = v.trim().to_owned();
        if !v.is_empty() {
            return v;
        }
    }
    // Try the older `VERSION` file.
    if let Ok(v) = std::fs::read_to_string(sdk_root.join("VERSION")) {
        let v = v.trim().to_owned();
        if !v.is_empty() {
            return v;
        }
    }
    fallback.to_owned()
}

/// Install a managed Flutter SDK.
///
/// ## Channel Validation
///
/// The `target.channel` field is validated before any I/O: only `[A-Za-z0-9._-]`
/// characters are permitted and a leading `-` is rejected to prevent git
/// argument injection.
///
/// ## Concurrent-Install Guard
///
/// An advisory lockfile (`.fdemon-install.lock` in `target.install_root`) is
/// held for the duration of the install. If the file already exists a clear
/// error is returned. The lock is released by a RAII `LockGuard` on both
/// success and error paths (including panics).
///
/// ## Install Method Selection
///
/// - Uses `git clone` when `git` is available on `PATH` and
///   `target.method != InstallMethod::Archive`.
/// - Falls back to the archive path when `git` is absent or `method ==
///   InstallMethod::Archive`.
///
/// ## Atomic Install
///
/// All work is performed inside a sibling temp directory
/// `.fdemon-install-tmp-<pid>` under `target.install_root`. On success the
/// temp dir is atomically renamed to `final_dir`. A pre-existing `final_dir`
/// that lacks `bin/flutter` (incomplete prior install) is removed before the
/// rename so it does not block. On any failure the temp dir is removed and the
/// error is propagated.
///
/// ## Precache
///
/// `flutter precache` is run after install. A failure is **non-fatal**: it is
/// logged via `on_event(InstallEvent::Log(...))` but the function still returns
/// `Ok(outcome)`. Rationale: the SDK is fully usable for most workflows without
/// precache; the user can run `flutter precache` manually or the caller can
/// retry.
///
/// ## Cancellation
///
/// Pass a [`CancellationToken`] to support user-initiated cancellation.  The
/// token is checked at attempt boundaries and forwarded to [`download_to_file`]
/// for fine-grained per-chunk cancellation.  A pre-cancelled token causes an
/// immediate return of [`fdemon_core::Error::Cancelled`] before any I/O.
///
/// For a non-cancellable install, pass `CancellationToken::new()`.
///
/// # Errors
///
/// Returns an error when:
/// - The token is already cancelled.
/// - The channel name fails validation.
/// - The lockfile cannot be acquired.
/// - The temp directory cannot be created.
/// - The git clone fails (git path) or the download/verify/extract fails
///   (archive path).
/// - The atomic rename fails.
pub async fn install_flutter<F>(
    target: &FlutterInstallTarget,
    cancel: CancellationToken,
    mut on_event: F,
) -> Result<FlutterInstallOutcome>
where
    F: FnMut(InstallEvent) + Send,
{
    // Pre-cancel check: if the token is already cancelled, return immediately.
    if cancel.is_cancelled() {
        return Err(Error::cancelled("Flutter install cancelled before start"));
    }
    // ── Channel validation (M2: prevent git argument injection) ──────────────
    validate_channel(&target.channel)?;

    let final_dir = target.install_root.join(&target.version_dir_name);

    // ── Short-circuit: already installed ────────────────────────────────────
    #[cfg(not(windows))]
    let flutter_bin = final_dir.join("bin").join("flutter");
    #[cfg(windows)]
    let flutter_bin = final_dir.join("bin").join("flutter.bat");

    if final_dir.exists() && flutter_bin.exists() {
        tracing::info!("Flutter SDK already installed at {}", final_dir.display());
        on_event(InstallEvent::Log(format!(
            "Flutter SDK already installed at {}",
            final_dir.display()
        )));
        let version = read_installed_version(&final_dir, &target.channel);
        let method = target.method;
        return Ok(FlutterInstallOutcome {
            sdk_path: final_dir,
            version,
            method,
        });
    }

    // ── Concurrent-install guard (M9) ────────────────────────────────────────
    let _lock = LockGuard::acquire(&target.install_root)?;

    // ── Preflight: reclaim all stale temp dirs (any PID) under the lock ──────
    // This recovers any leaked `.fdemon-install-tmp-*` trees from a prior run
    // that was aborted via `JoinHandle::abort()` before the RAII guard could
    // fire (e.g. a different PID's temp dir, or a dir that survived a crash).
    reclaim_stale_flutter_tmps(&target.install_root);

    // ── Temp directory ───────────────────────────────────────────────────────
    let pid = std::process::id();
    let tmp_dir_path = target
        .install_root
        .join(format!(".fdemon-install-tmp-{pid}"));

    std::fs::create_dir_all(&tmp_dir_path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("create temp dir {tmp_dir_path:?}: {e}"),
        ))
    })?;

    // Arm the RAII guard. It calls `remove_dir_all` on the temp dir in its
    // `Drop` implementation — this runs even when the outer `JoinHandle` is
    // aborted mid-`await`, ensuring no partially-extracted SDK tree is leaked
    // regardless of how the future exits.  The guard is disarmed by
    // `install_inner` just before the atomic rename so it does not attempt to
    // remove the now-renamed directory.
    let mut tmp_guard = TempDirGuard::new(tmp_dir_path.clone());

    install_inner(
        target,
        &tmp_dir_path,
        &final_dir,
        cancel,
        &mut on_event,
        &mut tmp_guard,
    )
    .await
}

/// Inner install logic, called from [`install_flutter`].
///
/// On success `tmp_dir` has been renamed to `final_dir`. If `final_dir` exists
/// but is an incomplete install (directory exists, `bin/flutter` absent), it is
/// removed before the rename so the install can proceed without an `ENOTEMPTY`
/// error.
///
/// `tmp_guard` is disarmed just before the atomic rename so the RAII guard
/// does not attempt to remove the directory after it has been renamed to
/// `final_dir`.
async fn install_inner<F>(
    target: &FlutterInstallTarget,
    tmp_dir: &Path,
    final_dir: &Path,
    cancel: CancellationToken,
    on_event: &mut F,
    tmp_guard: &mut TempDirGuard,
) -> Result<FlutterInstallOutcome>
where
    F: FnMut(InstallEvent) + Send,
{
    // Determine which install method to use.
    let use_git = target.method != InstallMethod::Archive && git_is_available();

    // The directory that will contain `bin/flutter` after extraction/clone.
    // For git clone: the clone target is `tmp_dir` directly (git creates a subdir).
    // For archive: Flutter archives extract a top-level `flutter/` dir inside `tmp_dir`.
    let sdk_root_in_tmp: PathBuf = if use_git {
        git_install(target, tmp_dir, cancel, on_event).await?
    } else {
        archive_install(target, tmp_dir, cancel, on_event).await?
    };

    // ── Reclaim incomplete final_dir (M5) ────────────────────────────────────
    // If a prior install was interrupted after the directory was created but
    // before it was fully populated (bin/flutter absent), remove it so the
    // rename below succeeds instead of failing with ENOTEMPTY.
    #[cfg(not(windows))]
    let flutter_bin_in_final = final_dir.join("bin").join("flutter");
    #[cfg(windows)]
    let flutter_bin_in_final = final_dir.join("bin").join("flutter.bat");

    if final_dir.exists() && !flutter_bin_in_final.exists() {
        tracing::warn!(
            "Removing incomplete Flutter install at {} (bin/flutter absent) before rename",
            final_dir.display()
        );
        on_event(InstallEvent::Log(format!(
            "Removing incomplete prior Flutter install at {} …",
            final_dir.display()
        )));
        std::fs::remove_dir_all(final_dir).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "remove incomplete Flutter install at {}: {e}",
                    final_dir.display()
                ),
            ))
        })?;
    }

    // ── Atomic rename ────────────────────────────────────────────────────────
    // Disarm the temp-dir guard before the rename so it does not attempt to
    // remove a directory that is being atomically moved to `final_dir`.
    tmp_guard.disarm();

    tracing::debug!(
        "Renaming {} → {}",
        sdk_root_in_tmp.display(),
        final_dir.display()
    );
    std::fs::rename(&sdk_root_in_tmp, final_dir).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!(
                "atomic rename {} → {}: {e}",
                sdk_root_in_tmp.display(),
                final_dir.display()
            ),
        ))
    })?;

    // ── Precache (non-fatal) ─────────────────────────────────────────────────
    run_precache(final_dir, on_event).await;

    // ── Version label ────────────────────────────────────────────────────────
    let version = read_installed_version(final_dir, &target.channel);

    let method = if use_git {
        InstallMethod::GitClone
    } else {
        InstallMethod::Archive
    };

    Ok(FlutterInstallOutcome {
        sdk_path: final_dir.to_owned(),
        version,
        method,
    })
}

/// Install via `git clone -b <channel> --depth 1`.
///
/// The git invocation uses a `--` option terminator before the URL and target
/// directory so they are always treated as positional operands and never
/// interpreted as flags, regardless of their content.
///
/// ## Cancellation
///
/// The `cancel` token is checked before starting the clone and is also wired
/// into a `tokio::select!` around the `run_streaming` await so that a cancel
/// during the clone exits cooperatively with `Error::Cancelled`.  Because
/// `run_streaming` spawns the git process with `kill_on_drop(true)`, dropping
/// the future kills the git child process.
///
/// Returns the path of the SDK root inside `tmp_dir` (i.e. `tmp_dir` itself,
/// since git clones directly into the target directory).
async fn git_install<F>(
    target: &FlutterInstallTarget,
    tmp_dir: &Path,
    cancel: CancellationToken,
    on_event: &mut F,
) -> Result<PathBuf>
where
    F: FnMut(InstallEvent) + Send,
{
    // Pre-cancel check.
    if cancel.is_cancelled() {
        return Err(Error::cancelled("git install cancelled before start"));
    }

    on_event(InstallEvent::Phase("Cloning"));
    on_event(InstallEvent::Log(format!(
        "Cloning Flutter channel '{}' into {}",
        target.channel,
        tmp_dir.display()
    )));

    let channel = &target.channel;
    let tmp_str = tmp_dir.to_string_lossy();

    // The `--` terminator ensures the URL and directory path are always
    // treated as positional arguments and never as git options, regardless
    // of their content. `channel` has already been validated by
    // `validate_channel` before reaching this point.
    let args = &[
        "clone",
        "-b",
        channel.as_str(),
        "--depth",
        "1",
        "--",
        "https://github.com/flutter/flutter.git",
        tmp_str.as_ref(),
    ];

    // Wrap run_streaming in a select! so the token cancels the clone
    // cooperatively.  `kill_on_drop(true)` in run_streaming ensures the git
    // child is killed when the future is dropped on the cancel branch.
    let status = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            return Err(Error::cancelled("git clone cancelled"));
        }
        result = run_streaming("git", args, None, |line| {
            on_event(InstallEvent::Log(line));
        }) => result?,
    };

    if !status.success() {
        return Err(Error::process(format!(
            "git clone failed with exit code {:?}",
            status.code()
        )));
    }

    Ok(tmp_dir.to_owned())
}

/// Install via archive download → SHA-256 verify → extract.
///
/// The release is resolved for `target.channel` and the detected host
/// architecture. If the manifest does not contain an archive for the requested
/// channel, the function falls back to `stable` and emits a visible warning
/// via [`InstallEvent::Log`].
///
/// ## SHA-256 note
///
/// The hash and archive both originate from the same HTTPS Google CDN server.
/// The digest therefore guards against **corruption** (bit-rot, truncated
/// download) rather than a CDN-level MITM — the CA chain provides transport
/// integrity.
///
/// Returns the path of the SDK root inside `tmp_dir` (the `flutter/` subdir
/// that Flutter archives extract to).
async fn archive_install<F>(
    target: &FlutterInstallTarget,
    tmp_dir: &Path,
    cancel: CancellationToken,
    on_event: &mut F,
) -> Result<PathBuf>
where
    F: FnMut(InstallEvent) + Send,
{
    let platform = HostPlatform::detect();

    // Capture arch once and reuse (m4: avoid calling detect() twice).
    let arch = HostArch::detect();

    let manifest = fetch_release_manifest(platform.clone()).await?;

    // M4: resolve the configured channel first; fall back to stable with warning.
    let release = if let Some(r) = resolve_channel_release(&manifest, &target.channel, arch) {
        r
    } else {
        // The configured channel is not available as an archive for this arch.
        let warning = format!(
            "channel '{}' unavailable as archive for arch {:?}; installing stable instead",
            target.channel, arch
        );
        tracing::warn!("{warning}");
        on_event(InstallEvent::Log(format!("[warning] {warning}")));

        manifest.resolve_stable(arch).ok_or_else(|| {
            Error::process(format!(
                "no stable Flutter release found for arch {:?} in manifest \
                     (configured channel '{}' was also unavailable)",
                arch, target.channel
            ))
        })?
    };

    let archive_url = archive_download_url(&manifest.base_url, &release.archive);
    let ext = archive_extension(&platform);
    let archive_path = tmp_dir.join(format!("archive{ext}"));

    // Download
    on_event(InstallEvent::Phase("Downloading"));
    on_event(InstallEvent::Log(format!("Downloading {archive_url}")));

    download_to_file(&archive_url, &archive_path, cancel, |p| {
        on_event(InstallEvent::Download(p));
    })
    .await?;

    // Verify SHA-256 (run blocking I/O in a thread pool).
    // Note: hash and archive originate from the same HTTPS server; the digest
    // guards against corruption, not a CDN-level MITM.
    on_event(InstallEvent::Phase("Verifying"));
    on_event(InstallEvent::Log("Verifying SHA-256 checksum …".to_owned()));

    let expected_sha = release.sha256.clone();
    let archive_path_clone = archive_path.clone();

    tokio::task::spawn_blocking(move || verify_sha256(&archive_path_clone, &expected_sha))
        .await
        .map_err(|e| Error::process(format!("spawn_blocking for verify_sha256 panicked: {e}")))??;

    // Extract (run blocking I/O in a thread pool).
    on_event(InstallEvent::Phase("Extracting"));
    on_event(InstallEvent::Log(format!(
        "Extracting archive into {}",
        tmp_dir.display()
    )));

    // Note: disk space was already checked before the download in
    // `download_to_file` (which calls `ensure_disk_space` with
    // `ARCHIVE_DISK_BUDGET_BYTES`). Repeating the check here on the same
    // filesystem after the ~300 MiB archive is already written would
    // effectively demand 1.5 GiB *plus* the archive size — a false-negative
    // refusal on a tight disk (F15). The pre-download check already budgets
    // both the compressed archive and the extracted tree.

    let tmp_dir_clone = tmp_dir.to_owned();
    let archive_path_clone2 = archive_path.clone();

    tokio::task::spawn_blocking(move || extract_archive(&archive_path_clone2, &tmp_dir_clone))
        .await
        .map_err(|e| {
            Error::process(format!("spawn_blocking for extract_archive panicked: {e}"))
        })??;

    // Remove the downloaded archive file to save space.
    if let Err(e) = std::fs::remove_file(&archive_path) {
        tracing::debug!("Could not remove archive file {archive_path:?}: {e}");
    }

    // Flutter archives extract a top-level `flutter/` directory.
    // The SDK root is therefore `tmp_dir/flutter`.
    let sdk_root = tmp_dir.join("flutter");
    if !sdk_root.exists() {
        return Err(Error::process(format!(
            "expected Flutter SDK root at {} after extraction, but it does not exist",
            sdk_root.display()
        )));
    }

    Ok(sdk_root)
}

/// Run `flutter precache` from the installed SDK, logging output through
/// `on_event`. Failures are **non-fatal**: this function always returns `()`.
async fn run_precache<F>(sdk_root: &Path, on_event: &mut F)
where
    F: FnMut(InstallEvent) + Send,
{
    on_event(InstallEvent::Phase("Precaching"));
    on_event(InstallEvent::Log("Running flutter precache …".to_owned()));

    #[cfg(not(windows))]
    let flutter_bin = sdk_root.join("bin").join("flutter");
    #[cfg(windows)]
    let flutter_bin = sdk_root.join("bin").join("flutter.bat");

    let flutter_str = flutter_bin.to_string_lossy().into_owned();

    let result = run_streaming(&flutter_str, &["precache"], Some(sdk_root), |line| {
        on_event(InstallEvent::Log(line));
    })
    .await;

    match result {
        Ok(status) if !status.success() => {
            // Non-fatal: log the warning and continue.
            let msg = format!(
                "flutter precache exited with code {:?}; the SDK is still usable but \
                 some prebuilt artifacts may be missing. Run 'flutter precache' manually.",
                status.code()
            );
            tracing::warn!("{msg}");
            on_event(InstallEvent::Log(format!("[warning] {msg}")));
        }
        Err(e) => {
            // Non-fatal: may fail when flutter binary is not yet executable, etc.
            let msg = format!(
                "flutter precache failed: {e}; the SDK is still usable. \
                 Run 'flutter precache' manually."
            );
            tracing::warn!("{msg}");
            on_event(InstallEvent::Log(format!("[warning] {msg}")));
        }
        Ok(_) => {
            on_event(InstallEvent::Log("flutter precache completed.".to_owned()));
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── validate_channel ──────────────────────────────────────────────────────

    #[test]
    fn test_validate_channel_accepts_stable() {
        assert!(validate_channel("stable").is_ok());
    }

    #[test]
    fn test_validate_channel_accepts_beta() {
        assert!(validate_channel("beta").is_ok());
    }

    #[test]
    fn test_validate_channel_accepts_version_like() {
        assert!(validate_channel("3.24.0").is_ok());
    }

    #[test]
    fn test_validate_channel_accepts_underscore_and_dash() {
        assert!(validate_channel("my_channel-1").is_ok());
    }

    #[test]
    fn test_validate_channel_rejects_empty() {
        let err = validate_channel("").unwrap_err();
        assert!(err.to_string().contains("empty"), "error: {err}");
    }

    #[test]
    fn test_validate_channel_rejects_leading_dash() {
        let err = validate_channel("--upload-pack=evil").unwrap_err();
        assert!(err.to_string().contains("starts with '-'"), "error: {err}");
    }

    #[test]
    fn test_validate_channel_rejects_leading_dash_single() {
        let err = validate_channel("-b").unwrap_err();
        assert!(err.to_string().contains("starts with '-'"), "error: {err}");
    }

    #[test]
    fn test_validate_channel_rejects_space() {
        let err = validate_channel("stable channel").unwrap_err();
        assert!(
            err.to_string().contains("invalid characters"),
            "error: {err}"
        );
    }

    #[test]
    fn test_validate_channel_rejects_equals() {
        let err = validate_channel("--config=x").unwrap_err();
        // Starts with '-' check fires first.
        assert!(err.to_string().contains("starts with '-'"), "error: {err}");
    }

    #[test]
    fn test_validate_channel_rejects_slash() {
        let err = validate_channel("stable/linux").unwrap_err();
        assert!(
            err.to_string().contains("invalid characters"),
            "error: {err}"
        );
    }

    #[test]
    fn test_validate_channel_rejects_null_byte() {
        let err = validate_channel("stable\0evil").unwrap_err();
        assert!(
            err.to_string().contains("invalid characters"),
            "error: {err}"
        );
    }

    // ── LockGuard ─────────────────────────────────────────────────────────────

    #[test]
    fn test_lock_guard_creates_and_releases_lockfile() {
        let tmp = TempDir::new().unwrap();
        let lock_path = tmp.path().join(".fdemon-install.lock");

        {
            let _guard = LockGuard::acquire(tmp.path()).expect("first acquire must succeed");
            assert!(
                lock_path.exists(),
                "lockfile must exist while guard is held"
            );
        }

        assert!(
            !lock_path.exists(),
            "lockfile must be removed after guard drops"
        );
    }

    #[test]
    fn test_lock_guard_second_acquire_fails() {
        let tmp = TempDir::new().unwrap();

        let _guard = LockGuard::acquire(tmp.path()).expect("first acquire must succeed");

        let err = LockGuard::acquire(tmp.path()).unwrap_err();
        assert!(
            err.to_string().contains("another install is in progress"),
            "error: {err}"
        );
    }

    #[test]
    fn test_lock_guard_released_after_first_guard_drops() {
        let tmp = TempDir::new().unwrap();

        {
            let _guard = LockGuard::acquire(tmp.path()).expect("first acquire must succeed");
        }

        // After the first guard drops, acquiring again must succeed.
        let _guard2 =
            LockGuard::acquire(tmp.path()).expect("second acquire must succeed after drop");
    }

    // ── resolve_channel_release ───────────────────────────────────────────────

    #[test]
    fn test_resolve_channel_release_picks_matching_channel_and_arch() {
        let manifest = FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![
                FlutterRelease {
                    version: "3.24.0".to_string(),
                    channel: "stable".to_string(),
                    archive: "stable/linux/flutter_linux_3.24.0-stable.tar.xz".to_string(),
                    sha256: "aaaa".to_string(),
                    dart_sdk_arch: Some("x64".to_string()),
                },
                FlutterRelease {
                    version: "3.25.0-0.1.pre".to_string(),
                    channel: "beta".to_string(),
                    archive: "beta/linux/flutter_linux_3.25.0-beta.tar.xz".to_string(),
                    sha256: "bbbb".to_string(),
                    dart_sdk_arch: Some("x64".to_string()),
                },
            ],
        };

        let r =
            resolve_channel_release(&manifest, "beta", HostArch::X64).expect("beta must resolve");
        assert_eq!(r.channel, "beta");
        assert_eq!(r.sha256, "bbbb");
    }

    #[test]
    fn test_resolve_channel_release_returns_none_for_missing_channel() {
        let manifest = FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![FlutterRelease {
                version: "3.24.0".to_string(),
                channel: "stable".to_string(),
                archive: "stable/linux/flutter_linux_3.24.0-stable.tar.xz".to_string(),
                sha256: "aaaa".to_string(),
                dart_sdk_arch: Some("x64".to_string()),
            }],
        };

        // "dev" channel does not exist in this manifest.
        assert!(resolve_channel_release(&manifest, "dev", HostArch::X64).is_none());
    }

    #[test]
    fn test_resolve_channel_release_falls_back_to_any_arch_within_channel() {
        // A channel with only an untagged-arch entry should still be resolved.
        let manifest = FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![FlutterRelease {
                version: "3.25.0".to_string(),
                channel: "beta".to_string(),
                archive: "beta/linux/flutter_linux_3.25.0-beta.tar.xz".to_string(),
                sha256: "cccc".to_string(),
                dart_sdk_arch: None, // no arch field
            }],
        };

        let r = resolve_channel_release(&manifest, "beta", HostArch::X64)
            .expect("should fall back to any beta entry");
        assert_eq!(r.sha256, "cccc");
    }

    // ── resolve_install_dir ───────────────────────────────────────────────────

    #[test]
    fn test_resolve_install_dir_explicit_override() {
        let tmp = TempDir::new().unwrap();
        let override_path = tmp.path().join("custom_root");

        let result = resolve_install_dir(Some(&override_path)).expect("should succeed");
        assert_eq!(result, override_path);
        assert!(override_path.is_dir(), "override path must be created");
    }

    #[test]
    fn test_resolve_install_dir_explicit_override_creates_nested_dirs() {
        let tmp = TempDir::new().unwrap();
        let override_path = tmp.path().join("a").join("b").join("c");

        let result = resolve_install_dir(Some(&override_path)).expect("should succeed");
        assert_eq!(result, override_path);
        assert!(override_path.is_dir(), "nested dirs must be created");
    }

    #[test]
    #[serial_test::serial]
    fn test_resolve_install_dir_fvm_cache_path_env() {
        // Temporarily override HOME so the fallback path is predictable and
        // independent of the real home directory.
        let tmp = TempDir::new().unwrap();
        let fvm_path = tmp.path().join("my_fvm_cache");
        std::fs::create_dir_all(&fvm_path).unwrap();

        // Use serial_test or just set the env var for this test.
        // We restore it afterwards to avoid leaking state.
        let orig = std::env::var("FVM_CACHE_PATH").ok();
        std::env::set_var("FVM_CACHE_PATH", fvm_path.as_os_str());

        let result = resolve_install_dir(None).expect("should succeed");
        assert_eq!(result, fvm_path);

        // Restore env var.
        match orig {
            Some(v) => std::env::set_var("FVM_CACHE_PATH", v),
            None => std::env::remove_var("FVM_CACHE_PATH"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_resolve_install_dir_ignores_relative_fvm_cache_path() {
        // A relative FVM_CACHE_PATH must be ignored with a warning and fall
        // through to the default ~/fvm/versions path.
        let orig = std::env::var("FVM_CACHE_PATH").ok();
        std::env::set_var("FVM_CACHE_PATH", "relative/path");

        // The function should not use the relative path — it must fall through
        // to the home-based default. We can't assert the exact value on all
        // platforms, but we can assert it does NOT start with "relative/".
        if dirs::home_dir().is_some() {
            let result = resolve_install_dir(None);
            if let Ok(path) = result {
                let path_str = path.to_string_lossy();
                assert!(
                    !path_str.starts_with("relative"),
                    "resolve_install_dir must not use a relative FVM_CACHE_PATH: {path_str}"
                );
                assert!(
                    path_str.contains("fvm"),
                    "default install dir should contain 'fvm': {path_str}"
                );
            }
        }

        match orig {
            Some(v) => std::env::set_var("FVM_CACHE_PATH", v),
            None => std::env::remove_var("FVM_CACHE_PATH"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_resolve_install_dir_default_creates_under_home() {
        // Remove FVM_CACHE_PATH so the default path is used.
        let orig_fvm = std::env::var("FVM_CACHE_PATH").ok();
        std::env::remove_var("FVM_CACHE_PATH");

        // If the real home directory is available, just call it and check it
        // returns *something* ending in fvm/versions without creating the dir
        // unconditionally (it may already exist).
        if dirs::home_dir().is_some() {
            let result = resolve_install_dir(None);
            // May fail if HOME is not writable; accept either outcome.
            if let Ok(path) = result {
                let path_str = path.to_string_lossy();
                assert!(
                    path_str.contains("fvm"),
                    "default install dir should contain 'fvm': {path_str}"
                );
                assert!(path.is_dir());
            }
        }

        // Restore.
        if let Some(v) = orig_fvm {
            std::env::set_var("FVM_CACHE_PATH", v);
        }
    }

    // ── archive_download_url ──────────────────────────────────────────────────

    #[test]
    fn test_archive_url_construction() {
        let base = "https://storage.googleapis.com/flutter_infra_release/releases";
        let archive = "stable/linux/flutter_linux_3.24.0-stable.tar.xz";
        let expected = format!("{base}/{archive}");

        assert_eq!(archive_download_url(base, archive), expected);
    }

    #[test]
    fn test_archive_url_construction_windows() {
        let base = "https://storage.googleapis.com/flutter_infra_release/releases";
        let archive = "stable/windows/flutter_windows_3.24.0-stable.zip";
        let expected = format!("{base}/{archive}");

        assert_eq!(archive_download_url(base, archive), expected);
    }

    #[test]
    fn test_archive_url_construction_macos() {
        let base = "https://storage.googleapis.com/flutter_infra_release/releases";
        let archive = "stable/macos/flutter_macos_arm64_3.24.0-stable.zip";
        let expected = format!("{base}/{archive}");

        assert_eq!(archive_download_url(base, archive), expected);
    }

    // ── manifest_url ─────────────────────────────────────────────────────────

    #[test]
    fn test_manifest_url_linux() {
        let url = manifest_url(&HostPlatform::Linux);
        assert!(
            url.contains("releases_linux.json"),
            "Linux manifest URL should contain releases_linux.json: {url}"
        );
    }

    #[test]
    fn test_manifest_url_macos() {
        let url = manifest_url(&HostPlatform::MacOs);
        assert!(
            url.contains("releases_macos.json"),
            "macOS manifest URL should contain releases_macos.json: {url}"
        );
    }

    #[test]
    fn test_manifest_url_windows() {
        let url = manifest_url(&HostPlatform::Windows);
        assert!(
            url.contains("releases_windows.json"),
            "Windows manifest URL should contain releases_windows.json: {url}"
        );
    }

    // ── fetch_release_manifest (wiremock) ────────────────────────────────────

    /// A minimal fixture that matches the real `releases_linux.json` shape.
    const MANIFEST_FIXTURE: &str = r#"{
        "base_url": "https://storage.googleapis.com/flutter_infra_release/releases",
        "current_release": {
            "beta": "abc123",
            "dev": "def456",
            "stable": "aabbccdd"
        },
        "releases": [
            {
                "hash": "aabbccdd",
                "channel": "stable",
                "version": "3.24.0",
                "dart_sdk_version": "3.5.0",
                "dart_sdk_arch": "x64",
                "release_date": "2024-08-21T17:10:03.737Z",
                "archive": "stable/linux/flutter_linux_3.24.0-stable.tar.xz",
                "sha256": "deadbeefdeadbeef"
            },
            {
                "hash": "11223344",
                "channel": "stable",
                "version": "3.24.0",
                "dart_sdk_version": "3.5.0",
                "dart_sdk_arch": "arm64",
                "release_date": "2024-08-21T17:10:03.737Z",
                "archive": "stable/linux/flutter_linux_arm64_3.24.0-stable.tar.xz",
                "sha256": "cafebabecafebabe"
            },
            {
                "hash": "aaaa1111",
                "channel": "beta",
                "version": "3.25.0-0.1.pre",
                "dart_sdk_version": "3.6.0",
                "dart_sdk_arch": "x64",
                "release_date": "2024-09-01T12:00:00.000Z",
                "archive": "beta/linux/flutter_linux_3.25.0-0.1.pre-beta.tar.xz",
                "sha256": "beefcafe"
            }
        ]
    }"#;

    #[test]
    fn test_fetch_manifest_parses_fixture() {
        // Parse the fixture using serde (bypassing the HTTP call).
        let raw: RawManifest =
            serde_json::from_str(MANIFEST_FIXTURE).expect("fixture must parse without error");

        assert_eq!(
            raw.base_url,
            "https://storage.googleapis.com/flutter_infra_release/releases"
        );
        assert_eq!(raw.releases.len(), 3);

        let stable_hash = raw
            .current_release
            .as_ref()
            .and_then(|cr| cr.stable.as_deref());
        assert_eq!(stable_hash, Some("aabbccdd"));

        // Convert to the public type and test resolve_stable.
        let current_stable_hash = raw.current_release.and_then(|cr| cr.stable);
        let releases: Vec<FlutterRelease> = raw
            .releases
            .into_iter()
            .map(|r| FlutterRelease {
                version: r.version,
                channel: r.channel,
                archive: r.archive,
                sha256: r.sha256,
                dart_sdk_arch: r.dart_sdk_arch,
            })
            .collect();

        let manifest = FlutterReleaseManifest {
            base_url: "https://storage.googleapis.com/flutter_infra_release/releases".to_owned(),
            current_stable_hash,
            releases,
        };

        // resolve_stable(X64) should pick the x64 entry.
        let x64 = manifest
            .resolve_stable(HostArch::X64)
            .expect("x64 must resolve");
        assert_eq!(x64.dart_sdk_arch.as_deref(), Some("x64"));
        assert_eq!(x64.sha256, "deadbeefdeadbeef");
        assert_eq!(
            x64.archive,
            "stable/linux/flutter_linux_3.24.0-stable.tar.xz"
        );

        // resolve_stable(Arm64) should pick the arm64 entry.
        let arm64 = manifest
            .resolve_stable(HostArch::Arm64)
            .expect("arm64 must resolve");
        assert_eq!(arm64.dart_sdk_arch.as_deref(), Some("arm64"));
        assert_eq!(arm64.sha256, "cafebabecafebabe");
    }

    /// Test that resolve_channel_release correctly resolves beta from the fixture.
    #[test]
    fn test_resolve_channel_release_from_manifest_fixture() {
        let raw: RawManifest = serde_json::from_str(MANIFEST_FIXTURE).expect("fixture must parse");
        let releases: Vec<FlutterRelease> = raw
            .releases
            .into_iter()
            .map(|r| FlutterRelease {
                version: r.version,
                channel: r.channel,
                archive: r.archive,
                sha256: r.sha256,
                dart_sdk_arch: r.dart_sdk_arch,
            })
            .collect();
        let manifest = FlutterReleaseManifest {
            base_url: "https://example.com".to_owned(),
            current_stable_hash: None,
            releases,
        };

        // "beta" with x64 should resolve to the beta entry.
        let beta = resolve_channel_release(&manifest, "beta", HostArch::X64)
            .expect("beta must resolve from fixture");
        assert_eq!(beta.channel, "beta");
        assert_eq!(beta.sha256, "beefcafe");

        // "dev" does not exist in the fixture — must return None.
        assert!(
            resolve_channel_release(&manifest, "dev", HostArch::X64).is_none(),
            "dev channel must not resolve from fixture"
        );
    }

    #[tokio::test]
    async fn test_fetch_release_manifest_with_mock_server() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // We cannot easily redirect the hard-coded manifest URL, so we test
        // the JSON parsing logic separately (see test_fetch_manifest_parses_fixture)
        // and here we just verify the wiremock integration works for a mock URL.
        //
        // This test exercises the HTTP parsing path using a local server by
        // directly calling the HTTP client with the same logic used internally.
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/releases_linux.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(MANIFEST_FIXTURE))
            .mount(&mock_server)
            .await;

        let url = format!("{}/releases_linux.json", mock_server.uri());

        let client = reqwest::Client::builder()
            .user_agent("fdemon-test")
            .connect_timeout(std::time::Duration::from_secs(
                MANIFEST_CONNECT_TIMEOUT_SECS,
            ))
            .timeout(std::time::Duration::from_secs(
                MANIFEST_REQUEST_TIMEOUT_SECS,
            ))
            .build()
            .unwrap();

        let raw: RawManifest = client
            .get(&url)
            .send()
            .await
            .expect("request must succeed")
            .json()
            .await
            .expect("JSON parse must succeed");

        assert_eq!(raw.releases.len(), 3);
        assert_eq!(
            raw.base_url,
            "https://storage.googleapis.com/flutter_infra_release/releases"
        );

        let stable_hash = raw.current_release.and_then(|cr| cr.stable);
        assert_eq!(stable_hash.as_deref(), Some("aabbccdd"));
    }

    // ── already-installed short-circuit ──────────────────────────────────────

    #[tokio::test]
    async fn test_install_flutter_short_circuits_when_already_installed() {
        let tmp = TempDir::new().unwrap();

        // Create a fake SDK at the expected location.
        let sdk_dir = tmp.path().join("stable");
        std::fs::create_dir_all(sdk_dir.join("bin")).unwrap();

        #[cfg(not(windows))]
        std::fs::write(sdk_dir.join("bin").join("flutter"), "#!/bin/sh\n").unwrap();
        #[cfg(windows)]
        std::fs::write(sdk_dir.join("bin").join("flutter.bat"), "@echo off\n").unwrap();

        std::fs::write(sdk_dir.join("version"), "3.24.0\n").unwrap();

        let target = FlutterInstallTarget {
            method: InstallMethod::GitClone,
            channel: "stable".to_owned(),
            install_root: tmp.path().to_owned(),
            version_dir_name: "stable".to_owned(),
        };

        let mut events: Vec<InstallEvent> = Vec::new();
        let outcome = install_flutter(&target, CancellationToken::new(), |e| events.push(e))
            .await
            .expect("short-circuit must succeed");

        assert_eq!(outcome.sdk_path, sdk_dir);
        assert_eq!(outcome.version, "3.24.0");
        assert!(
            !events.is_empty(),
            "should emit at least one event for already-installed"
        );
    }

    // ── install_flutter rejects invalid channel ───────────────────────────────

    #[tokio::test]
    async fn test_install_flutter_rejects_invalid_channel() {
        let tmp = TempDir::new().unwrap();

        let target = FlutterInstallTarget {
            method: InstallMethod::Archive,
            channel: "--upload-pack=evil".to_owned(),
            install_root: tmp.path().to_owned(),
            version_dir_name: "bad".to_owned(),
        };

        let err = install_flutter(&target, CancellationToken::new(), |_| {})
            .await
            .expect_err("must fail on invalid channel");
        assert!(err.to_string().contains("starts with '-'"), "error: {err}");
    }

    // ── partial final_dir reclamation ─────────────────────────────────────────

    #[test]
    fn test_partial_final_dir_detected() {
        // Simulate install_inner's partial-dir detection logic directly, since
        // invoking install_inner requires spawning a real git/archive process.
        let tmp = TempDir::new().unwrap();
        let final_dir = tmp.path().join("stable");

        // Create the directory but NOT bin/flutter → "incomplete install".
        std::fs::create_dir_all(&final_dir).unwrap();

        #[cfg(not(windows))]
        let flutter_bin_in_final = final_dir.join("bin").join("flutter");
        #[cfg(windows)]
        let flutter_bin_in_final = final_dir.join("bin").join("flutter.bat");

        let is_incomplete = final_dir.exists() && !flutter_bin_in_final.exists();
        assert!(
            is_incomplete,
            "should detect directory without bin/flutter as incomplete"
        );

        // Simulate reclamation.
        std::fs::remove_dir_all(&final_dir).unwrap();
        assert!(!final_dir.exists(), "partial dir must be removed");
    }

    #[test]
    fn test_complete_final_dir_not_flagged_as_incomplete() {
        let tmp = TempDir::new().unwrap();
        let final_dir = tmp.path().join("stable");
        std::fs::create_dir_all(final_dir.join("bin")).unwrap();

        #[cfg(not(windows))]
        std::fs::write(final_dir.join("bin").join("flutter"), "#!/bin/sh\n").unwrap();
        #[cfg(windows)]
        std::fs::write(final_dir.join("bin").join("flutter.bat"), "@echo off\n").unwrap();

        #[cfg(not(windows))]
        let flutter_bin_in_final = final_dir.join("bin").join("flutter");
        #[cfg(windows)]
        let flutter_bin_in_final = final_dir.join("bin").join("flutter.bat");

        let is_incomplete = final_dir.exists() && !flutter_bin_in_final.exists();
        assert!(
            !is_incomplete,
            "directory with bin/flutter must NOT be flagged as incomplete"
        );
    }

    // ── read_installed_version ────────────────────────────────────────────────

    #[test]
    fn test_read_installed_version_prefers_lowercase_version_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("version"), "3.24.0\n").unwrap();
        std::fs::write(tmp.path().join("VERSION"), "old-version").unwrap();

        // On case-insensitive filesystems (default on macOS and Windows) the two
        // writes resolve to the same file, so the second clobbers the first and
        // lowercase-vs-uppercase precedence cannot be exercised. Skip there — the
        // preference is only observable when both files coexist (case-sensitive
        // filesystems, e.g. Linux CI).
        let case_sensitive_fs = std::fs::read_to_string(tmp.path().join("version"))
            .unwrap()
            .trim()
            == "3.24.0";
        if !case_sensitive_fs {
            return;
        }

        let v = read_installed_version(tmp.path(), "fallback");
        assert_eq!(v, "3.24.0");
    }

    #[test]
    fn test_read_installed_version_falls_back_to_version_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("VERSION"), "3.22.0\n").unwrap();

        let v = read_installed_version(tmp.path(), "fallback");
        assert_eq!(v, "3.22.0");
    }

    #[test]
    fn test_read_installed_version_uses_fallback_when_no_files() {
        let tmp = TempDir::new().unwrap();
        let v = read_installed_version(tmp.path(), "stable");
        assert_eq!(v, "stable");
    }

    // ── platform helpers ──────────────────────────────────────────────────────

    #[test]
    fn test_platform_os_label() {
        assert_eq!(platform_os_label(&HostPlatform::Linux), "linux");
        assert_eq!(platform_os_label(&HostPlatform::MacOs), "macos");
        assert_eq!(platform_os_label(&HostPlatform::Windows), "windows");
        assert_eq!(platform_os_label(&HostPlatform::Unknown), "linux");
    }

    #[test]
    fn test_archive_extension_linux() {
        assert_eq!(archive_extension(&HostPlatform::Linux), ".tar.xz");
    }

    #[test]
    fn test_archive_extension_macos() {
        assert_eq!(archive_extension(&HostPlatform::MacOs), ".zip");
    }

    #[test]
    fn test_archive_extension_windows() {
        assert_eq!(archive_extension(&HostPlatform::Windows), ".zip");
    }

    // ── InstallEvent is Debug + Clone ─────────────────────────────────────────

    #[test]
    fn test_install_event_debug_clone() {
        let e1 = InstallEvent::Log("hello".to_owned());
        let e2 = e1.clone();
        let _dbg = format!("{e2:?}");

        let e3 = InstallEvent::Phase("Cloning");
        let _e4 = e3.clone();

        let e5 = InstallEvent::Download(DownloadProgress {
            received: 100,
            total: Some(1000),
        });
        let _e6 = e5.clone();
    }

    // ── fetch_release_manifest_from error and happy paths (wiremock) ────────────

    /// A 404 response from the manifest endpoint must produce a clear
    /// `Error::Process` whose message mentions the HTTP status code.
    ///
    /// Calls the real `fetch_release_manifest_from` against a wiremock server so
    /// the production HEAD→GET→parse sequencing (and `is_success()` branch) is
    /// exercised end-to-end.  Changing the error string in production would fail
    /// this test.
    #[tokio::test]
    async fn fetch_manifest_404_is_clear_error() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // HEAD probe — must succeed so the function proceeds to the GET.
        Mock::given(method("HEAD"))
            .and(path("/releases_linux.json"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // GET returns 404.
        Mock::given(method("GET"))
            .and(path("/releases_linux.json"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let url = format!("{}/releases_linux.json", mock_server.uri());

        let err = fetch_release_manifest_from(&url)
            .await
            .expect_err("HTTP 404 must return an error");

        let msg = err.to_string();
        assert!(
            msg.contains("404"),
            "error message must contain the HTTP status code: {msg}"
        );
        // Confirm the real production error-string prefix is present.
        assert!(
            msg.contains("manifest HTTP"),
            "error message must use the 'manifest HTTP' prefix: {msg}"
        );
    }

    /// A 200 response with malformed JSON must produce a clear `Error::Process`
    /// whose message mentions a parse failure.
    ///
    /// Calls the real `fetch_release_manifest_from` so that the production JSON
    /// parse path is exercised end-to-end.  Changing the error string in
    /// production would fail this test.
    #[tokio::test]
    async fn fetch_manifest_malformed_json_is_clear_error() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // HEAD probe — must succeed.
        Mock::given(method("HEAD"))
            .and(path("/releases_linux.json"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // GET returns 200 with malformed JSON body.
        Mock::given(method("GET"))
            .and(path("/releases_linux.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string("this is not valid { json }!"))
            .mount(&mock_server)
            .await;

        let url = format!("{}/releases_linux.json", mock_server.uri());

        let err = fetch_release_manifest_from(&url)
            .await
            .expect_err("malformed JSON must return an error");

        let msg = err.to_string();
        assert!(
            msg.contains("failed to parse manifest"),
            "error message must use the 'failed to parse manifest' prefix: {msg}"
        );
    }

    /// HEAD→GET→parse happy path: a well-formed manifest returned by the mock
    /// server must be parsed into a `FlutterReleaseManifest` with correctly
    /// mapped fields, proving that both the HEAD probe path and the
    /// `RawManifest → FlutterReleaseManifest` field mapping are exercised.
    #[tokio::test]
    async fn fetch_manifest_from_happy_path_exercises_head_get_parse() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // HEAD probe — must succeed.
        Mock::given(method("HEAD"))
            .and(path("/releases_linux.json"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // GET returns the standard fixture.
        Mock::given(method("GET"))
            .and(path("/releases_linux.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(MANIFEST_FIXTURE))
            .mount(&mock_server)
            .await;

        let url = format!("{}/releases_linux.json", mock_server.uri());

        let manifest = fetch_release_manifest_from(&url)
            .await
            .expect("well-formed manifest must parse without error");

        // Field-mapping assertions: RawManifest → FlutterReleaseManifest.
        assert_eq!(
            manifest.base_url, "https://storage.googleapis.com/flutter_infra_release/releases",
            "base_url must be passed through unchanged"
        );
        assert_eq!(
            manifest.current_stable_hash.as_deref(),
            Some("aabbccdd"),
            "current_stable_hash must be extracted from current_release.stable"
        );
        assert_eq!(
            manifest.releases.len(),
            3,
            "all three fixture releases must be present"
        );

        // Verify one release is correctly mapped.
        let stable_x64 = manifest
            .releases
            .iter()
            .find(|r| r.channel == "stable" && r.dart_sdk_arch.as_deref() == Some("x64"))
            .expect("stable/x64 release must be present");
        assert_eq!(stable_x64.version, "3.24.0");
        assert_eq!(stable_x64.sha256, "deadbeefdeadbeef");
        assert_eq!(
            stable_x64.archive,
            "stable/linux/flutter_linux_3.24.0-stable.tar.xz"
        );
    }

    // ── Cancellation ──────────────────────────────────────────────────────────

    /// A pre-cancelled token must cause `install_flutter` to return
    /// `Error::Cancelled` before any I/O is performed.
    #[tokio::test]
    async fn test_install_flutter_precancelled_returns_cancelled() {
        let tmp = TempDir::new().unwrap();

        let target = FlutterInstallTarget {
            method: InstallMethod::GitClone,
            channel: "stable".to_owned(),
            install_root: tmp.path().to_owned(),
            version_dir_name: "stable".to_owned(),
        };

        let token = CancellationToken::new();
        token.cancel();

        let err = install_flutter(&target, token, |_| {})
            .await
            .expect_err("pre-cancelled install must return Err");

        assert!(err.is_cancelled(), "error must be Cancelled, got: {err:?}");

        // No lockfile or temp dirs should have been created.
        let lock_path = tmp.path().join(".fdemon-install.lock");
        assert!(
            !lock_path.exists(),
            "lockfile must not be created for pre-cancelled install"
        );
    }

    // ── TempDirGuard (F14) ────────────────────────────────────────────────────

    /// An armed `TempDirGuard` removes the directory (and contents) on drop.
    #[test]
    fn temp_dir_guard_removes_dir_on_drop() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("some-install-tmp");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("partial.file"), b"partial").unwrap();
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

    /// A `TempDirGuard` pointing to a non-existent directory must not panic on drop.
    #[test]
    fn temp_dir_guard_missing_dir_no_panic() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("does-not-exist");
        let guard = TempDirGuard::new(dir);
        drop(guard); // Must not panic.
    }

    /// `reclaim_stale_flutter_tmps` removes all `.fdemon-install-tmp-*` dirs
    /// regardless of PID suffix.
    #[test]
    fn reclaim_stale_flutter_tmps_removes_all_tmp_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Plant two stale temp dirs with different PIDs.
        let stale1 = root.join(".fdemon-install-tmp-12345");
        let stale2 = root.join(".fdemon-install-tmp-99999");
        std::fs::create_dir_all(&stale1).unwrap();
        std::fs::write(stale1.join("partial.sdk"), b"").unwrap();
        std::fs::create_dir_all(&stale2).unwrap();

        // A non-tmp dir must be left alone.
        let keep = root.join("stable");
        std::fs::create_dir_all(&keep).unwrap();

        reclaim_stale_flutter_tmps(root);

        assert!(!stale1.exists(), "stale temp dir 1 must be removed");
        assert!(!stale2.exists(), "stale temp dir 2 must be removed");
        assert!(keep.exists(), "non-tmp dir must not be removed");
    }

    // ── git_install cancel (F23) ──────────────────────────────────────────────

    /// `git_install` with a pre-cancelled token must return `Error::Cancelled`
    /// without spawning `git`.
    #[tokio::test]
    async fn git_install_precancelled_returns_cancelled() {
        let tmp = TempDir::new().unwrap();
        let target = FlutterInstallTarget {
            method: InstallMethod::GitClone,
            channel: "stable".to_owned(),
            install_root: tmp.path().to_owned(),
            version_dir_name: "stable".to_owned(),
        };

        let token = CancellationToken::new();
        token.cancel();

        let mut events: Vec<InstallEvent> = Vec::new();
        let err = git_install(&target, tmp.path(), token, &mut |e| events.push(e))
            .await
            .expect_err("pre-cancelled git_install must return Err");

        assert!(err.is_cancelled(), "error must be Cancelled, got: {err:?}");
    }
}

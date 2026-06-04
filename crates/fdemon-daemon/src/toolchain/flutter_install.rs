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

use super::download::{download_to_file, extract_archive, verify_sha256};
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
/// The HTTP client applies a [`MANIFEST_CONNECT_TIMEOUT_SECS`] TCP connect
/// timeout and a [`MANIFEST_REQUEST_TIMEOUT_SECS`] total request timeout.
///
/// # Errors
///
/// Returns an error on network failure, non-2xx HTTP status, or JSON parse
/// error.
pub async fn fetch_release_manifest(platform: HostPlatform) -> Result<FlutterReleaseManifest> {
    let url = manifest_url(&platform);
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

    let response = client
        .get(&url)
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
/// # Errors
///
/// Returns an error when:
/// - The channel name fails validation.
/// - The lockfile cannot be acquired.
/// - The temp directory cannot be created.
/// - The git clone fails (git path) or the download/verify/extract fails
///   (archive path).
/// - The atomic rename fails.
pub async fn install_flutter<F>(
    target: &FlutterInstallTarget,
    mut on_event: F,
) -> Result<FlutterInstallOutcome>
where
    F: FnMut(InstallEvent) + Send,
{
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

    // ── Temp directory ───────────────────────────────────────────────────────
    let pid = std::process::id();
    let tmp_dir = target
        .install_root
        .join(format!(".fdemon-install-tmp-{pid}"));

    // Remove any stale temp dir from a previous interrupted install.
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("remove stale temp dir {tmp_dir:?}: {e}"),
            ))
        })?;
    }

    std::fs::create_dir_all(&tmp_dir).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("create temp dir {tmp_dir:?}: {e}"),
        ))
    })?;

    // Wrap the install body in a closure to ensure temp-dir cleanup on error.
    let result = install_inner(target, &tmp_dir, &final_dir, &mut on_event).await;

    match result {
        Ok(outcome) => Ok(outcome),
        Err(e) => {
            // Best-effort cleanup of the temp dir.
            if let Err(rm_err) = std::fs::remove_dir_all(&tmp_dir) {
                tracing::warn!(
                    "Failed to remove temp dir {} after install error: {}",
                    tmp_dir.display(),
                    rm_err
                );
            }
            Err(e)
        }
    }
}

/// Inner install logic, called from [`install_flutter`].
///
/// On success `tmp_dir` has been renamed to `final_dir`. If `final_dir` exists
/// but is an incomplete install (directory exists, `bin/flutter` absent), it is
/// removed before the rename so the install can proceed without an `ENOTEMPTY`
/// error.
async fn install_inner<F>(
    target: &FlutterInstallTarget,
    tmp_dir: &Path,
    final_dir: &Path,
    on_event: &mut F,
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
        git_install(target, tmp_dir, on_event).await?
    } else {
        archive_install(target, tmp_dir, on_event).await?
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
/// Returns the path of the SDK root inside `tmp_dir` (i.e. `tmp_dir` itself,
/// since git clones directly into the target directory).
async fn git_install<F>(
    target: &FlutterInstallTarget,
    tmp_dir: &Path,
    on_event: &mut F,
) -> Result<PathBuf>
where
    F: FnMut(InstallEvent) + Send,
{
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

    let status = run_streaming("git", args, None, |line| {
        on_event(InstallEvent::Log(line));
    })
    .await?;

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

    download_to_file(&archive_url, &archive_path, |p| {
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
        let outcome = install_flutter(&target, |e| events.push(e))
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

        let err = install_flutter(&target, |_| {})
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
}

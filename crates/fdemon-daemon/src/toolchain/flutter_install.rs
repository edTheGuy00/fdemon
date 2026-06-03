//! # Managed Flutter SDK Installer
//!
//! Implements the high-level Flutter SDK install flow:
//!
//! 1. Resolve the install root (`~/fvm/versions` by default, `$FVM_CACHE_PATH`
//!    if set, or an explicit caller override).
//! 2. Fetch the Flutter releases manifest from the Google CDN and select the
//!    best stable release for the current OS + CPU architecture.
//! 3. Install via `git clone` (default) or archive download+verify+extract
//!    (fallback when `git` is absent or the caller forces `Archive` mode).
//! 4. Atomically rename the temp dir into the final install location.
//! 5. Run `flutter precache` (non-fatal on failure — the SDK is usable; the
//!    caller may retry precache separately).
//!
//! ## Design Notes
//!
//! - **Atomic install**: all work happens inside `.fdemon-install-tmp-<pid>`;
//!   the final rename is atomic on POSIX. On failure the temp dir is removed.
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

use std::path::{Path, PathBuf};

use fdemon_core::{Error, Result};
use serde::Deserialize;

use super::download::{download_to_file, extract_archive, verify_sha256};
use super::process_stream::run_streaming;
use super::types::{
    DownloadProgress, FlutterInstallOutcome, FlutterInstallTarget, FlutterRelease,
    FlutterReleaseManifest, HostArch, HostPlatform, InstallMethod,
};

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

// ── resolve_install_dir ───────────────────────────────────────────────────────

/// Determine the directory under which the Flutter SDK will be installed.
///
/// Resolution order:
/// 1. `explicit_root` — caller-supplied override (e.g. from `.fdemon/config.toml`).
/// 2. `$FVM_CACHE_PATH` — environment variable honoured by fvm.
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
        let path = PathBuf::from(env_path);
        std::fs::create_dir_all(&path).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("create $FVM_CACHE_PATH directory {path:?}: {e}"),
            ))
        })?;
        return Ok(path);
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
/// # Errors
///
/// Returns an error on network failure, non-2xx HTTP status, or JSON parse
/// error.
pub async fn fetch_release_manifest(platform: HostPlatform) -> Result<FlutterReleaseManifest> {
    let url = manifest_url(&platform);
    tracing::debug!("Fetching Flutter releases manifest from {url}");

    let client = reqwest::Client::builder()
        .user_agent(concat!("fdemon/", env!("CARGO_PKG_VERSION")))
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
/// temp dir is atomically renamed to `final_dir`. On any failure the temp dir
/// is removed and the error is propagated; `final_dir` is never left in a
/// partial state.
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
/// On success `tmp_dir` has been renamed to `final_dir`.
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
        archive_install(tmp_dir, on_event).await?
    };

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
    let args = &[
        "clone",
        "-b",
        channel.as_str(),
        "--depth",
        "1",
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
/// Returns the path of the SDK root inside `tmp_dir` (the `flutter/` subdir
/// that Flutter archives extract to).
async fn archive_install<F>(tmp_dir: &Path, on_event: &mut F) -> Result<PathBuf>
where
    F: FnMut(InstallEvent) + Send,
{
    let platform = HostPlatform::detect();
    let manifest = fetch_release_manifest(platform.clone()).await?;

    let release = manifest.resolve_stable(HostArch::detect()).ok_or_else(|| {
        Error::process(format!(
            "no stable Flutter release found for arch {:?} in manifest",
            HostArch::detect()
        ))
    })?;

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

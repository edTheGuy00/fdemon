//! # Download and Archive Extraction Primitives
//!
//! Low-level helpers for the Flutter SDK installer:
//!
//! - [`ensure_disk_space`] — preflight check that a filesystem has sufficient
//!   free bytes before a large download/extract.
//! - [`check_network_connectivity`] — fast HEAD probe to verify network reach
//!   before starting a download, bounding the offline stall to ≤5 s.
//! - [`download_to_file`] — streaming HTTP download with connect/idle timeouts,
//!   bounded retry, `.part`-file staging, and progress reporting.
//!   Takes a [`tokio_util::sync::CancellationToken`] so an in-flight download
//!   can be cancelled cleanly without leaving a `.part` file behind.
//! - [`verify_sha256`] — synchronous SHA-256 checksum verification.
//! - [`extract_zip`] — extract a `.zip` archive, preserving Unix mode bits.
//!   Rejects path-traversal entries (zip-slip).
//! - [`extract_tar_xz`] — extract a `.tar.xz` archive using pure-Rust decoders.
//!   Streams the XZ decode to avoid full-archive RAM usage; rejects traversal
//!   entries.
//! - [`extract_archive`] — dispatch to the correct extractor by file extension.
//!
//! ## Design Notes
//!
//! - `extract_*` and `verify_sha256` are **synchronous** functions. Callers
//!   that need async context (task 03) should wrap them in
//!   `tokio::task::spawn_blocking`.
//! - No UI types; progress is reported via a plain `FnMut(DownloadProgress)`
//!   callback.
//! - All errors use the workspace `Error` / `Result` types.
//! - Both preflight helpers are **best-effort and fail-clear**: a probe API
//!   failure (e.g. `fs4` error on an exotic FS) surfaces a readable error
//!   rather than silently continuing or panicking.
//!
//! ## Cancellation
//!
//! `download_to_file` accepts a `CancellationToken`. When the token is
//! cancelled, the streaming loop exits on the next chunk boundary and returns
//! [`fdemon_core::Error::Cancelled`].  A [`PartFileGuard`] is armed on entry
//! and disarmed only on success; it removes the `.part` file from disk in its
//! `Drop` implementation, ensuring no orphan files remain after cancellation or
//! any other early-exit path (including an `abort()` on the outer `JoinHandle`).
//!
//! ## Streaming XZ Decompression
//!
//! `lzma-rs` 0.3's XZ decoder (`lzma_rs::xz_decompress`) uses a push API
//! (`Write`-based) rather than a pull API (`Read`-based).  To pipe decoded
//! bytes to the `tar` crate (which needs a `Read`) without materialising the
//! full decompressed archive in RAM, we spawn a background thread that writes
//! decoded bytes through an in-process channel, and expose a `Read` adapter on
//! the receiving end.  This keeps peak RAM proportional to the XZ block size
//! (typically a few MiB) rather than the full decompressed SDK (~1 GB).
//!
//! ## XZ Decode Thread Teardown on Cancellation
//!
//! The XZ decode thread in [`extract_tar_xz`] is a `std::thread`, not a Tokio
//! task, so it cannot be cancelled via `JoinHandle::abort()`.  When the
//! `ReceiverReader` is dropped (e.g. because the tar crate returned early due to
//! an error), the `SenderWriter` receives `BrokenPipe` on the next
//! `SenderWriter::write` call and the decode loop terminates promptly.
//! Verification: `SenderWriter::write` calls `SyncSender::send`, whose only
//! error variant is `SendError` (receiver disconnected), which we map to
//! `io::ErrorKind::BrokenPipe`.  `lzma_rs::xz_decompress` propagates `Write`
//! errors up immediately via `?`, so the thread exits on the very next write
//! after the receiver is dropped — no looping or blocking.  No additional
//! token-check in `SenderWriter::write` is needed.

use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use tracing::debug;

use fdemon_core::{Error, Result};

use super::types::DownloadProgress;

// ── Download constants ────────────────────────────────────────────────────────

/// TCP connect timeout for archive downloads.
///
/// 10 seconds is generous for local-area and well-peered CDN connections while
/// still bounding the wizard stall on a totally unreachable endpoint.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Per-read idle guard for archive downloads.
///
/// If no bytes arrive within this window on a single read the stream is
/// considered stalled and the attempt is abandoned.  30 s accommodates slow
/// CDN edge nodes without letting a stalled socket hang the wizard
/// indefinitely.  This is wired via `ClientBuilder::read_timeout` (resets
/// after each successful chunk) — **not** `ClientBuilder::timeout` (which is a
/// total-request deadline and would abort any ~300 MiB download over a slow
/// link before the transfer completes).
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of download attempts before giving up.
const MAX_DOWNLOAD_ATTEMPTS: u32 = 3;

/// Conservative minimum free-disk-space budget for a single archive download.
///
/// Flutter SDK archives are ~300 MiB compressed; the extracted SDK plus
/// precache artifacts can exceed 1 GiB. This constant budgets 1.5 GiB —
/// generous enough for both the compressed download and the extraction working
/// space. Callers that have a more precise `Content-Length` may pass that value
/// directly to [`ensure_disk_space`] instead.
pub(crate) const ARCHIVE_DISK_BUDGET_BYTES: u64 = 1_572_864_000; // 1.5 GiB

/// Fast HEAD probe timeout for the network-connectivity check.
///
/// 5 seconds bounds the stall when the host is offline.  For comparison,
/// `IDLE_TIMEOUT` is a per-read idle guard (not a total deadline), so the
/// offline stall without this probe could be much longer than 30 seconds.
const CONNECTIVITY_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

// ── Transport security constants ─────────────────────────────────────────────

/// Maximum number of HTTP redirects to follow for a single download.
///
/// Bounds the redirect chain so a CDN misconfiguration or adversarial server
/// cannot cause an unbounded redirect loop.  Five hops is more than enough for
/// well-behaved CDN redirect chains; the Flutter and Android CDNs typically
/// require at most 2-3 redirects.
const MAX_REDIRECTS: usize = 5;

// ── URL-scheme guard ──────────────────────────────────────────────────────────

/// Validate that `url` uses the `https://` scheme.
///
/// All production download targets use HTTPS. Accepting a plain `http://` URL
/// (or any other scheme) could allow a transparent downgrade attack if a URL
/// were somehow supplied from an untrusted source or misconfigured redirect
/// chain.
///
/// # Errors
///
/// Returns [`Error::Process`] when `url` does not start with `https://`.
pub(crate) fn validate_https_url(url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        return Err(Error::process(format!(
            "download URL must use HTTPS (got: {url:?})"
        )));
    }
    Ok(())
}

// ── Preflight helpers ─────────────────────────────────────────────────────────

/// Assert that the filesystem holding `dir` has at least `required` free bytes.
///
/// Uses `fs4::available_space` which delegates to `statvfs(2)` on POSIX and
/// `GetDiskFreeSpaceExW` on Windows — no libc, pure-Rust implementation.
///
/// The `dir` path is used as the filesystem probe point. The directory must
/// already exist (or the parent must exist); otherwise the probe itself will
/// fail and the error is surfaced as an `Error::Process`.
///
/// # Errors
///
/// Returns `Error::Process` when:
/// - `fs4::available_space` fails (exotic FS, permission error, …).
/// - The available space is less than `required`.
pub(crate) fn ensure_disk_space(dir: &Path, required: u64) -> Result<()> {
    let avail = fs4::available_space(dir).map_err(|e| {
        Error::process(format!(
            "disk-space probe failed for {}: {e}",
            dir.display()
        ))
    })?;
    if avail < required {
        return Err(Error::process(format!(
            "insufficient disk space in {}: need ~{} MiB, have {} MiB",
            dir.display(),
            required / 1_048_576,
            avail / 1_048_576,
        )));
    }
    Ok(())
}

/// Send a ≤5s HEAD request to `url` to verify network reachability.
///
/// A successful response (any HTTP status code) is treated as "reachable" —
/// the goal is to detect a completely unreachable host quickly rather than to
/// validate the URL. Any transport error (DNS, TCP, TLS) within the 5-second
/// budget returns a "no network connectivity" error.
///
/// Call this once before the first download attempt to bound the offline
/// stall. Skip the probe if a previous request to the same origin already
/// succeeded in the same install session (connectivity is implicitly proven).
///
/// ## Captive-portal limitation
///
/// This probe **cannot reliably detect captive portals**.  When a portal
/// returns an HTTP 2xx/3xx response (e.g. a redirect to a login page), the
/// probe succeeds even though the real download endpoint is not reachable.
/// The subsequent download then fails with a parse or content error rather
/// than a fast "no network" message.
///
/// However, because the target URL uses HTTPS (`storage.googleapis.com`), a
/// transparent MITM portal that cannot present a valid certificate for that
/// host will fail the TLS handshake and *is* caught by this probe (fast
/// failure).  Only a portal that passively passes HTTPS traffic without
/// interception (and instead intercepts DNS or TCP at the application layer)
/// can sneak past this check.
///
/// # Errors
///
/// Returns `Error::Process` when the request fails to complete (timeout,
/// DNS resolution failure, TCP refused, TLS error, etc.).
pub(crate) async fn check_network_connectivity(client: &reqwest::Client, url: &str) -> Result<()> {
    client
        .head(url)
        .timeout(CONNECTIVITY_PROBE_TIMEOUT)
        .send()
        .await
        .map(|_| ())
        .map_err(|e| Error::process(format!("no network connectivity: cannot reach {url} ({e})")))
}

// ── Part-file Drop Guard ──────────────────────────────────────────────────────

/// RAII guard that removes a `.part` file from disk when dropped.
///
/// The guard is **armed** on construction and **disarmed** only on success via
/// [`PartFileGuard::disarm`].  This guarantees that no orphaned `.part` file
/// is left behind after any early-exit path, including:
///
/// - A cancelled download (user pressed Esc / cancellation token fired).
/// - A network or I/O error during streaming.
/// - An outer `JoinHandle::abort()` that drops the future mid-await.
///
/// Panics in the guarded code path are also covered because `Drop` runs even
/// on unwind.
pub(crate) struct PartFileGuard {
    path: PathBuf,
    armed: bool,
}

impl PartFileGuard {
    /// Create a new armed guard for `path`.
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    /// Disarm the guard so the file is **not** removed on drop.
    ///
    /// Call this after a successful rename (`.part` → final destination) so
    /// the guard does not attempt to delete a file that no longer exists.
    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PartFileGuard {
    fn drop(&mut self) {
        if self.armed {
            if let Err(e) = std::fs::remove_file(&self.path) {
                // Not finding the file (NotFound) is fine — it may never have
                // been created (e.g. pre-cancel) or may have already been
                // removed by a previous cleanup attempt.
                if e.kind() != io::ErrorKind::NotFound {
                    debug!(path = ?self.path, error = %e,
                        "PartFileGuard: failed to remove .part file (best-effort)");
                }
            }
        }
    }
}

// ── Download ─────────────────────────────────────────────────────────────────

/// Stream a URL to `dest`, invoking `on_progress` after each chunk arrives.
///
/// Downloads are staged to `<dest>.part` and renamed to `dest` only on
/// success.  On failure or cancellation the `.part` file is removed by a
/// [`PartFileGuard`] that is armed on entry and disarmed only on success.
///
/// The function retries up to [`MAX_DOWNLOAD_ATTEMPTS`] times on transient
/// failures (network errors, non-4xx HTTP errors).  Each retry restarts from
/// byte 0.
///
/// The `Content-Length` response header, when present, is surfaced as
/// [`DownloadProgress::total`] so the caller can render a progress bar.  When
/// the server omits `Content-Length`, `total` is `None` for all callbacks.
///
/// ## Cancellation
///
/// Pass a [`CancellationToken`] to support user-initiated cancellation.  When
/// the token is cancelled, the streaming loop exits at the next chunk boundary
/// and returns [`fdemon_core::Error::Cancelled`].  No `.part` file is left
/// behind — the `PartFileGuard` removes it in `Drop`.
///
/// For a non-cancellable download, pass `CancellationToken::new()` (an
/// unowned, never-cancelled token).
///
/// # Errors
///
/// - [`fdemon_core::Error::Cancelled`] when the token is cancelled mid-stream.
/// - [`fdemon_core::Error::Process`] when all attempts fail or on HTTP 4xx.
/// - [`fdemon_core::Error::Io`] on I/O errors while writing to `dest`.
pub async fn download_to_file<F>(
    url: &str,
    dest: &Path,
    cancel: CancellationToken,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(DownloadProgress),
{
    // Pre-cancel check: if the token is already cancelled, return immediately
    // without performing any I/O.
    if cancel.is_cancelled() {
        return Err(Error::cancelled("download cancelled before start"));
    }

    // Reject non-HTTPS URLs up front. In test builds this check is skipped so
    // that wiremock (http://) tests can exercise the download pipeline without
    // requiring a real TLS server.
    #[cfg(not(test))]
    validate_https_url(url)?;

    // Install a custom redirect policy that:
    // - Bounds the redirect chain to MAX_REDIRECTS hops.
    // - Rejects any redirect that downgrades from HTTPS to HTTP (scheme check
    //   on the target URL).  This prevents a CDN misconfiguration or MITM from
    //   silently moving the download to a plaintext channel.
    //
    // In test builds we allow HTTP redirects so that wiremock redirect tests
    // work correctly without a TLS server.
    let redirect_policy = reqwest::redirect::Policy::custom(|attempt| {
        let target_url = attempt.url().clone();
        if attempt.previous().len() >= MAX_REDIRECTS {
            attempt.error(format!(
                "too many redirects (limit: {MAX_REDIRECTS}) for {target_url}"
            ))
        } else {
            // In production (non-test) builds, reject any redirect that
            // downgrades to a non-HTTPS scheme.
            #[cfg(not(test))]
            if target_url.scheme() != "https" {
                return attempt.error(format!("redirect to non-HTTPS URL rejected: {target_url}"));
            }
            attempt.follow()
        }
    });

    let client = reqwest::Client::builder()
        .user_agent(concat!("fdemon/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(IDLE_TIMEOUT)
        .redirect(redirect_policy)
        .build()
        .map_err(|e| Error::process(format!("failed to build HTTP client: {e}")))?;

    let part_path = dest.with_extension(
        dest.extension()
            .map(|ext| format!("{}.part", ext.to_string_lossy()))
            .unwrap_or_else(|| "part".to_string()),
    );

    // Preflight: verify that the filesystem holding `dest` has enough free space
    // for the archive download. Use the parent directory as the probe point (it
    // must exist; `dest` itself may not yet). Fall back to the current directory
    // when `dest` has no parent (pathological case).
    let probe_dir = dest.parent().unwrap_or_else(|| Path::new("."));
    ensure_disk_space(probe_dir, ARCHIVE_DISK_BUDGET_BYTES)?;

    // Arm the Drop guard. It removes the `.part` file unless `disarm()` is
    // called on the success path just before the rename.  This covers all
    // early-exit paths including cancellation, errors, and `JoinHandle::abort`.
    let mut part_guard = PartFileGuard::new(part_path.clone());

    let mut last_err: Option<Error> = None;

    for attempt in 1..=MAX_DOWNLOAD_ATTEMPTS {
        // Per-attempt cancellation check so a cancel between retries exits
        // immediately without issuing another HTTP request.
        if cancel.is_cancelled() {
            return Err(Error::cancelled("download cancelled"));
        }

        if attempt > 1 {
            debug!(
                attempt,
                MAX_DOWNLOAD_ATTEMPTS, url, "retrying download after transient failure"
            );
        }

        // Truncate/create the .part file for this attempt.
        let mut file = match File::create(&part_path) {
            Ok(f) => f,
            Err(e) => {
                let err = Error::Io(io::Error::new(
                    e.kind(),
                    format!("create {part_path:?}: {e}"),
                ));
                last_err = Some(err);
                continue;
            }
        };

        let response = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(Error::process(format!(
                    "HTTP request failed for {url}: {e}"
                )));
                continue;
            }
        };

        let status = response.status();

        // 4xx errors are not retriable (bad URL, auth, etc.).
        if status.is_client_error() {
            return Err(Error::process(format!("HTTP {status} for {url}")));
        }

        if !status.is_success() {
            last_err = Some(Error::process(format!("HTTP {status} for {url}")));
            continue;
        }

        let total = response.content_length();
        let mut received: u64 = 0;
        let mut stream = response.bytes_stream();
        let mut stream_err: Option<Error> = None;

        loop {
            tokio::select! {
                // Biased: check cancellation first so a pre-signalled token
                // is detected before blocking on the next chunk.
                biased;

                _ = cancel.cancelled() => {
                    return Err(Error::cancelled("download cancelled"));
                }

                chunk_result = stream.next() => {
                    match chunk_result {
                        Some(Ok(chunk)) => {
                            if let Err(e) = file.write_all(&chunk) {
                                stream_err = Some(Error::Io(io::Error::new(
                                    e.kind(),
                                    format!("write to {part_path:?}: {e}"),
                                )));
                                break;
                            }
                            received += chunk.len() as u64;
                            on_progress(DownloadProgress { received, total });
                        }
                        Some(Err(e)) => {
                            stream_err = Some(Error::process(format!(
                                "stream read error for {url}: {e}"
                            )));
                            break;
                        }
                        None => {
                            // Stream exhausted — download complete.
                            break;
                        }
                    }
                }
            }
        }

        if let Some(err) = stream_err {
            last_err = Some(err);
            continue;
        }

        // Flush before rename.
        if let Err(e) = file.flush() {
            last_err = Some(Error::Io(io::Error::new(
                e.kind(),
                format!("flush {part_path:?}: {e}"),
            )));
            continue;
        }

        // Disarm the guard *before* the rename so it does not attempt to
        // remove a file that is being atomically moved to its final location.
        part_guard.disarm();

        // Success: rename .part → dest.
        if let Err(e) = std::fs::rename(&part_path, dest) {
            // Re-arm the guard would be unsafe here since the file was not
            // renamed. Remove it manually and propagate the rename error.
            let _ = std::fs::remove_file(&part_path);
            return Err(Error::Io(io::Error::new(
                e.kind(),
                format!("rename {part_path:?} → {dest:?}: {e}"),
            )));
        }

        return Ok(());
    }

    // All attempts failed — the Drop guard removes the .part file.
    Err(last_err.unwrap_or_else(|| {
        Error::process(format!(
            "download failed after {MAX_DOWNLOAD_ATTEMPTS} attempts: {url}"
        ))
    }))
}

// ── SHA-256 Verification ──────────────────────────────────────────────────────

/// Verify a file's SHA-256 against an expected lowercase hex digest.
///
/// Reads `path` in streaming chunks so that large archives do not need to fit
/// in memory.
///
/// # Errors
///
/// - [`Error::Io`] when `path` cannot be opened or read.
/// - [`Error::Process`] when the computed digest does not match
///   `expected_hex`.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<()> {
    let file = File::open(path)
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("{path:?}: {e}"))))?;

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];

    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("read {path:?}: {e}"))))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    let computed = format!("{:x}", hasher.finalize());

    if computed.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(Error::process(format!(
            "SHA-256 mismatch for {path:?}: expected {expected_hex}, got {computed}"
        )))
    }
}

// ── Path traversal guard ──────────────────────────────────────────────────────

/// Sanitize an archive entry path against traversal attacks.
///
/// Rejects any entry whose name:
/// - is absolute (starts with `/` or a drive letter),
/// - contains a `..` component.
///
/// Returns the resolved output path `dest_dir.join(raw_name)` after asserting
/// that it starts with `dest_dir` (normalised without following symlinks).
///
/// # Errors
///
/// Returns [`Error::Process`] with the offending entry name if the path would
/// escape `dest_dir`.
fn sanitize_entry_path(dest_dir: &Path, raw_name: &str) -> Result<PathBuf> {
    let raw = Path::new(raw_name);

    // Reject absolute paths.
    if raw.is_absolute() {
        return Err(Error::process(format!(
            "archive entry has absolute path (path traversal rejected): {raw_name:?}"
        )));
    }

    // Reject any component that is `..`.
    for component in raw.components() {
        if component == Component::ParentDir {
            return Err(Error::process(format!(
                "archive entry contains '..' component (path traversal rejected): {raw_name:?}"
            )));
        }
    }

    let out_path = dest_dir.join(raw);

    // Belt-and-suspenders: ensure the joined path stays inside dest_dir.
    // We use `starts_with` on the un-canonicalized path because the dest_dir
    // may not exist yet (we create it entry-by-entry).  The component checks
    // above are the primary guard; this is a secondary assertion.
    if !out_path.starts_with(dest_dir) {
        return Err(Error::process(format!(
            "archive entry escapes destination directory (path traversal rejected): {raw_name:?}"
        )));
    }

    Ok(out_path)
}

// ── ZIP Extraction ────────────────────────────────────────────────────────────

/// Number of archive entries to process between cancellation-token checks.
///
/// Checking on every entry would incur a small but measurable overhead for
/// archives with tens of thousands of entries (the Flutter SDK has ~80k entries
/// in its tar.xz). Checking every 256 entries bounds the cancellation latency
/// to at most the time to process 256 entries — well under a second — without
/// the per-entry check cost.
const CANCEL_CHECK_INTERVAL: usize = 256;

/// Extract a `.zip` archive into `dest_dir`.
///
/// On Unix, executable mode bits recorded in the zip's external attributes
/// (`ZipFile::unix_mode()`) are applied to the extracted file. Flutter's
/// bundled binaries (e.g. `bin/flutter`, `bin/dart`) rely on `+x` to run.
///
/// Entries with absolute paths or `..` components are rejected to prevent
/// zip-slip / path traversal attacks.
///
/// ## Cancellation
///
/// The extraction loop checks `cancel` every [`CANCEL_CHECK_INTERVAL`] entries.
/// When cancelled, extraction stops promptly and returns
/// [`fdemon_core::Error::Cancelled`].  No partial file is left behind — the
/// caller's [`TempDirGuard`] (in `flutter_install.rs`) removes the destination
/// directory.
///
/// For a non-cancellable extraction, pass `CancellationToken::new()`.
///
/// # Errors
///
/// Returns an error on archive I/O failures, path traversal attempts, or when
/// a file cannot be created inside `dest_dir`.
pub fn extract_zip(archive: &Path, dest_dir: &Path, cancel: &CancellationToken) -> Result<()> {
    let file = File::open(archive)
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("open {archive:?}: {e}"))))?;

    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::process(format!("open zip {archive:?}: {e}")))?;

    for i in 0..zip.len() {
        // Check for cancellation every CANCEL_CHECK_INTERVAL entries so a
        // token fired during extraction stops the loop promptly without
        // incurring a per-entry atomic load.
        if i.is_multiple_of(CANCEL_CHECK_INTERVAL) && cancel.is_cancelled() {
            return Err(Error::cancelled("archive extraction cancelled"));
        }

        let mut entry = zip
            .by_index(i)
            .map_err(|e| Error::process(format!("zip entry {i} in {archive:?}: {e}")))?;

        // Guard against zip-slip / path traversal.
        let out_path = sanitize_entry_path(dest_dir, entry.name())?;

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| {
                Error::Io(io::Error::new(
                    e.kind(),
                    format!("create dir {out_path:?}: {e}"),
                ))
            })?;
        } else {
            // Ensure parent directory exists.
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    Error::Io(io::Error::new(
                        e.kind(),
                        format!("create dir {parent:?}: {e}"),
                    ))
                })?;
            }

            let mut out_file = File::create(&out_path).map_err(|e| {
                Error::Io(io::Error::new(
                    e.kind(),
                    format!("create file {out_path:?}: {e}"),
                ))
            })?;

            io::copy(&mut entry, &mut out_file).map_err(|e| {
                Error::Io(io::Error::new(e.kind(), format!("write {out_path:?}: {e}")))
            })?;

            // Preserve unix executable bits on Unix platforms.
            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(mode);
                std::fs::set_permissions(&out_path, perms).map_err(|e| {
                    Error::Io(io::Error::new(
                        e.kind(),
                        format!("set permissions on {out_path:?}: {e}"),
                    ))
                })?;
            }
        }
    }

    Ok(())
}

// ── TAR.XZ Extraction ─────────────────────────────────────────────────────────

/// A `Read` adapter that drains bytes from an `mpsc::Receiver<Vec<u8>>`.
///
/// Used to bridge lzma-rs' push-based (`Write`) XZ decoder with the tar
/// crate's pull-based (`Read`) archive reader without buffering the entire
/// decompressed stream.
struct ReceiverReader {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    buf: Vec<u8>,
    pos: usize,
}

impl ReceiverReader {
    fn new(rx: std::sync::mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            buf: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for ReceiverReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        // Refill from channel when local buffer is exhausted.
        while self.pos >= self.buf.len() {
            match self.rx.recv() {
                Ok(chunk) => {
                    self.buf = chunk;
                    self.pos = 0;
                }
                // Sender dropped → EOF.
                Err(_) => return Ok(0),
            }
        }

        let available = self.buf.len() - self.pos;
        let n = out.len().min(available);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// A `Write` adapter that forwards chunks to an `mpsc::SyncSender<Vec<u8>>`.
///
/// If the receiver is gone (e.g. tar extraction failed), write errors are
/// surfaced as [`io::ErrorKind::BrokenPipe`].
struct SenderWriter {
    tx: std::sync::mpsc::SyncSender<Vec<u8>>,
}

impl Write for SenderWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        self.tx
            .send(data.to_vec())
            .map(|_| data.len())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "tar reader dropped"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Extract a `.tar.xz` archive into `dest_dir` using pure-Rust decoders.
///
/// Uses `lzma-rs` for XZ decompression and the `tar` crate for archive
/// unpacking. No C library dependencies.
///
/// XZ decompression is streamed through an in-process channel to avoid
/// materialising the full decompressed archive in RAM (the Flutter SDK
/// decompresses to ≈ 1 GB).  Peak RAM usage is proportional to the XZ block
/// size rather than the total archive size.
///
/// Entries are iterated explicitly; each entry path is passed through
/// [`sanitize_entry_path`] before any file is written.  Any entry whose path
/// contains `..` components, is absolute, or would escape `dest_dir` causes
/// an immediate `Err` — matching the fail-closed behaviour of [`extract_zip`].
/// Unix mode bits on extracted files are preserved.
///
/// ## Cancellation
///
/// The extraction loop checks `cancel` every [`CANCEL_CHECK_INTERVAL`] entries.
/// When cancelled, extraction stops promptly and returns
/// [`fdemon_core::Error::Cancelled`].  Dropping the `ReceiverReader` signals
/// the XZ decode thread (via `BrokenPipe` on its next `SenderWriter::write`)
/// so it self-terminates without needing an explicit abort — see module-level
/// doc for details.
///
/// For a non-cancellable extraction, pass `CancellationToken::new()`.
///
/// # Errors
///
/// Returns an error on decompression failures, path-traversal/symlink-escape
/// entries, or I/O failures during extraction.
pub fn extract_tar_xz(archive: &Path, dest_dir: &Path, cancel: &CancellationToken) -> Result<()> {
    let file = File::open(archive)
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("open {archive:?}: {e}"))))?;

    // Bounded channel: 8 slots × ~1 MiB chunks ≈ 8 MiB of pipeline buffer.
    // This decouples the XZ thread from the tar thread without unbounded
    // allocation.
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(8);

    let archive_path_display = archive.to_path_buf();

    // Spawn a thread to drive lzma-rs XZ decoding, pushing bytes via the channel.
    let decode_thread = std::thread::spawn(move || -> std::result::Result<(), String> {
        let mut reader = BufReader::new(file);
        let mut writer = SenderWriter { tx };
        lzma_rs::xz_decompress(&mut reader, &mut writer)
            .map_err(|e| format!("xz decompress {archive_path_display:?}: {e}"))
    });

    // Main thread: pull bytes from the channel and feed them to the tar reader.
    let receiver_reader = ReceiverReader::new(rx);
    let mut tar_archive = tar::Archive::new(receiver_reader);
    tar_archive.set_preserve_permissions(true);
    tar_archive.set_unpack_xattrs(false);

    // Iterate entries explicitly so we can fail-closed on any traversal entry,
    // matching the behaviour of extract_zip / sanitize_entry_path.
    let unpack_result: Result<()> = (|| {
        let entries = tar_archive.entries().map_err(|e| {
            Error::Io(io::Error::new(
                e.kind(),
                format!("read tar entries from {archive:?}: {e}"),
            ))
        })?;

        for (entry_count, entry_result) in entries.enumerate() {
            // Check for cancellation every CANCEL_CHECK_INTERVAL entries.
            // Dropping out of this closure drops the ReceiverReader, which
            // causes BrokenPipe in the XZ decode thread so it self-terminates.
            if entry_count.is_multiple_of(CANCEL_CHECK_INTERVAL) && cancel.is_cancelled() {
                return Err(Error::cancelled("archive extraction cancelled"));
            }

            let mut entry = entry_result.map_err(|e| {
                Error::Io(io::Error::new(
                    e.kind(),
                    format!("read tar entry from {archive:?}: {e}"),
                ))
            })?;

            // Validate path against traversal before writing anything.
            let raw_path = entry
                .path()
                .map_err(|e| {
                    Error::process(format!("invalid path in tar entry from {archive:?}: {e}"))
                })?
                .into_owned();

            let raw_str = raw_path.to_string_lossy();
            let out_path = sanitize_entry_path(dest_dir, &raw_str)?;

            let entry_type = entry.header().entry_type();

            if entry_type.is_dir() {
                std::fs::create_dir_all(&out_path).map_err(|e| {
                    Error::Io(io::Error::new(
                        e.kind(),
                        format!("create dir {out_path:?}: {e}"),
                    ))
                })?;
            } else if entry_type.is_symlink() {
                // Reject symlinks: a symlink whose target points outside
                // dest_dir is a symlink-escape attack.  Fail closed rather
                // than trusting the link target.
                return Err(Error::process(format!(
                    "archive entry is a symlink (rejected to prevent symlink-escape): {:?}",
                    raw_str.as_ref()
                )));
            } else {
                // Regular file (or hard link, treated as file).
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        Error::Io(io::Error::new(
                            e.kind(),
                            format!("create dir {parent:?}: {e}"),
                        ))
                    })?;
                }

                let mut out_file = File::create(&out_path).map_err(|e| {
                    Error::Io(io::Error::new(
                        e.kind(),
                        format!("create file {out_path:?}: {e}"),
                    ))
                })?;

                io::copy(&mut entry, &mut out_file).map_err(|e| {
                    Error::Io(io::Error::new(e.kind(), format!("write {out_path:?}: {e}")))
                })?;

                // Preserve Unix mode bits.
                #[cfg(unix)]
                if let Ok(mode) = entry.header().mode() {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(mode);
                    std::fs::set_permissions(&out_path, perms).map_err(|e| {
                        Error::Io(io::Error::new(
                            e.kind(),
                            format!("set permissions on {out_path:?}: {e}"),
                        ))
                    })?;
                }
            }
        }

        Ok(())
    })();

    // Wait for the decode thread and surface any decoding error.
    let decode_result = decode_thread
        .join()
        .map_err(|_| Error::process(format!("xz decode thread panicked for {archive:?}")))
        .and_then(|r| r.map_err(Error::process));

    // Prefer the decode error (more informative) if both fail.
    match (decode_result, unpack_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(e), _) => Err(e),
        (Ok(()), Err(e)) => Err(e),
    }
}

// ── Archive Dispatch ──────────────────────────────────────────────────────────

/// Detect archive kind from the file extension and dispatch to the correct
/// extractor.
///
/// Supported extensions:
/// - `.zip` → [`extract_zip`]
/// - `.tar.xz`, `.txz` → [`extract_tar_xz`]
///
/// ## Cancellation
///
/// `cancel` is forwarded to the underlying extractor.  When cancelled,
/// extraction stops promptly and returns [`fdemon_core::Error::Cancelled`].
/// For a non-cancellable extraction, pass `CancellationToken::new()`.
///
/// # Errors
///
/// Returns an error when the extension is not recognised or extraction fails.
pub fn extract_archive(archive: &Path, dest_dir: &Path, cancel: &CancellationToken) -> Result<()> {
    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        extract_tar_xz(archive, dest_dir, cancel)
    } else if name.ends_with(".zip") {
        extract_zip(archive, dest_dir, cancel)
    } else {
        Err(Error::process(format!(
            "unsupported archive format: {archive:?}; expected .zip, .tar.xz, or .txz"
        )))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use tempfile::TempDir;

    // ── validate_https_url ────────────────────────────────────────────────────

    #[test]
    fn test_validate_https_url_accepts_https() {
        validate_https_url("https://storage.googleapis.com/flutter_infra/releases/stable/linux/flutter_linux_3.0.0-stable.tar.xz")
            .expect("HTTPS URL must be accepted");
    }

    #[test]
    fn test_validate_https_url_rejects_http() {
        let err = validate_https_url("http://storage.googleapis.com/file.tar.xz")
            .expect_err("HTTP URL must be rejected");
        assert!(
            err.to_string().contains("HTTPS"),
            "error should mention HTTPS: {err}"
        );
    }

    #[test]
    fn test_validate_https_url_rejects_ftp() {
        let err = validate_https_url("ftp://example.com/file.tar.xz")
            .expect_err("FTP URL must be rejected");
        assert!(
            err.to_string().contains("HTTPS"),
            "error should mention HTTPS: {err}"
        );
    }

    #[test]
    fn test_validate_https_url_rejects_empty() {
        let err = validate_https_url("").expect_err("empty URL must be rejected");
        assert!(
            err.to_string().contains("HTTPS"),
            "error should mention HTTPS: {err}"
        );
    }

    // ── verify_sha256 ─────────────────────────────────────────────────────────

    /// Build a known SHA-256 for a byte slice without writing to disk.
    fn sha256_hex(data: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(data);
        format!("{:x}", h.finalize())
    }

    #[test]
    fn test_verify_sha256_match_and_mismatch() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.bin");
        let content = b"hello, SHA-256 world!";
        std::fs::write(&path, content).unwrap();

        let correct_hex = sha256_hex(content);
        // Correct digest → Ok
        verify_sha256(&path, &correct_hex).expect("correct digest must pass");

        // Wrong digest → Err
        let bad_hex = "0".repeat(64);
        let err = verify_sha256(&path, &bad_hex).expect_err("wrong digest must fail");
        assert!(
            err.to_string().contains("SHA-256 mismatch"),
            "error should mention mismatch: {err}"
        );
    }

    #[test]
    fn test_verify_sha256_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.bin");
        let content = b"case test";
        std::fs::write(&path, content).unwrap();

        let lower = sha256_hex(content);
        let upper = lower.to_uppercase();
        // Both cases must pass
        verify_sha256(&path, &lower).expect("lowercase must pass");
        verify_sha256(&path, &upper).expect("uppercase must pass");
    }

    #[test]
    fn test_verify_sha256_missing_file() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("no_such_file.bin");
        let err = verify_sha256(&missing, &"0".repeat(64)).expect_err("must fail on missing file");
        assert!(
            matches!(err, Error::Io(_)),
            "expected Io error, got {err:?}"
        );
    }

    // ── sanitize_entry_path ───────────────────────────────────────────────────

    #[test]
    fn test_sanitize_entry_path_normal() {
        let tmp = TempDir::new().unwrap();
        let result = sanitize_entry_path(tmp.path(), "subdir/file.txt");
        assert!(result.is_ok(), "normal path should be accepted: {result:?}");
        assert_eq!(result.unwrap(), tmp.path().join("subdir/file.txt"));
    }

    #[test]
    fn test_sanitize_entry_path_rejects_dotdot() {
        let tmp = TempDir::new().unwrap();
        let err = sanitize_entry_path(tmp.path(), "../escape.txt")
            .expect_err(".. component must be rejected");
        assert!(
            err.to_string().contains("traversal"),
            "error should mention traversal: {err}"
        );
    }

    #[test]
    fn test_sanitize_entry_path_rejects_nested_dotdot() {
        let tmp = TempDir::new().unwrap();
        let err = sanitize_entry_path(tmp.path(), "subdir/../../escape.txt")
            .expect_err("nested .. must be rejected");
        assert!(
            err.to_string().contains("traversal"),
            "error should mention traversal: {err}"
        );
    }

    #[test]
    fn test_sanitize_entry_path_rejects_absolute() {
        let tmp = TempDir::new().unwrap();
        let err = sanitize_entry_path(tmp.path(), "/etc/passwd")
            .expect_err("absolute path must be rejected");
        assert!(
            err.to_string().contains("traversal"),
            "error should mention traversal: {err}"
        );
    }

    // ── extract_zip ───────────────────────────────────────────────────────────

    /// Build an in-memory zip containing the given files.
    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, data) in files {
                writer.start_file(*name, opts).unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        buf
    }

    /// Compute a CRC-32 checksum (IEEE polynomial) without an external crate.
    ///
    /// This is a test-only helper used to craft raw ZIP bytes for traversal
    /// tests.  Production code never calls this function.
    fn crc32_ieee(data: &[u8]) -> u32 {
        // Standard CRC-32/ISO-HDLC (IEEE 802.3) polynomial, bit-reversed.
        const POLY: u32 = 0xEDB8_8320;
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ POLY;
                } else {
                    crc >>= 1;
                }
            }
        }
        crc ^ 0xFFFF_FFFF
    }

    /// Build an in-memory zip with a malicious entry (simulates zip-slip).
    fn make_malicious_zip(entry_name: &str, data: &[u8]) -> Vec<u8> {
        // We write raw zip bytes because the zip crate correctly sanitizes
        // paths via its normal API. Instead we craft the local file header
        // manually to inject the traversal path.
        //
        // ZIP local file header layout:
        //   signature         : 4 bytes  (0x04034b50)
        //   version needed    : 2 bytes
        //   general purpose   : 2 bytes
        //   compression method: 2 bytes  (0 = stored)
        //   last mod time     : 2 bytes
        //   last mod date     : 2 bytes
        //   crc-32            : 4 bytes
        //   compressed size   : 4 bytes
        //   uncompressed size : 4 bytes
        //   file name length  : 2 bytes
        //   extra field length: 2 bytes
        //   file name         : variable
        //   extra field       : variable
        //   file data         : variable
        //
        // Followed by central directory + end-of-central-directory record.

        let name_bytes = entry_name.as_bytes();
        let name_len = name_bytes.len() as u16;
        let data_len = data.len() as u32;
        let crc = crc32_ieee(data);

        let mut zip_bytes: Vec<u8> = Vec::new();

        // Local file header offset (for central directory).
        let local_header_offset: u32 = 0;

        // Local file header.
        zip_bytes.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]); // signature
        zip_bytes.extend_from_slice(&20u16.to_le_bytes()); // version needed
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // general purpose bits
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // compression (stored)
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // last mod time
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // last mod date
        zip_bytes.extend_from_slice(&crc.to_le_bytes()); // crc-32
        zip_bytes.extend_from_slice(&data_len.to_le_bytes()); // compressed size
        zip_bytes.extend_from_slice(&data_len.to_le_bytes()); // uncompressed size
        zip_bytes.extend_from_slice(&name_len.to_le_bytes()); // file name length
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // extra field length
        zip_bytes.extend_from_slice(name_bytes); // file name
                                                 // (no extra field)
        zip_bytes.extend_from_slice(data); // file data

        let central_dir_offset = zip_bytes.len() as u32;

        // Central directory header.
        zip_bytes.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]); // signature
        zip_bytes.extend_from_slice(&20u16.to_le_bytes()); // version made by
        zip_bytes.extend_from_slice(&20u16.to_le_bytes()); // version needed
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // general purpose bits
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // compression
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // last mod time
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // last mod date
        zip_bytes.extend_from_slice(&crc.to_le_bytes()); // crc-32
        zip_bytes.extend_from_slice(&data_len.to_le_bytes()); // compressed size
        zip_bytes.extend_from_slice(&data_len.to_le_bytes()); // uncompressed size
        zip_bytes.extend_from_slice(&name_len.to_le_bytes()); // file name length
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // extra field length
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // file comment length
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // disk number start
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // internal file attrs
        zip_bytes.extend_from_slice(&0u32.to_le_bytes()); // external file attrs
        zip_bytes.extend_from_slice(&local_header_offset.to_le_bytes()); // rel offset
        zip_bytes.extend_from_slice(name_bytes); // file name

        let central_dir_size = (zip_bytes.len() as u32) - central_dir_offset;

        // End of central directory record.
        zip_bytes.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]); // signature
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // disk number
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // disk with start of CD
        zip_bytes.extend_from_slice(&1u16.to_le_bytes()); // entries on this disk
        zip_bytes.extend_from_slice(&1u16.to_le_bytes()); // total entries
        zip_bytes.extend_from_slice(&central_dir_size.to_le_bytes()); // CD size
        zip_bytes.extend_from_slice(&central_dir_offset.to_le_bytes()); // CD offset
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // comment length

        zip_bytes
    }

    #[test]
    fn test_extract_zip_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("test.zip");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        let zip_data = make_zip(&[
            ("hello.txt", b"hello world"),
            ("subdir/nested.txt", b"nested content"),
        ]);
        std::fs::write(&archive_path, &zip_data).unwrap();

        extract_zip(&archive_path, &dest_dir, &CancellationToken::new())
            .expect("extract_zip must succeed");

        // Top-level file
        let hello = std::fs::read(dest_dir.join("hello.txt")).unwrap();
        assert_eq!(hello, b"hello world");

        // Nested file
        let nested = std::fs::read(dest_dir.join("subdir").join("nested.txt")).unwrap();
        assert_eq!(nested, b"nested content");
    }

    #[test]
    fn test_extract_zip_missing_archive() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("no_such.zip");
        let dest = tmp.path().join("out");
        let err = extract_zip(&missing, &dest, &CancellationToken::new())
            .expect_err("must fail on missing archive");
        assert!(
            matches!(err, Error::Io(_)),
            "expected Io error, got {err:?}"
        );
    }

    #[test]
    fn test_extract_zip_rejects_dotdot_traversal() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("evil.zip");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        // Craft a zip with a traversal entry name.
        let zip_data = make_malicious_zip("../escape.txt", b"evil content");
        std::fs::write(&archive_path, &zip_data).unwrap();

        let err = extract_zip(&archive_path, &dest_dir, &CancellationToken::new())
            .expect_err("zip-slip entry must be rejected");
        assert!(
            err.to_string().contains("traversal"),
            "error should mention traversal: {err}"
        );

        // The parent of dest_dir must not contain "escape.txt".
        let escaped = tmp.path().join("escape.txt");
        assert!(
            !escaped.exists(),
            "traversal file must not have been written: {escaped:?}"
        );
    }

    // ── extract_tar_xz ────────────────────────────────────────────────────────

    /// Build a `.tar.xz` archive in memory from the given files.
    fn make_tar_xz(files: &[(&str, &[u8])]) -> Vec<u8> {
        // Build tar in memory
        let mut tar_buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_buf);
            for (name, data) in files {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, name, *data).unwrap();
            }
            builder.finish().unwrap();
        }

        // XZ-compress with lzma-rs
        let mut xz_buf = Vec::new();
        lzma_rs::xz_compress(&mut tar_buf.as_slice(), &mut xz_buf).unwrap();
        xz_buf
    }

    /// Build a `.tar.xz` archive with a traversal entry using raw bytes.
    ///
    /// The `tar` crate sanitizes paths when using its normal builder API, so
    /// we craft the tar header manually to inject a traversal path.
    ///
    /// POSIX ustar header layout (512 bytes per record):
    ///   0-99   : file name (null-terminated)
    ///   100-107: file permissions (octal, null-terminated)
    ///   108-115: uid
    ///   116-123: gid
    ///   124-135: file size (octal, null-terminated)
    ///   136-147: mtime (octal, null-terminated)
    ///   148-155: checksum
    ///   156    : type flag ('0' = regular file)
    ///   157-256: link name
    ///   257-262: "ustar" magic
    ///   ... (rest zeroed)
    fn make_traversal_tar_xz(entry_name: &str, data: &[u8]) -> Vec<u8> {
        let mut tar_buf: Vec<u8> = Vec::new();

        // Build a 512-byte POSIX ustar header manually.
        let mut header = [0u8; 512];

        // File name (bytes 0..100, null-terminated).
        let name_bytes = entry_name.as_bytes();
        let name_len = name_bytes.len().min(99);
        header[..name_len].copy_from_slice(&name_bytes[..name_len]);

        // File mode: 0644 → "0000644\0"
        header[100..108].copy_from_slice(b"0000644\0");

        // UID/GID: zeroed.
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");

        // File size (octal, 11 chars + null).
        let size_octal = format!("{:011o}\0", data.len());
        header[124..136].copy_from_slice(size_octal.as_bytes());

        // Modification time: zeroed.
        header[136..148].copy_from_slice(b"00000000000\0");

        // Type flag: '0' = regular file.
        header[156] = b'0';

        // ustar magic.
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        // Checksum: sum of all header bytes with checksum field set to spaces.
        header[148..156].copy_from_slice(b"        ");
        let checksum: u32 = header.iter().map(|&b| b as u32).sum();
        let cksum_str = format!("{:06o}\0 ", checksum);
        header[148..156].copy_from_slice(cksum_str.as_bytes());

        tar_buf.extend_from_slice(&header);

        // File data, padded to 512-byte boundary.
        tar_buf.extend_from_slice(data);
        let padding = (512 - (data.len() % 512)) % 512;
        tar_buf.extend(std::iter::repeat_n(0u8, padding));

        // Two 512-byte zero blocks mark end of archive.
        tar_buf.extend(std::iter::repeat_n(0u8, 1024));

        let mut xz_buf = Vec::new();
        lzma_rs::xz_compress(&mut tar_buf.as_slice(), &mut xz_buf).unwrap();
        xz_buf
    }

    #[test]
    fn test_extract_tar_xz_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("test.tar.xz");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        let xz_data = make_tar_xz(&[
            ("alpha.txt", b"alpha content"),
            ("beta.txt", b"beta content"),
        ]);
        std::fs::write(&archive_path, &xz_data).unwrap();

        extract_tar_xz(&archive_path, &dest_dir, &CancellationToken::new())
            .expect("extract_tar_xz must succeed");

        let alpha = std::fs::read(dest_dir.join("alpha.txt")).unwrap();
        assert_eq!(alpha, b"alpha content");

        let beta = std::fs::read(dest_dir.join("beta.txt")).unwrap();
        assert_eq!(beta, b"beta content");
    }

    #[test]
    fn test_extract_tar_xz_missing_archive() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("no_such.tar.xz");
        let dest = tmp.path().join("out");
        let err = extract_tar_xz(&missing, &dest, &CancellationToken::new())
            .expect_err("must fail on missing archive");
        assert!(
            matches!(err, Error::Io(_)),
            "expected Io error, got {err:?}"
        );
    }

    #[test]
    fn test_extract_tar_xz_rejects_traversal() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("evil.tar.xz");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        // extract_tar_xz now iterates entries explicitly and calls
        // sanitize_entry_path on each one.  A `../` traversal entry must
        // cause an immediate Err (fail-closed), matching extract_zip.
        let xz_data = make_traversal_tar_xz("../escape.txt", b"evil content");
        std::fs::write(&archive_path, &xz_data).unwrap();

        let err = extract_tar_xz(&archive_path, &dest_dir, &CancellationToken::new())
            .expect_err("traversal entry must be rejected with Err");

        assert!(
            err.to_string().contains("traversal"),
            "error should mention traversal: {err}"
        );

        // The parent of dest_dir must not contain "escape.txt".
        let escaped = tmp.path().join("escape.txt");
        assert!(
            !escaped.exists(),
            "traversal file must not have been written: {escaped:?}"
        );
    }

    /// Build a `.tar.xz` with a symlink entry (type flag `2`).
    ///
    /// Used to verify that `extract_tar_xz` rejects symlinks fail-closed.
    fn make_symlink_tar_xz(link_name: &str, link_target: &str) -> Vec<u8> {
        let mut tar_buf: Vec<u8> = Vec::new();
        let mut header = [0u8; 512];

        // File name (bytes 0..100).
        let name_bytes = link_name.as_bytes();
        let name_len = name_bytes.len().min(99);
        header[..name_len].copy_from_slice(&name_bytes[..name_len]);

        // File mode.
        header[100..108].copy_from_slice(b"0000777\0");
        // UID/GID zeroed.
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        // File size: 0 for symlinks.
        header[124..136].copy_from_slice(b"00000000000\0");
        // Mtime.
        header[136..148].copy_from_slice(b"00000000000\0");
        // Type flag: '2' = symbolic link.
        header[156] = b'2';
        // Link target (bytes 157..257).
        let target_bytes = link_target.as_bytes();
        let target_len = target_bytes.len().min(99);
        header[157..157 + target_len].copy_from_slice(&target_bytes[..target_len]);
        // ustar magic.
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        // Checksum.
        header[148..156].copy_from_slice(b"        ");
        let checksum: u32 = header.iter().map(|&b| b as u32).sum();
        let cksum_str = format!("{:06o}\0 ", checksum);
        header[148..156].copy_from_slice(cksum_str.as_bytes());

        tar_buf.extend_from_slice(&header);
        // End-of-archive markers.
        tar_buf.extend(std::iter::repeat_n(0u8, 1024));

        let mut xz_buf = Vec::new();
        lzma_rs::xz_compress(&mut tar_buf.as_slice(), &mut xz_buf).unwrap();
        xz_buf
    }

    #[test]
    fn test_extract_tar_xz_rejects_symlink() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("symlink.tar.xz");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        // An archive with a symlink pointing outside dest_dir must be rejected.
        let xz_data = make_symlink_tar_xz("safe_name.txt", "/etc/passwd");
        std::fs::write(&archive_path, &xz_data).unwrap();

        let err = extract_tar_xz(&archive_path, &dest_dir, &CancellationToken::new())
            .expect_err("symlink entry must be rejected with Err");
        assert!(
            err.to_string().contains("symlink"),
            "error should mention symlink: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_extract_tar_xz_preserves_mode_bits() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("exec.tar.xz");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        let mut tar_buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_buf);
            let data = b"#!/bin/sh\necho hello\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755); // executable
            header.set_cksum();
            builder
                .append_data(&mut header, "bin/flutter", &data[..])
                .unwrap();
            builder.finish().unwrap();
        }
        let mut xz_buf = Vec::new();
        lzma_rs::xz_compress(&mut tar_buf.as_slice(), &mut xz_buf).unwrap();
        std::fs::write(&archive_path, &xz_buf).unwrap();

        extract_tar_xz(&archive_path, &dest_dir, &CancellationToken::new()).expect("must succeed");

        let meta = std::fs::metadata(dest_dir.join("bin/flutter")).unwrap();
        let mode = meta.permissions().mode();
        // Owner execute bit must be set.
        assert!(
            mode & 0o100 != 0,
            "owner execute bit must be preserved: mode {mode:o}"
        );
    }

    // ── extract_archive dispatch ──────────────────────────────────────────────

    #[test]
    fn test_extract_archive_dispatches_zip() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("file.zip");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        let zip_data = make_zip(&[("x.txt", b"zip dispatch")]);
        std::fs::write(&archive_path, &zip_data).unwrap();

        extract_archive(&archive_path, &dest_dir, &CancellationToken::new())
            .expect("zip dispatch must succeed");
        let x = std::fs::read(dest_dir.join("x.txt")).unwrap();
        assert_eq!(x, b"zip dispatch");
    }

    #[test]
    fn test_extract_archive_dispatches_tar_xz() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("file.tar.xz");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        let xz_data = make_tar_xz(&[("y.txt", b"tar xz dispatch")]);
        std::fs::write(&archive_path, &xz_data).unwrap();

        extract_archive(&archive_path, &dest_dir, &CancellationToken::new())
            .expect("tar.xz dispatch must succeed");
        let y = std::fs::read(dest_dir.join("y.txt")).unwrap();
        assert_eq!(y, b"tar xz dispatch");
    }

    #[test]
    fn test_extract_archive_unsupported_extension() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("file.bz2");
        let dest_dir = tmp.path().join("out");

        let err = extract_archive(&archive_path, &dest_dir, &CancellationToken::new())
            .expect_err("unsupported extension must fail");
        assert!(
            err.to_string().contains("unsupported archive format"),
            "error should mention unsupported format: {err}"
        );
    }

    // ── download_to_file (wiremock) ───────────────────────────────────────────

    #[tokio::test]
    async fn test_download_to_file_with_content_length() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let body = b"hello download world".to_vec();
        let body_len = body.len() as u64;

        Mock::given(method("GET"))
            .and(path("/flutter.zip"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(body.clone())
                    .insert_header("content-length", body_len.to_string().as_str()),
            )
            .mount(&mock_server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("flutter.zip");
        let url = format!("{}/flutter.zip", mock_server.uri());

        let mut progress_events: Vec<DownloadProgress> = Vec::new();
        download_to_file(&url, &dest, CancellationToken::new(), |p| {
            progress_events.push(p)
        })
        .await
        .expect("download must succeed");

        // File contents match
        let downloaded = std::fs::read(&dest).unwrap();
        assert_eq!(downloaded, body);

        // At least one progress event was emitted
        assert!(
            !progress_events.is_empty(),
            "must emit at least one progress event"
        );

        // Progress is monotonically increasing
        let mut last_received = 0u64;
        for p in &progress_events {
            assert!(
                p.received >= last_received,
                "received must be non-decreasing"
            );
            last_received = p.received;
        }

        // Final received == total
        let last = progress_events.last().unwrap();
        assert_eq!(last.received, body_len);
        assert_eq!(last.total, Some(body_len));
    }

    /// Verify that download works when no explicit Content-Length header is provided.
    ///
    /// Note: wiremock may or may not inject a content-length header for the response.
    /// We verify that: (a) the download succeeds and produces the correct bytes, and
    /// (b) at least one progress event is emitted with a monotonically increasing
    /// `received` value. We do not assert on `total` since that depends on the server.
    #[tokio::test]
    async fn test_download_to_file_no_explicit_content_length() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let body = b"no explicit content-length here".to_vec();

        Mock::given(method("GET"))
            .and(path("/noheader.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&mock_server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("noheader.bin");
        let url = format!("{}/noheader.bin", mock_server.uri());

        let mut progress_events: Vec<DownloadProgress> = Vec::new();
        download_to_file(&url, &dest, CancellationToken::new(), |p| {
            progress_events.push(p)
        })
        .await
        .expect("download must succeed");

        // File contents match
        let downloaded = std::fs::read(&dest).unwrap();
        assert_eq!(downloaded, body);

        // At least one progress event must have been emitted
        assert!(
            !progress_events.is_empty(),
            "must emit at least one progress event"
        );

        // Progress is monotonically non-decreasing
        let mut last_received = 0u64;
        for p in &progress_events {
            assert!(
                p.received >= last_received,
                "received must be non-decreasing"
            );
            last_received = p.received;
        }
    }

    #[tokio::test]
    async fn test_download_to_file_http_error() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/notfound.bin"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("notfound.bin");
        let url = format!("{}/notfound.bin", mock_server.uri());

        let err = download_to_file(&url, &dest, CancellationToken::new(), |_| {})
            .await
            .expect_err("HTTP 404 must return error");
        assert!(
            err.to_string().contains("404"),
            "error should mention HTTP status: {err}"
        );
    }

    #[tokio::test]
    async fn test_download_to_file_part_file_renamed_on_success() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let body = b"staged download content".to_vec();

        Mock::given(method("GET"))
            .and(path("/staged.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&mock_server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("staged.bin");
        let url = format!("{}/staged.bin", mock_server.uri());

        download_to_file(&url, &dest, CancellationToken::new(), |_| {})
            .await
            .expect("download must succeed");

        // Destination file exists with correct content.
        assert!(dest.exists(), "dest file must exist after success");
        let downloaded = std::fs::read(&dest).unwrap();
        assert_eq!(downloaded, body);

        // .part file must have been removed.
        let part = dest.with_extension("bin.part");
        assert!(
            !part.exists(),
            ".part file must not exist after success: {part:?}"
        );
    }

    #[tokio::test]
    async fn test_download_to_file_retry_on_transient_failure() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

        struct FailThenSucceed {
            attempts: Arc<AtomicU32>,
            body: Vec<u8>,
        }

        impl Respond for FailThenSucceed {
            fn respond(&self, _req: &Request) -> ResponseTemplate {
                let n = self.attempts.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    // First two attempts return a server error (retriable).
                    ResponseTemplate::new(500)
                } else {
                    // Third attempt succeeds.
                    ResponseTemplate::new(200).set_body_bytes(self.body.clone())
                }
            }
        }

        let mock_server = MockServer::start().await;
        let body = b"retry success content".to_vec();
        let attempts = Arc::new(AtomicU32::new(0));

        Mock::given(method("GET"))
            .and(path("/retry.bin"))
            .respond_with(FailThenSucceed {
                attempts: Arc::clone(&attempts),
                body: body.clone(),
            })
            .expect(3)
            .mount(&mock_server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("retry.bin");
        let url = format!("{}/retry.bin", mock_server.uri());

        download_to_file(&url, &dest, CancellationToken::new(), |_| {})
            .await
            .expect("download must succeed after retries");

        // File contents are correct.
        let downloaded = std::fs::read(&dest).unwrap();
        assert_eq!(downloaded, body);

        // Server was called exactly 3 times.
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    // ── ensure_disk_space ─────────────────────────────────────────────────────

    /// `ensure_disk_space` must succeed when `required` is 1 byte (any real
    /// temporary directory has at least 1 byte free).
    #[test]
    fn ensure_disk_space_passes_for_tempdir() {
        let dir = TempDir::new().unwrap();
        ensure_disk_space(dir.path(), 1).expect("a tempdir must have > 1 byte free");
    }

    /// `ensure_disk_space` must return an `Error::Process` whose message
    /// contains "insufficient disk space" when the required budget exceeds any
    /// real filesystem's available capacity.
    #[test]
    fn ensure_disk_space_errors_when_required_exceeds_available() {
        let dir = TempDir::new().unwrap();
        let err = ensure_disk_space(dir.path(), u64::MAX)
            .expect_err("u64::MAX bytes required must be rejected");
        assert!(
            err.to_string().contains("insufficient disk space"),
            "error message must mention 'insufficient disk space': {err}"
        );
    }

    /// `ensure_disk_space` error message must name the required and available
    /// MiB counts so the user understands the shortfall.
    #[test]
    fn ensure_disk_space_error_mentions_mib_counts() {
        let dir = TempDir::new().unwrap();
        // Request a budget larger than any filesystem could have so the check
        // always fails, regardless of the CI host's available space.
        let err = ensure_disk_space(dir.path(), u64::MAX).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("MiB"),
            "error message must include 'MiB' for human-readable counts: {msg}"
        );
    }

    // ── check_network_connectivity ────────────────────────────────────────────

    /// `check_network_connectivity` must succeed when the server is reachable.
    #[tokio::test]
    async fn check_network_connectivity_succeeds_when_reachable() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("HEAD"))
            .and(path("/probe"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/probe", mock_server.uri());

        check_network_connectivity(&client, &url)
            .await
            .expect("HEAD to reachable server must succeed");
    }

    /// `check_network_connectivity` must return a "no network connectivity"
    /// error when the URL is unreachable (e.g. connection refused).
    #[tokio::test]
    async fn check_network_connectivity_errors_when_unreachable() {
        // Use a port that is very unlikely to have a listener.
        let unreachable_url = "http://127.0.0.1:1/probe";
        let client = reqwest::Client::new();

        let err = check_network_connectivity(&client, unreachable_url)
            .await
            .expect_err("unreachable host must return error");
        assert!(
            err.to_string().contains("no network connectivity"),
            "error must mention 'no network connectivity': {err}"
        );
    }

    // ── Cancellation tests ────────────────────────────────────────────────────

    /// A pre-cancelled token must cause `download_to_file` to return
    /// `Error::Cancelled` immediately without creating any `.part` file.
    #[tokio::test]
    async fn precancelled_token_does_no_io() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("never.bin");
        let part = dest.with_extension("bin.part");

        // Cancel the token before calling download_to_file.
        let token = CancellationToken::new();
        token.cancel();

        // Use a URL that is guaranteed to refuse connections quickly so the
        // test is not sensitive to network availability.  The pre-cancel check
        // fires before any HTTP request is issued, so the URL is never reached.
        let err = download_to_file("http://127.0.0.1:1/never.bin", &dest, token, |_| {})
            .await
            .expect_err("pre-cancelled token must return Err");

        assert!(err.is_cancelled(), "error must be Cancelled, got: {err:?}");

        // Neither the dest file nor the .part file should have been created.
        assert!(
            !dest.exists(),
            "dest must not exist when cancelled before any I/O"
        );
        assert!(
            !part.exists(),
            ".part file must not exist when cancelled before any I/O"
        );
    }

    /// A token cancelled after the download starts must cause the streaming loop
    /// to exit with `Error::Cancelled` and must not leave a `.part` file behind.
    ///
    /// ## Determinism rationale (F2)
    ///
    /// The token is cancelled **synchronously from inside the first progress
    /// callback**.  After `on_progress` returns, the `select!` loop re-enters
    /// with the biased arm; `cancel.cancelled()` is already resolved, so the
    /// cancellation branch fires before `stream.next()` is polled again —
    /// regardless of whether the body arrived as one chunk or many.  This is
    /// deterministic on loopback where a 200 KiB body may arrive as a single
    /// chunk, unlike the previous design that used an external `Notify` +
    /// `token.cancel()` from a separate task (which raced against the stream
    /// exhausting before the cancel was observed).
    #[tokio::test]
    async fn cancel_mid_stream_returns_cancelled_and_cleans_part() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

        /// A responder that sends a sizeable body so at least one progress
        /// callback fires.  The exact chunk count does not matter: cancellation
        /// is set synchronously inside the callback, guaranteeing the biased
        /// `select!` arm picks it up on the next iteration.
        struct LargeBodyResponder;

        impl Respond for LargeBodyResponder {
            fn respond(&self, _req: &Request) -> ResponseTemplate {
                // 200 KiB — enough to trigger at least one progress callback.
                ResponseTemplate::new(200).set_body_bytes(vec![0u8; 200 * 1024])
            }
        }

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/large.bin"))
            .respond_with(LargeBodyResponder)
            .mount(&mock_server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("large.bin");
        let part = dest.with_extension("bin.part");
        let url = format!("{}/large.bin", mock_server.uri());

        let token = CancellationToken::new();
        let token_for_callback = token.clone();

        // Cancel the token from within the first progress callback.  This is
        // synchronous: by the time `on_progress` returns, `cancel.is_cancelled()`
        // is `true`, and the biased `select!` in the next loop iteration will
        // observe it before calling `stream.next()` again.
        let mut cancelled_in_cb = false;
        let result = download_to_file(&url, &dest, token, |_p| {
            if !cancelled_in_cb {
                cancelled_in_cb = true;
                token_for_callback.cancel();
            }
        })
        .await;

        let err = result.expect_err("cancelled download must return Err");
        assert!(err.is_cancelled(), "error must be Cancelled, got: {err:?}");

        // The .part file must have been removed by the PartFileGuard.
        assert!(
            !part.exists(),
            ".part file must not exist after cancellation: {part:?}"
        );
    }

    // ── PartFileGuard ─────────────────────────────────────────────────────────

    /// An armed `PartFileGuard` removes the file on drop.
    #[test]
    fn part_file_guard_removes_file_on_drop() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.part");
        std::fs::write(&path, b"test").unwrap();
        assert!(path.exists(), "file must exist before drop");

        {
            let guard = PartFileGuard::new(path.clone());
            // Guard is armed; drop triggers removal.
            drop(guard);
        }

        assert!(
            !path.exists(),
            ".part file must be removed by armed guard on drop"
        );
    }

    /// A disarmed `PartFileGuard` does NOT remove the file on drop.
    #[test]
    fn part_file_guard_disarmed_does_not_remove_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("keep.part");
        std::fs::write(&path, b"keep").unwrap();

        {
            let mut guard = PartFileGuard::new(path.clone());
            guard.disarm();
            // Drop should not remove the file.
            drop(guard);
        }

        assert!(
            path.exists(),
            ".part file must remain when guard is disarmed"
        );
    }

    /// A `PartFileGuard` pointing to a non-existent file must not panic on drop.
    #[test]
    fn part_file_guard_missing_file_no_panic() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.part");
        // Do not create the file; dropping the guard must not panic.
        let guard = PartFileGuard::new(path);
        drop(guard);
    }

    // ── Extraction cancellation ───────────────────────────────────────────────

    /// A pre-cancelled token passed to `extract_zip` must cause extraction to
    /// return `Error::Cancelled` at the first CANCEL_CHECK_INTERVAL boundary
    /// (index 0, since 0 % CANCEL_CHECK_INTERVAL == 0 and the token is already
    /// cancelled on entry).
    #[test]
    fn extract_zip_cancelled_token_stops_extraction() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("cancel_test.zip");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        // Build a small zip with at least one entry.
        let zip_data = make_zip(&[("file.txt", b"content"), ("file2.txt", b"content2")]);
        std::fs::write(&archive_path, &zip_data).unwrap();

        // Pre-cancel the token before passing to extract_zip.
        let token = CancellationToken::new();
        token.cancel();

        let err = extract_zip(&archive_path, &dest_dir, &token)
            .expect_err("pre-cancelled token must cause extract_zip to return Err");

        assert!(err.is_cancelled(), "error must be Cancelled, got: {err:?}");
    }

    /// A pre-cancelled token passed to `extract_tar_xz` must cause extraction
    /// to return `Error::Cancelled` at the first CANCEL_CHECK_INTERVAL boundary.
    #[test]
    fn extract_tar_xz_cancelled_token_stops_extraction() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("cancel_test.tar.xz");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        // Build a small tar.xz with at least one entry.
        let xz_data = make_tar_xz(&[("file.txt", b"content"), ("file2.txt", b"content2")]);
        std::fs::write(&archive_path, &xz_data).unwrap();

        // Pre-cancel the token before passing to extract_tar_xz.
        let token = CancellationToken::new();
        token.cancel();

        let err = extract_tar_xz(&archive_path, &dest_dir, &token)
            .expect_err("pre-cancelled token must cause extract_tar_xz to return Err");

        assert!(err.is_cancelled(), "error must be Cancelled, got: {err:?}");
    }

    /// A pre-cancelled token passed to `extract_archive` must cause extraction
    /// to return `Error::Cancelled` regardless of the archive format.
    #[test]
    fn extract_archive_cancelled_token_stops_extraction() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("cancel_test.zip");
        let dest_dir = tmp.path().join("out");
        std::fs::create_dir_all(&dest_dir).unwrap();

        let zip_data = make_zip(&[("file.txt", b"content")]);
        std::fs::write(&archive_path, &zip_data).unwrap();

        let token = CancellationToken::new();
        token.cancel();

        let err = extract_archive(&archive_path, &dest_dir, &token)
            .expect_err("pre-cancelled token must cause extract_archive to return Err");

        assert!(err.is_cancelled(), "error must be Cancelled, got: {err:?}");
    }
}

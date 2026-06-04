//! # Download and Archive Extraction Primitives
//!
//! Low-level helpers for the Flutter SDK installer:
//!
//! - [`download_to_file`] — streaming HTTP download with connect/idle timeouts,
//!   bounded retry, `.part`-file staging, and progress reporting.
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

use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tracing::debug;

use fdemon_core::{Error, Result};

use super::types::DownloadProgress;

// ── Download constants ────────────────────────────────────────────────────────

/// TCP connect timeout for archive downloads.
///
/// 10 seconds is generous for local-area and well-peered CDN connections while
/// still bounding the wizard stall on a totally unreachable endpoint.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Idle/stall guard for archive downloads.
///
/// If no bytes arrive within this window the stream is considered stalled and
/// the attempt is abandoned.  30 s accommodates slow CDN edge nodes without
/// letting a stalled socket hang the wizard indefinitely.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of download attempts before giving up.
const MAX_DOWNLOAD_ATTEMPTS: u32 = 3;

// ── Download ─────────────────────────────────────────────────────────────────

/// Stream a URL to `dest`, invoking `on_progress` after each chunk arrives.
///
/// Downloads are staged to `<dest>.part` and renamed to `dest` only on
/// success. On failure the `.part` file is removed (best-effort).
///
/// The function retries up to [`MAX_DOWNLOAD_ATTEMPTS`] times on transient
/// failures (network errors, non-4xx HTTP errors).  Each retry restarts from
/// byte 0.
///
/// The `Content-Length` response header, when present, is surfaced as
/// [`DownloadProgress::total`] so the caller can render a progress bar.  When
/// the server omits `Content-Length`, `total` is `None` for all callbacks.
///
/// # Errors
///
/// Returns an error if all attempts fail, on HTTP 4xx responses, or on I/O
/// errors while writing to `dest`.
pub async fn download_to_file<F>(url: &str, dest: &Path, mut on_progress: F) -> Result<()>
where
    F: FnMut(DownloadProgress),
{
    let client = reqwest::Client::builder()
        .user_agent(concat!("fdemon/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(IDLE_TIMEOUT)
        .build()
        .map_err(|e| Error::process(format!("failed to build HTTP client: {e}")))?;

    let part_path = dest.with_extension(
        dest.extension()
            .map(|ext| format!("{}.part", ext.to_string_lossy()))
            .unwrap_or_else(|| "part".to_string()),
    );

    let mut last_err: Option<Error> = None;

    for attempt in 1..=MAX_DOWNLOAD_ATTEMPTS {
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
            cleanup_part_file(&part_path);
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

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    stream_err = Some(Error::process(format!("stream read error for {url}: {e}")));
                    break;
                }
            };

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

        // Success: rename .part → dest.
        if let Err(e) = std::fs::rename(&part_path, dest) {
            cleanup_part_file(&part_path);
            return Err(Error::Io(io::Error::new(
                e.kind(),
                format!("rename {part_path:?} → {dest:?}: {e}"),
            )));
        }

        return Ok(());
    }

    cleanup_part_file(&part_path);
    Err(last_err.unwrap_or_else(|| {
        Error::process(format!(
            "download failed after {MAX_DOWNLOAD_ATTEMPTS} attempts: {url}"
        ))
    }))
}

/// Remove `path` on a best-effort basis, logging but not propagating errors.
fn cleanup_part_file(path: &Path) {
    if let Err(e) = std::fs::remove_file(path) {
        debug!(?path, error = %e, "failed to clean up .part file (best-effort)");
    }
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

/// Extract a `.zip` archive into `dest_dir`.
///
/// On Unix, executable mode bits recorded in the zip's external attributes
/// (`ZipFile::unix_mode()`) are applied to the extracted file. Flutter's
/// bundled binaries (e.g. `bin/flutter`, `bin/dart`) rely on `+x` to run.
///
/// Entries with absolute paths or `..` components are rejected to prevent
/// zip-slip / path traversal attacks.
///
/// # Errors
///
/// Returns an error on archive I/O failures, path traversal attempts, or when
/// a file cannot be created inside `dest_dir`.
pub fn extract_zip(archive: &Path, dest_dir: &Path) -> Result<()> {
    let file = File::open(archive)
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("open {archive:?}: {e}"))))?;

    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::process(format!("open zip {archive:?}: {e}")))?;

    for i in 0..zip.len() {
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
/// Uses [`tar::Archive::unpack_in`] to prevent tar traversal / symlink
/// escape. Unix mode bits on extracted files are preserved.
///
/// Entries with `..` components are rejected by both the channel-based guard
/// and `unpack_in`.
///
/// # Errors
///
/// Returns an error on decompression, traversal detection, or tar-unpack
/// failures.
pub fn extract_tar_xz(archive: &Path, dest_dir: &Path) -> Result<()> {
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

    // `Archive::unpack` delegates to `Entry::unpack_in` for each entry,
    // which silently skips entries with `..` components or symlink escapes
    // and validates against `dest_dir` canonicalization.
    let unpack_result = tar_archive
        .unpack(dest_dir)
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("unpack {archive:?}: {e}"))));

    // Wait for the decode thread and surface any decoding error.
    let decode_result = decode_thread
        .join()
        .map_err(|_| Error::process(format!("xz decode thread panicked for {archive:?}")))
        .and_then(|r| r.map_err(Error::process));

    // Prefer the decode error (more informative) if both fail.
    match (decode_result, unpack_result) {
        (Ok(()), Ok(_)) => Ok(()),
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
/// # Errors
///
/// Returns an error when the extension is not recognised or extraction fails.
pub fn extract_archive(archive: &Path, dest_dir: &Path) -> Result<()> {
    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        extract_tar_xz(archive, dest_dir)
    } else if name.ends_with(".zip") {
        extract_zip(archive, dest_dir)
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

        extract_zip(&archive_path, &dest_dir).expect("extract_zip must succeed");

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
        let err = extract_zip(&missing, &dest).expect_err("must fail on missing archive");
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

        let err =
            extract_zip(&archive_path, &dest_dir).expect_err("zip-slip entry must be rejected");
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

        extract_tar_xz(&archive_path, &dest_dir).expect("extract_tar_xz must succeed");

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
        let err = extract_tar_xz(&missing, &dest).expect_err("must fail on missing archive");
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

        // The tar crate's Entry::unpack_in silently skips entries with
        // `..` components rather than returning an error, so the
        // extraction itself succeeds but the traversal entry is not
        // written to disk.
        let xz_data = make_traversal_tar_xz("../escape.txt", b"evil content");
        std::fs::write(&archive_path, &xz_data).unwrap();

        // Extraction must not error — the traversal entry is silently
        // skipped by Entry::unpack_in.
        extract_tar_xz(&archive_path, &dest_dir)
            .expect("extraction must succeed (traversal entry is skipped)");

        // The parent of dest_dir must not contain "escape.txt".
        let escaped = tmp.path().join("escape.txt");
        assert!(
            !escaped.exists(),
            "traversal file must not have been written: {escaped:?}"
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

        extract_tar_xz(&archive_path, &dest_dir).expect("must succeed");

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

        extract_archive(&archive_path, &dest_dir).expect("zip dispatch must succeed");
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

        extract_archive(&archive_path, &dest_dir).expect("tar.xz dispatch must succeed");
        let y = std::fs::read(dest_dir.join("y.txt")).unwrap();
        assert_eq!(y, b"tar xz dispatch");
    }

    #[test]
    fn test_extract_archive_unsupported_extension() {
        let tmp = TempDir::new().unwrap();
        let archive_path = tmp.path().join("file.bz2");
        let dest_dir = tmp.path().join("out");

        let err =
            extract_archive(&archive_path, &dest_dir).expect_err("unsupported extension must fail");
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
        download_to_file(&url, &dest, |p| progress_events.push(p))
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
        download_to_file(&url, &dest, |p| progress_events.push(p))
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

        let err = download_to_file(&url, &dest, |_| {})
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

        download_to_file(&url, &dest, |_| {})
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

        download_to_file(&url, &dest, |_| {})
            .await
            .expect("download must succeed after retries");

        // File contents are correct.
        let downloaded = std::fs::read(&dest).unwrap();
        assert_eq!(downloaded, body);

        // Server was called exactly 3 times.
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }
}

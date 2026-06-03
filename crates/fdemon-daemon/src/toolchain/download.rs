//! # Download and Archive Extraction Primitives
//!
//! Low-level helpers for the Flutter SDK installer:
//!
//! - [`download_to_file`] — streaming HTTP download with progress reporting
//!   and optional SHA-256 post-verification.
//! - [`verify_sha256`] — synchronous SHA-256 checksum verification.
//! - [`extract_zip`] — extract a `.zip` archive, preserving unix mode bits.
//! - [`extract_tar_xz`] — extract a `.tar.xz` archive using pure-Rust decoders.
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

use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::Path;

use futures_util::StreamExt;
use sha2::{Digest, Sha256};

use fdemon_core::{Error, Result};

use super::types::DownloadProgress;

// ── Download ─────────────────────────────────────────────────────────────────

/// Stream a URL to `dest`, invoking `on_progress` after each chunk arrives.
///
/// The `Content-Length` response header, when present, is surfaced as
/// [`DownloadProgress::total`] so the caller can render a progress bar.  When
/// the server omits `Content-Length`, `total` is `None` for all callbacks.
///
/// The destination file is created (or truncated) at `dest`. The caller is
/// responsible for atomic-move / cleanup on error.
///
/// # Errors
///
/// Returns an error on any network failure, HTTP non-2xx response, or I/O
/// error while writing to `dest`.
pub async fn download_to_file<F>(url: &str, dest: &Path, mut on_progress: F) -> Result<()>
where
    F: FnMut(DownloadProgress),
{
    let client = reqwest::Client::builder()
        .user_agent(concat!("fdemon/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| Error::process(format!("failed to build HTTP client: {e}")))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| Error::process(format!("HTTP request failed for {url}: {e}")))?;

    if !response.status().is_success() {
        return Err(Error::process(format!(
            "HTTP {} for {url}",
            response.status()
        )));
    }

    let total = response.content_length();
    let mut received: u64 = 0;

    let mut file = File::create(dest)
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("create {dest:?}: {e}"))))?;

    let mut stream = response.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result
            .map_err(|e| Error::process(format!("stream read error for {url}: {e}")))?;

        file.write_all(&chunk)
            .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("write to {dest:?}: {e}"))))?;

        received += chunk.len() as u64;
        on_progress(DownloadProgress { received, total });
    }

    file.flush()
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("flush {dest:?}: {e}"))))?;

    Ok(())
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

// ── ZIP Extraction ────────────────────────────────────────────────────────────

/// Extract a `.zip` archive into `dest_dir`.
///
/// On Unix, executable mode bits recorded in the zip's external attributes
/// (`ZipFile::unix_mode()`) are applied to the extracted file. Flutter's
/// bundled binaries (e.g. `bin/flutter`, `bin/dart`) rely on `+x` to run.
///
/// # Errors
///
/// Returns an error on archive I/O failures or when a file cannot be created
/// inside `dest_dir`.
pub fn extract_zip(archive: &Path, dest_dir: &Path) -> Result<()> {
    let file = File::open(archive)
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("open {archive:?}: {e}"))))?;

    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::process(format!("open zip {archive:?}: {e}")))?;

    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| Error::process(format!("zip entry {i} in {archive:?}: {e}")))?;

        let out_path = dest_dir.join(entry.name());

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

/// Extract a `.tar.xz` archive into `dest_dir` using pure-Rust decoders.
///
/// Uses `lzma-rs` for XZ decompression and the `tar` crate for archive
/// unpacking. No C library dependencies.
///
/// # Errors
///
/// Returns an error on decompression or tar-unpack failures.
pub fn extract_tar_xz(archive: &Path, dest_dir: &Path) -> Result<()> {
    let file = File::open(archive)
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("open {archive:?}: {e}"))))?;

    let mut reader = BufReader::new(file);
    let mut decoded: Vec<u8> = Vec::new();

    lzma_rs::xz_decompress(&mut reader, &mut decoded)
        .map_err(|e| Error::process(format!("xz decompress {archive:?}: {e}")))?;

    let mut tar_archive = tar::Archive::new(decoded.as_slice());
    tar_archive
        .unpack(dest_dir)
        .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("unpack {archive:?}: {e}"))))?;

    Ok(())
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
}

## Task: Streaming download + archive extraction + process streaming

**Objective**: Provide the low-level I/O primitives the Flutter installer needs:
a streaming HTTP download with progress + SHA-256 verification, zip and tar.xz
extractors, and a child-process runner that streams stdout/stderr lines back
through a callback.

**Depends on**: 01

**Agent:** implementor

**Estimated Time**: 5-6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/download.rs` — **NEW**
- `crates/fdemon-daemon/src/toolchain/process_stream.rs` — **NEW**
- `crates/fdemon-daemon/src/toolchain/mod.rs` — add `mod download;`,
  `mod process_stream;` and re-export the public helpers.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `DownloadProgress`.
- `crates/fdemon-daemon/src/native_logs/` — reference for the existing
  child-process + line-streaming patterns (tokio `Command`, `BufReader::lines`).

### Details

**`download.rs`** — async, callback-based progress (no UI types here):

```rust
/// Stream a URL to `dest`, invoking `on_progress` as bytes arrive.
pub async fn download_to_file<F: FnMut(DownloadProgress)>(
    url: &str,
    dest: &Path,
    mut on_progress: F,
) -> Result<()> { /* reqwest::get → bytes_stream() → write chunks, track received/total */ }

/// Verify a file's SHA-256 against an expected lowercase hex digest.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<()> { /* sha2::Sha256 streaming */ }

/// Extract a .zip into `dest_dir`.
pub fn extract_zip(archive: &Path, dest_dir: &Path) -> Result<()> { /* zip::ZipArchive */ }

/// Extract a .tar.xz into `dest_dir` (pure-Rust xz via lzma-rs → tar).
pub fn extract_tar_xz(archive: &Path, dest_dir: &Path) -> Result<()> {
    // lzma_rs::xz_decompress(reader, &mut decoded) → tar::Archive::new(decoded).unpack(dest_dir)
}

/// Detect archive kind from extension and dispatch to the right extractor.
pub fn extract_archive(archive: &Path, dest_dir: &Path) -> Result<()> { ... }
```

Notes:
- Use the workspace `Error` enum + `Result<T>` alias; map reqwest/io/zip errors
  with `.context()`. Never `unwrap()`.
- Heavy CPU work (extraction, sha256) must run under `tokio::task::spawn_blocking`
  *at the call site* (task 03), so keep `extract_*`/`verify_sha256` as plain sync
  functions; only `download_to_file` is async.
- Reuse `futures_util::StreamExt` for `bytes_stream()`.

**`process_stream.rs`** — stream a child process's combined output line-by-line:

```rust
/// Run a command, forwarding each stdout/stderr line to `on_line`, and return
/// the exit status. Used for `git clone`, `flutter precache`, and (Phase 3)
/// `sdkmanager`.
pub async fn run_streaming<F: FnMut(String) + Send>(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    mut on_line: F,
) -> Result<std::process::ExitStatus> {
    // tokio::process::Command, piped stdout+stderr, merge lines, await child.
}
```

Notes:
- Merge stdout + stderr so progress lines from `git`/`flutter` (which often write
  to stderr) are not lost.
- The callback runs on the spawned task; the caller (task 08) bridges lines into
  `Message::WizardStepLog` via an `mpsc::Sender`.

### Acceptance Criteria

1. `download_to_file` streams to disk and reports cumulative `received` plus
   `total` (from `Content-Length` when present, else `None`).
2. `verify_sha256` returns `Ok` on match and a typed `Error` on mismatch.
3. `extract_zip` and `extract_tar_xz` round-trip a known fixture into the expected
   file tree; `extract_archive` dispatches by extension.
4. `run_streaming` invokes the callback once per output line and returns the exit
   status; a non-zero exit is surfaced to the caller (not swallowed).
5. New public functions are documented and unit-tested. No clippy warnings.

### Testing

```rust
#[test]
fn test_verify_sha256_match_and_mismatch() { /* hash a temp file, compare */ }

#[test]
fn test_extract_zip_roundtrip() {
    // build an in-memory zip with `zip::ZipWriter`, write to tempdir, extract, assert files
}

#[test]
fn test_extract_tar_xz_roundtrip() {
    // create tar, xz-compress test bytes, extract, assert contents
}

#[tokio::test]
async fn test_run_streaming_captures_lines_and_status() {
    // run `echo`-equivalent (e.g. a small `printf` via sh -c, or `git --version`) and
    // assert at least one line + success status. Gate platform-specific commands.
}
```

For `download_to_file`, prefer a `wiremock` (already a dev-dep) server serving a
small body with a `Content-Length` header, asserting progress monotonicity.

### Notes

- Keep all networking inside `toolchain/` per the layering decision.
- Temp-dir + atomic-move orchestration lives in task 03, not here — these helpers
  operate on caller-provided paths.
- `extract_zip` must preserve unix executable bits where the zip records them
  (Flutter/cmdline-tools rely on `+x` on `bin/*`). Set mode from
  `ZipFile::unix_mode()` on unix.

---

## Completion Summary

**Status:** Not Started
</content>

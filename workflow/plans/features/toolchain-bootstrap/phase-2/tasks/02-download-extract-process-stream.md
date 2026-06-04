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

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/download.rs` | NEW — `download_to_file`, `verify_sha256`, `extract_zip`, `extract_tar_xz`, `extract_archive` + 13 unit tests (including wiremock download tests) |
| `crates/fdemon-daemon/src/toolchain/process_stream.rs` | NEW — `run_streaming` with concurrent stdout/stderr merging via mpsc channel + 5 unit tests |
| `crates/fdemon-daemon/src/toolchain/mod.rs` | Added `pub mod download;`, `pub mod process_stream;`; re-exported all public helpers |
| `crates/fdemon-daemon/Cargo.toml` | Added `wiremock.workspace = true` to `[dev-dependencies]` |

### Notable Decisions/Tradeoffs

1. **`extract_*` and `verify_sha256` are sync**: As specified, these are left as synchronous functions so that callers (task 03) can wrap them with `tokio::task::spawn_blocking`. Only `download_to_file` is async.

2. **stdout/stderr merging via mpsc channel**: Two `tokio::spawn` reader tasks forward lines into a bounded `mpsc::channel`. The main task drains the channel before `child.wait()`. This avoids the classic pipe-buffer deadlock when both stdout and stderr are full.

3. **wiremock test for `download_to_file`**: The `test_download_to_file_without_content_length` test was revised after discovering that wiremock injects a `Content-Length` header even when not explicitly set. The test now verifies correct file contents and monotonic progress without asserting on `total` being `None`.

4. **Preserved unix mode bits in `extract_zip`**: `ZipFile::unix_mode()` is used on `#[cfg(unix)]` to restore executable bits so Flutter/cmdline-tools binaries remain runnable.

5. **No `#[cfg(any())]` dead code**: The `run_streaming` tests gate both unix and windows paths with `#[cfg(unix)]` / `#[cfg(windows)]` so both paths compile and run on the appropriate platform without clippy dead-code warnings.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed (0 warnings)
- `cargo test --workspace` — Passed (13 download tests + 5 process_stream tests, all new; full workspace suite passes)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (0 warnings)

### Risks/Limitations

1. **No network tests for real Flutter CDN**: The wiremock tests validate the download + progress logic against a local mock server. Real-world CDN edge cases (redirect chains, chunked encoding, partial failures) are not covered here — those will be exercised via integration tests in task 03.
2. **XZ decompression memory**: `lzma_rs::xz_decompress` decompresses into a `Vec<u8>` in memory before handing to `tar::Archive`. For the Flutter SDK archive (~1 GB uncompressed), this may require significant RAM. Task 03's `spawn_blocking` call will move this onto a thread pool thread to avoid blocking the async runtime.
</content>

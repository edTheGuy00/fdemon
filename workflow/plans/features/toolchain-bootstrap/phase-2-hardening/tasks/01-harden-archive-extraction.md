# Task 01 — Harden `download.rs`: extraction safety + network robustness

**Agent:** implementor
**Status:** Not Started
**Depends On:** -
**Estimated Hours:** 4-5h
**Module:** `crates/fdemon-daemon/src/toolchain/download.rs`

## Context

`download.rs` provides the download + archive-extraction primitives for the managed
Flutter installer. The Phase 2 review found a **CRITICAL zip-slip** vulnerability, a
**MAJOR tar traversal/symlink** gap, an unbounded in-memory `.tar.xz` decode, and no
download timeout/retry. These all live in this one file and are fixed together.

References: `workflow/reviews/features/toolchain-bootstrap-phase-2/ACTION_ITEMS.md`
(C1, M1, M7, M6a) and `REVIEW.md`.

## Findings to Fix

### C1 — Zip-slip / path traversal (CRITICAL) — `extract_zip`, ~line 158
`let out_path = dest_dir.join(entry.name());` is used to create files without verifying
the result stays inside `dest_dir`. A tampered archive entry named `../../.bashrc` or an
absolute path escapes the destination and overwrites arbitrary user files.

**Fix:** Compute a sanitized output path and reject traversal. Reject any entry whose
name contains a `..` path component or is absolute; additionally assert the normalized
`out_path` starts with `dest_dir`. Apply the guard to **both** the directory-create and
file-create branches. Return `Error::process` with the offending entry name on rejection.

### M1 — Tar traversal / symlink follow (MAJOR security) — `extract_tar_xz`, ~line 227
Uses `tar::Archive::unpack(dest_dir)`. The plain `unpack` is weaker against traversal and
follows symlink entries, which can redirect later entries outside `dest_dir`.

**Fix:** Use `tar::Archive::unpack_in(dest_dir)` (returns an error if an entry would
escape). Explicitly set `archive.set_preserve_permissions(true)` (Flutter binaries need
`+x`) and `archive.set_unpack_xattrs(false)`. Do not follow symlinks out of the
destination. If `unpack_in` is unavailable in the pinned `tar` version, iterate entries
and validate each `entry.path()` against the same traversal guard used for zip.

### M7 — Full-archive in-memory xz decode (MAJOR) — `extract_tar_xz`, ~line 222
`lzma_rs::xz_decompress(&mut reader, &mut decoded: Vec<u8>)` buffers the entire
decompressed tar (~1GB+ for the Flutter SDK) in RAM before unpacking → OOM on
RAM-constrained hosts (exactly the bare Linux/container environments that hit the archive
path because `git` is absent).

**Fix:** Stream the decode into the tar reader. Use `lzma-rs`'s streaming decoder
(`lzma_rs::xz::XzDecoder` / `XzStreamDecoder`, which implements `Read`) wrapping the
file `BufReader`, and hand that directly to `tar::Archive::new(decoder)`. The `stream`
feature for `lzma-rs` is already enabled (added in Phase 2 task 01). If a streaming API
is genuinely unavailable, document a minimum-RAM limitation in the module doc and keep
buffering — but prefer streaming.

### M6a — Download timeout / retry / partial-file cleanup (MAJOR) — `download_to_file`, ~line 47
The `reqwest::Client` has no timeout; a stalled socket hangs the wizard indefinitely, and
a single dropped stream aborts the whole install leaving a partial file at `dest`.

**Fix:**
- Add `.connect_timeout(...)` and a read/idle `.timeout(...)` to the client builder
  (mirror the 3s-discipline documented for `version_check.rs`; pick a generous overall
  budget appropriate for large downloads, e.g. connect 10s, no hard overall cap but an
  idle/stall guard — use a named `const`).
- Download to `<dest>.part` and `std::fs::rename` to `dest` only on success; remove the
  `.part` file on failure (best-effort, `debug!`-logged).
- Wrap the request+stream in a bounded retry (e.g. `const MAX_DOWNLOAD_ATTEMPTS: u32 = 3;`)
  with a short backoff. Retries restart from byte 0 (resume is out of scope). Each retry
  re-truncates the `.part` file. Hand-roll the loop — **do not add a retry crate.**

## Acceptance Criteria

- [ ] `extract_zip` rejects an entry with a `..` component or absolute path and writes
      nothing outside `dest_dir`; a unit test using a hand-built malicious zip asserts
      `Err` and an empty/untouched parent dir.
- [ ] `extract_tar_xz` uses traversal-safe unpacking and does not escape `dest_dir`; a
      traversal fixture test passes. Unix mode bits on `bin/flutter` are still preserved.
- [ ] `extract_tar_xz` streams the xz decode (no full `Vec<u8>` of the decompressed tar).
- [ ] `download_to_file` has connect/idle timeouts (named consts) and a bounded retry;
      downloads land via a `.part` file renamed on success; partial files are cleaned up.
- [ ] All existing `download.rs` tests still pass (wiremock download, SHA round-trip, zip
      and tar.xz round-trip, dispatch-by-extension).
- [ ] New tests: zip-slip rejection, tar traversal rejection, retry-on-transient-failure
      (wiremock returning an error then success), `.part`-renamed-on-success.
- [ ] Public function signatures unchanged (so Task 02 and Phase 2 task 03 callers keep
      compiling): `download_to_file`, `verify_sha256`, `extract_zip`, `extract_tar_xz`,
      `extract_archive` keep their current signatures.
- [ ] `cargo fmt`, `cargo check --workspace --all-targets`, `cargo test -p fdemon-daemon`,
      `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Notes

- Keep the public API stable — only internal behavior changes. This guarantees zero
  write-overlap with Task 02 (which only *reads* these functions).
- Reuse the workspace `Error`/`Result` types; no `unwrap()` in non-test code; `tracing`
  for the best-effort cleanup logs.
- A robust traversal-guard helper (used by both zip and tar paths) can live as a private
  `fn sanitize_entry_path(dest_dir, raw) -> Result<PathBuf>` in this module.

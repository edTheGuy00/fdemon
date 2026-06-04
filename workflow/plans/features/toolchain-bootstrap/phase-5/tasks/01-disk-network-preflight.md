## Task: Disk-space + network preflight before large downloads

**Objective**: Add a free-disk-space check and a fast network-reachability probe
before fdemon starts a large download/extract, so the user gets an immediate, clear
error instead of a mid-extraction disk-full failure or a 90-second offline stall.

**Depends on**: None

**Estimated Time**: 4-6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/Cargo.toml`: add `fs4` (workspace or direct dep).
- `crates/fdemon-daemon/src/toolchain/download.rs`: add `ensure_disk_space()` and
  `check_network_connectivity()` helpers; call the space check before the first
  download attempt; expose a reusable connectivity probe.
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs`: HEAD-probe the manifest
  host before `fetch_release_manifest`; space-check the install dir before extract.
- `crates/fdemon-daemon/src/toolchain/android_install.rs`: space-check the SDK root
  before cmdline-tools extract.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/mod.rs`: error/result conventions.
- `crates/fdemon-core/src/error.rs`: `Error::process` constructor for surfaced errors.
- root `Cargo.toml`: workspace dependency table.

### Details

**Disk space — use `fs4`, not `sysinfo`.** Research: `fs4` (v1.x) has only 2 direct
deps (`rustix`, `windows-sys`), no `libc`, cross-platform (`statvfs` /
`GetDiskFreeSpaceExW`). `sysinfo` is the wrong tool (telemetry-oriented, dozens of
deps); `fs2` is unmaintained. Verify `windows-sys` major version aligns with what
the workspace already pulls (risk: duplicate major versions).

```rust
// download.rs
use std::path::Path;

/// Error if the filesystem holding `dir` has fewer than `required` free bytes.
fn ensure_disk_space(dir: &Path, required: u64) -> Result<()> {
    let avail = fs4::available_space(dir)
        .map_err(|e| Error::process(format!("disk-space probe failed for {}: {e}", dir.display())))?;
    if avail < required {
        return Err(Error::process(format!(
            "insufficient disk space in {}: need ~{} MiB, have {} MiB",
            dir.display(), required / 1_048_576, avail / 1_048_576
        )));
    }
    Ok(())
}
```

- `required` comes from the archive `Content-Length` (extend the existing HEAD/GET
  to read it) or a conservative known-size constant for the Flutter archive
  (~300 MiB compressed, budget ~1.5 GiB for the extracted SDK + precache). Document
  the margin chosen.
- Call `ensure_disk_space(install_dir, required)` in `download_to_file` after the
  `.part` path is resolved but before attempt 1, and again before each extraction
  call in `flutter_install.rs` / `android_install.rs` (extracted size >> archive).

**Network preflight — fast HEAD.** Today an offline user waits `IDLE_TIMEOUT`
(30s) × `MAX_DOWNLOAD_ATTEMPTS` (3) = 90s before feedback.

```rust
// download.rs — reuse the existing reqwest::Client
async fn check_network_connectivity(client: &reqwest::Client, url: &str) -> Result<()> {
    client.head(url).timeout(Duration::from_secs(5)).send().await
        .map(|_| ())
        .map_err(|e| Error::process(format!("no network connectivity: cannot reach {url} ({e})")))
}
```

- Call it once in `fetch_release_manifest` against the manifest URL before the full
  GET. Skip a second probe in the archive path if the manifest fetch already
  succeeded (it proves connectivity) — keep it cheap.

**Folded test gap (from audit):** add wiremock error-path tests for
`fetch_release_manifest` — one returning HTTP 404, one returning malformed JSON —
asserting the distinct error messages. `wiremock` is already a dev-dependency
(used by `test_fetch_release_manifest_with_mock_server`).

### Acceptance Criteria

1. `download_to_file` and both installers refuse to start when free space on the
   target filesystem is below the required budget, returning an `Error::process`
   naming required vs available MiB.
2. `fetch_release_manifest` performs a ≤5s HEAD probe first; an unreachable host
   yields a "no network connectivity" error in well under 90s.
3. `fs4` is added to `fdemon-daemon` and the workspace builds with no duplicate
   `windows-sys` major version (`cargo tree -d` clean for that crate).
4. wiremock tests cover the manifest 404 and malformed-JSON error paths.

### Testing

```rust
#[test]
fn ensure_disk_space_passes_for_tempdir() {
    let dir = tempfile::tempdir().unwrap();
    ensure_disk_space(dir.path(), 1).unwrap(); // a tempdir has > 1 byte free
}

#[test]
fn ensure_disk_space_errors_when_required_exceeds_available() {
    let dir = tempfile::tempdir().unwrap();
    let err = ensure_disk_space(dir.path(), u64::MAX).unwrap_err();
    assert!(err.to_string().contains("insufficient disk space"));
}

#[tokio::test]
async fn fetch_manifest_404_is_clear_error() { /* wiremock 404 -> assert message */ }
#[tokio::test]
async fn fetch_manifest_malformed_json_is_clear_error() { /* wiremock bad body */ }
```

### Notes

- Keep both checks **best-effort and fail-clear**: never panic; a probe-API failure
  (e.g. `fs4` returns `Err` on an exotic FS) should surface a readable error, not
  abort the process.
- Do **not** add config toggles this phase (a future "skip preflight" flag is an
  enhancement). No `CONFIGURATION.md` change.
- This task only touches the daemon download trio + Cargo; it shares those files
  with task 02 (abort), so the 01→02 ordering is mandatory.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/toolchain-bootstrap

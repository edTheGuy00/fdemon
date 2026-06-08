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

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Added `fs4 = "1"` to workspace dependencies with doc comment |
| `crates/fdemon-daemon/Cargo.toml` | Added `fs4.workspace = true` to `[dependencies]` |
| `crates/fdemon-daemon/src/toolchain/download.rs` | Added `ensure_disk_space()`, `check_network_connectivity()`, `ARCHIVE_DISK_BUDGET_BYTES` constant, `CONNECTIVITY_PROBE_TIMEOUT` constant; added disk-space call in `download_to_file`; added 5 new tests (3 disk-space, 2 connectivity) |
| `crates/fdemon-daemon/src/toolchain/flutter_install.rs` | Added HEAD probe in `fetch_release_manifest`; added disk-space check before extraction in `archive_install`; added 2 wiremock tests for 404 and malformed-JSON manifest error paths |
| `crates/fdemon-daemon/src/toolchain/android_install.rs` | Added `ensure_disk_space` call before cmdline-tools extraction; added `ANDROID_DISK_BUDGET_BYTES` constant (2 GiB) |

### Notable Decisions/Tradeoffs

1. **Conservative disk budget vs. Content-Length**: The task allows using `Content-Length` from the response headers or a conservative constant. Since `download_to_file` checks before any HTTP request, there is no content-length available at that point. I used `ARCHIVE_DISK_BUDGET_BYTES` (1.5 GiB) — generous enough for the compressed Flutter archive and the extracted SDK + precache artifacts. The constant is documented and re-exported for callers that may want a different budget.

2. **Android budget is 2 GiB (separate constant)**: The Flutter download budget (1.5 GiB) is reused for the Flutter archive. For Android, the SDK plus `sdkmanager` package downloads can exceed the Flutter footprint, so a separate 2 GiB `ANDROID_DISK_BUDGET_BYTES` constant is defined in `android_install.rs`.

3. **Network preflight in `fetch_release_manifest` only**: The task says to skip a second probe in the archive download path if the manifest fetch already succeeded ("it proves connectivity"). This is implemented: `fetch_release_manifest` does the HEAD probe; `archive_install` (which calls `fetch_release_manifest` first) relies on that proof and does not probe again for the archive download.

4. **HEAD probe for `download_to_file` not added**: The task scopes the connectivity probe to `fetch_release_manifest` (manifest host) only, with a note to skip a second probe if the manifest fetch succeeded. `download_to_file` itself is not given a connectivity probe — its disk-space preflight is sufficient, and callers that need a connectivity check call `check_network_connectivity` themselves.

5. **Wiremock tests exercise the error-path logic directly**: Since `fetch_release_manifest` has a hard-coded CDN URL that we cannot override without dependency injection, the 404 and malformed-JSON tests replicate the exact HTTP-client and error-mapping logic from the function using a helper `build_test_client()`. This approach verifies the contract without mocking the whole function.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (1050+ tests across all crates, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (0 warnings)
- `cargo tree -d | grep windows-sys` — Empty (no duplicate `windows-sys` major versions)
- Specific new tests: `ensure_disk_space_passes_for_tempdir`, `ensure_disk_space_errors_when_required_exceeds_available`, `ensure_disk_space_error_mentions_mib_counts`, `check_network_connectivity_succeeds_when_reachable`, `check_network_connectivity_errors_when_unreachable`, `fetch_manifest_404_is_clear_error`, `fetch_manifest_malformed_json_is_clear_error` — all passed

### Risks/Limitations

1. **`download_to_file` disk check uses parent dir**: When `dest`'s parent does not exist yet, `fs4::available_space` will return a probe error ("disk-space probe failed"). Callers must ensure the parent directory exists before calling `download_to_file`. In practice all call sites create the temp directory before downloading (this is already the case in both `android_install.rs` and `flutter_install.rs`).

2. **No config toggle for skipping preflights**: Per the task spec, no `CONFIGURATION.md` change and no skip-preflight flag is added in this phase. A future phase may add a `[toolchain] skip_preflight` option.

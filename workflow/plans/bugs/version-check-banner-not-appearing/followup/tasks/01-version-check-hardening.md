## Task: Version-Check Cache & Fetch Hardening

**Objective**: Harden `version_check.rs` against malformed/oversized cache files (S1), eliminate
the predictable atomic-write temp-file name (S2), add a regression test that the HTTP client
rejects redirects (C6), and rename a misleading test (C8). No behavior change for valid inputs.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/version_check.rs`: add a cache-read size cap, a unique temp-file name,
  a redirect-rejection test, and a test rename.

**Files Read (Dependencies):**
- None.

### Details

#### S1 (MEDIUM) — Cap cache file size before reading

`read_cache_at` currently does `std::fs::read(path)` then `serde_json::from_slice` with no size
pre-check (`version_check.rs:88-91`). The network path already caps at `MAX_RESPONSE_BYTES`
(512 KiB); the cache read path has no parallel guard. Add a metadata size check before the read so
a corrupt/adversarial cache file cannot trigger an unbounded allocation at startup.

```rust
/// Maximum allowed on-disk cache file size. A well-formed entry is a few
/// hundred bytes; this cap mirrors the network-side MAX_RESPONSE_BYTES guard
/// and prevents a corrupt or hostile cache file from causing a large
/// allocation at startup. (1 MiB — generous vs. the real ~200-byte payload.)
const MAX_CACHE_BYTES: u64 = 1024 * 1024;

pub(crate) fn read_cache_at(path: &Path) -> Option<CacheEntry> {
    // Reject oversized files before reading them into memory.
    let len = std::fs::metadata(path).ok()?.len();
    if len > MAX_CACHE_BYTES {
        tracing::debug!(
            "Version check: cache file too large ({} bytes) at {:?}, treating as miss",
            len,
            path
        );
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}
```

Keep the existing miss-on-error semantics: an oversized file is a cache miss, not an error.

#### S2 (MEDIUM/low-risk) — Unique atomic-write temp name

`write_cache_at` writes to `path.with_extension("tmp")` (`version_check.rs:129`) — a fixed,
predictable sibling (`version_check.tmp`). `check_for_newer_release` runs exactly once per process
(fire-and-forget spawn), so a concurrent-write collision cannot happen today; this is
defense-in-depth. Give the temp file a per-process-unique name so the rename is robust even if a
second writer is ever introduced, and so a stale/foreign `version_check.tmp` cannot interfere.

```rust
// Derive a per-process-unique temp path so concurrent or stale writers cannot
// collide on a fixed `.tmp` name. The temp file is renamed over `path` on
// success and removed on failure, so it never lingers.
let tmp_path = path.with_extension(format!("{}.tmp", std::process::id()));
```

Keep the rest of the write/rename/cleanup logic unchanged (including the Windows
`remove_file(path)` pre-step). Ensure the failure path still removes *this* temp file.

> If you judge the unique-name change not worth the churn, you MUST instead record an explicit
> "S2 deferred — single-writer guarantee, see REVIEW.md" note in the Completion Summary. Do not
> silently skip it.

#### C6 — Redirect-rejection test

The client sets `reqwest::redirect::Policy::none()` (`version_check.rs:183`) but no test exercises
it. Add a `wiremock` test (mirroring the existing `fetch_latest_tag` tests in the `#[cfg(test)]`
module) that serves a `301`/`302` and asserts `fetch_latest_tag` returns `None`, locking in the
no-follow defense.

#### C8 — Rename misleading test

`write_stores_raw_tag_not_result` (`version_check.rs:735`) drives `fetch_latest_tag` +
`write_cache_at` directly rather than `check_for_newer_release`. Rename it to describe what it
actually verifies, e.g. `cache_always_stores_raw_tag_on_successful_fetch`. (Optional, only if
cheap: add an end-to-end `check_for_newer_release` test — but the rename alone satisfies C8.)

### Acceptance Criteria

1. `read_cache_at` returns `None` (a cache miss, no panic) for a file larger than `MAX_CACHE_BYTES`,
   verified by a new unit test using a `tempdir()` file padded past the cap.
2. A valid, small cache file still deserializes exactly as before (no regression in existing cache
   tests).
3. `write_cache_at` no longer uses a fixed `version_check.tmp`; the temp name includes the process
   id (or another per-process-unique token). The existing `cache_atomic_write_via_rename` test still
   passes (update it if it asserts the literal temp name).
4. A new test asserts `fetch_latest_tag` returns `None` on a 301/302 redirect response.
5. The misnamed test is renamed; the rename does not change its assertions.
6. `cargo test -p fdemon-app` green; `cargo clippy -p fdemon-app --all-targets -- -D warnings`
   clean; `cargo fmt --all -- --check` clean.

### Testing

```rust
#[test]
fn read_cache_at_rejects_oversized_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("version_check.json");
    // Write a file larger than MAX_CACHE_BYTES.
    std::fs::write(&path, vec![b'{'; (MAX_CACHE_BYTES as usize) + 1]).unwrap();
    assert!(read_cache_at(&path).is_none());
}

#[tokio::test]
async fn fetch_latest_tag_rejects_redirect() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(302).insert_header("Location", "https://evil.example/"))
        .mount(&server)
        .await;
    let result = fetch_latest_tag(&server.uri(), std::time::Duration::from_secs(3)).await;
    assert!(result.is_none());
}
```

### Notes

- Do NOT change `MAX_RESPONSE_BYTES`, the TTL, the endpoint, the version comparison, or the
  cross-version cache-key logic — those are correct and validated.
- The size cap is a guard, not a hard error: keep the "any failure → cache miss → fetch fresh"
  contract intact.
- Use `tempfile::tempdir()` for all file-based tests (per `docs/CODE_STANDARDS.md` testing rules);
  never touch the real `dirs::cache_dir()` path in tests.

---

## Completion Summary

**Status:** Done
**Branch:** fix/version-check-banner-not-appearing

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/version_check.rs` | Added `MAX_CACHE_BYTES` constant; added size guard in `read_cache_at`; switched `write_cache_at` to per-PID temp name; updated `cache_atomic_write_via_rename` test; added `read_cache_at_rejects_oversized_file` test; added `fetch_latest_tag_rejects_redirect` test; renamed `write_stores_raw_tag_not_result` → `cache_always_stores_raw_tag_on_successful_fetch` |

### Notable Decisions/Tradeoffs

1. **S2 unique temp name implemented**: Used `path.with_extension(format!("{}.tmp", std::process::id()))` giving a file like `version_check.<pid>.tmp`. This produces a file where the OS-level "extension" (after the last dot) is still `tmp`, so the updated `cache_atomic_write_via_rename` test checks for any leftover `.tmp`-extension file in the dir — which correctly catches both old-style and new-style temp files.

2. **Test update strategy for `cache_atomic_write_via_rename`**: The old assertion checked that the literal `path.with_extension("tmp")` (`version_check.tmp`) did not exist. With the PID-based name, that file is never created, so the old assertion would have passed trivially but tested the wrong thing. The updated test uses `read_dir` to scan for any file whose extension is `tmp`, which correctly validates that no temp file lingers after a successful atomic write regardless of the naming scheme.

3. **Size cap semantics preserved**: `read_cache_at` returns `None` (a cache miss) for oversized files, consistent with the "any failure → cache miss" contract. The debug log message includes the file size and path for diagnostics.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app version_check` - Passed (36 tests: 34 existing + 2 new)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (clean)

### Risks/Limitations

1. **None**: All changes are in test-covered paths with no behavior change for valid inputs. The PID-based temp name is strictly more robust than the fixed name for the defense-in-depth purpose.

# Task 02: Gate the live manifest-fetch executor test out of the default suite

**Status:** Not Started
**Agent:** implementor
**Complexity:** low
**Depends On:** —
**Estimated Hours:** 0.5

## Objective

Make `cargo test --workspace` hermetic again: the manifest-fetch executor test must not perform a
live CDN call in the default run (review finding M2).

## Root Cause (verified)

`crates/fdemon-app/src/actions/mod.rs` line ~2845:
`test_fetch_flutter_release_manifest_emits_fetched_or_failed` is a plain `#[tokio::test]` that
drives the real `FetchFlutterReleaseManifest` executor → daemon `fetch_release_manifest()` → live
HTTPS GET to storage.googleapis.com, with a 30 s timeout. It cannot be wiremock-backed from
fdemon-app today because the daemon's URL seam `fetch_release_manifest_from` is `pub(crate)`
(flutter_install.rs:478).

## Required Changes

1. Add `#[ignore = "live network: requires outbound HTTPS to storage.googleapis.com; run with --ignored"]`
   to the test.
2. Keep the test body as-is (it must still pass when run explicitly with
   `cargo test -p fdemon-app --lib -- --ignored test_fetch_flutter_release_manifest` in a connected
   environment).
3. Add a brief doc comment on the test explaining why it is ignored and pointing at the daemon's
   wiremock-backed `fetch_release_manifest_from` tests for the hermetic coverage of the fetch path.
4. Do NOT widen the daemon API this round (the injectable-URL refactor is explicitly deferred —
   see followup TASKS.md Notes).

## Acceptance Criteria

- [ ] The test carries `#[ignore = "..."]` with a reason string.
- [ ] `cargo test --workspace` completes with no outbound network call from fdemon-app tests
      (this test is reported as ignored).
- [ ] `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Files

**Write:** `crates/fdemon-app/src/actions/mod.rs`
**Read:** `crates/fdemon-daemon/src/toolchain/flutter_install.rs` (wiremock precedent, read-only)

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a0b87e70733dad41a

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/mod.rs` | Added `#[ignore = "live network: requires outbound HTTPS to storage.googleapis.com; run with --ignored"]` attribute to `test_fetch_flutter_release_manifest_emits_fetched_or_failed` test; updated doc comment to explain why it is ignored and point to daemon's wiremock-backed tests for hermetic coverage. |

### Notable Decisions/Tradeoffs

1. **Ignore attribute placement**: Placed `#[ignore]` directly before the `async fn` (after `#[tokio::test]`), following the established pattern in the codebase (e.g., `handler/tests.rs`).
2. **Documentation approach**: Added a "Note" section to the doc comment rather than replacing the existing documentation, to preserve the original context about what the test is trying to verify.

### Testing Performed

- `cargo test --workspace` - Passed (test marked as ignored, no network call made)
- `cargo test -p fdemon-app --lib test_fetch_flutter_release_manifest` - Passed (test correctly appears as ignored with the reason string)
- `cargo fmt --all -- --check` - Passed (no formatting issues)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no clippy warnings)

### Risks/Limitations

None identified. The test body remains unchanged, so it can still be run explicitly with `--ignored` flag in a connected environment. No API changes were made to the daemon (as deferred in the task requirements).

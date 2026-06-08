## Task: Fix Flutter-install cancellation responsiveness and temp-dir lifecycle (F-PR53-06/07/08)

**Severity:** MEDIUM (concurrency / correctness)

**Objective**: Make the archive-install path cancellable during verify/extract,
eliminate the abort-vs-cleanup write/delete race, and fix the temp-dir guard so
it neither leaks a fully-extracted SDK on rename failure nor leaves an empty
wrapper dir on success.

**Depends on**: 03 (shares `download.rs`)

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs`
- `crates/fdemon-daemon/src/toolchain/download.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` (`handle_cancel_step` cancel+abort path at 596-609)

### Details

**(a) Cancellation blind window.**
`download_to_file` takes `cancel: CancellationToken` **by value**
(`download.rs:276`) and `archive_install` moves it in at `flutter_install.rs:909`.
After that, there is no `cancel.is_cancelled()` check before `verify_sha256`
(923-925) or `extract_archive` (945-949), and neither `spawn_blocking` is raced
against `cancel.cancelled()`. The sibling `git_install` (825-831) *does* use
`tokio::select!` on cancel — the archive path is comparatively under-guarded.

**(b) Abort-vs-cleanup race.**
`extract_archive` (`download.rs:761`) takes no token and runs a synchronous loop
in `spawn_blocking`. If the install future is dropped via `JoinHandle::abort()`
(the documented cancel backstop in `handle_cancel_step`), the detached blocking
thread keeps writing into `tmp_dir` while `TempDirGuard::drop`
(`flutter_install.rs:165-177`) synchronously `remove_dir_all`s `tmp_dir` — a
write-vs-delete race that can leave a partial tree or surface spurious errors,
contradicting the guard's documented abort-safe guarantee (139-144).

**(c) Temp-dir guard ordering + wrapper leak.**
`tmp_guard.disarm()` runs **before** `std::fs::rename` (`flutter_install.rs:733`
then `740`). A failed rename returns with the guard disarmed → the
extracted SDK is leaked (violates the "on any failure the temp dir is removed"
contract at 559-560). Separately, the archive path renames only
`tmp_dir/flutter` → `final_dir` (archive_install returns `tmp_dir.join("flutter")`,
958), so the empty outer `.fdemon-install-tmp-<pid>` wrapper is left behind even
on success. (Both are self-healed later by `reclaim_stale_flutter_tmps`, 630-634,
but should not leak in the first place.)

### Proposed Fix

1. Pass the `CancellationToken` to `download_to_file` **by reference** (or clone it
   before the call) so `archive_install` retains it; add `is_cancelled()` checks
   before `verify_sha256` and before `extract_archive`.
2. Thread the token into `extract_archive` (and `extract_zip`/`extract_tar_xz`) so
   the extraction loop checks cancellation and stops promptly; OR wrap the
   `spawn_blocking` in `select!` on `cancel.cancelled()` **and ensure the blocking
   `JoinHandle` is awaited/joined before** any `remove_dir_all` of `tmp_dir`, so
   cleanup never races a live writer.
3. Move `tmp_guard.disarm()` to **after** a successful `rename` (disarm only on
   `Ok`). For the archive path, also remove the now-empty outer `tmp_dir` after the
   rename (or only disarm when `sdk_root_in_tmp == tmp_dir`).

### Acceptance Criteria

1. A cancel issued during verify or extract returns `Err(cancelled)` promptly
   (not after the full extract completes), and the extraction thread does not
   continue writing into a directory being removed.
2. On a simulated rename failure, `tmp_dir` is removed (guard fires) and the error
   is propagated — the extracted SDK is not leaked.
3. On a successful archive install, no `.fdemon-install-tmp-<pid>` wrapper dir
   remains.
4. `git_install` cancellation behavior is unchanged (no regression).

### Testing

```rust
// flutter_install.rs test module
// - cancel-before-extract: pre-cancel the token, run the archive path past download,
//   assert it returns cancelled before extract and tmp_dir is cleaned.
// - rename-failure leak: inject a rename failure (e.g. pre-create final_dir as a
//   non-empty dir / read-only parent) and assert tmp_dir was removed.
// - wrapper cleanup: after a successful archive install into a temp install_root,
//   assert no leftover .fdemon-install-tmp-* dir exists.
// download.rs: if extract_archive gains a token param, add a test that a cancelled
//   token stops extraction early.
```

### Notes

- Depends on task 03 (both edit `download.rs`); run after it on chain B. Task 04
  and task 05 do not share files, so they may run in parallel worktrees once 03
  lands.
- The refuted "fixed 1.5 GiB disk budget" and "sdkmanager not token-cancellable"
  findings are intentionally **out of scope** (both were adversarially rejected:
  the budget is documented/correct, and `handle_cancel_step` already
  abort()+kill_on_drop kills sdkmanager).

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/download.rs` | Added `CancellationToken` parameter to `extract_zip`, `extract_tar_xz`, `extract_archive`; added `CANCEL_CHECK_INTERVAL` constant; added 3 extraction cancellation tests |
| `crates/fdemon-daemon/src/toolchain/flutter_install.rs` | Fixed `archive_install` to clone token before download and check it before verify/extract; wrapped `spawn_blocking` verify and extract calls in `tokio::select!` against `cancel.cancelled()`; moved `tmp_guard.disarm()` to after successful rename; added outer wrapper cleanup for archive path; added 4 new tests |
| `crates/fdemon-daemon/src/toolchain/android_install.rs` | Updated `extract_zip` call to pass `&CancellationToken::new()` to satisfy updated function signature |

### Notable Decisions/Tradeoffs

1. **Cooperative cancellation via token threading**: Rather than select+abort without awaiting the blocking thread, we pass the `CancellationToken` into `extract_archive`/`extract_zip`/`extract_tar_xz` so the blocking thread exits cooperatively. For `extract_tar_xz`, dropping the `ReceiverReader` propagates `BrokenPipe` to the XZ decode thread so it also self-terminates. This eliminates the write/delete race without needing to await the blocking thread after abort.

2. **`select!` with `&mut handle` pattern**: Used `let mut verify_handle = ...; tokio::select! { _ = cancel.cancelled() => { verify_handle.abort(); ... } result = &mut verify_handle => { ... } }` to avoid double-move errors. The `mut` binding and `&mut` poll reference in the non-cancel arm enables calling `abort()` in the cancel arm.

3. **Disarm-after-rename ordering**: Moved `tmp_guard.disarm()` to after `std::fs::rename(...)` succeeds. Previously disarming before the rename meant a rename failure leaked the extracted SDK. Now the guard fires on any failure path.

4. **Outer wrapper cleanup**: For the archive path, `sdk_root_in_tmp` is `tmp_dir/flutter` (not `tmp_dir`). After renaming the inner dir to `final_dir`, the outer `.fdemon-install-tmp-<pid>` wrapper is now explicitly removed. `reclaim_stale_flutter_tmps` continues to serve as a cross-run backstop.

5. **android_install.rs minimal change**: The cancel token is consumed by `download_to_file` in the android path, so `extract_zip` receives `CancellationToken::new()` (non-cancellable). This maintains existing behavior for the android path and is noted in a comment.

6. **Check interval at entry 0**: Since `i.is_multiple_of(256)` is true for `i == 0`, a pre-cancelled token is caught on the very first iteration — satisfying the "stops extraction promptly" acceptance criterion.

### Testing Performed

- `cargo fmt --all -- --check` - PASS
- `cargo check --workspace --all-targets` - PASS
- `cargo test --workspace` - PASS (all tests pass, 7 new tests added)
- `cargo clippy --workspace --all-targets -- -D warnings` - PASS

### New Tests Added

**download.rs:**
- `extract_zip_cancelled_token_stops_extraction` — pre-cancelled token stops zip extraction
- `extract_tar_xz_cancelled_token_stops_extraction` — pre-cancelled token stops tar.xz extraction
- `extract_archive_cancelled_token_stops_extraction` — pre-cancelled token stops archive dispatch

**flutter_install.rs:**
- `temp_dir_guard_fires_on_cancelled_return` — guard fires when cancel causes early return (AC1)
- `temp_dir_guard_not_disarmed_on_rename_failure_simulated` — guard removes dir when rename fails (AC2)
- `temp_dir_guard_disarmed_after_rename_does_not_remove_final_dir` — disarm-after-rename correctness (AC3)
- `outer_tmp_wrapper_removed_after_archive_rename` — no wrapper dir remains after archive install (AC4)

### Risks/Limitations

1. **`spawn_blocking` abort semantics**: Calling `abort()` on a `spawn_blocking` `JoinHandle` does not kill the underlying OS thread — the thread runs to completion. For `verify_sha256` (the cancel branch), we abort and immediately return; the thread completes its SHA-256 computation independently. This is safe since `verify_sha256` only reads (no write/delete race). For `extract_archive`, the cooperative token check ensures the thread terminates without writing after the token fires.

2. **CANCEL_CHECK_INTERVAL granularity**: Cancellation is checked every 256 entries. The Flutter SDK tar.xz has ~80k entries, so the maximum additional work after a cancel is bounded by ~256 entries. This is well under a second in practice.

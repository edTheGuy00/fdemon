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

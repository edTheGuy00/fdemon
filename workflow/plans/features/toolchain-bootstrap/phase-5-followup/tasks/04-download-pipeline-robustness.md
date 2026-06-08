## Task: Download pipeline robustness — idle timeout, temp-dir RAII, disk budget, git cancel (F5, F14, F15, F16, F23, F26)

**Severity:** MEDIUM (F5, F14) + LOW (F15, F16, F23) + NIT (F26)

**Objective**: Make the managed-install download/extract path correct under slow
networks and cancellation: a real per-read idle guard, no leaked extraction temp dir
on abort, no double-counted disk budget, a documented captive-portal limitation,
cancellable git clones, and a minimal dependency feature set.

**Depends on**: — (first in chain B)

**Estimated Time**: 4–5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/download.rs`
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs`
- `crates/fdemon-daemon/src/toolchain/android_install.rs`
- `Cargo.toml` (workspace `tokio-util` feature)

### Details & Fixes

**F5 (MEDIUM) — `IDLE_TIMEOUT` is a total-request deadline, not per-read idle.**
`IDLE_TIMEOUT` (30s) is documented as an idle/stall guard (`download.rs:88-92`) but
wired via `.timeout(IDLE_TIMEOUT)` on the client builder (`download.rs:271`). In
reqwest 0.12, `ClientBuilder::timeout` is a **total** request deadline; the per-read
semantic the comment describes is the separate `read_timeout`. So any legitimate
~300 MiB SDK download whose *total* transfer exceeds 30s (any link slower than
~10 MiB/s) is aborted as a "stall" and retried from byte 0, failing identically all 3
attempts (no Range/resume).
**Fix:** replace `.timeout(IDLE_TIMEOUT)` with `.read_timeout(IDLE_TIMEOUT)` (resets
after each successful chunk read). Keep `connect_timeout(CONNECT_TIMEOUT)`. Optionally
add a generous overall `.timeout(...)` (minutes) as a hard ceiling — never 30s. Fix
the constant doc (`88-92`) and the worst-case reasoning (`110-111`) accordingly. This
is a latent pre-existing bug (unchanged since base `56a2f95`) that Phase 5's
cancellation rewrite shares the loop with — the `tokio::select!` cancellation
(`349-371`) is independent of this change.

**F14 (MEDIUM) — abort() backstop leaks the install temp dir.** `handle_cancel_step`
fires both `cancel.cancel()` and `join.abort()`. The token path is clean
(`install_inner` returns `Err(Cancelled)` → `match result { Err(e) => remove_dir_all(&tmp_dir) }`),
but if `abort()` wins it drops the future mid-`await`, the `match` arm never runs, and
`tmp_dir` (`std::fs::create_dir_all`, **not** `tempfile::TempDir`) leaks a
partially-extracted ~1 GB SDK tree or partial git clone. Same in
`install_android_tools` (`android_install.rs:171-187`). The per-PID stale-temp
reclamation (`flutter_install.rs:534-542`) does **not** fire for the common
different-PID next run.
**Fix:** wrap the install working tree in a Drop-based guard (or
`tempfile::TempDir::new_in(&target.install_root)`) whose `Drop` runs `remove_dir_all` —
`Drop` runs even when the future is aborted. Apply to both `install_flutter` and
`install_android_tools`. Additionally harden preflight to glob+remove **all**
`.fdemon-install-tmp-*` / `.fdemon-android-tmp-*` dirs (under the `LockGuard`), so any
leaked tree is reclaimed regardless of PID. (`abort()` must stay — see F23 — so the
RAII guard is necessary, not optional.)

**F15 (LOW) — disk budget double-counted.** `download_to_file` checks
`ARCHIVE_DISK_BUDGET_BYTES` (1.5 GiB) on `tmp_dir`'s filesystem before download
(`download.rs:286`), then `archive_install` checks the **same** 1.5 GiB on the **same**
filesystem after the ~300 MiB archive is already written (`flutter_install.rs:808`).
The second check therefore effectively demands 1.5 GiB *plus* the archive size — a
false-negative refusal on a tight disk. (Fail-safe direction; never under-provisions.)
**Fix:** drop the second `ensure_disk_space` at `flutter_install.rs:808` (the
pre-download check already budgets archive + extracted tree on the same FS), or lower
its budget to the extraction delta (`ARCHIVE_DISK_BUDGET_BYTES − actual archive size`
via `fs::metadata`) and correct the comment (`806-807`).

**F16 (LOW) — captive-portal false negative (doc).** `check_network_connectivity`
treats any HTTP response as reachable (`.map(|_| ())`, `download.rs:168`), so a portal
returning 2xx/3xx over a valid-cert path passes the probe; the real GET then yields
"failed to parse manifest" instead of the fast "no network" message. (HTTPS to
`storage.googleapis.com` means a transparent MITM portal fails the handshake and *is*
caught fast, narrowing the blast radius.)
**Fix:** update the `check_network_connectivity` doc (`147-152`) to state the
limitation explicitly (cannot distinguish a captive portal from the real host).
Optional hardening (not required): after HEAD, compare `Response::url()` host to the
requested host to catch transparent redirects. Note in the doc that TASKS.md's
captive-portal criterion is only partially met.

**F23 (LOW) — git_install not cancellable via the token.** `install_inner` forwards
`cancel` only to `archive_install`; `git_install` (the default channel-install path)
takes no token (`flutter_install.rs:592-596, 671`). A clone is only stoppable via the
lossy `join.abort()` backstop.
**Fix:** add `cancel: CancellationToken` to `git_install` and wrap its
`run_streaming(...)` await in `tokio::select! { biased; _ = cancel.cancelled() => return Err(Error::cancelled(...)); r = run_streaming(...) => r? }`. Dropping the future
kills the git child via `kill_on_drop(true)` (`process_stream.rs:78`).

**F26 (NIT) — `tokio-util` `rt` feature unused.** Only
`tokio_util::sync::CancellationToken` is used anywhere; `rt` gates `tokio_util::task`,
which is not used.
**Fix:** in `Cargo.toml:58` change
`tokio-util = { version = "0.7", features = ["rt"] }` to
`tokio-util = { version = "0.7" }`.

### Acceptance Criteria

1. The download client uses `read_timeout(IDLE_TIMEOUT)` (per-read), not
   `timeout(IDLE_TIMEOUT)`; the constant doc reflects per-read idle semantics (F5).
2. A cancelled or aborted Flutter/Android install does not leave a
   `.fdemon-install-tmp-*` / `.fdemon-android-tmp-*` tree behind — RAII cleanup runs
   on future-drop; preflight reclaims any stray trees regardless of PID (F14).
3. The pre-extraction disk check is not additive with the pre-download check on the
   same filesystem (F15).
4. `check_network_connectivity` doc states the captive-portal limitation (F16).
5. `git_install` accepts and honours the cancel token; a cancel during clone returns
   `Error::Cancelled` cooperatively (F23).
6. `Cargo.toml` no longer enables `tokio-util`'s `rt` feature;
   `cargo check --workspace` passes (F26).

### Testing

```rust
// toolchain/download.rs + flutter_install.rs + android_install.rs test modules
// - NEW (F14): a Drop-guard / TempDir over the extraction dir removes it when the
//     future is dropped mid-extract (simulate by dropping the guard); preflight glob
//     removes a planted stale .fdemon-install-tmp-<otherpid> dir.
// - NEW (F15): the post-download disk check does not require full budget on top of the
//     archive (or is removed) — assert install proceeds with budget+epsilon free.
// - NEW (F23): git_install with a pre-cancelled token returns Error::Cancelled.
// - F5/F16 are wiring/doc changes verified by the existing download tests staying green
//   (and the F2/F13 test work in Task 05).
```

### Notes

- Shares `download.rs`/`flutter_install.rs` with Task 05 — serialise (chain B).
- Keep `cancel.cancel()` as the primary cancellation path and `join.abort()` as the
  backstop; the RAII temp-dir guard is what makes the backstop leak-free.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ad70e3eb5390ed07a

### Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | F26: removed `rt` feature from `tokio-util` workspace dep |
| `crates/fdemon-daemon/src/toolchain/download.rs` | F5: changed `.timeout()` to `.read_timeout()` on client builder; updated `IDLE_TIMEOUT` doc to clarify per-read semantics; updated `CONNECTIVITY_PROBE_TIMEOUT` doc accordingly; F16: added captive-portal limitation doc to `check_network_connectivity` |
| `crates/fdemon-daemon/src/toolchain/flutter_install.rs` | F14: added `TempDirGuard` RAII struct + `reclaim_stale_flutter_tmps` fn; updated `install_flutter` to use them; updated `install_inner` to accept + disarm guard before rename; F15: removed redundant `ensure_disk_space` call before extraction; F23: added `cancel: CancellationToken` param to `git_install` + `tokio::select!` wrapper around `run_streaming`; removed `ensure_disk_space`/`ARCHIVE_DISK_BUDGET_BYTES` from imports (no longer used directly); added 5 new unit tests |
| `crates/fdemon-daemon/src/toolchain/android_install.rs` | F14: added `TempDirGuard` RAII struct + `reclaim_stale_android_tmps` fn; replaced manual cleanup in `install_android_tools` with RAII guard + preflight reclamation; added 2 new unit tests |

### Notable Decisions/Tradeoffs

1. **`TempDirGuard` vs `tempfile::TempDir`**: Used a custom RAII guard instead of `tempfile::TempDir::new_in()` because the temp dir name is derived deterministically from the PID (`.fdemon-install-tmp-<pid>`) for the glob-based stale reclamation to work. `tempfile::TempDir` uses a random suffix which would prevent the preflight glob from finding dirs from other processes. The custom guard is simpler and sufficient.

2. **`reclaim_stale_flutter_tmps` placement**: Called under the `LockGuard` in `install_flutter` so there's no race with a concurrent install. The lock serializes both the reclamation and the new install.

3. **F15 removal vs delta budget**: Removed the second `ensure_disk_space` call entirely (rather than computing a delta) since the pre-download check at `ARCHIVE_DISK_BUDGET_BYTES` (1.5 GiB) already conservatively budgets both the archive and the extracted tree. Adding complexity for a delta calculation was not warranted.

4. **`git_install` cancel token**: The `tokio::select! { biased; ... }` pattern matches the existing `download_to_file` streaming loop. Because `run_streaming` uses `kill_on_drop(true)`, dropping the future on the cancel branch kills the git child process cleanly.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo test --workspace` - Passed (6 new tests added, all pass)
  - `temp_dir_guard_removes_dir_on_drop` - ok
  - `temp_dir_guard_missing_dir_no_panic` - ok
  - `reclaim_stale_flutter_tmps_removes_all_tmp_dirs` - ok
  - `git_install_precancelled_returns_cancelled` - ok
  - `android_temp_dir_guard_removes_dir_on_drop` - ok
  - `reclaim_stale_android_tmps_removes_all_tmp_dirs` - ok

### Risks/Limitations

1. **F14 abort-race note**: `Drop` on a `TempDirGuard` inside a future that is `abort()`ed runs synchronously during the drop cascade — this is correct Rust behavior and ensures cleanup. The preflight `reclaim_stale_*` functions provide a second line of defence for any dir that might be leaked across process restarts (different PID).

2. **F16 partial coverage**: The captive-portal limitation is now documented but not programmatically detected. A portal returning 2xx over HTTPS to a different host (e.g. DNS-based interception with a valid wildcard cert) could still slip past. This is an inherent limitation of the pre-flight HEAD probe design, documented as per the task spec.

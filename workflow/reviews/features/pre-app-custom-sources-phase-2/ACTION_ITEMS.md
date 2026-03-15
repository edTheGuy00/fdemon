# Action Items: Pre-App Custom Sources Phase 2

**Review Date:** 2026-03-15
**Verdict:** NEEDS WORK
**Blocking Issues:** 1 (MAJOR)

## Critical Issues (Must Fix)

_(none)_

## Major Issues (Must Fix)

### 1. Add deduplication guard to `SharedSourceStarted` handler

- **Source:** Architecture Enforcer, Logic Checker, Risks Analyzer (3/4 agents)
- **File:** `crates/fdemon-app/src/handler/update.rs`
- **Line:** ~2328 (the `Message::SharedSourceStarted` match arm)
- **Problem:** The handler unconditionally pushes a new `SharedSourceHandle` without checking for duplicates. A TOCTOU race between two concurrent session launches can produce duplicate shared source processes (port conflicts, duplicate log lines).
- **Required Action:** Before pushing, check `state.is_shared_source_running(&name)`. If already running, shut down the incoming duplicate (send `true` on `shutdown_tx`, abort the task handle) and return early. Add a `tracing::warn!` for observability.
- **Acceptance:** Add a test `test_shared_source_started_duplicate_is_rejected` that sends two `SharedSourceStarted` messages with the same name and verifies only one handle is stored, and the second's shutdown channel receives `true`.

## Minor Issues (Should Fix)

### 2. Flush batched logs in `SharedSourceStopped` handler

- **Source:** All 4 agents
- **File:** `crates/fdemon-app/src/handler/update.rs`
- **Line:** ~2361 (the `Message::SharedSourceStopped` match arm)
- **Problem:** `queue_log(entry)` return value is ignored and `flush_batched_logs()` is never called. Warning log may be delayed until next engine tick. Inconsistent with every other log-queueing site in the file.
- **Suggested Action:** Change to `if handle.session.queue_log(entry) { handle.session.flush_batched_logs(); }`.
- **Acceptance:** Existing test `test_shared_source_stopped_removes_handle_and_warns` already verifies the log appears after a manual `flush_all_pending_logs()` call — update it to verify the log appears without an explicit external flush.

### 3. Fix `has_unstarted_post_app` guard for shared sources

- **Source:** Architecture Enforcer, Logic Checker (2/4 agents)
- **File:** `crates/fdemon-app/src/handler/session.rs`
- **Line:** ~319-348 (`maybe_start_native_log_capture`)
- **Problem:** `running_names` is built from `handle.custom_source_handles` only. Shared post-app sources (stored on `AppState.shared_source_handles`) always appear "unstarted," causing a spurious `StartNativeLogCapture` dispatch on hot-restart.
- **Suggested Action:** Extend `has_unstarted_post_app` to check `state.shared_source_handles` for shared post-app sources.
- **Acceptance:** Add a test that configures a shared post-app source, marks it running, and verifies `maybe_start_native_log_capture` returns `None` (no action needed) on hot-restart.

### 4. Fix ARCHITECTURE.md documentation inaccuracy

- **Source:** Risks Analyzer
- **File:** `docs/ARCHITECTURE.md`
- **Line:** ~1181
- **Problem:** States shared sources require `start_before_app = true`, but code supports shared post-app sources.
- **Suggested Action:** Update to: "Shared sources can be started either as pre-app sources (`start_before_app = true`) or as post-app sources (`start_before_app = false`). They are shut down during `AppState::shutdown_shared_sources()` when fdemon exits."

## Nitpick Issues (Consider Fixing)

### 5. Demote unused public helpers

- `has_shared_sources()` and `shared_sources()` in `config/types.rs` — no production callers. Consider `pub(crate)` or removal.

### 6. Add `#[derive(Debug)]` to `CustomSourceHandle`

- For parity with `SharedSourceHandle` in `session/handle.rs`.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] Issue 1 resolved: dedup guard added with test
- [ ] Issue 2 resolved: flush added in `SharedSourceStopped`
- [ ] Issue 3 resolved: `has_unstarted_post_app` accounts for shared sources
- [ ] Issue 4 resolved: ARCHITECTURE.md corrected
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`

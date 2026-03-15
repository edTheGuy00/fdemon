# Code Review: Pre-App Custom Sources Phase 2 — Shared Custom Sources

**Review Date:** 2026-03-15
**Branch:** `feature/pre-app-custom-sources`
**Scope:** 27 files, ~2,468 insertions across `crates/fdemon-app/`
**Task File:** `workflow/plans/features/pre-app-custom-sources/phase-2/TASKS.md`

---

## Verdict: NEEDS WORK

All 10 tasks are functionally complete and the core design is sound. However, a missing handler-level deduplication guard creates a TOCTOU window that can produce duplicate shared source processes, and several minor inconsistencies were found across the implementation. None are blocking from a safety perspective, but they should be addressed before merge.

---

## Agent Verdicts

| Agent | Verdict | Key Findings |
|-------|---------|-------------|
| Architecture Enforcer | CONCERNS | Layer boundaries clean; TEA compliance good; hot-restart guard blind spot for shared post-app sources |
| Code Quality Inspector | NEEDS WORK | Missing `flush_batched_logs()` in `SharedSourceStopped`; unused public helpers; `Vec::contains` for set semantics |
| Logic & Reasoning Checker | CONCERNS | TOCTOU race on `running_shared_names` snapshot; gate-skip logic verified correct; shutdown ordering correct |
| Risks & Tradeoffs Analyzer | CONCERNS | No dedup guard on `SharedSourceStarted` handler; ARCHITECTURE.md inaccuracy; overall risk level manageable |

---

## Consolidated Findings

### MAJOR — No deduplication guard on `SharedSourceStarted` handler

**Found by:** Architecture, Logic, Risks (3/4 agents)
**File:** `crates/fdemon-app/src/handler/update.rs` ~line 2328
**Severity:** MAJOR

The `SharedSourceStarted` handler unconditionally pushes a new `SharedSourceHandle` onto `state.shared_source_handles` without checking whether a handle with the same name already exists.

**TOCTOU scenario:** Session A dispatches `SpawnPreAppSources` with `running_shared_names = []`. Before the spawned task sends `SharedSourceStarted` back through the channel, Session B also dispatches `SpawnPreAppSources` with `running_shared_names = []` (stale snapshot). Both spawn the same shared source, producing two OS processes for the same command (port conflicts, duplicate log lines).

The spawn-side guards (`running_shared_names.contains()`) reduce the window but don't eliminate it because the check-and-spawn happen asynchronously.

**Fix:** Add a duplicate-name check in the handler:
```rust
if state.is_shared_source_running(&name) {
    tracing::warn!("Duplicate SharedSourceStarted for '{}' — shutting down extra", name);
    let _ = shutdown_tx.send(true);
    if let Some(task) = task_handle.lock().ok().and_then(|mut s| s.take()) {
        task.abort();
    }
    return UpdateResult::none();
}
```

---

### MINOR — `SharedSourceStopped` does not flush batched logs

**Found by:** Architecture, Code Quality, Logic, Risks (4/4 agents)
**File:** `crates/fdemon-app/src/handler/update.rs` ~line 2361
**Severity:** MINOR

The handler calls `handle.session.queue_log(entry)` but ignores the return value and never calls `flush_batched_logs()`. Every other log-queueing site in the same file (`SharedSourceLog`, `NativeLog`, etc.) checks the return value and flushes when needed. The warning log may be delayed until the next engine tick.

**Fix:**
```rust
if handle.session.queue_log(entry) {
    handle.session.flush_batched_logs();
}
```

---

### MINOR — `has_unstarted_post_app` guard blind spot for shared post-app sources

**Found by:** Architecture, Logic (2/4 agents)
**File:** `crates/fdemon-app/src/handler/session.rs` ~lines 319-348
**Severity:** MINOR

`maybe_start_native_log_capture` builds `running_names` from `handle.custom_source_handles` (per-session only). Shared post-app sources are tracked on `AppState.shared_source_handles`, so they always appear "unstarted," causing an unnecessary `StartNativeLogCapture` dispatch on hot-restart. The downstream `spawn_custom_sources` correctly skips them via its own guard, so this is a wasted action dispatch, not a correctness bug.

**Fix:** Extend the check to include `state.shared_source_handles` names when evaluating `has_unstarted_post_app`.

---

### MINOR — ARCHITECTURE.md documentation inaccuracy

**Found by:** Risks (1/4 agents)
**File:** `docs/ARCHITECTURE.md` ~line 1181
**Severity:** MINOR

States "Shared sources are started as part of the pre-app source flow (they require `start_before_app = true`)" — but the code explicitly handles shared post-app sources (`shared = true`, `start_before_app = false`) in `spawn_custom_sources()`. The `CONFIGURATION.md` correctly documents both modes.

**Fix:** Update the sentence to: "Shared sources can be started either as pre-app sources (`start_before_app = true`) or as post-app sources (`start_before_app = false`). They are shut down during `AppState::shutdown_shared_sources()` when fdemon exits."

---

### NITPICK — Unused public helpers `has_shared_sources()` / `shared_sources()`

**Found by:** Code Quality (1/4 agents)
**File:** `crates/fdemon-app/src/config/types.rs` ~lines 920-928
**Severity:** NITPICK

These two methods are `pub` but have no production callers — only used in their own test module. Consider demoting to `pub(crate)` or removing until needed.

---

### NITPICK — `CustomSourceHandle` lacks `#[derive(Debug)]` while `SharedSourceHandle` has it

**Found by:** Code Quality (1/4 agents)
**File:** `crates/fdemon-app/src/session/handle.rs`
**Severity:** NITPICK

Minor asymmetry between the two structurally similar handle types.

---

## Strengths

- **Clean TEA compliance**: All three new handlers (`SharedSourceLog`, `SharedSourceStarted`, `SharedSourceStopped`) are pure state transitions returning `UpdateResult::none()`. Side effects correctly flow through `UpdateAction`.
- **Layer boundaries respected**: All changes confined to `fdemon-app`. No improper cross-layer imports.
- **Thorough test coverage**: 44+ new tests across config (11), state (8), actions (9), handlers (16+). Edge cases covered: zero sessions, unknown names, tag filtering, session survival.
- **Correct shutdown ordering**: Per-session first, then shared, then global signal.
- **Gate-skip logic is correct**: Boolean algebra verified for all cases — first session, second session, mixed shared/non-shared.
- **Good documentation**: All new public items have doc comments. CONFIGURATION.md updated with clear examples.

---

## Quality Metrics

| Metric | Score | Notes |
|--------|-------|-------|
| Architecture Compliance | 4/5 | Clean layers; one guard blind spot |
| Rust Idioms | 4/5 | Good iterators, `retain`, `drain`; minor `Vec::contains` for set semantics |
| Error Handling | 4/5 | Channel errors handled correctly; no raw `unwrap()` in production |
| Testing | 4/5 | Strong coverage; missing test for flush-on-stop behavior |
| Documentation | 4/5 | Thorough; one ARCHITECTURE.md inaccuracy |
| Maintainability | 4/5 | Clear shared/per-session separation; two unused public helpers |

---

## Verification Status

| Check | Status |
|-------|--------|
| `cargo fmt --all` | Not verified in this review |
| `cargo check --workspace` | Not verified in this review |
| `cargo test --workspace` | Not verified in this review |
| `cargo clippy --workspace -- -D warnings` | Not verified in this review |

---

## Action Items

See [ACTION_ITEMS.md](ACTION_ITEMS.md) for the prioritized fix list.

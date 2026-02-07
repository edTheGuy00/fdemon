# Plan: Post-Restructure Cleanup

## TL;DR

Address 10 issues identified in the workspace restructure phase 3 code review. Three major issues (protocol parsing misplacement, headless log duplication bug, eprintln violations) require immediate fixes. Seven minor issues (crossterm coupling, spawn duplication, try_lock race, dual imports, large files, debug_assertions visibility, view mutability) are tracked for future cleanup. Single-phase plan with 7 actionable tasks (issue #5 accepted as-is, no task needed).

---

## Background

The workspace restructure (phases 1-3) successfully split Flutter Demon from a single crate into a 4-crate workspace with compile-time layer enforcement. The phase 3 code review (APPROVED WITH CONCERNS) identified 3 major and 7 minor issues. This plan addresses all actionable findings. The review is at `workflow/reviews/features/workspace-restructure-phase-3/REVIEW.md`.

---

## Affected Modules

### Major Fixes

- `crates/fdemon-core/src/events.rs` - Move ~306 lines of parsing logic out (Issue #1)
- `crates/fdemon-daemon/src/protocol.rs` - Receive parsing logic as free functions (Issue #1)
- `src/headless/runner.rs` - Fix log re-emission bug with index tracking (Issue #2)
- `src/headless/mod.rs` - Replace 3x eprintln! with tracing::error! (Issue #3)

### Minor Fixes

- `crates/fdemon-app/src/message.rs` - Abstract KeyEvent behind app-local enum (Issue #4)
- `crates/fdemon-app/src/handler/keys.rs` - Update to use app-local key types (Issue #4)
- `crates/fdemon-daemon/src/process.rs` - Consolidate 3 spawn methods into 1 (Issue #6)
- `crates/fdemon-app/src/actions.rs` - Replace try_lock() with blocking lock (Issue #7)
- Multiple files - Standardize DaemonMessage import paths (Issue #8)
- `crates/fdemon-daemon/src/lib.rs` - Add `test-helpers` feature flag (Issue #10)

### Not Addressed (Accepted)

- Issue #5 (`view()` takes `&mut`) - Framework limitation, documented, no action
- Issue #9 (Large files) - Pre-existing tech debt, not introduced by restructure, tracked separately

---

## Development Phases

### Phase 1: Review Cleanup (All Tasks)

**Goal**: Address all actionable findings from the phase 3 review. Major issues first (tasks 01-03), then minor issues (tasks 04-07).

#### Wave 1 (Independent, can run in parallel)

1. **Task 01: Move parse logic from core to daemon** (Issue #1 - MAJOR)
   - Move `DaemonMessage::parse()`, `parse_event()`, `unknown()`, `to_log_entry()`, `parse_flutter_log()`, `detect_log_level()` from `fdemon-core/src/events.rs` to `fdemon-daemon/src/protocol.rs` as free functions
   - Move `LogEntryInfo` struct to daemon
   - Update 3 production call sites + ~40 test call sites
   - Update re-exports in both crates

2. **Task 02: Fix headless log re-emission** (Issue #2 - MAJOR)
   - Add `last_emitted_log_index: usize` local state to `headless_event_loop()`
   - Rewrite `emit_post_message_events()` to emit only new logs since last index
   - Handle VecDeque eviction edge case with min() clamping
   - Add unit tests for the emission tracking

3. **Task 03: Replace eprintln! with tracing** (Issue #3 - MAJOR)
   - Replace 3 `eprintln!` calls in `HeadlessEvent::emit()` with `tracing::error!()`
   - Quick, isolated fix

#### Wave 2 (Independent, can run in parallel)

4. **Task 04: Consolidate FlutterProcess spawn methods** (Issue #6 - MINOR)
   - Extract `spawn_internal(args, project_path, event_tx)` containing shared logic
   - Convert `spawn()`, `spawn_with_device()`, `spawn_with_args()` to thin wrappers
   - Eliminates ~70 lines of duplication

5. **Task 05: Fix try_lock() race in session task tracking** (Issue #7 - MINOR)
   - Change `SessionTaskMap` from `tokio::sync::Mutex` to `std::sync::Mutex`
   - Replace `try_lock()` with `.lock().unwrap()` (sync lock in sync function)
   - Add else-branch warning log for any lock poisoning

6. **Task 06: Standardize DaemonMessage imports** (Issue #8 + #10 - MINOR)
   - Remove `DaemonMessage` re-export from `fdemon-daemon` (after task 01 moves parse to daemon)
   - Establish canonical import: `fdemon_core::DaemonMessage` for the type, `fdemon_daemon::parse_daemon_message` for parsing
   - Add `test-helpers` feature flag to fdemon-daemon for test utility visibility
   - Update all import paths for consistency

7. **Task 07: Abstract crossterm KeyEvent** (Issue #4 - MINOR)
   - Define `InputKey` enum in `fdemon-app` covering all used key combinations
   - Convert `Message::Key(KeyEvent)` to `Message::Key(InputKey)`
   - Update `handler/keys.rs` to match on `InputKey` instead of crossterm types
   - Move crossterm conversion to `fdemon-tui/src/event.rs` boundary
   - Remove crossterm dependency from fdemon-app

---

## Task Dependency Graph

```
Wave 1 (parallel):
┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────────────────────┐
│ 01-move-parse-to-daemon │  │ 02-fix-headless-log-dup │  │ 03-replace-eprintln     │
└────────────┬────────────┘  └─────────────────────────┘  └─────────────────────────┘
             │
             ▼
Wave 2 (parallel, 06 depends on 01):
┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────────────────────┐
│ 04-consolidate-spawn    │  │ 05-fix-try-lock-race    │  │ 06-standardize-imports  │  │ 07-abstract-key-event   │
└─────────────────────────┘  └─────────────────────────┘  └─────────────────────────┘  └─────────────────────────┘
```

---

## Edge Cases & Risks

### Task 01: Orphan Rule Workaround
- **Risk:** Moving `impl DaemonMessage` methods to daemon is impossible due to orphan rule
- **Mitigation:** Use free functions (`parse_daemon_message()`) instead of inherent methods. The API changes from `DaemonMessage::parse(json)` to `parse_daemon_message(json)`.

### Task 02: VecDeque Ring Buffer Eviction
- **Risk:** If logs are evicted from the front of the VecDeque (max_logs limit), `last_emitted_log_index` becomes stale
- **Mitigation:** Clamp with `last_emitted = last_emitted.min(current_count)` before computing the delta.

### Task 05: Mutex Type Change
- **Risk:** Changing `tokio::sync::Mutex` to `std::sync::Mutex` could block the tokio runtime
- **Mitigation:** The critical section is a single `HashMap::insert` (~nanoseconds). Blocking is negligible and safe per tokio docs for very short critical sections.

### Task 07: Key Abstraction Scope
- **Risk:** `handler/keys.rs` has 895 lines matching on crossterm types. Large refactoring surface.
- **Mitigation:** Define `InputKey` variants that map 1:1 to the crossterm patterns currently matched. Mechanical translation, validated by compiler.

---

## Success Criteria

### Complete When:
- [ ] `cargo fmt --all` passes
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace --lib` passes (1,532+ tests, 0 failures)
- [ ] `cargo clippy --workspace --lib -- -D warnings` passes
- [ ] `DaemonMessage::parse()` no longer exists in fdemon-core
- [ ] Headless mode does not emit duplicate log entries
- [ ] Zero `eprintln!` calls in the codebase
- [ ] No crossterm dependency in fdemon-app's `[dependencies]`
- [ ] Single `spawn_internal()` in FlutterProcess
- [ ] No `try_lock()` in session task tracking
- [ ] Consistent DaemonMessage import paths across codebase

---

## References

- [Phase 3 Review](../../reviews/features/workspace-restructure-phase-3/REVIEW.md)
- [Workspace Restructure Plan](../workspace-restructure/PLAN.md)
- [Architecture](../../../../docs/ARCHITECTURE.md)
- [Code Standards](../../../../docs/CODE_STANDARDS.md)

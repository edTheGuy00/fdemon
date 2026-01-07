# Phase 1: Mock Daemon Foundation - Task Index

## Overview

Create mock daemon infrastructure enabling fast, deterministic integration tests without Flutter installation. This establishes the foundation for comprehensive E2E testing with <30 second execution time.

**Total Tasks:** 13 (7 complete + 6 follow-up)
**Parent Plan:** [../PLAN.md](../PLAN.md)

## Task Dependency Graph

```
┌─────────────────────────┐
│  01-add-dependencies    │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐     ┌─────────────────────────┐
│  02-create-fixtures     │     │  03-test-utilities      │
└───────────┬─────────────┘     └───────────┬─────────────┘
            │                               │
            └───────────┬───────────────────┘
                        ▼
            ┌─────────────────────────┐
            │  04-mock-daemon         │
            └───────────┬─────────────┘
                        │
        ┌───────────────┼───────────────┐
        ▼               ▼               ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│ 05-daemon-    │ │ 06-hot-reload │ │ 07-session-   │
│ interaction   │ │ -tests        │ │ management    │
└───────────────┘ └───────────────┘ └───────────────┘

─────────────────── Follow-up Tasks ───────────────────

┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│ 08-event-     │ │ 09-channel-   │ │ 10-select-    │
│ queue-perf    │ │ error-handle  │ │ race-fix      │
└───────────────┘ └───────────────┘ └───────────────┘
       (Critical)        (Critical)        (Critical)

┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│ 11-expect-    │ │ 12-document-  │ │ 13-config-    │
│ context       │ │ limitations   │ │ timeout       │
└───────────────┘ └───────────────┘ └───────────────┘
       (Major)           (Major)          (Minor)
```

## Tasks

### Initial Implementation (Complete)

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-dependencies](tasks/01-add-dependencies.md) | Done | - | `Cargo.toml` |
| 2 | [02-create-fixtures](tasks/02-create-fixtures.md) | Done | 1 | `tests/fixtures/` |
| 3 | [03-test-utilities](tasks/03-test-utilities.md) | Done | 1 | `tests/e2e/mod.rs` |
| 4 | [04-mock-daemon](tasks/04-mock-daemon.md) | Done | 2, 3 | `tests/e2e/mock_daemon.rs` |
| 5 | [05-daemon-interaction-tests](tasks/05-daemon-interaction-tests.md) | Done | 4 | `tests/e2e/daemon_interaction.rs` |
| 6 | [06-hot-reload-tests](tasks/06-hot-reload-tests.md) | Done | 4 | `tests/e2e/hot_reload.rs` |
| 7 | [07-session-management-tests](tasks/07-session-management-tests.md) | Done | 4 | `tests/e2e/session_management.rs` |

### Follow-up Tasks (From Code Review)

These tasks address issues identified in [REVIEW.md](../../REVIEW.md).

| # | Task | Status | Priority | Depends On | Modules |
|---|------|--------|----------|------------|---------|
| 8 | [08-fix-event-queue-performance](tasks/08-fix-event-queue-performance.md) | Not Started | Critical | - | `tests/e2e/mock_daemon.rs` |
| 9 | [09-fix-channel-error-handling](tasks/09-fix-channel-error-handling.md) | Not Started | Critical | - | `tests/e2e/mock_daemon.rs` |
| 10 | [10-fix-select-race-condition](tasks/10-fix-select-race-condition.md) | Not Started | Critical | - | `tests/e2e/mock_daemon.rs` |
| 11 | [11-add-expect-context](tasks/11-add-expect-context.md) | Not Started | Major | - | `tests/e2e/mock_daemon.rs` |
| 12 | [12-document-mock-limitations](tasks/12-document-mock-limitations.md) | Not Started | Major | - | `tests/e2e/mock_daemon.rs` |
| 13 | [13-add-configurable-timeout](tasks/13-add-configurable-timeout.md) | Not Started | Minor | - | `tests/e2e/mock_daemon.rs` |

## Parallel Execution Opportunities

### Initial Implementation (Complete)

**Wave 1 (Sequential):**
- Task 01: Add dependencies

**Wave 2 (Parallel):**
- Task 02: Create fixtures
- Task 03: Test utilities

**Wave 3 (Sequential):**
- Task 04: Mock daemon implementation

**Wave 4 (Parallel):**
- Task 05: Daemon interaction tests
- Task 06: Hot reload tests
- Task 07: Session management tests

### Follow-up Tasks

**Wave 5 (Parallel - All Critical):**
- Task 08: Fix event queue performance (VecDeque)
- Task 09: Fix channel error handling
- Task 10: Fix select race condition

**Wave 6 (Parallel - Quality & Docs):**
- Task 11: Add expect() context
- Task 12: Document mock limitations
- Task 13: Add configurable timeout

## Success Criteria

### Initial Implementation (Complete)

Phase 1 initial implementation is complete:

- [x] `mockall = "0.13"` added to `[dev-dependencies]`
- [x] JSON fixture files created for daemon responses
- [x] `MockFlutterDaemon` simulates core JSON-RPC protocol events
- [x] Test utilities provide helpers for `Device`, `Session`, `AppState`
- [x] 10+ integration tests covering:
  - [x] Device discovery flow (2+ tests)
  - [x] Daemon connection/disconnection (2+ tests)
  - [x] Hot reload trigger and completion (3+ tests)
  - [x] Session lifecycle (3+ tests)
- [x] Tests run in <30 seconds without Flutter installed
- [x] `cargo test --test e2e` passes on clean checkout
- [x] No regressions in existing unit tests

### Follow-up Tasks (From Code Review)

Phase 1 follow-up is complete when:

- [ ] Event queue uses `VecDeque::pop_front()` for O(1) performance (Task 8)
- [ ] Channel send failures are handled, not silently ignored (Task 9)
- [ ] `tokio::select!` race condition is fixed (Task 10)
- [ ] All `.unwrap()` calls have `.expect()` context (Task 11)
- [ ] Mock limitations are documented in module-level docs (Task 12)
- [ ] Timeout is configurable for CI environments (Task 13)
- [ ] All 56+ tests continue to pass
- [ ] `cargo clippy --test e2e` passes with no warnings

## Test Execution

```bash
# Run all E2E tests
cargo test --test e2e

# Run specific test file
cargo test --test e2e daemon_interaction

# Run with output
cargo test --test e2e -- --nocapture
```

## Notes

- The mock daemon does NOT require trait extraction from `FlutterProcess` - it operates at the channel/event level
- Tests use `tokio::test` for async support
- Fixtures are recorded JSON responses from real Flutter daemon for accuracy
- Focus on handler state transitions, not UI rendering (covered by widget tests)

## Code Review Summary

The initial implementation was reviewed on 2026-01-07 and **approved with concerns**. See [REVIEW.md](../../REVIEW.md) for the full review.

**Key Findings:**
- **Architecture:** PASS - No layer violations, proper test organization
- **Code Quality:** APPROVED (minor) - Strong quality, minor inefficiencies
- **Logic Reasoning:** CONCERNS - Critical issues in mock daemon event loop
- **Risks & Tradeoffs:** ACCEPTABLE - With documentation requirements

**Critical Issues Identified:**
1. `tokio::select!` else branch race condition (Task 10)
2. Unchecked `event_tx.send()` failures (Task 9)
3. O(n) event queue performance (Task 8)

**Recommendations:**
- Document mock limitations (Task 12)
- Add configurable timeouts for CI (Task 13)
- Replace `.unwrap()` with `.expect()` (Task 11)

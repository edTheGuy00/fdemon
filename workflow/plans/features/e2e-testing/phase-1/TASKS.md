# Phase 1: Mock Daemon Foundation - Task Index

## Overview

Create mock daemon infrastructure enabling fast, deterministic integration tests without Flutter installation. This establishes the foundation for comprehensive E2E testing with <30 second execution time.

**Total Tasks:** 7
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
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-dependencies](tasks/01-add-dependencies.md) | Not Started | - | `Cargo.toml` |
| 2 | [02-create-fixtures](tasks/02-create-fixtures.md) | Not Started | 1 | `tests/fixtures/` |
| 3 | [03-test-utilities](tasks/03-test-utilities.md) | Not Started | 1 | `tests/e2e/mod.rs` |
| 4 | [04-mock-daemon](tasks/04-mock-daemon.md) | Not Started | 2, 3 | `tests/e2e/mock_daemon.rs` |
| 5 | [05-daemon-interaction-tests](tasks/05-daemon-interaction-tests.md) | Not Started | 4 | `tests/e2e/daemon_interaction.rs` |
| 6 | [06-hot-reload-tests](tasks/06-hot-reload-tests.md) | Not Started | 4 | `tests/e2e/hot_reload.rs` |
| 7 | [07-session-management-tests](tasks/07-session-management-tests.md) | Not Started | 4 | `tests/e2e/session_management.rs` |

## Parallel Execution Opportunities

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

## Success Criteria

Phase 1 is complete when:

- [ ] `mockall = "0.13"` added to `[dev-dependencies]`
- [ ] JSON fixture files created for daemon responses
- [ ] `MockFlutterDaemon` simulates core JSON-RPC protocol events
- [ ] Test utilities provide helpers for `Device`, `Session`, `AppState`
- [ ] 10+ integration tests covering:
  - [ ] Device discovery flow (2+ tests)
  - [ ] Daemon connection/disconnection (2+ tests)
  - [ ] Hot reload trigger and completion (3+ tests)
  - [ ] Session lifecycle (3+ tests)
- [ ] Tests run in <30 seconds without Flutter installed
- [ ] `cargo test --test e2e` passes on clean checkout
- [ ] No regressions in existing unit tests

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

# Phase 3: PTY-Based TUI Testing - Task Index

## Overview

Enable full end-to-end testing of keyboard input and terminal output using PTY interaction. This phase adds the `expectrl` crate for PTY-based testing and `insta` for snapshot testing, enabling automated verification of TUI behavior including keyboard navigation, hot reload triggers, and session management.

**Total Tasks:** 12
**Parent Plan:** [../PLAN.md](../PLAN.md)
**Prerequisite:** Phase 2 complete (Docker infrastructure, headless mode)

## Task Dependency Graph

```
Wave 1: Dependencies & Foundation
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  01-add-pty-dependencies    │     │  02-pty-test-utilities      │
└─────────────┬───────────────┘     └─────────────┬───────────────┘
              │                                   │
              └───────────────┬───────────────────┘
                              │
Wave 2: Basic TUI Tests       ▼
              ┌───────────────────────────────────┐
              │                                   │
              ▼                                   ▼
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  03-test-startup-header     │     │  04-test-device-selector    │
└─────────────┬───────────────┘     └─────────────┬───────────────┘
              │                                   │
              └───────────────┬───────────────────┘
                              │
Wave 3: Keyboard Input Tests  ▼
              ┌───────────────┬───────────────────┐
              │               │                   │
              ▼               ▼                   ▼
┌────────────────────┐ ┌────────────────────┐ ┌────────────────────┐
│ 05-test-reload-key │ │ 06-test-session-   │ │ 07-test-quit-key   │
│                    │ │     keys           │ │                    │
└─────────┬──────────┘ └─────────┬──────────┘ └─────────┬──────────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
Wave 4: Snapshot Testing         ▼
              ┌─────────────────────────────────────┐
              │                                     │
              ▼                                     ▼
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  08-snapshot-infrastructure │     │  09-golden-files            │
└─────────────┬───────────────┘     └─────────────┬───────────────┘
              │                                   │
              └───────────────┬───────────────────┘
                              │
Wave 5: Complex Workflows     ▼
              ┌───────────────┬───────────────────┐
              │               │                   │
              ▼               ▼                   ▼
┌────────────────────┐ ┌────────────────────┐ ┌────────────────────┐
│ 10-session-        │ │ 11-multi-session   │ │ 12-error-recovery  │
│     lifecycle      │ │     workflow       │ │     workflow       │
└────────────────────┘ └────────────────────┘ └────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-pty-dependencies](tasks/01-add-pty-dependencies.md) | Not Started | - | `Cargo.toml` |
| 2 | [02-pty-test-utilities](tasks/02-pty-test-utilities.md) | Not Started | 1 | `tests/e2e/pty_utils.rs` |
| 3 | [03-test-startup-header](tasks/03-test-startup-header.md) | Not Started | 2 | `tests/e2e/tui_interaction.rs` |
| 4 | [04-test-device-selector](tasks/04-test-device-selector.md) | Not Started | 2 | `tests/e2e/tui_interaction.rs` |
| 5 | [05-test-reload-key](tasks/05-test-reload-key.md) | Not Started | 3, 4 | `tests/e2e/tui_interaction.rs` |
| 6 | [06-test-session-keys](tasks/06-test-session-keys.md) | Not Started | 3, 4 | `tests/e2e/tui_interaction.rs` |
| 7 | [07-test-quit-key](tasks/07-test-quit-key.md) | Not Started | 3, 4 | `tests/e2e/tui_interaction.rs` |
| 8 | [08-snapshot-infrastructure](tasks/08-snapshot-infrastructure.md) | Not Started | 5, 6, 7 | `tests/e2e/snapshots/` |
| 9 | [09-golden-files](tasks/09-golden-files.md) | Not Started | 8 | `tests/e2e/snapshots/` |
| 10 | [10-session-lifecycle](tasks/10-session-lifecycle.md) | Not Started | 8, 9 | `tests/e2e/tui_workflows.rs` |
| 11 | [11-multi-session-workflow](tasks/11-multi-session-workflow.md) | Not Started | 10 | `tests/e2e/tui_workflows.rs` |
| 12 | [12-error-recovery-workflow](tasks/12-error-recovery-workflow.md) | Not Started | 10 | `tests/e2e/tui_workflows.rs` |

## Parallel Execution Opportunities

**Wave 1 (Foundation):**
- Task 01: Add expectrl and insta dependencies
- Task 02: Create PTY test utilities (depends on 01)

**Wave 2 (Parallel - Basic Tests):**
- Task 03: Test startup header
- Task 04: Test device selector navigation

**Wave 3 (Parallel - Keyboard Tests):**
- Task 05: Test 'r' key reload
- Task 06: Test number keys for sessions
- Task 07: Test 'q' key quit confirmation

**Wave 4 (Parallel - Snapshot Testing):**
- Task 08: Set up snapshot infrastructure
- Task 09: Create golden files

**Wave 5 (Parallel - Complex Workflows):**
- Task 10: Full session lifecycle
- Task 11: Multi-session parallel operations
- Task 12: Error recovery scenarios

## Success Criteria

Phase 3 is complete when:

- [ ] `expectrl` and `insta` dependencies added to Cargo.toml
- [ ] PTY test utilities provide reliable terminal interaction
- [ ] TUI interaction tests verify keyboard input handling:
  - [ ] Startup shows header with project name
  - [ ] Device selector responds to arrow keys and Enter
  - [ ] 'r' key triggers hot reload
  - [ ] Number keys (1-9) switch sessions
  - [ ] 'q' key shows quit confirmation dialog
- [ ] Snapshot tests catch UI regressions:
  - [ ] Golden files for key UI states (startup, running, reloading, error)
  - [ ] Visual regression detection integrated in CI
- [ ] Complex workflow tests verify end-to-end flows:
  - [ ] Session lifecycle: create -> run -> reload -> stop -> remove
  - [ ] Multi-session: parallel reloads, session ordering
  - [ ] Error recovery: daemon crash -> reconnect -> resume
- [ ] All tests pass in CI (Linux Docker environment)
- [ ] Test execution completes in <60 seconds

## Test Execution

```bash
# Run all PTY-based TUI tests
cargo test --test e2e tui_

# Run snapshot tests
cargo test --test e2e snapshot

# Run workflow tests
cargo test --test e2e workflow

# Update snapshots (when intentional changes)
cargo insta test --review

# Run in Docker (matches CI)
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test cargo test --test e2e
```

## Architecture Notes

### PTY vs Mock Daemon Testing

| Aspect | Mock Daemon (Phase 1) | PTY Testing (Phase 3) |
|--------|----------------------|----------------------|
| Speed | Fast (<30s) | Medium (~60s) |
| Coverage | Handler logic | Full TUI interaction |
| Environment | No Flutter needed | Requires fdemon binary |
| Isolation | Message-level | Process-level |
| Use case | State transitions | User workflow verification |

### Test File Organization

```
tests/
├── e2e/
│   ├── mod.rs                    # Existing: exports mock daemon
│   ├── mock_daemon.rs            # Existing: mock daemon implementation
│   ├── daemon_interaction.rs     # Existing: mock daemon tests
│   ├── hot_reload.rs             # Existing: mock daemon tests
│   ├── session_management.rs     # Existing: mock daemon tests
│   ├── pty_utils.rs              # NEW: PTY interaction utilities
│   ├── tui_interaction.rs        # NEW: Keyboard/TUI tests
│   ├── tui_workflows.rs          # NEW: Complex workflow tests
│   └── snapshots/                # NEW: Insta snapshots
│       ├── tui_interaction__startup.snap
│       ├── tui_interaction__running.snap
│       └── ...
```

## Platform Considerations

- **Linux (CI)**: Primary test environment, runs in Docker with Xvfb
- **macOS**: Development testing, native terminal support
- **Windows**: Not supported for PTY tests (use mock daemon tests)

PTY behavior can vary across platforms. Tests should:
1. Use platform-agnostic assertions where possible
2. Skip platform-specific tests with `#[cfg_attr(not(target_os = "linux"), ignore)]`
3. Focus on semantic verification over exact byte matching

## References

- [expectrl Documentation](https://docs.rs/expectrl/latest/expectrl/)
- [insta Documentation](https://docs.rs/insta/latest/insta/)
- [Portable PTY Crate](https://docs.rs/portable-pty/latest/portable_pty/)
- [Phase 1 TASKS.md](../phase-1/TASKS.md) - Mock daemon infrastructure
- [Phase 2 TASKS.md](../phase-2/TASKS.md) - Docker infrastructure

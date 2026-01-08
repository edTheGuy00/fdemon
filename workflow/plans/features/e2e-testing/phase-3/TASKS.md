# Phase 3: PTY-Based TUI Testing - Task Index

## Overview

Enable full end-to-end testing of keyboard input and terminal output using PTY interaction. This phase adds the `expectrl` crate for PTY-based testing and `insta` for snapshot testing, enabling automated verification of TUI behavior including keyboard navigation, hot reload triggers, and session management.

**Total Tasks:** 25
**Parent Plan:** [../PLAN.md](../PLAN.md)
**Prerequisite:** Phase 2 complete (Docker infrastructure, headless mode)

## Task Dependency Graph

```
Wave 1: Dependencies & Foundation
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  01-add-pty-dependencies    │     │  02-pty-test-utilities      │
└─────────────────────────────┘     └─────────────┬───────────────┘
                                                  │
Wave 1.5: Follow-up Fixes (Blocking)              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ 02a-drop     │ │ 02b-quit     │ │ 02c-serial   │ │ 02d-capture  │ │ 02e-quality  │
│ trait        │ │ race fix     │ │ test         │ │ screen       │ │ improvements │
└──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬───────┘
       │                │                │                │                │
       └────────────────┴────────────────┼────────────────┴────────────────┘
                                         │
Wave 2: Basic TUI Tests                  ▼
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
Wave 3.5: Review Fixes (Blocking)▼
              ┌───────────────┬───────────────────┐
              │               │                   │
              ▼               ▼                   ▼
┌────────────────────┐ ┌────────────────────┐ ┌────────────────────┐
│ 07a-fix-double-q   │ │ 07b-extract-magic  │ │ 07h-tokio-sleep    │
│     test           │ │     -numbers       │ │  (minor)           │
└─────────┬──────────┘ └─────────┬──────────┘ └────────────────────┘
          │                      │
          │                      ▼
          │            ┌────────────────────┐
          │            │ 07c-extract-       │
          │            │   termination-help │
          │            └─────────┬──────────┘
          │                      │
          └──────────┬───────────┘
                     ▼
    ┌────────────────────────────────────────────┐
    │               │                            │
    ▼               ▼                            ▼
┌──────────────┐ ┌──────────────┐ ┌────────────────────┐
│ 07d-module   │ │ 07e-quit-flow│ │ 07g-tighten-regex  │
│   -docs      │ │  verification│ │  (minor)           │
└──────┬───────┘ └──────┬───────┘ └────────────────────┘
       │                │
       └────────┬───────┘
                ▼
       ┌──────────────┐
       │ 07f-standard │
       │   -cleanup   │
       └──────┬───────┘
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
| 1 | [01-add-pty-dependencies](tasks/01-add-pty-dependencies.md) | Done | - | `Cargo.toml` |
| 2 | [02-pty-test-utilities](tasks/02-pty-test-utilities.md) | Done | 1 | `tests/e2e/pty_utils.rs` |
| 2a | [02a-implement-drop-trait](tasks/02a-implement-drop-trait.md) | Done | 2 | `tests/e2e/pty_utils.rs` |
| 2b | [02b-fix-quit-race-condition](tasks/02b-fix-quit-race-condition.md) | Done | 2 | `tests/e2e/pty_utils.rs` |
| 2c | [02c-add-test-isolation](tasks/02c-add-test-isolation.md) | Done | 2 | `Cargo.toml`, `tests/e2e/pty_utils.rs` |
| 2d | [02d-fix-capture-screen](tasks/02d-fix-capture-screen.md) | Done | 2 | `tests/e2e/pty_utils.rs` |
| 2e | [02e-code-quality-improvements](tasks/02e-code-quality-improvements.md) | Done | 2 | `tests/e2e/pty_utils.rs` |
| 3 | [03-test-startup-header](tasks/03-test-startup-header.md) | Done | 2a-2e | `tests/e2e/tui_interaction.rs` |
| 4 | [04-test-device-selector](tasks/04-test-device-selector.md) | Done | 2a-2e | `tests/e2e/tui_interaction.rs` |
| 5 | [05-test-reload-key](tasks/05-test-reload-key.md) | Done | 3, 4 | `tests/e2e/tui_interaction.rs` |
| 6 | [06-test-session-keys](tasks/06-test-session-keys.md) | Done | 3, 4 | `tests/e2e/tui_interaction.rs` |
| 7 | [07-test-quit-key](tasks/07-test-quit-key.md) | Done | 3, 4 | `tests/e2e/tui_interaction.rs` |
| 7a | [07a-fix-double-q-test](tasks/07a-fix-double-q-test.md) | Not Started | 7 | `src/app/handler/keys.rs`, `docs/KEYBINDINGS.md` |
| 7b | [07b-extract-magic-numbers](tasks/07b-extract-magic-numbers.md) | Not Started | 7 | `tests/e2e/tui_interaction.rs` |
| 7c | [07c-extract-termination-helper](tasks/07c-extract-termination-helper.md) | Not Started | 7b | `tests/e2e/tui_interaction.rs` |
| 7d | [07d-module-documentation](tasks/07d-module-documentation.md) | Not Started | 7c | `tests/e2e/tui_interaction.rs` |
| 7e | [07e-quit-flow-verification](tasks/07e-quit-flow-verification.md) | Not Started | 7c | `tests/e2e/tui_interaction.rs` |
| 7f | [07f-standardize-cleanup](tasks/07f-standardize-cleanup.md) | Not Started | 7d | `tests/e2e/tui_interaction.rs` |
| 7g | [07g-tighten-regex-patterns](tasks/07g-tighten-regex-patterns.md) | Not Started | 7f | `tests/e2e/tui_interaction.rs` |
| 7h | [07h-tokio-sleep](tasks/07h-tokio-sleep.md) | Not Started | 7b | `tests/e2e/tui_interaction.rs` |
| 8 | [08-snapshot-infrastructure](tasks/08-snapshot-infrastructure.md) | Not Started | 7a-7f | `tests/e2e/snapshots/` |
| 9 | [09-golden-files](tasks/09-golden-files.md) | Not Started | 8 | `tests/e2e/snapshots/` |
| 10 | [10-session-lifecycle](tasks/10-session-lifecycle.md) | Not Started | 8, 9 | `tests/e2e/tui_workflows.rs` |
| 11 | [11-multi-session-workflow](tasks/11-multi-session-workflow.md) | Not Started | 10 | `tests/e2e/tui_workflows.rs` |
| 12 | [12-error-recovery-workflow](tasks/12-error-recovery-workflow.md) | Not Started | 10 | `tests/e2e/tui_workflows.rs` |

## Parallel Execution Opportunities

**Wave 1 (Foundation):**
- Task 01: Add expectrl and insta dependencies
- Task 02: Create PTY test utilities (depends on 01)

**Wave 1.5 (Parallel - Follow-up Fixes):** ⚠️ **Must complete before Wave 2**
- Task 02a: Implement Drop trait for process cleanup
- Task 02b: Fix quit() race condition
- Task 02c: Add test isolation with serial_test
- Task 02d: Fix capture_screen() logic
- Task 02e: Code quality improvements (docs, constants, traits)

**Wave 2 (Parallel - Basic Tests):**
- Task 03: Test startup header
- Task 04: Test device selector navigation

**Wave 3 (Parallel - Keyboard Tests):**
- Task 05: Test 'r' key reload
- Task 06: Test number keys for sessions
- Task 07: Test 'q' key quit confirmation

**Wave 3.5 (Review Fixes - Blocking):** ⚠️ **Must complete before Wave 4**
- Task 07a: Implement double-'q' quick quit feature (CRITICAL)
- Task 07b: Extract magic numbers to constants (CRITICAL)
- Task 07c: Extract termination check helper (CRITICAL, depends on 07b)
- Task 07d: Add module documentation (depends on 07c)
- Task 07e: Improve quit flow verification (depends on 07c)
- Task 07f: Standardize cleanup approach (depends on 07d)
- Task 07g: Tighten regex patterns (MINOR, depends on 07f)
- Task 07h: Use tokio sleep (MINOR, depends on 07b)

**Wave 4 (Parallel - Snapshot Testing):**
- Task 08: Set up snapshot infrastructure
- Task 09: Create golden files

**Wave 5 (Parallel - Complex Workflows):**
- Task 10: Full session lifecycle
- Task 11: Multi-session parallel operations
- Task 12: Error recovery scenarios

## Success Criteria

Phase 3 is complete when:

- [x] `expectrl` and `insta` dependencies added to Cargo.toml
- [x] PTY test utilities provide reliable terminal interaction
- [x] Wave 1 follow-up fixes complete:
  - [x] `FdemonSession` implements `Drop` for cleanup on panic
  - [x] `quit()` uses polling loop with termination verification
  - [x] PTY tests use `#[serial]` for test isolation
  - [x] `capture_screen()` works correctly or behavior documented
  - [x] Public methods have doc comments, magic numbers extracted
- [x] TUI interaction tests verify keyboard input handling:
  - [x] Startup shows header with project name
  - [x] Device selector responds to arrow keys and Enter
  - [x] 'r' key triggers hot reload
  - [x] Number keys (1-9) switch sessions
  - [x] 'q' key shows quit confirmation dialog
- [ ] Wave 3.5 review fixes complete (BLOCKING):
  - [ ] Double-'q' quick quit implemented and `test_double_q_quick_quit` passes
  - [ ] All timing values use named constants (no magic numbers)
  - [ ] Termination check helper function exists and is used everywhere
  - [ ] Module documentation explains test organization and cleanup strategy
  - [ ] Quit flow tests verify dialog appearance before checking exit
  - [ ] Tests use `quit()` for cleanup by default (not `kill()`)
  - [ ] `cargo clippy --test e2e -- -D warnings` passes
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

# Phase 3.5: PTY Test Infrastructure Fix + TestBackend Tests

## Overview

Fix the PTY-based test infrastructure and add ratatui-recommended TestBackend-based tests for fast, reliable TUI verification.

**Total Tasks:** 13
**Parent Plan:** [PLAN.md](PLAN.md)
**Prerequisite:** Phase 3 complete

## Task Dependency Graph

```
Wave 1: Core PTY Fix
┌─────────────────────────────────────┐
│  01-fix-spawn-default               │
│  (Change default to TUI mode)       │
└─────────────────┬───────────────────┘
                  │
Wave 2: PTY Improvements (Parallel)
┌─────────────────┼─────────────────────┐
│                 │                     │
▼                 ▼                     ▼
┌───────────────┐ ┌───────────────────┐ ┌───────────────────┐
│ 02-ci-timeout │ │ 03-test-categori- │ │ 04-retry-config   │
│ -extension    │ │     zation        │ │                   │
└───────────────┘ └───────────────────┘ └───────────────────┘
                  │
Wave 3: PTY Validation
┌─────────────────┴───────────────────┐
│  05-validate-pty-tests              │
└─────────────────┬───────────────────┘
                  │
Wave 4: TestBackend Infrastructure
┌─────────────────┴───────────────────┐
│  06-testbackend-utilities           │
│  (Test harness, helpers, macros)    │
└─────────────────┬───────────────────┘
                  │
Wave 5: Widget Unit Tests (Parallel)
┌─────────┬───────┴───────┬───────────┐
│         │               │           │
▼         ▼               ▼           ▼
┌───────┐ ┌─────────────┐ ┌─────────┐ ┌─────────────┐
│ 07    │ │ 08          │ │ 09      │ │ 10          │
│Header │ │ StatusBar   │ │ Device  │ │ ConfirmDlg  │
│Widget │ │ Widget      │ │ Selector│ │ Widget      │
└───────┘ └─────────────┘ └─────────┘ └─────────────┘
                  │
Wave 6: Full-Screen Tests (Parallel)
┌─────────────────┼───────────────────┐
│                 │                   │
▼                 ▼                   ▼
┌───────────────────┐ ┌───────────────────────┐
│ 11-screen-        │ │ 12-ui-mode-           │
│ snapshots         │ │ transitions           │
└───────────────────┘ └───────────────────────┘
                  │
Wave 7: Final Validation
┌─────────────────┴───────────────────┐
│  13-final-validation                │
│  (All tests pass, coverage report)  │
└─────────────────────────────────────┘
```

## Tasks

### PTY Infrastructure (Tasks 01-05)

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-spawn-default](tasks/01-fix-spawn-default.md) | Done | - | `tests/e2e/pty_utils.rs` |
| 2 | [02-ci-timeout-extension](tasks/02-ci-timeout-extension.md) | Done | 1 | `tests/e2e/pty_utils.rs` |
| 3 | [03-test-categorization](tasks/03-test-categorization.md) | Done | 1 | `tests/e2e/*.rs` |
| 4 | [04-retry-config](tasks/04-retry-config.md) | Done | 1 | `.config/nextest.toml` |
| 5 | [05-validate-all-tests](tasks/05-validate-all-tests.md) | Done | 2, 3, 4 | validation |

### TestBackend Infrastructure (Task 06)

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 6 | [06-testbackend-utilities](tasks/06-testbackend-utilities.md) | Done | 5 | `src/tui/test_utils.rs` |

### Widget Unit Tests (Tasks 07-10)

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 7 | [07-header-widget-tests](tasks/07-header-widget-tests.md) | Done | 6 | `src/tui/widgets/header.rs` |
| 8 | [08-statusbar-widget-tests](tasks/08-statusbar-widget-tests.md) | Done | 6 | `src/tui/widgets/status_bar.rs` |
| 9 | [09-device-selector-tests](tasks/09-device-selector-tests.md) | Done | 6 | `src/tui/widgets/device_selector.rs` |
| 10 | [10-confirm-dialog-tests](tasks/10-confirm-dialog-tests.md) | Done | 6 | `src/tui/widgets/confirm_dialog.rs` |

### Full-Screen Tests (Tasks 11-12)

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 11 | [11-screen-snapshots](tasks/11-screen-snapshots.md) | Done | 7, 8, 9, 10 | `src/tui/render/` |
| 12 | [12-ui-mode-transitions](tasks/12-ui-mode-transitions.md) | Done | 7, 8, 9, 10 | `src/tui/render/` |

### Final Validation (Task 13)

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 13 | [13-final-validation](tasks/13-final-validation.md) | Superseded | 11, 12 | validation |

> **Note:** Task 13 superseded by Phase 3.6 final validation. The Phase 3.5 Wave 4-6 review identified issues that need fixing before meaningful validation. See `phase-3.6/TASKS.md`.

## Parallel Execution Opportunities

**Wave 1 (Sequential):** Task 01 - Core PTY fix
**Wave 2 (Parallel):** Tasks 02, 03, 04 - PTY improvements
**Wave 3 (Sequential):** Task 05 - PTY validation
**Wave 4 (Sequential):** Task 06 - TestBackend infrastructure
**Wave 5 (Parallel):** Tasks 07, 08, 09, 10 - Widget tests
**Wave 6 (Parallel):** Tasks 11, 12 - Full-screen tests
**Wave 7 (Sequential):** Task 13 - Final validation

## Success Criteria

Phase 3.5 is complete when:

### PTY Tests
- [ ] `FdemonSession::spawn()` defaults to TUI mode
- [ ] `spawn_headless()` available for JSON event tests
- [ ] CI has extended timeouts (2x multiplier)
- [ ] Retry mechanism configured (nextest)
- [ ] All PTY tests pass reliably

### TestBackend Tests
- [ ] TestBackend utilities provide easy test setup
- [ ] All major widgets have unit tests:
  - [ ] Header widget (project name, session tabs)
  - [ ] StatusBar widget (phase, device, stats)
  - [ ] DeviceSelector widget (list, selection, navigation)
  - [ ] ConfirmDialog widget (quit confirmation)
- [ ] Full-screen snapshots for all UI modes:
  - [ ] Normal mode
  - [ ] DeviceSelector mode
  - [ ] ConfirmDialog mode
  - [ ] Loading mode
- [ ] UI mode transitions tested

### Overall
- [ ] `cargo test` passes with 0 failures
- [ ] Test execution <30 seconds (TestBackend tests)
- [ ] PTY tests <60 seconds
- [ ] CI pass rate >99%

## Test Execution

```bash
# Run PTY tests only
cargo test --test e2e -- --nocapture

# Run TestBackend widget tests
cargo test tui::widgets --lib

# Run TestBackend render tests
cargo test tui::render --lib

# Run all tests
cargo test

# Run with coverage
cargo tarpaulin --out Html
```

## Architecture Notes

### Test Hierarchy (Best to Worst)

| Type | Speed | Reliability | Use For |
|------|-------|-------------|---------|
| TestBackend unit | ~1ms | 100% | Widget rendering |
| TestBackend snapshot | ~5ms | 100% | Full-screen states |
| Handler unit tests | ~1ms | 100% | State transitions |
| PTY tests | ~5s | ~95% | Critical user flows |

### File Organization

```
src/tui/
├── widgets/
│   ├── header.rs          # Widget + inline tests
│   ├── status_bar.rs      # Widget + inline tests
│   ├── device_selector.rs # Widget + inline tests
│   ├── confirm_dialog.rs  # Widget + inline tests
│   └── ...
├── render.rs              # View function
├── render/
│   └── tests.rs           # Full-screen snapshot tests
└── test_utils.rs          # NEW: TestBackend helpers

tests/e2e/
├── pty_utils.rs           # PTY helpers
├── tui_interaction.rs     # PTY tests (critical flows)
└── tui_workflows.rs       # PTY tests (complex scenarios)
```

## Notes

- TestBackend tests are the ratatui-recommended approach
- They're 1000x faster than PTY tests and 100% reliable
- PTY tests remain for critical user-facing workflows
- Widget tests use inline `#[cfg(test)]` modules
- Full-screen tests use separate test file with insta snapshots

# Phase 3.5: Fix PTY Test Infrastructure + Add TestBackend Tests

## Overview

**Problem**: 25 PTY-based tests are failing with timeouts because they expect TUI output but the default `spawn()` method uses `--headless` mode, which outputs JSON events instead.

**Solution**:
1. Fix PTY infrastructure (change `spawn()` default to TUI mode)
2. Add ratatui-recommended TestBackend tests for fast, reliable TUI verification

## Root Cause Analysis

### PTY Test Failures

Tests expect TUI output ("Flutter Demon", "Running") but `spawn()` uses `--headless` flag:

```
FdemonSession::spawn()
  └── spawn_with_args(&["--headless"])  ← Outputs JSON, not TUI
      └── Tests timeout waiting for text that never appears
```

### The Better Approach: TestBackend

Ratatui recommends TestBackend + insta snapshots over PTY testing:

| Approach | Speed | Reliability | Use Case |
|----------|-------|-------------|----------|
| TestBackend unit | ~1ms | 100% | Widget rendering |
| TestBackend snapshot | ~5ms | 100% | Full-screen states |
| PTY tests | ~5s | ~95% | Critical user workflows |

## Scope

### Part 1: PTY Infrastructure Fix (Tasks 01-05)
- Change `spawn()` default to TUI mode
- Add CI timeout extension (2x multiplier)
- Categorize tests (TUI vs headless)
- Configure nextest retry mechanism
- Validate all PTY tests pass

### Part 2: TestBackend Tests (Tasks 06-13)
- Create TestBackend utilities module
- Add widget unit tests:
  - Header widget
  - StatusBar widget
  - DeviceSelector widget
  - ConfirmDialog widget
- Add full-screen snapshot tests
- Add UI mode transition tests
- Final validation

## Architecture

### Test Hierarchy

```
Fastest, Most Reliable
         │
         ▼
┌─────────────────────────┐
│  TestBackend Unit Tests │  ~1ms, 100% reliable
│  (widget rendering)     │
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│  TestBackend Snapshots  │  ~5ms, 100% reliable
│  (full-screen states)   │
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│  Handler Unit Tests     │  ~1ms, 100% reliable
│  (state transitions)    │  (already exist)
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│  PTY E2E Tests          │  ~5s, ~95% reliable
│  (critical user flows)  │
└─────────────────────────┘
         │
         ▼
Slowest, Least Reliable
```

### File Organization

```
src/tui/
├── widgets/
│   ├── header.rs          # Widget + inline #[cfg(test)] tests
│   ├── status_bar.rs      # Widget + inline #[cfg(test)] tests
│   ├── device_selector.rs # Widget + inline #[cfg(test)] tests
│   ├── confirm_dialog.rs  # Widget + inline #[cfg(test)] tests
│   └── ...
├── render/
│   ├── mod.rs             # View function
│   └── tests.rs           # Full-screen snapshot tests
├── test_utils.rs          # NEW: TestBackend helpers
└── snapshots/             # insta snapshot files

tests/e2e/
├── pty_utils.rs           # PTY helpers (fixed spawn())
├── tui_interaction.rs     # PTY tests (critical flows only)
└── tui_workflows.rs       # PTY tests (complex scenarios)
```

## Success Criteria

### PTY Tests
- [ ] `spawn()` defaults to TUI mode
- [ ] All 25 PTY tests pass
- [ ] CI pass rate >95%
- [ ] Execution time <60s

### TestBackend Tests
- [ ] TestBackend utilities available
- [ ] All major widgets have unit tests
- [ ] Full-screen snapshots for all UI modes
- [ ] UI mode transitions tested
- [ ] Execution time <5s

### Overall
- [ ] `cargo test` passes with 0 failures
- [ ] CI pass rate >99%
- [ ] Total execution time <90s

## Dependencies

- Phase 3 complete (all existing tasks done)
- insta crate (already added in Phase 3)
- ratatui TestBackend (already available)

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| PTY tests still flaky | Medium | Medium | TestBackend provides reliable alternative |
| Widget API changes break tests | Low | Low | Tests catch regressions early |
| Snapshot maintenance burden | Low | Low | Only update when intentional changes |

## References

- [Ratatui Snapshot Testing Guide](https://ratatui.rs/recipes/testing/snapshots/)
- [expectrl Documentation](https://docs.rs/expectrl)
- [insta Documentation](https://insta.rs/docs/)
- Phase 3 TASKS.md (prerequisite tasks)

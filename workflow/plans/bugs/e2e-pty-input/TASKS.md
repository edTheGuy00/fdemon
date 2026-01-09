# E2E PTY Input Issue - Task Index

## Overview

Investigate and fix the PTY input issue where Enter/Space keys don't trigger expected actions in E2E tests, while other keys (j/k, Escape, arrows) work correctly.

**Total Tasks:** 4
**Parent Plan:** [BUG.md](BUG.md)

## Task Dependency Graph

```
┌─────────────────────────────────────────┐
│  01-investigate-event-kinds             │
│  (Add logging to understand events)     │
└─────────────────────┬───────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────┐
│  02-test-alternative-sequences          │
│  (Try different Enter/Space bytes)      │
└─────────────────────┬───────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────┐
│  03-implement-fix                       │
│  (Apply working solution)               │
└─────────────────────┬───────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────┐
│  04-enable-toggle-tests                 │
│  (Remove #[ignore] from tests)          │
└─────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-investigate-event-kinds](tasks/01-investigate-event-kinds.md) | Not Started | - | `src/tui/event.rs` |
| 2 | [02-test-alternative-sequences](tasks/02-test-alternative-sequences.md) | Not Started | 1 | `tests/e2e/pty_utils.rs` |
| 3 | [03-implement-fix](tasks/03-implement-fix.md) | Not Started | 2 | TBD based on findings |
| 4 | [04-enable-toggle-tests](tasks/04-enable-toggle-tests.md) | Not Started | 3 | `tests/e2e/settings_page.rs` |

## Success Criteria

Bug fix is complete when:

- [ ] Root cause identified and documented
- [ ] Enter key triggers toggle in E2E tests
- [ ] Space key triggers toggle in E2E tests
- [ ] All toggle tests pass (currently ignored)
- [ ] No regressions in other E2E tests
- [ ] Solution doesn't break real terminal usage

## Quick Experiments

Before diving into full investigation, try these quick tests:

### Experiment A: Different Enter Byte
```rust
// In tests/e2e/pty_utils.rs, change:
SpecialKey::Enter => b"\r",
// To:
SpecialKey::Enter => b"\n",
```

### Experiment B: Use send_line
```rust
// Add method to FdemonSession:
pub fn send_enter(&mut self) -> PtyResult<()> {
    self.session.send_line("")?;
    Ok(())
}
```

### Experiment C: Accept All Key Kinds
```rust
// In src/tui/event.rs, change:
Event::Key(key) if key.kind == event::KeyEventKind::Press => {
// To:
Event::Key(key) => {
```

## Notes

- Unit tests for toggle functionality pass - this is purely an E2E test infrastructure issue
- Other keys (j, k, Escape, arrows) work correctly in E2E tests
- The issue may be specific to crossterm's handling of PTY input
- Consider checking crossterm's GitHub issues for similar reports

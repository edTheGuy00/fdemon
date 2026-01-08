## Task: Add StatusBar Widget Tests

**Objective**: Add TestBackend-based unit tests for the StatusBar and StatusBarCompact widgets to verify phase, device, and statistics display.

**Depends on**: 06-testbackend-utilities

### Scope

- `src/tui/widgets/status_bar.rs`: Add inline test module

### Details

#### 1. Review StatusBar Widget

The StatusBar displays:
- Current phase (Initializing, Running, Reloading, Error, etc.)
- Device name
- Reload count
- Hot reload timing
- Keybinding hints

StatusBarCompact shows abbreviated info for small terminals.

#### 2. Add Test Module

Add to `src/tui/widgets/status_bar.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_utils::{TestTerminal, create_test_state, create_test_state_with_name};
    use crate::core::AppPhase;

    #[test]
    fn test_statusbar_renders_phase() {
        let mut term = TestTerminal::new();
        let mut state = create_test_state();
        state.phase = AppPhase::Running;

        let status_bar = StatusBar::new(&state);
        term.render_widget(status_bar, term.area());

        assert!(
            term.buffer_contains("Running") || term.buffer_contains("RUNNING"),
            "Status bar should show Running phase"
        );
    }

    #[test]
    fn test_statusbar_renders_device_name() {
        let mut term = TestTerminal::new();
        let mut state = create_test_state();
        state.device_name = Some("iPhone 15 Pro".to_string());

        let status_bar = StatusBar::new(&state);
        term.render_widget(status_bar, term.area());

        assert!(
            term.buffer_contains("iPhone") || term.buffer_contains("15"),
            "Status bar should show device name"
        );
    }

    #[test]
    fn test_statusbar_renders_reload_count() {
        let mut term = TestTerminal::new();
        let mut state = create_test_state();
        state.reload_count = 5;
        state.phase = AppPhase::Running;

        let status_bar = StatusBar::new(&state);
        term.render_widget(status_bar, term.area());

        // May show as "Reloads: 5" or "5 reloads" or similar
        assert!(
            term.buffer_contains("5"),
            "Status bar should show reload count"
        );
    }

    #[test]
    fn test_statusbar_phase_initializing() {
        let mut term = TestTerminal::new();
        let mut state = create_test_state();
        state.phase = AppPhase::Initializing;

        let status_bar = StatusBar::new(&state);
        term.render_widget(status_bar, term.area());

        assert!(
            term.buffer_contains("Initializing") || term.buffer_contains("Init"),
            "Should show initializing phase"
        );
    }

    #[test]
    fn test_statusbar_phase_reloading() {
        let mut term = TestTerminal::new();
        let mut state = create_test_state();
        state.phase = AppPhase::Reloading;

        let status_bar = StatusBar::new(&state);
        term.render_widget(status_bar, term.area());

        assert!(
            term.buffer_contains("Reloading") || term.buffer_contains("Reload"),
            "Should show reloading phase"
        );
    }

    #[test]
    fn test_statusbar_phase_error() {
        let mut term = TestTerminal::new();
        let mut state = create_test_state();
        state.phase = AppPhase::Error;

        let status_bar = StatusBar::new(&state);
        term.render_widget(status_bar, term.area());

        assert!(
            term.buffer_contains("Error") || term.buffer_contains("ERROR"),
            "Should show error phase"
        );
    }

    #[test]
    fn test_statusbar_no_device() {
        let mut term = TestTerminal::new();
        let mut state = create_test_state();
        state.device_name = None;

        let status_bar = StatusBar::new(&state);
        term.render_widget(status_bar, term.area());

        // Should render without panic
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_statusbar_compact() {
        let mut term = TestTerminal::compact();
        let state = create_test_state();

        let status_bar = StatusBarCompact::new(&state);
        term.render_widget(status_bar, term.area());

        // Compact bar should fit in small terminal
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_statusbar_compact_vs_full() {
        let state = create_test_state();

        let mut term_full = TestTerminal::new();
        let mut term_compact = TestTerminal::compact();

        term_full.render_widget(StatusBar::new(&state), term_full.area());
        term_compact.render_widget(StatusBarCompact::new(&state), term_compact.area());

        // Both should render, but content differs
        assert!(!term_full.content().is_empty());
        assert!(!term_compact.content().is_empty());
    }
}
```

### Test Coverage

| Test Case | Verifies |
|-----------|----------|
| `test_statusbar_renders_phase` | Phase indicator |
| `test_statusbar_renders_device_name` | Device display |
| `test_statusbar_renders_reload_count` | Reload counter |
| `test_statusbar_phase_*` | All phase states |
| `test_statusbar_no_device` | None device handling |
| `test_statusbar_compact` | Compact variant works |
| `test_statusbar_compact_vs_full` | Both variants render |

### Acceptance Criteria

1. All phase states have test coverage
2. Device name display tested (Some/None)
3. Reload count display tested
4. Compact variant tested
5. All tests pass in <10ms each

### Testing

```bash
# Run status bar tests
cargo test widgets::status_bar --lib -- --nocapture
```

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/status_bar.rs` | Added 9 TestTerminal-based unit tests (lines 864-1030) |

### Notable Decisions/Tradeoffs

1. **Adapted "Error" phase test**: The task template included a test for `AppPhase::Error`, but this phase doesn't exist in the codebase. Instead, added test for `AppPhase::Stopped` to cover all actual phase states (Initializing, Running, Reloading, Stopped, Quitting).

2. **Device name test adjustment**: The status bar doesn't directly display device names (it shows config info like Debug/Profile/Release instead). The test verifies that the widget renders correctly when a session with a device is present.

3. **Reload count test adjustment**: The status bar shows reload timing (last reload time) rather than a reload count. The test verifies this timing display renders correctly.

### Testing Performed

- `cargo test widgets::status_bar --lib -- --nocapture` - Passed (34 tests, including 9 new TestTerminal-based tests)
- All tests complete in <10ms each as required
- `cargo clippy -- status_bar.rs` - Passed (no warnings)

### Risks/Limitations

None identified. All tests pass and meet performance requirements.

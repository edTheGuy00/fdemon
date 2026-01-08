## Task: Add Header Widget Tests

**Objective**: Add TestBackend-based unit tests for the MainHeader widget to verify project name display and session tab rendering.

**Depends on**: 06-testbackend-utilities

### Scope

- `src/tui/widgets/header.rs`: Add inline test module

### Details

#### 1. Review Header Widget

The `MainHeader` widget displays:
- "Flutter Demon" or "fdemon" title
- Project name
- Session tabs (when multiple sessions)

#### 2. Add Test Module

Add to `src/tui/widgets/header.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_utils::TestTerminal;
    use crate::app::session_manager::SessionManager;

    #[test]
    fn test_header_renders_title() {
        let mut term = TestTerminal::new();
        let header = MainHeader::new(None);

        term.render_widget(header, term.area());

        // Should contain app name
        assert!(
            term.buffer_contains("Flutter Demon") || term.buffer_contains("fdemon"),
            "Header should contain app title"
        );
    }

    #[test]
    fn test_header_renders_project_name() {
        let mut term = TestTerminal::new();
        let header = MainHeader::new(Some("my_flutter_app"));

        term.render_widget(header, term.area());

        assert!(
            term.buffer_contains("my_flutter_app"),
            "Header should contain project name"
        );
    }

    #[test]
    fn test_header_without_project_name() {
        let mut term = TestTerminal::new();
        let header = MainHeader::new(None);

        term.render_widget(header, term.area());

        // Should still render without crashing
        let content = term.content();
        assert!(!content.is_empty(), "Header should render something");
    }

    #[test]
    fn test_header_with_sessions() {
        let mut term = TestTerminal::new();
        let mut session_manager = SessionManager::new();

        // Add mock sessions (may need helper function)
        // session_manager.add_session(...);

        let header = MainHeader::new(Some("test_app"))
            .with_sessions(&session_manager);

        term.render_widget(header, term.area());

        // Verify session tabs appear
        // assert!(term.buffer_contains("[1]"));
    }

    #[test]
    fn test_header_truncates_long_project_name() {
        let mut term = TestTerminal::with_size(40, 5); // Narrow terminal
        let long_name = "this_is_a_very_long_flutter_project_name_that_should_truncate";
        let header = MainHeader::new(Some(long_name));

        term.render_widget(header, term.area());

        // Should not overflow - verify no panic and content fits
        let content = term.content();
        assert!(content.len() > 0, "Should render without panic");
    }

    #[test]
    fn test_header_compact_mode() {
        let mut term = TestTerminal::compact();
        let header = MainHeader::new(Some("app"));

        term.render_widget(header, term.area());

        // Should adapt to compact size
        let content = term.content();
        assert!(!content.is_empty());
    }
}
```

### Test Coverage

| Test Case | Verifies |
|-----------|----------|
| `test_header_renders_title` | App name appears |
| `test_header_renders_project_name` | Project name appears |
| `test_header_without_project_name` | Handles None gracefully |
| `test_header_with_sessions` | Session tabs render |
| `test_header_truncates_long_name` | Long names don't overflow |
| `test_header_compact_mode` | Works in small terminals |

### Acceptance Criteria

1. All test cases pass
2. Tests are fast (<10ms each)
3. No panics on edge cases (None, long names, small terminal)
4. Tests document expected behavior

### Testing

```bash
# Run header tests
cargo test widgets::header --lib -- --nocapture

# Run with verbose output
cargo test widgets::header --lib -- --nocapture --show-output
```

### Notes

- Adjust tests based on actual MainHeader API
- May need to create mock SessionManager or session helpers
- Focus on rendering correctness, not styling

---

## Completion Summary

**Status:** Not Started

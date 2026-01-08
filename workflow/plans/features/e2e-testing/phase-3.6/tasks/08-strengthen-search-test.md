## Task: Strengthen SearchInput Snapshot Test

**Objective**: Replace the weak `content.len() > 0` assertion in the SearchInput snapshot test with proper content validation.

**Depends on**: 06-improve-testterminal

### Scope

- `src/tui/render/tests.rs`: Lines ~226-238 (SearchInput snapshot test)

### Details

**Current implementation (weak):**
```rust
#[test]
fn test_search_input_screen() {
    let mut term = TestTerminal::new();
    let mut state = create_test_state_with_name("SearchTest");
    state.ui_mode = UiMode::SearchInput;

    term.draw_with(|frame| view(frame, &state)).unwrap();

    let content = term.content();
    assert!(content.len() > 0);  // Weak: just checks non-empty
}
```

**Problem:**
- The assertion only checks that *something* rendered
- Doesn't verify search input UI is actually visible
- A blank screen would fail, but wrong content would pass

**Improved implementation:**
```rust
#[test]
fn test_search_input_screen() {
    let mut term = TestTerminal::new();
    let mut state = create_test_state_with_name("SearchTest");

    // Set up search mode with a query
    state.ui_mode = UiMode::SearchInput;
    state.search_query = "test query".to_string();

    term.draw_with(|frame| view(frame, &state)).unwrap();

    let content = term.content();

    // Verify search UI elements are visible
    assert!(
        content.contains("Search") || content.contains("search") || content.contains("/"),
        "Search mode should show search indicator"
    );

    // Verify the query is displayed
    assert!(
        content.contains("test query"),
        "Search input should display the current query"
    );

    // Optionally, create a snapshot
    insta::assert_snapshot!(content);
}
```

**Alternative: Create session with actual log content**

If SearchInput mode requires a session for meaningful display:
```rust
#[test]
fn test_search_input_with_logs() {
    let mut term = TestTerminal::new();
    let mut state = create_test_state_with_name("SearchTest");

    // Add some logs to search through
    add_test_log(&mut state, "First log message");
    add_test_log(&mut state, "Second log with searchable content");
    add_test_log(&mut state, "Third test message");

    // Enter search mode
    state.ui_mode = UiMode::SearchInput;
    state.search_query = "searchable".to_string();

    term.draw_with(|frame| view(frame, &state)).unwrap();

    // Verify search highlights or focuses the matching log
    // ...
}
```

### Acceptance Criteria

1. Test verifies search UI indicator is visible
2. Test verifies search query is displayed (when set)
3. Assertion failure messages are descriptive
4. Test has snapshot for regression detection
5. All tests pass

### Testing

```bash
# Run the specific test
cargo test --lib test_search_input

# Run all render tests
cargo test --lib render

# Update snapshot if needed
cargo insta review
```

### Notes

- Review the actual SearchInput UI to determine what should be asserted
- May need to examine `UiMode::SearchInput` handling in render code
- If search mode requires specific state, set it up properly

---

## Completion Summary

**Status:** ❌ Not done

**Files Modified:**
- (pending)

**Testing Performed:**
- (pending)

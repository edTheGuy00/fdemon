## Task: Add loading indicator during entry point discovery

**Objective**: Show visual feedback (spinner) in the entry point fuzzy modal while discovery is in progress.

**Depends on**: Task 09 (async discovery must be in place)

### Scope

- `src/tui/widgets/new_session_dialog/fuzzy_modal.rs`: Render loading state
- `src/app/new_session_dialog/types.rs`: Expose loading state to widget

### Details

When entry point discovery is running asynchronously, users need visual feedback. This follows the existing loading spinner pattern used in the target selector.

#### 1. Check loading state in render

The fuzzy modal widget should check if entry points are loading and show a spinner:

```rust
// In tui/widgets/new_session_dialog/fuzzy_modal.rs

fn render_fuzzy_modal(
    frame: &mut Frame,
    area: Rect,
    state: &NewSessionDialogState,
) {
    if let Some(ref modal) = state.fuzzy_modal {
        // ... existing modal rendering ...

        // Check if this is an entry point modal that's loading
        let is_loading = modal.modal_type == FuzzyModalType::EntryPoint
            && state.launch_context.entry_points_loading;

        if is_loading {
            // Render spinner instead of items
            let spinner = throbber_widgets_tui::Throbber::default()
                .label("Discovering entry points...")
                .style(Style::default().fg(Color::Cyan));

            // Center the spinner in the list area
            let spinner_area = centered_rect(list_area, 30, 3);
            frame.render_widget(spinner, spinner_area);
        } else {
            // Normal rendering of filtered items
            // ... existing list rendering code ...
        }
    }
}
```

#### 2. Alternative: Show loading in title

A simpler approach is to modify the modal title during loading:

```rust
// In fuzzy_modal.rs

fn modal_title(modal: &FuzzyModalState, launch_context: &LaunchContextState) -> String {
    match modal.modal_type {
        FuzzyModalType::Config => "Select Configuration".to_string(),
        FuzzyModalType::Flavor => "Select Flavor".to_string(),
        FuzzyModalType::EntryPoint => {
            if launch_context.entry_points_loading {
                "Entry Point (discovering...)".to_string()
            } else {
                "Select Entry Point".to_string()
            }
        }
    }
}
```

#### 3. Recommended approach: Both

Show "(discovering...)" in the title AND replace the list with a spinner for best UX.

### Acceptance Criteria

1. Loading spinner visible during entry point discovery
2. Spinner replaced with items when discovery completes
3. Title indicates loading state
4. User can close modal during loading (Esc key)
5. No visual glitches when transitioning loading → loaded
6. Consistent with existing loading patterns (device loading)
7. Code compiles without warnings

### Testing

Manual testing is recommended for UI changes:

1. Open NewSessionDialog
2. Navigate to Entry Point field
3. Press Enter to open modal
4. Observe spinner while discovery runs
5. Verify items appear when discovery completes
6. Verify Esc closes modal during loading

Unit test for state logic:

```rust
#[test]
fn test_modal_shows_loading_for_entry_point() {
    let mut state = NewSessionDialogState::default();

    // Open modal with loading state
    state.fuzzy_modal = Some(FuzzyModalState::new(
        FuzzyModalType::EntryPoint,
        vec![],
    ));
    state.launch_context.entry_points_loading = true;

    // Verify loading is detectable
    assert!(is_entry_point_modal_loading(&state));
}

fn is_entry_point_modal_loading(state: &NewSessionDialogState) -> bool {
    state.fuzzy_modal.as_ref()
        .map(|m| m.modal_type == FuzzyModalType::EntryPoint && state.launch_context.entry_points_loading)
        .unwrap_or(false)
}
```

### Notes

- Follow existing spinner usage in target selector widget
- The `throbber-widgets-tui` crate is already a dependency
- Keep the loading indicator simple - this is a short operation
- Discovery typically completes in < 500ms, so spinner may flash briefly
- Consider debouncing spinner appearance for very fast discovery (optional optimization)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/fuzzy_modal.rs` | Added loading state parameter to FuzzyModal widget, modified title to show "(discovering...)" when loading for EntryPoint modal, show loading message in list area instead of items when loading, added 4 unit tests |
| `src/tui/widgets/new_session_dialog/mod.rs` | Updated render_fuzzy_modal_overlay to detect entry point loading state from launch_context and pass to FuzzyModal widget via loading() builder method |

### Notable Decisions/Tradeoffs

1. **Simple text-based loading instead of spinner**: Since `throbber-widgets-tui` is not actually in the dependencies (despite task notes), I followed the existing pattern from target_selector which uses simple text-based loading indicators. This is consistent with the codebase and avoids adding new dependencies.

2. **Builder pattern for loading state**: Added `.loading(bool)` builder method to FuzzyModal to make it optional and backwards compatible. All existing call sites work without modification since loading defaults to false.

3. **EntryPoint-specific loading check**: The loading indicator only appears for EntryPoint modals when entry_points_loading is true. Other modal types (Config, Flavor) ignore the loading flag to prevent confusion.

4. **Title and list area both show loading**: Title displays "(discovering...)" suffix and list area shows centered yellow "Discovering entry points..." message, providing clear visual feedback without a spinner.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (no warnings)
- `cargo test --lib fuzzy_modal` - Passed (32 tests including 4 new loading indicator tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### New Tests Added

1. `test_fuzzy_modal_entry_point_loading_title` - Verifies title shows "discovering" during loading
2. `test_fuzzy_modal_entry_point_loading_message` - Verifies list area shows "Discovering entry points..." message
3. `test_fuzzy_modal_entry_point_not_loading` - Verifies normal rendering when not loading
4. `test_fuzzy_modal_other_types_ignore_loading` - Verifies Config/Flavor modals don't show loading even if flag is true

### Risks/Limitations

1. **No actual spinner animation**: The loading indicator is static text rather than an animated spinner. This is acceptable since discovery is expected to complete quickly (< 500ms) and matches existing patterns in the codebase.

2. **ESC during loading**: User can still close modal with Esc during loading (this was a requirement). The async discovery will complete in the background and update the cached entry points, but the modal will be closed.

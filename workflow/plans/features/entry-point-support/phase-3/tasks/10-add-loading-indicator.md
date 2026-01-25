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

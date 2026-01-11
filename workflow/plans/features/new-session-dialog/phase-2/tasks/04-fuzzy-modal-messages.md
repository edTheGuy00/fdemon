## Task: Wire Up Fuzzy Modal Messages and Handlers

**Objective**: Add message types and handlers for fuzzy modal interactions.

**Depends on**: Task 03 (Fuzzy Modal Widget)

**Estimated Time**: 15 minutes

### Background

The fuzzy modal needs messages for opening, closing, navigation, and input. Handlers update the `FuzzyModalState` within `NewSessionDialogState`.

### Scope

- `src/app/message.rs`: Add fuzzy modal messages
- `src/app/handler/update.rs`: Add handlers (stub for now, full implementation in Phase 7)

### Changes Required

**Add to `message.rs`:**

```rust
// ─────────────────────────────────────────────────────────
// NewSessionDialog - Fuzzy Modal Messages
// ─────────────────────────────────────────────────────────

/// Open fuzzy search modal for config or flavor selection
NewSessionDialogOpenFuzzyModal { modal_type: FuzzyModalType },

/// Close fuzzy modal without selecting
NewSessionDialogCloseFuzzyModal,

/// Navigate up in fuzzy modal list
NewSessionDialogFuzzyUp,

/// Navigate down in fuzzy modal list
NewSessionDialogFuzzyDown,

/// Confirm selection in fuzzy modal
NewSessionDialogFuzzyConfirm,

/// Input character in fuzzy modal
NewSessionDialogFuzzyInput { c: char },

/// Backspace in fuzzy modal
NewSessionDialogFuzzyBackspace,

/// Clear fuzzy modal query
NewSessionDialogFuzzyClear,
```

**Add import:**

```rust
use crate::tui::widgets::new_session_dialog::FuzzyModalType;
```

**Add handlers in `update.rs`:**

```rust
// ─────────────────────────────────────────────────────────
// NewSessionDialog - Fuzzy Modal Handlers
// ─────────────────────────────────────────────────────────

Message::NewSessionDialogOpenFuzzyModal { modal_type } => {
    let items = match modal_type {
        FuzzyModalType::Config => {
            state.new_session_dialog_state.configs.configs
                .iter()
                .map(|c| c.display_name.clone())
                .collect()
        }
        FuzzyModalType::Flavor => {
            // TODO: Get flavors from project analysis
            // For now, use any existing flavor as suggestion
            let mut flavors = Vec::new();
            if !state.new_session_dialog_state.flavor.is_empty() {
                flavors.push(state.new_session_dialog_state.flavor.clone());
            }
            flavors
        }
    };

    state.new_session_dialog_state.open_fuzzy_modal(modal_type, items);
    UpdateResult::none()
}

Message::NewSessionDialogCloseFuzzyModal => {
    state.new_session_dialog_state.close_fuzzy_modal();
    UpdateResult::none()
}

Message::NewSessionDialogFuzzyUp => {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.navigate_up();
    }
    UpdateResult::none()
}

Message::NewSessionDialogFuzzyDown => {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.navigate_down();
    }
    UpdateResult::none()
}

Message::NewSessionDialogFuzzyConfirm => {
    if let Some(ref modal) = state.new_session_dialog_state.fuzzy_modal {
        if let Some(value) = modal.selected_value() {
            match modal.modal_type {
                FuzzyModalType::Config => {
                    // Find config index by name
                    let idx = state.new_session_dialog_state.configs.configs
                        .iter()
                        .position(|c| c.display_name == value);
                    state.new_session_dialog_state.select_config(idx);
                }
                FuzzyModalType::Flavor => {
                    state.new_session_dialog_state.flavor = value;
                }
            }
        }
    }
    state.new_session_dialog_state.close_fuzzy_modal();
    UpdateResult::none()
}

Message::NewSessionDialogFuzzyInput { c } => {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.input_char(c);
    }
    UpdateResult::none()
}

Message::NewSessionDialogFuzzyBackspace => {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.backspace();
    }
    UpdateResult::none()
}

Message::NewSessionDialogFuzzyClear => {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.clear_query();
    }
    UpdateResult::none()
}
```

**Update state.rs `open_fuzzy_modal`:**

```rust
impl NewSessionDialogState {
    /// Open fuzzy modal with items
    pub fn open_fuzzy_modal(&mut self, modal_type: FuzzyModalType, items: Vec<String>) {
        self.fuzzy_modal = Some(FuzzyModalState::new(modal_type, items));
    }

    /// Close fuzzy modal
    pub fn close_fuzzy_modal(&mut self) {
        self.fuzzy_modal = None;
    }
}
```

### Acceptance Criteria

1. All fuzzy modal message variants added to `Message` enum
2. Handlers implemented for all messages
3. `open_fuzzy_modal()` creates modal state with correct items
4. Navigation handlers update modal state
5. Confirm handler applies selection and closes modal
6. Input handlers modify query and trigger filter
7. Close handler clears modal state
8. `cargo check` passes
9. `cargo clippy -- -D warnings` passes

### Testing

Handler tests:

```rust
#[cfg(test)]
mod fuzzy_modal_handler_tests {
    use super::*;

    #[test]
    fn test_open_fuzzy_modal_for_flavor() {
        let mut state = AppState::new();
        state.new_session_dialog_state.flavor = "existing".into();

        let _ = update(&mut state, Message::NewSessionDialogOpenFuzzyModal {
            modal_type: FuzzyModalType::Flavor
        });

        assert!(state.new_session_dialog_state.fuzzy_modal.is_some());
        let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
        assert_eq!(modal.modal_type, FuzzyModalType::Flavor);
    }

    #[test]
    fn test_fuzzy_confirm_sets_flavor() {
        let mut state = AppState::new();
        state.new_session_dialog_state.open_fuzzy_modal(
            FuzzyModalType::Flavor,
            vec!["dev".into(), "staging".into()]
        );

        // Select "staging" (index 1)
        let _ = update(&mut state, Message::NewSessionDialogFuzzyDown);
        let _ = update(&mut state, Message::NewSessionDialogFuzzyConfirm);

        assert!(state.new_session_dialog_state.fuzzy_modal.is_none());
        assert_eq!(state.new_session_dialog_state.flavor, "staging");
    }

    #[test]
    fn test_fuzzy_custom_input() {
        let mut state = AppState::new();
        state.new_session_dialog_state.open_fuzzy_modal(
            FuzzyModalType::Flavor,
            vec![]  // Empty list
        );

        // Type custom value
        let _ = update(&mut state, Message::NewSessionDialogFuzzyInput { c: 'c' });
        let _ = update(&mut state, Message::NewSessionDialogFuzzyInput { c: 'u' });
        let _ = update(&mut state, Message::NewSessionDialogFuzzyInput { c: 's' });
        let _ = update(&mut state, Message::NewSessionDialogFuzzyInput { c: 't' });
        let _ = update(&mut state, Message::NewSessionDialogFuzzyConfirm);

        assert_eq!(state.new_session_dialog_state.flavor, "cust");
    }
}
```

### Notes

- Config modal gets items from `LoadedConfigs`
- Flavor modal allows custom input when no match
- Confirm applies selection based on modal type
- Full key binding wiring happens in Phase 7 (Integration)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/message.rs` | Added 8 fuzzy modal message variants: `NewSessionDialogOpenFuzzyModal`, `NewSessionDialogCloseFuzzyModal`, `NewSessionDialogFuzzyUp`, `NewSessionDialogFuzzyDown`, `NewSessionDialogFuzzyConfirm`, `NewSessionDialogFuzzyInput`, `NewSessionDialogFuzzyBackspace`, `NewSessionDialogFuzzyClear` |
| `src/app/handler/update.rs` | Implemented handlers for all 8 fuzzy modal messages with proper state updates and modal type-specific logic |
| `src/tui/widgets/new_session_dialog/state.rs` | Updated `open_fuzzy_modal()` signature to accept `items: Vec<String>` parameter and updated test to pass items |
| `src/app/handler/tests.rs` | Added 3 tests: `test_open_fuzzy_modal_for_flavor`, `test_fuzzy_confirm_sets_flavor`, `test_fuzzy_custom_input` |

### Notable Decisions/Tradeoffs

1. **Handler logic separation**: Items for the fuzzy modal are determined in the handler (update.rs) rather than in the state method. This allows the handler to choose different item sources based on modal type while keeping the state method simple.

2. **Config vs Flavor behavior**: Config modal gets items from `LoadedConfigs.configs`, while Flavor modal currently uses the existing flavor as a suggestion (with TODO for project analysis). This matches the spec and allows for future enhancement.

3. **Custom input support**: The Confirm handler properly handles custom input for Flavor modal (when no match exists) by using `selected_value()` which returns the query text for flavor modals.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib` - Passed (1387 tests passed, including 3 new fuzzy modal tests)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

1. **TODO for flavor project analysis**: Currently, the Flavor modal only suggests the existing flavor value. Future enhancement needed to scan project for available flavors from build.gradle, etc.

2. **Phase 7 dependency**: Full keyboard integration (key bindings) will be implemented in Phase 7. These handlers are tested but not yet connected to user input.

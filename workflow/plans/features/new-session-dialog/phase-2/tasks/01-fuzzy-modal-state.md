## Task: Define Fuzzy Modal State Structure

**Objective**: Create the state structure for the fuzzy search modal.

**Depends on**: Phase 1 (State Foundation)

**Estimated Time**: 25 minutes

### Background

The fuzzy modal needs state to track the search query, available items, filtered results, and current selection. This state lives within `NewSessionDialogState` as an `Option<FuzzyModalState>`.

### Scope

- `src/tui/widgets/new_session_dialog/state.rs`: Add `FuzzyModalState` and related types

### Changes Required

**Add to state.rs:**

```rust
/// Type of fuzzy modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyModalType {
    /// Configuration selection (from LoadedConfigs)
    Config,
    /// Flavor selection (from project + custom)
    Flavor,
}

impl FuzzyModalType {
    /// Get the modal title
    pub fn title(&self) -> &'static str {
        match self {
            Self::Config => "Select Configuration",
            Self::Flavor => "Select Flavor",
        }
    }

    /// Whether custom input is allowed
    pub fn allows_custom(&self) -> bool {
        match self {
            Self::Config => false,  // Must select from list
            Self::Flavor => true,   // Can type custom flavor
        }
    }
}

/// State for the fuzzy search modal
#[derive(Debug, Clone)]
pub struct FuzzyModalState {
    /// Type of modal (determines title and behavior)
    pub modal_type: FuzzyModalType,

    /// User's search query
    pub query: String,

    /// All available items (original order)
    pub items: Vec<String>,

    /// Indices of items matching the query (into `items`)
    pub filtered_indices: Vec<usize>,

    /// Currently highlighted index (into `filtered_indices`)
    pub selected_index: usize,

    /// Scroll offset for long lists
    pub scroll_offset: usize,
}

impl FuzzyModalState {
    /// Create a new fuzzy modal state
    pub fn new(modal_type: FuzzyModalType, items: Vec<String>) -> Self {
        let filtered_indices: Vec<usize> = (0..items.len()).collect();
        Self {
            modal_type,
            query: String::new(),
            items,
            filtered_indices,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    /// Get the currently selected item, or the query if no match
    pub fn selected_value(&self) -> Option<String> {
        if let Some(&idx) = self.filtered_indices.get(self.selected_index) {
            Some(self.items[idx].clone())
        } else if self.modal_type.allows_custom() && !self.query.is_empty() {
            Some(self.query.clone())
        } else {
            None
        }
    }

    /// Check if there are any filtered results
    pub fn has_results(&self) -> bool {
        !self.filtered_indices.is_empty()
    }

    /// Get the number of filtered results
    pub fn result_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Navigate up in the list
    pub fn navigate_up(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.filtered_indices.len() - 1
            } else {
                self.selected_index - 1
            };
            self.adjust_scroll();
        }
    }

    /// Navigate down in the list
    pub fn navigate_down(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
            self.adjust_scroll();
        }
    }

    /// Adjust scroll offset to keep selection visible
    fn adjust_scroll(&mut self) {
        const VISIBLE_ITEMS: usize = 7;  // Number of items visible in modal

        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + VISIBLE_ITEMS {
            self.scroll_offset = self.selected_index - VISIBLE_ITEMS + 1;
        }
    }

    /// Add a character to the query
    pub fn input_char(&mut self, c: char) {
        self.query.push(c);
        self.update_filter();
    }

    /// Remove the last character from the query
    pub fn backspace(&mut self) {
        self.query.pop();
        self.update_filter();
    }

    /// Clear the query
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.update_filter();
    }

    /// Update filtered indices based on current query
    /// (Placeholder - actual algorithm in Task 02)
    pub fn update_filter(&mut self) {
        // Reset selection when filter changes
        self.selected_index = 0;
        self.scroll_offset = 0;

        if self.query.is_empty() {
            // Show all items
            self.filtered_indices = (0..self.items.len()).collect();
        } else {
            // Placeholder: simple case-insensitive substring match
            let query_lower = self.query.to_lowercase();
            self.filtered_indices = self.items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.to_lowercase().contains(&query_lower))
                .map(|(i, _)| i)
                .collect();
        }
    }
}
```

**Update NewSessionDialogState:**

Ensure `fuzzy_modal: Option<FuzzyModalState>` field exists (from Phase 1).

### Acceptance Criteria

1. `FuzzyModalType` enum with Config and Flavor variants
2. `FuzzyModalState` struct with all fields
3. Constructor `new(modal_type, items)`
4. Navigation methods: `navigate_up()`, `navigate_down()`
5. Input methods: `input_char()`, `backspace()`, `clear_query()`
6. `selected_value()` returns item or custom query
7. `update_filter()` placeholder with substring match
8. Scroll offset tracking
9. `cargo check` passes
10. `cargo clippy -- -D warnings` passes

### Testing

```rust
#[cfg(test)]
mod fuzzy_modal_tests {
    use super::*;

    #[test]
    fn test_fuzzy_modal_new() {
        let items = vec!["alpha".into(), "beta".into(), "gamma".into()];
        let state = FuzzyModalState::new(FuzzyModalType::Flavor, items);

        assert_eq!(state.query, "");
        assert_eq!(state.filtered_indices.len(), 3);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_fuzzy_navigation() {
        let items = vec!["a".into(), "b".into(), "c".into()];
        let mut state = FuzzyModalState::new(FuzzyModalType::Config, items);

        assert_eq!(state.selected_index, 0);
        state.navigate_down();
        assert_eq!(state.selected_index, 1);
        state.navigate_down();
        assert_eq!(state.selected_index, 2);
        state.navigate_down();  // Wrap
        assert_eq!(state.selected_index, 0);
        state.navigate_up();  // Wrap back
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn test_fuzzy_filter_basic() {
        let items = vec!["dev".into(), "staging".into(), "production".into()];
        let mut state = FuzzyModalState::new(FuzzyModalType::Flavor, items);

        state.input_char('d');
        assert_eq!(state.filtered_indices.len(), 2);  // dev, production

        state.input_char('e');
        assert_eq!(state.filtered_indices.len(), 2);  // dev, production (both have "de")

        state.input_char('v');
        assert_eq!(state.filtered_indices.len(), 1);  // dev only
    }

    #[test]
    fn test_fuzzy_custom_value() {
        let items = vec!["existing".into()];
        let mut state = FuzzyModalState::new(FuzzyModalType::Flavor, items);

        state.input_char('c');
        state.input_char('u');
        state.input_char('s');
        state.input_char('t');
        state.input_char('o');
        state.input_char('m');

        // No matches, but Flavor allows custom
        assert!(!state.has_results());
        assert_eq!(state.selected_value(), Some("custom".into()));
    }

    #[test]
    fn test_config_no_custom() {
        let items = vec!["existing".into()];
        let mut state = FuzzyModalState::new(FuzzyModalType::Config, items);

        state.input_char('x');  // No match

        assert!(!state.has_results());
        assert_eq!(state.selected_value(), None);  // Config doesn't allow custom
    }
}
```

### Notes

- Actual fuzzy matching algorithm implemented in Task 02
- Scroll offset needed for lists longer than visible area
- `selected_value()` behavior differs based on `allows_custom()`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/state.rs` | Enhanced `FuzzyModalType` with `title()` and `allows_custom()` methods; Replaced minimal `FuzzyModalState` with full implementation including navigation, input handling, filtering, scroll offset tracking, and all required methods; Updated `open_fuzzy_modal()` to use new constructor; Added comprehensive test suite with 5 tests covering all functionality |

### Notable Decisions/Tradeoffs

1. **Test Corrections**: Fixed two tests from the task specification:
   - `test_fuzzy_filter_basic`: Corrected expected count for "de" query from 2 to 1 (only "dev" contains "de", not "production")
   - `test_config_no_custom`: Changed test query from 'x' to 'z' because 'x' is contained in "existing", which would return a result

2. **Scroll Visibility Constant**: Used `VISIBLE_ITEMS = 7` as a const inside `adjust_scroll()` method, matching the task specification for modal display height

3. **Removed Default Trait**: Changed `FuzzyModalState` from `#[derive(Debug, Clone, Default)]` to `#[derive(Debug, Clone)]` because the new structure requires `modal_type` which cannot have a sensible default (removed `Option<FuzzyModalType>` wrapper in favor of required `FuzzyModalType`)

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test fuzzy_modal_tests` - Passed (5/5 tests)
- `cargo test --lib` - Passed (1370 tests)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

1. **Placeholder Filtering**: The `update_filter()` method uses simple case-insensitive substring matching as specified. The actual fuzzy matching algorithm will be implemented in Task 02, which may require interface changes if the scoring/ranking logic needs additional state fields.

2. **Scroll Offset Hardcoded**: The `VISIBLE_ITEMS = 7` constant is defined inside the method. If the modal's visible height changes in the rendering code, this constant would need to be updated or extracted to a shared configuration.

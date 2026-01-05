## Task: UiMode::Settings & Keyboard Shortcut

**Objective**: Add the Settings UI mode and wire up the `,` keyboard shortcut to open the settings panel.

**Depends on**: 01-settings-types, 02-local-settings-file

**Estimated Time**: 1-1.5 hours

### Scope

- `src/app/state.rs`: Add `UiMode::Settings` and `SettingsViewState`
- `src/app/message.rs`: Add settings-related messages
- `src/app/handler/keys.rs`: Add `,` shortcut handler
- `src/app/handler/update.rs`: Handle settings messages

### Details

#### 1. UiMode Addition (`state.rs`)

```rust
/// Current UI mode/screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    #[default]
    Normal,
    DeviceSelector,
    EmulatorSelector,
    ConfirmDialog,
    Loading,
    SearchInput,
    LinkHighlight,
    Settings,  // NEW: Full-screen settings panel
}
```

#### 2. SettingsViewState (`state.rs`)

```rust
use crate::config::types::{SettingsTab, SettingItem, UserPreferences};

/// State for the settings panel view
#[derive(Debug, Clone)]
pub struct SettingsViewState {
    /// Currently active tab
    pub active_tab: SettingsTab,

    /// Currently selected item index within the active tab
    pub selected_index: usize,

    /// Whether we're in edit mode for the current item
    pub editing: bool,

    /// Text buffer for string editing
    pub edit_buffer: String,

    /// Dirty flag - have settings been modified?
    pub dirty: bool,

    /// Loaded user preferences (for User tab)
    pub user_prefs: UserPreferences,

    /// Error message to display (if any)
    pub error: Option<String>,
}

impl Default for SettingsViewState {
    fn default() -> Self {
        Self {
            active_tab: SettingsTab::Project,
            selected_index: 0,
            editing: false,
            edit_buffer: String::new(),
            dirty: false,
            user_prefs: UserPreferences::default(),
            error: None,
        }
    }
}

impl SettingsViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load user preferences from disk
    pub fn load_user_prefs(&mut self, project_path: &std::path::Path) {
        if let Some(prefs) = crate::config::load_user_preferences(project_path) {
            self.user_prefs = prefs;
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
        self.selected_index = 0;
        self.editing = false;
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.prev();
        self.selected_index = 0;
        self.editing = false;
    }

    /// Jump to specific tab
    pub fn goto_tab(&mut self, tab: SettingsTab) {
        self.active_tab = tab;
        self.selected_index = 0;
        self.editing = false;
    }

    /// Select next item
    pub fn select_next(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_index = (self.selected_index + 1) % item_count;
        }
    }

    /// Select previous item
    pub fn select_previous(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_index = if self.selected_index == 0 {
                item_count - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Enter edit mode
    pub fn start_editing(&mut self, initial_value: &str) {
        self.editing = true;
        self.edit_buffer = initial_value.to_string();
    }

    /// Exit edit mode
    pub fn stop_editing(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Mark settings as modified
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clear dirty flag (after save)
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}
```

#### 3. Add to AppState (`state.rs`)

```rust
pub struct AppState {
    // ... existing fields ...

    /// Settings view state (for Settings UI mode)
    pub settings_view_state: SettingsViewState,
}

impl AppState {
    pub fn with_settings(project_path: PathBuf, settings: Settings) -> Self {
        // ... existing code ...

        Self {
            // ... existing fields ...
            settings_view_state: SettingsViewState::new(),
        }
    }

    /// Show settings panel
    pub fn show_settings(&mut self) {
        self.settings_view_state = SettingsViewState::new();
        self.settings_view_state.load_user_prefs(&self.project_path);
        self.ui_mode = UiMode::Settings;
    }

    /// Hide settings panel
    pub fn hide_settings(&mut self) {
        self.ui_mode = UiMode::Normal;
    }
}
```

#### 4. Messages (`message.rs`)

```rust
pub enum Message {
    // ... existing variants ...

    // ─────────────────────────────────────────────────────────
    // Settings Messages
    // ─────────────────────────────────────────────────────────
    /// Open settings panel
    ShowSettings,

    /// Close settings panel
    HideSettings,

    /// Switch to next settings tab
    SettingsNextTab,

    /// Switch to previous settings tab
    SettingsPrevTab,

    /// Jump to specific settings tab (0-3)
    SettingsGotoTab(usize),

    /// Select next setting item
    SettingsNextItem,

    /// Select previous setting item
    SettingsPrevItem,

    /// Toggle or edit the selected setting
    SettingsToggleEdit,

    /// Save settings to disk
    SettingsSave,

    /// Reset current setting to default
    SettingsResetItem,
}
```

#### 5. Key Handler (`keys.rs`)

Add to `handle_key_normal`:

```rust
pub fn handle_key_normal(state: &AppState, key: KeyEvent) -> Option<Message> {
    match key.code {
        // ... existing handlers ...

        // Settings (`,` key)
        KeyCode::Char(',') => Some(Message::ShowSettings),

        // ... rest of handlers ...
    }
}
```

Add new handler function:

```rust
/// Handle keys in settings mode
pub fn handle_key_settings(state: &AppState, key: KeyEvent) -> Option<Message> {
    // If editing, handle text input
    if state.settings_view_state.editing {
        return handle_key_settings_edit(state, key);
    }

    match key.code {
        // Close settings
        KeyCode::Esc | KeyCode::Char('q') => Some(Message::HideSettings),

        // Tab navigation
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                Some(Message::SettingsPrevTab)
            } else {
                Some(Message::SettingsNextTab)
            }
        }

        // Number keys for direct tab access
        KeyCode::Char('1') => Some(Message::SettingsGotoTab(0)),
        KeyCode::Char('2') => Some(Message::SettingsGotoTab(1)),
        KeyCode::Char('3') => Some(Message::SettingsGotoTab(2)),
        KeyCode::Char('4') => Some(Message::SettingsGotoTab(3)),

        // Item navigation
        KeyCode::Char('j') | KeyCode::Down => Some(Message::SettingsNextItem),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::SettingsPrevItem),

        // Toggle/edit
        KeyCode::Enter | KeyCode::Char(' ') => Some(Message::SettingsToggleEdit),

        // Save
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::SettingsSave)
        }

        _ => None,
    }
}

fn handle_key_settings_edit(state: &AppState, key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Esc => Some(Message::SettingsToggleEdit), // Cancel edit
        KeyCode::Enter => Some(Message::SettingsToggleEdit), // Confirm edit
        // Text editing handled in update.rs
        _ => None,
    }
}
```

Update main key dispatch:

```rust
pub fn handle_key(state: &AppState, key: KeyEvent) -> Option<Message> {
    match state.ui_mode {
        UiMode::Settings => handle_key_settings(state, key),
        UiMode::SearchInput => handle_key_search_input(state, key),
        // ... rest of modes ...
    }
}
```

#### 6. Update Handler (`update.rs`)

```rust
Message::ShowSettings => {
    state.show_settings();
    UpdateResult::default()
}

Message::HideSettings => {
    // Check for unsaved changes
    if state.settings_view_state.dirty {
        // Could show confirmation dialog here (future enhancement)
    }
    state.hide_settings();
    UpdateResult::default()
}

Message::SettingsNextTab => {
    state.settings_view_state.next_tab();
    UpdateResult::default()
}

Message::SettingsPrevTab => {
    state.settings_view_state.prev_tab();
    UpdateResult::default()
}

Message::SettingsGotoTab(idx) => {
    if let Some(tab) = SettingsTab::from_index(idx) {
        state.settings_view_state.goto_tab(tab);
    }
    UpdateResult::default()
}

Message::SettingsNextItem => {
    // Item count depends on active tab - will be calculated by widget
    // For now, use a reasonable max
    state.settings_view_state.select_next(20);
    UpdateResult::default()
}

Message::SettingsPrevItem => {
    state.settings_view_state.select_previous(20);
    UpdateResult::default()
}
```

### Acceptance Criteria

1. `UiMode::Settings` variant added to enum
2. `SettingsViewState` tracks tab, selection, edit mode, and dirty flag
3. `AppState` includes `settings_view_state` field
4. Pressing `,` in Normal mode triggers `Message::ShowSettings`
5. Pressing `Esc` or `q` in Settings mode triggers `Message::HideSettings`
6. Tab/Shift+Tab cycles through tabs
7. Number keys 1-4 jump to specific tabs
8. j/k or arrows navigate items
9. Handler functions compile without errors
10. Unit tests for key handlers and state transitions

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_with_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn test_comma_opens_settings() {
        let state = AppState::new();
        let msg = handle_key_normal(&state, key(KeyCode::Char(',')));
        assert_eq!(msg, Some(Message::ShowSettings));
    }

    #[test]
    fn test_escape_closes_settings() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, key(KeyCode::Esc));
        assert_eq!(msg, Some(Message::HideSettings));
    }

    #[test]
    fn test_tab_navigation() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        let msg = handle_key_settings(&state, key(KeyCode::Tab));
        assert_eq!(msg, Some(Message::SettingsNextTab));

        let msg = handle_key_settings(&state, key_with_mod(KeyCode::Tab, KeyModifiers::SHIFT));
        assert_eq!(msg, Some(Message::SettingsPrevTab));
    }

    #[test]
    fn test_number_keys_jump_to_tab() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Settings;

        assert_eq!(
            handle_key_settings(&state, key(KeyCode::Char('1'))),
            Some(Message::SettingsGotoTab(0))
        );
        assert_eq!(
            handle_key_settings(&state, key(KeyCode::Char('3'))),
            Some(Message::SettingsGotoTab(2))
        );
    }

    #[test]
    fn test_settings_view_state_tab_navigation() {
        let mut state = SettingsViewState::new();
        assert_eq!(state.active_tab, SettingsTab::Project);

        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::UserPrefs);

        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::LaunchConfig);

        state.prev_tab();
        assert_eq!(state.active_tab, SettingsTab::UserPrefs);
    }

    #[test]
    fn test_settings_view_state_item_selection() {
        let mut state = SettingsViewState::new();
        assert_eq!(state.selected_index, 0);

        state.select_next(5);
        assert_eq!(state.selected_index, 1);

        state.select_previous(5);
        assert_eq!(state.selected_index, 0);

        state.select_previous(5); // Wrap around
        assert_eq!(state.selected_index, 4);
    }

    #[test]
    fn test_show_settings_message() {
        let mut state = AppState::new();
        assert_eq!(state.ui_mode, UiMode::Normal);

        // Simulate handling ShowSettings
        state.show_settings();
        assert_eq!(state.ui_mode, UiMode::Settings);
    }
}
```

### Notes

- The dirty flag is set when settings are modified, prompting save on close (future enhancement)
- Item count for navigation will be dynamic based on active tab content
- Consider adding `Ctrl+S` visual feedback (flash or message) in future

---

## Completion Summary

**Status:** (Not Started)

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**

(To be filled after implementation)

**Testing Performed:**
- `cargo fmt` -
- `cargo check` -
- `cargo clippy -- -D warnings` -
- `cargo test handler` -

**Notable Decisions:**
- (To be filled after implementation)

# Action Items: Phase 7 - Main Dialog Assembly

**Review Date:** 2026-01-15
**Verdict:** **NEEDS WORK**
**Blocking Issues:** 4

---

## Critical Issues (Must Fix Before Merge)

### 1. Fix Test Suite Compilation

**Source:** All Agents
**Files:**
- `src/app/handler/tests.rs`
- `src/tui/widgets/new_session_dialog/state/tests/dialog_tests.rs`

**Problem:** 168 compilation errors due to API changes

**Required Actions:**

1. Update field access patterns:
   ```rust
   // OLD:
   state.flavor
   state.loading_bootable
   state.active_pane
   state.target_tab

   // NEW:
   state.launch_context.flavor
   state.target_selector.bootable_loading
   state.focused_pane
   state.target_selector.active_tab
   ```

2. Update method calls:
   ```rust
   // OLD:
   state.switch_tab(tab)
   state.open_fuzzy_modal(type, items)
   state.target_up()
   state.context_down()

   // NEW:
   state.target_selector.set_tab(tab)
   state.open_config_modal() / state.open_flavor_modal(items)
   state.target_selector.select_previous()
   state.launch_context.focused_field = field.next()
   ```

3. Update constructor calls:
   ```rust
   // OLD:
   NewSessionDialogState::new()
   NewSessionDialogState::with_configs(configs)

   // NEW:
   NewSessionDialogState::new(LoadedConfigs::default())
   NewSessionDialogState::new(configs)
   ```

**Acceptance:** `cargo test --lib` compiles without errors

---

### 2. Fix Layer Boundary Violations

**Source:** Architecture Enforcer
**Files:**
- `src/app/state.rs:12`
- `src/app/message.rs:10`
- `src/app/handler/keys.rs:680`
- `src/app/handler/new_session/*.rs`

**Problem:** App layer imports types from TUI layer

**Required Actions:**

**Option A (Recommended):** Move state types to App layer
1. Create `src/app/new_session_dialog/mod.rs`
2. Move `src/tui/widgets/new_session_dialog/state/*.rs` to new location
3. Update all imports
4. TUI widget imports state from App (correct direction)

**Option B:** Move types to Core layer
1. Create `src/core/new_session_dialog.rs`
2. Move domain types (DialogPane, TargetTab, LaunchParams, DartDefine)
3. Both App and TUI import from Core

**Acceptance:** No TUI imports in App layer files

---

### 3. Complete Key Routing

**Source:** Architecture Enforcer, Logic Reasoning Checker
**File:** `src/app/handler/keys.rs:679-703`

**Problem:** Only handles Tab, Escape, 1/2 keys; all others ignored

**Required Action:** Replace current implementation with:

```rust
fn handle_key_new_session_dialog(key: KeyEvent, state: &AppState) -> Option<Message> {
    use crate::tui::widgets::TargetTab;

    let dialog = &state.new_session_dialog_state;

    match (key.code, key.modifiers) {
        // Ctrl+C to quit (highest priority)
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // Check if modal is open first
        _ if dialog.is_fuzzy_modal_open() => handle_fuzzy_modal_key(key),
        _ if dialog.is_dart_defines_modal_open() => handle_dart_defines_modal_key(key),

        // Main dialog keys
        (KeyCode::Esc, _) => Some(Message::NewSessionDialogEscape),
        (KeyCode::Tab, KeyModifiers::NONE) => Some(Message::NewSessionDialogSwitchPane),
        (KeyCode::Char('1'), KeyModifiers::NONE) => Some(Message::NewSessionDialogSwitchTab(TargetTab::Connected)),
        (KeyCode::Char('2'), KeyModifiers::NONE) => Some(Message::NewSessionDialogSwitchTab(TargetTab::Bootable)),

        // Route based on focused pane
        _ => match dialog.focused_pane {
            DialogPane::TargetSelector => handle_target_selector_key(key),
            DialogPane::LaunchContext => handle_launch_context_key(key),
        },
    }
}

fn handle_fuzzy_modal_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogFuzzyUp),
        KeyCode::Down => Some(Message::NewSessionDialogFuzzyDown),
        KeyCode::Enter => Some(Message::NewSessionDialogFuzzyConfirm),
        KeyCode::Esc => Some(Message::NewSessionDialogCloseFuzzyModal),
        KeyCode::Backspace => Some(Message::NewSessionDialogFuzzyBackspace),
        KeyCode::Char(c) => Some(Message::NewSessionDialogFuzzyInput { c }),
        _ => None,
    }
}

fn handle_target_selector_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogDeviceUp),
        KeyCode::Down => Some(Message::NewSessionDialogDeviceDown),
        KeyCode::Enter => Some(Message::NewSessionDialogDeviceSelect),
        KeyCode::Char('r') => Some(Message::NewSessionDialogRefreshDevices),
        _ => None,
    }
}

fn handle_launch_context_key(key: KeyEvent, dialog: &NewSessionDialogState) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogFieldPrev),
        KeyCode::Down => Some(Message::NewSessionDialogFieldNext),
        KeyCode::Enter => Some(Message::NewSessionDialogFieldActivate),
        KeyCode::Left if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModePrev)
        }
        KeyCode::Right if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModeNext)
        }
        _ => None,
    }
}
```

**Acceptance:** All keys are handled appropriately; dialog is fully navigable

---

### 4. Remove Unsafe Unwrap

**Source:** Code Quality Inspector
**File:** `src/app/handler/new_session/launch_context.rs:239-248`

**Problem:**
```rust
let device = state
    .new_session_dialog_state
    .selected_device()
    .unwrap()  // Panic risk
    .clone();
```

**Required Action:** Use proper error handling:

```rust
pub fn handle_launch(state: &mut AppState) -> UpdateResult {
    // Build launch params (already validates device exists)
    let params = match state.new_session_dialog_state.build_launch_params() {
        Some(p) => p,
        None => {
            // Should never happen if build_launch_params returned Some,
            // but handle gracefully
            state.new_session_dialog_state.target_selector
                .set_error("No device selected".to_string());
            return UpdateResult::none();
        }
    };

    // Get device reference without unwrap
    let device = match state.new_session_dialog_state.selected_device() {
        Some(d) => d.clone(),
        None => {
            state.new_session_dialog_state.target_selector
                .set_error("Device no longer available".to_string());
            return UpdateResult::none();
        }
    };

    UpdateResult::action(UpdateAction::LaunchFlutterSession {
        device,
        mode: params.mode,
        flavor: params.flavor,
        dart_defines: params.dart_defines,
        config_name: params.config_name,
    })
}
```

**Acceptance:** No `unwrap()` calls in handler code; errors shown to user

---

## Major Issues (Should Fix Before Merge)

### 5. Add Modal Exclusivity Assertions

**Source:** Risks & Tradeoffs Analyzer
**File:** `src/tui/widgets/new_session_dialog/state/dialog.rs`

**Required Action:** Add assertions to modal open methods:

```rust
pub fn open_config_modal(&mut self) {
    debug_assert!(
        !self.has_modal_open(),
        "Cannot open config modal: another modal is already open"
    );
    // ... rest of implementation
}

pub fn open_flavor_modal(&mut self, known_flavors: Vec<String>) {
    debug_assert!(
        !self.has_modal_open(),
        "Cannot open flavor modal: another modal is already open"
    );
    // ... rest of implementation
}

pub fn open_dart_defines_modal(&mut self) {
    debug_assert!(
        !self.has_modal_open(),
        "Cannot open dart defines modal: another modal is already open"
    );
    // ... rest of implementation
}
```

**Acceptance:** Debug assertions catch state corruption during development

---

### 6. Add Error Handling to Config Loading

**Source:** Code Quality Inspector
**File:** `src/app/handler/new_session/navigation.rs:160-166`

**Required Action:**
```rust
pub fn handle_open_new_session_dialog(state: &mut AppState) -> UpdateResult {
    // Load configs with error handling
    let configs = crate::config::load_all_configs(&state.project_path);

    // Log warning if no configs found (not an error, just informational)
    if configs.configs.is_empty() {
        tracing::info!("No launch configurations found, using defaults");
    }

    // Show the dialog
    state.show_new_session_dialog(configs);

    // Trigger device discovery
    UpdateResult::action(UpdateAction::DiscoverDevices)
}
```

**Acceptance:** Config loading failures are handled gracefully

---

## Minor Issues (Consider Fixing)

### 7. Add Doc Comments to Public Functions

**Files:**
- `src/app/handler/new_session/navigation.rs:158-176`
- `src/app/handler/new_session/launch_context.rs:9-115`

**Action:** Add `///` doc comments per CODE_STANDARDS.md

---

### 8. Extract Footer Strings to Constants

**File:** `src/tui/widgets/new_session_dialog/mod.rs:75-83`

**Action:** Move hard-coded strings to module constants

---

## Re-review Checklist

After addressing issues, verify:

- [ ] `cargo fmt` - Code is formatted
- [ ] `cargo check` - No compilation errors
- [ ] `cargo test --lib` - All tests pass
- [ ] `cargo clippy -- -D warnings` - No clippy warnings
- [ ] No `unwrap()` calls in handler code
- [ ] No TUI imports in App layer (or documented exemption)
- [ ] All keys routed correctly in NewSessionDialog
- [ ] Modal assertions in place

---

## Estimated Time

| Item | Estimate |
|------|----------|
| 1. Fix test suite | 4-6 hours |
| 2. Fix layer boundaries | 2-3 hours |
| 3. Complete key routing | 1-2 hours |
| 4. Remove unsafe unwrap | 15 minutes |
| 5-8. Minor fixes | 1-2 hours |
| **Total** | **8-13 hours** |

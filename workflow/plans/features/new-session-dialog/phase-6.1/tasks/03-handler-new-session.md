## Task: Split update.rs - NewSessionDialog Handler Module

**Objective**: Extract all NewSessionDialog message handlers (~650 lines) into a dedicated `handler/new_session/` module.

**Depends on**: 01-state-types-and-modals, 02-state-main-types

**Estimated Time**: 90 minutes

### Scope

- Extract from `src/app/handler/update.rs`:
  - Lines ~1725-2470 (NewSessionDialog handlers)
- Create:
  - `src/app/handler/new_session/mod.rs`
  - `src/app/handler/new_session/navigation.rs`
  - `src/app/handler/new_session/target_selector.rs`
  - `src/app/handler/new_session/launch_context.rs`
  - `src/app/handler/new_session/fuzzy_modal.rs`
  - `src/app/handler/new_session/dart_defines_modal.rs`

### Details

#### Step 1: Create Directory Structure

```bash
mkdir -p src/app/handler/new_session
```

#### Step 2: Identify Handler Groups

Find in `update.rs` all handlers for these message patterns:
- `Message::NewSessionDialogSwitchPane` → navigation.rs
- `Message::NewSessionDialogSwitchTab` → navigation.rs
- `Message::NewSessionDialogNavigateField*` → navigation.rs
- `Message::NewSessionDialogTargetUp/Down/Select` → target_selector.rs
- `Message::NewSessionDialogBootDevice` → target_selector.rs
- `Message::NewSessionDialogSelectConfig/Mode/Flavor` → launch_context.rs
- `Message::NewSessionDialogLaunch` → launch_context.rs
- `Message::NewSessionDialogOpenFuzzyModal` → fuzzy_modal.rs
- `Message::NewSessionDialogFuzzy*` → fuzzy_modal.rs
- `Message::NewSessionDialogOpenDartDefinesModal` → dart_defines_modal.rs
- `Message::NewSessionDialogDartDefines*` → dart_defines_modal.rs

#### Step 3: Create Handler Functions

Each submodule exports a handler function that takes `&mut AppState` and message-specific params:

```rust
// new_session/navigation.rs
use crate::app::state::AppState;
use crate::tui::actions::UpdateAction;

pub fn handle_switch_pane(state: &mut AppState) -> Option<UpdateAction> {
    // ... moved from update.rs
}

pub fn handle_switch_tab(state: &mut AppState) -> Option<UpdateAction> {
    // ...
}

pub fn handle_navigate_field_up(state: &mut AppState) -> Option<UpdateAction> {
    // ...
}

pub fn handle_navigate_field_down(state: &mut AppState) -> Option<UpdateAction> {
    // ...
}
```

#### Step 4: Create new_session/mod.rs

Re-export all handler functions:

```rust
// new_session/mod.rs
mod navigation;
mod target_selector;
mod launch_context;
mod fuzzy_modal;
mod dart_defines_modal;

pub use navigation::*;
pub use target_selector::*;
pub use launch_context::*;
pub use fuzzy_modal::*;
pub use dart_defines_modal::*;
```

#### Step 5: Update update.rs

Replace inline handler code with calls to module functions:

```rust
// In update.rs match statement:
Message::NewSessionDialogSwitchPane => {
    new_session::handle_switch_pane(state)
}
Message::NewSessionDialogSwitchTab { tab } => {
    new_session::handle_switch_tab(state, tab)
}
// ... etc
```

#### Step 6: Update handler/mod.rs

Add the new module:

```rust
// handler/mod.rs
mod new_session;
pub use new_session::*;  // or keep as module if preferred
```

### Target File Sizes

| File | Estimated Lines | Content |
|------|-----------------|---------|
| `navigation.rs` | ~100 | Pane/tab/field switching |
| `target_selector.rs` | ~200 | Device list, boot |
| `launch_context.rs` | ~150 | Config/mode/flavor/launch |
| `fuzzy_modal.rs` | ~150 | Fuzzy search handlers |
| `dart_defines_modal.rs` | ~150 | Key-value editor handlers |

### Acceptance Criteria

1. `handler/new_session/` directory created with 5 submodules
2. All NewSessionDialog handlers moved to appropriate submodules
3. `update.rs` Message match arms delegate to module functions
4. Handler logic unchanged (only code organization)
5. `cargo check` passes
6. `cargo test` passes (all handler tests)

### Testing

```bash
cargo fmt
cargo check
cargo test --lib handler
cargo test  # Full suite
cargo clippy -- -D warnings
```

### Notes

- This is the largest extraction (~650 lines)
- Keep helper functions close to where they're used
- If a helper is shared, put it in `new_session/mod.rs`
- Message routing stays in `update.rs`, only handler logic moves
- Verify no behavior changes through tests

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/new_session/mod.rs` | Created module with re-exports for all handler submodules |
| `src/app/handler/new_session/navigation.rs` | Created (122 lines) - Pane/tab/field navigation handlers |
| `src/app/handler/new_session/target_selector.rs` | Created (146 lines) - Device list, boot, discovery handlers |
| `src/app/handler/new_session/launch_context.rs` | Created (266 lines) - Config/mode/flavor selection, launch handlers |
| `src/app/handler/new_session/fuzzy_modal.rs` | Created (121 lines) - Fuzzy search modal handlers |
| `src/app/handler/new_session/dart_defines_modal.rs` | Created (145 lines) - Key-value editor modal handlers |
| `src/app/handler/mod.rs` | Added new_session module declaration |
| `src/app/handler/update.rs` | Reduced from 2786 to 2211 lines (575 lines removed) - Replaced inline handler code with module delegation |

### Notable Decisions/Tradeoffs

1. **Function Pointer Pattern**: Some handlers (`handle_field_activate`, `handle_fuzzy_confirm`, `handle_close_dart_defines_modal`) need to recursively call `update()` for sub-messages. These accept a function pointer `fn(&mut AppState, Message) -> UpdateResult` to avoid circular dependencies.

2. **Handler Organization**: Organized handlers by functional area rather than message type prefix, improving cohesion:
   - navigation: UI navigation (panes, tabs, fields)
   - target_selector: Device management (selection, booting, discovery)
   - launch_context: Launch configuration (config, mode, flavor, dart defines, launch)
   - fuzzy_modal: Fuzzy search modal interactions
   - dart_defines_modal: Key-value editor modal interactions

3. **Import Cleanup**: Removed unused imports identified by compiler warnings to maintain code quality.

4. **TODO Update**: Updated the file-splitting TODO comment in `update.rs` to mark this task as complete and reflect the new line count.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib` - Passed (1603 tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

None identified. All tests pass and handler logic is preserved exactly as before. The extraction is purely organizational with no behavior changes.
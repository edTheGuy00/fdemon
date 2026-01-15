## Task: Split update.rs - Remaining Handler Modules

**Objective**: Extract remaining large handler groups from `update.rs` to bring the file under the 500-line guideline.

**Depends on**: 03-handler-new-session

**Estimated Time**: 90 minutes

### Scope

Extract from `src/app/handler/update.rs`:
- StartupDialog handlers → `startup_dialog.rs` (~250 lines)
- Session handlers → `session.rs` (~200 lines)
- Scroll handlers → `scroll.rs` (~150 lines)
- Log view handlers → `log_view.rs` (~200 lines)
- Device selector handlers → `device_selector.rs` (~200 lines)
- Settings handlers → `settings.rs` (~400 lines)

### Details

#### Step 1: Extract startup_dialog.rs

Handlers for `Message::StartupDialog*` messages:
- Navigation (up/down, select tab)
- Device selection
- Config selection
- Launch action

```rust
// handler/startup_dialog.rs
use crate::app::state::AppState;
use crate::tui::actions::UpdateAction;

pub fn handle_startup_dialog_up(state: &mut AppState) -> Option<UpdateAction> { ... }
pub fn handle_startup_dialog_down(state: &mut AppState) -> Option<UpdateAction> { ... }
pub fn handle_startup_dialog_select(state: &mut AppState) -> Option<UpdateAction> { ... }
pub fn handle_startup_dialog_launch(state: &mut AppState) -> Option<UpdateAction> { ... }
// ... etc
```

#### Step 2: Extract session.rs

Handlers for session lifecycle:
- `Message::SpawnSession`
- `Message::AttachSession`
- `Message::CloseSession`
- `Message::SwitchSession`
- `Message::RenameSession`

```rust
// handler/session.rs
pub fn handle_spawn_session(state: &mut AppState, ...) -> Option<UpdateAction> { ... }
pub fn handle_close_session(state: &mut AppState, id: SessionId) -> Option<UpdateAction> { ... }
// ... etc
```

#### Step 3: Extract scroll.rs

Handlers for scroll messages:
- `Message::ScrollUp/Down`
- `Message::ScrollPageUp/Down`
- `Message::ScrollToTop/Bottom`
- `Message::ScrollLeft/Right`

```rust
// handler/scroll.rs
pub fn handle_scroll_up(state: &mut AppState) -> Option<UpdateAction> { ... }
pub fn handle_scroll_down(state: &mut AppState) -> Option<UpdateAction> { ... }
pub fn handle_scroll_page_up(state: &mut AppState) -> Option<UpdateAction> { ... }
// ... etc
```

#### Step 4: Extract log_view.rs

Handlers for log view operations:
- `Message::ToggleFilter`
- `Message::ClearLogs`
- `Message::ToggleLinkMode`
- `Message::Search*`

```rust
// handler/log_view.rs
pub fn handle_toggle_filter(state: &mut AppState, level: LogLevel) -> Option<UpdateAction> { ... }
pub fn handle_clear_logs(state: &mut AppState) -> Option<UpdateAction> { ... }
// ... etc
```

#### Step 5: Extract device_selector.rs

Handlers for legacy device selector:
- `Message::DeviceSelector*`
- `Message::ShowDeviceSelector`
- `Message::HideDeviceSelector`

```rust
// handler/device_selector.rs
pub fn handle_show_device_selector(state: &mut AppState) -> Option<UpdateAction> { ... }
pub fn handle_device_selector_up(state: &mut AppState) -> Option<UpdateAction> { ... }
// ... etc
```

#### Step 6: Extract settings.rs

Handlers for settings page:
- `Message::Settings*`
- Navigation
- Edit/save operations

```rust
// handler/settings.rs
pub fn handle_settings_up(state: &mut AppState) -> Option<UpdateAction> { ... }
pub fn handle_settings_edit(state: &mut AppState, field: SettingsField) -> Option<UpdateAction> { ... }
pub fn handle_settings_save(state: &mut AppState) -> Option<UpdateAction> { ... }
// ... etc
```

#### Step 7: Update handler/mod.rs

```rust
// handler/mod.rs
mod new_session;
mod startup_dialog;
mod session;
mod scroll;
mod log_view;
mod device_selector;
mod settings;

pub use new_session::*;
pub use startup_dialog::*;
pub use session::*;
pub use scroll::*;
pub use log_view::*;
pub use device_selector::*;
pub use settings::*;
```

### Target: update.rs After Extraction

After this task, `update.rs` should contain:
- Main `update()` function with message routing (~200-300 lines)
- Any small handlers not worth extracting
- Core state transition logic

### Acceptance Criteria

1. Six new handler modules created
2. Each module under 500 lines
3. `update.rs` reduced to ~300 lines (routing only)
4. All message handlers accessible via module functions
5. No behavior changes
6. `cargo test` passes

### Testing

After each module extraction:
```bash
cargo fmt
cargo check
cargo test --lib
```

Final verification:
```bash
cargo test
cargo clippy -- -D warnings
```

### Notes

- Extract in order of independence (less deps first)
- Settings handlers may be largest - can split further if needed
- Keep related handlers together even if slightly over guideline
- Device selector handlers will be removed in Phase 8, so minimal effort here

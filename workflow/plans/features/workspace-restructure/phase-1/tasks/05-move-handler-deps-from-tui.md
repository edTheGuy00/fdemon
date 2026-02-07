## Task: Move Handler Dependencies from tui/ to app/ or core/

**Objective**: Eliminate all remaining `app/handler/ -> tui/` imports by moving pure-logic functions and types out of `tui/` into their correct layers (`app/` or `core/`).

**Depends on**: Task 04 (state types must be moved first, especially `ConfirmDialogState`)

**Estimated Time**: 4-6 hours

### Scope

5 distinct violations to fix across `app/handler/` files:

| # | Import | From | To |
|---|--------|------|-----|
| A | `open_in_editor`, `sanitize_path` | `tui/editor.rs` | `app/editor.rs` |
| B | `fuzzy_filter` | `tui/widgets/new_session_dialog/fuzzy_modal.rs` | `app/new_session_dialog/fuzzy.rs` |
| C | `SettingsPanel::get_selected_item()` logic | `tui/widgets/settings_panel/mod.rs` | `app/settings_items.rs` |
| D | `TargetSelectorState` | `tui/widgets/new_session_dialog/target_selector.rs` | `app/new_session_dialog/target_selector_state.rs` |
| E | `GroupedBootableDevice` | `tui/widgets/new_session_dialog/device_groups.rs` | `app/new_session_dialog/device_groups.rs` |

### Details

---

#### Part A: Move editor functions to app/

##### The Violation

`src/app/handler/log_view.rs:6`:
```rust
use crate::tui::editor::{open_in_editor, sanitize_path};
```

##### What Moves

From `src/tui/editor.rs`:
- `EditorError` enum (derives `Debug, thiserror::Error`)
- `OpenResult` struct (`pub file: String, pub line: Option<u32>`)
- `open_in_editor(file_ref, settings, project_root) -> Result<OpenResult, EditorError>` -- spawns editor process
- `sanitize_path(path: &str) -> Option<String>` -- cleans file path strings
- `detect_editor()` -- finds system editor
- Supporting types/functions used by the above

**Dependencies**: `crate::config::EditorSettings`, `crate::app::hyperlinks::FileReference` (after Task 04 moves it), `std::process::Command`. **No ratatui dependency.**

##### Where It Goes

Create `src/app/editor.rs` (move the entire `tui/editor.rs` content).

Update `src/app/mod.rs`:
```rust
pub mod editor;
```

Leave `src/tui/editor.rs` as a thin re-export:
```rust
pub use crate::app::editor::*;
```

Or delete it if no TUI code imports from `tui::editor` directly.

##### Files to Update

| File | Line | Change |
|------|------|--------|
| `src/app/handler/log_view.rs` | 6 | `use crate::app::editor::{open_in_editor, sanitize_path};` |
| `src/tui/editor.rs` | all | Becomes re-export or is deleted |

---

#### Part B: Move fuzzy_filter to app/

##### The Violation

`src/app/handler/update.rs:1144` (scoped import inside function):
```rust
use crate::tui::widgets::new_session_dialog::fuzzy_modal::fuzzy_filter;
```

##### What Moves

From `src/tui/widgets/new_session_dialog/fuzzy_modal.rs`:
- `fuzzy_filter(query: &str, items: &[String]) -> Vec<usize>` -- returns indices of matching items
- `fuzzy_score(query: &str, target: &str) -> Option<i32>` -- scores a match (private, used by `fuzzy_filter`)
- `FuzzyMatch { index: usize, score: i32 }` -- intermediate type (private)

These are pure string matching algorithms with **zero ratatui dependency**.

**What stays**: The `FuzzyModal` widget struct and its `StatefulWidget` impl stay in `tui/`.

##### Where It Goes

Create `src/app/new_session_dialog/fuzzy.rs`:

```rust
//! Fuzzy string matching for search/filter operations.

/// Filter items by fuzzy matching against a query string.
/// Returns indices of matching items, sorted by best match.
pub fn fuzzy_filter(query: &str, items: &[String]) -> Vec<usize> { ... }

fn fuzzy_score(query: &str, target: &str) -> Option<i32> { ... }

struct FuzzyMatch { index: usize, score: i32 }
```

Update `src/app/new_session_dialog/mod.rs` to include:
```rust
pub mod fuzzy;
```

##### Files to Update

| File | Line | Change |
|------|------|--------|
| `src/app/handler/update.rs` | 1144 | `use crate::app::new_session_dialog::fuzzy::fuzzy_filter;` |
| `src/tui/widgets/new_session_dialog/fuzzy_modal.rs` | internal | Import `fuzzy_filter` from `crate::app::new_session_dialog::fuzzy` for its own use in the widget |

---

#### Part C: Extract settings item lookup from SettingsPanel

##### The Violation

`src/app/handler/settings_handlers.rs:8`:
```rust
use crate::tui::widgets::{ConfirmDialogState, SettingsPanel};
```

`src/app/handler/keys.rs:375` (scoped):
```rust
use crate::tui::widgets::SettingsPanel;
```

**`ConfirmDialogState`** was already moved in Task 04. This part handles **`SettingsPanel`**.

##### The Problem

Handlers use `SettingsPanel` not for rendering but to call `get_selected_item(&SettingsViewState) -> Option<SettingItem>`. This method builds a list of setting items and returns the one at the selected index. The method depends on `Settings`, `SettingsViewState`, config types -- not ratatui.

However, `SettingsPanel` itself implements `StatefulWidget` (ratatui), so moving the whole struct is wrong.

##### What Moves

Extract from `src/tui/widgets/settings_panel/mod.rs`:
- `SettingItem` enum/struct (the return type of `get_selected_item`)
- `get_selected_item(settings: &Settings, project_path: &Path, view_state: &SettingsViewState) -> Option<SettingItem>` -- as a free function instead of a method on the widget
- The item-building functions that `get_selected_item` calls internally (functions that enumerate settings per tab)

##### Where It Goes

Create `src/app/settings_items.rs`:

```rust
//! Settings item enumeration.
//!
//! Builds the list of configurable setting items per tab,
//! used by both the settings handler (for editing) and the
//! settings panel widget (for rendering).

use std::path::Path;
use crate::config::{Settings, ...};
use crate::app::state::SettingsViewState;

pub fn get_selected_item(
    settings: &Settings,
    project_path: &Path,
    view_state: &SettingsViewState,
) -> Option<SettingItem> { ... }

// ... supporting types and functions
```

Update `src/app/mod.rs`:
```rust
pub mod settings_items;
```

##### Files to Update

| File | Line | Change |
|------|------|--------|
| `src/app/handler/settings_handlers.rs` | 8 | Remove `SettingsPanel` import, add `use crate::app::settings_items::get_selected_item;` |
| `src/app/handler/keys.rs` | 375 | Remove scoped `SettingsPanel` import, use `crate::app::settings_items::get_selected_item` |
| `src/tui/widgets/settings_panel/mod.rs` | internal | Import `get_selected_item` and `SettingItem` from `crate::app::settings_items` instead of defining them locally |

**Note**: The `SettingsPanel` widget itself may still call `get_selected_item()` for rendering (to highlight the selected item). It would import from `app/settings_items`.

---

#### Part D: Move TargetSelectorState to app/

##### The Violation

`src/app/new_session_dialog/state.rs:700`:
```rust
pub use crate::tui::widgets::new_session_dialog::target_selector::TargetSelectorState;
```

##### What Moves

From `src/tui/widgets/new_session_dialog/target_selector.rs`:
- `TargetSelectorState` struct (line 24):
  ```rust
  pub struct TargetSelectorState {
      pub active_tab: TargetTab,
      pub connected_devices: Vec<Device>,
      pub ios_simulators: Vec<IosSimulator>,
      pub android_avds: Vec<AndroidAvd>,
      pub selected_index: usize,
      pub loading: bool,
      pub bootable_loading: bool,
      pub error: Option<String>,
      pub scroll_offset: usize,
      cached_flat_list: Option<Vec<DeviceListItem<String>>>,
  }
  ```
- All `impl TargetSelectorState` methods (state management: `new()`, `set_devices()`, `select_next/prev()`, `selected_item()`, etc.)
- `TargetTab` enum (if defined in same file)
- `DeviceListItem<T>` struct (used in the cache)

**Dependencies**: `crate::daemon::{Device, IosSimulator, AndroidAvd}`, `DeviceListItem` (from `device_groups.rs` in tui). No ratatui dependency on the struct itself.

##### Complication: `DeviceListItem` and `device_groups`

`TargetSelectorState` uses `DeviceListItem<String>` in its `cached_flat_list` field. `DeviceListItem` is defined in `tui/widgets/new_session_dialog/device_groups.rs`. This type also needs to move (covered in Part E).

**Strategy**: Move Parts D and E together since they depend on each other.

##### Where It Goes

Create `src/app/new_session_dialog/target_selector_state.rs`:

```rust
//! Target selector state for the new session dialog.

use crate::daemon::{Device, IosSimulator, AndroidAvd};
use crate::app::new_session_dialog::device_groups::{DeviceListItem, ...};

pub struct TargetSelectorState { ... }
impl TargetSelectorState { ... }
```

Update `src/app/new_session_dialog/mod.rs`:
```rust
pub mod target_selector_state;
```

Update `src/app/new_session_dialog/state.rs:700`:
```rust
// Before:
pub use crate::tui::widgets::new_session_dialog::target_selector::TargetSelectorState;
// After:
pub use crate::app::new_session_dialog::target_selector_state::TargetSelectorState;
```

##### Files to Update

| File | Line | Change |
|------|------|--------|
| `src/app/new_session_dialog/state.rs` | 700 | Update re-export path |
| `src/app/handler/tests.rs` | 3023 | Update import path |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | internal | Import `TargetSelectorState` from `crate::app::new_session_dialog::target_selector_state` |

---

#### Part E: Move GroupedBootableDevice and DeviceListItem to app/

##### The Violation

`src/app/handler/new_session/target_selector.rs:66` (scoped):
```rust
use crate::tui::widgets::GroupedBootableDevice;
```

##### What Moves

From `src/tui/widgets/new_session_dialog/device_groups.rs`:
- `GroupedBootableDevice` enum:
  ```rust
  pub enum GroupedBootableDevice {
      IosSimulator(IosSimulator),
      AndroidAvd(AndroidAvd),
  }
  ```
- `DeviceListItem<T>` struct (used by `TargetSelectorState`)
- Device grouping/flattening logic (pure data transformation, no ratatui)

**What stays in `tui/`**: Any rendering helpers that format `DeviceListItem` for display using ratatui `Span`s.

##### Where It Goes

Create `src/app/new_session_dialog/device_groups.rs`:

```rust
//! Device grouping types for the new session dialog.

use crate::daemon::{Device, IosSimulator, AndroidAvd};

pub enum GroupedBootableDevice {
    IosSimulator(IosSimulator),
    AndroidAvd(AndroidAvd),
}

pub struct DeviceListItem<T> { ... }

// ... grouping/flattening logic
```

Update `src/app/new_session_dialog/mod.rs`:
```rust
pub mod device_groups;
```

##### Files to Update

| File | Line | Change |
|------|------|--------|
| `src/app/handler/new_session/target_selector.rs` | 66 | `use crate::app::new_session_dialog::device_groups::GroupedBootableDevice;` |
| `src/tui/widgets/new_session_dialog/device_groups.rs` | internal | Import from `crate::app::new_session_dialog::device_groups` |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | internal | Import state from `crate::app::new_session_dialog::target_selector_state` |

---

### Acceptance Criteria

1. `src/app/handler/log_view.rs` has no `use crate::tui::*` imports
2. `src/app/handler/update.rs` has no `use crate::tui::*` imports
3. `src/app/handler/settings_handlers.rs` has no `use crate::tui::*` imports
4. `src/app/handler/keys.rs` has no `use crate::tui::*` imports (in non-test code)
5. `src/app/handler/new_session/target_selector.rs` has no `use crate::tui::*` imports
6. `src/app/new_session_dialog/state.rs` does not re-export from `tui/`
7. New files exist: `app/editor.rs`, `app/new_session_dialog/fuzzy.rs`, `app/settings_items.rs`, `app/new_session_dialog/target_selector_state.rs`, `app/new_session_dialog/device_groups.rs`
8. `cargo build` succeeds
9. `cargo test` passes
10. `cargo clippy` is clean

### Testing

```bash
cargo test                    # Full suite
cargo test handler            # Handler tests
cargo test settings           # Settings-related tests
cargo test new_session        # New session dialog tests
cargo test fuzzy              # Fuzzy filter tests
cargo clippy                  # Lint check
```

### Notes

- **Order within this task**: Do Parts D+E first (TargetSelectorState + DeviceListItem + GroupedBootableDevice), then Part A (editor), Part B (fuzzy), Part C (settings items). Parts D+E have the most cross-dependencies.
- **Re-exports**: It's acceptable to leave thin re-exports in `tui/` files so that the TUI widget code can still `use super::*` without changes. The key requirement is that `app/handler/` does NOT import from `tui/`.
- **SettingsPanel widget**: After extracting `get_selected_item()`, the `SettingsPanel` widget struct stays in `tui/` and imports the extracted function from `app/settings_items`. The widget's `StatefulWidget` impl stays.
- **Test code in handlers**: Some test imports may still reference `tui/` -- that's handled in Task 07.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/new_session_dialog/device_groups.rs` | **NEW** - Moved device grouping types and logic from tui/ |
| `src/app/new_session_dialog/target_selector_state.rs` | **NEW** - Moved TargetSelectorState from tui/ |
| `src/app/new_session_dialog/fuzzy.rs` | **NEW** - Moved fuzzy filter logic from tui/ |
| `src/app/editor.rs` | **NEW** - Moved editor functions from tui/ |
| `src/app/settings_items.rs` | **NEW** - Extracted get_selected_item from SettingsPanel widget |
| `src/app/new_session_dialog/mod.rs` | Added exports for new modules |
| `src/app/new_session_dialog/state.rs` | Updated TargetSelectorState re-export to use app layer |
| `src/app/mod.rs` | Added editor and settings_items modules |
| `src/app/handler/log_view.rs` | Updated to import from app/editor instead of tui/editor |
| `src/app/handler/update.rs` | Updated to import fuzzy_filter from app layer |
| `src/app/handler/settings_handlers.rs` | Updated to use get_selected_item from app/settings_items |
| `src/app/handler/keys.rs` | Updated to use get_selected_item from app/settings_items |
| `src/app/handler/new_session/target_selector.rs` | Updated to import GroupedBootableDevice from app layer |
| `src/tui/editor.rs` | Replaced with re-export from app layer |
| `src/tui/widgets/mod.rs` | Made settings_panel module public |
| `src/tui/widgets/settings_panel/mod.rs` | Updated get_selected_item to delegate to app layer |
| `src/tui/widgets/new_session_dialog/mod.rs` | Added allow annotation for hidden glob re-export warning |
| `src/tui/widgets/new_session_dialog/device_groups.rs` | Replaced with re-exports from app layer, kept tests |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Removed duplicate TargetSelectorState, added re-export, fixed test imports |
| `src/tui/widgets/new_session_dialog/fuzzy_modal.rs` | Replaced fuzzy_filter/fuzzy_score with re-export, removed duplicate tests |

### Notable Decisions/Tradeoffs

1. **Re-exports for backward compatibility**: Left thin re-exports in `tui/` modules so that TUI widgets can continue using `use super::*` without changes. The key goal was to eliminate `app/handler/ -> tui/` dependencies, which is achieved.

2. **Made cached_flat_list pub(crate)**: The `TargetSelectorState.cached_flat_list` field was made `pub(crate)` to allow TUI tests to access it while keeping it internal to the crate.

3. **Kept settings_panel module public**: Made `tui::widgets::settings_panel` public so that `app/settings_items.rs` can import the item generator functions. This is temporary until those functions are also moved to the app layer in a future task.

4. **Test consolidation**: Moved tests for fuzzy_filter and fuzzy_score to the app layer (`app/new_session_dialog/fuzzy.rs`). TUI tests now only test widget rendering, not the underlying algorithms.

5. **calculate_scroll_offset duplication**: Duplicated the small `calculate_scroll_offset` function in `app/new_session_dialog/target_selector_state.rs` to avoid complex cross-module dependencies. The function is pure logic with no ratatui dependency.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib` - Passed (1510 tests)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

1. **Settings panel item generators still in TUI**: The functions `project_settings_items`, `user_prefs_items`, `launch_config_items`, and `vscode_config_items` are still in `tui/widgets/settings_panel/items.rs`. They should be moved to the app layer in a future refactoring for complete separation.

2. **Test code in handlers**: Some handler test code (in `src/app/handler/tests.rs`) may still reference TUI types. This is acceptable for now and will be addressed in Task 07.

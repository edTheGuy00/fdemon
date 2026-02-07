## Task: Move State Types from tui/ to app/ (LogViewState, LinkHighlightState, ConfirmDialogState)

**Objective**: Eliminate the `app/ -> tui/` dependency for state types. Move `LogViewState`, `LinkHighlightState` + related types, and `ConfirmDialogState` from `tui/` into `app/` so that `Session` and `AppState` no longer import from `tui/`.

**Depends on**: Task 01 (DaemonMessage must be in core/ first, since events.rs is reorganized)

**Estimated Time**: 4-5 hours

### Scope

- `src/tui/widgets/log_view/state.rs` -> move `LogViewState`, `FocusInfo` to `src/app/log_view_state.rs`
- `src/tui/hyperlinks.rs` -> move `LinkHighlightState`, `DetectedLink`, `FileReference`, scan logic to `src/app/hyperlinks.rs`
- `src/tui/widgets/confirm_dialog.rs` -> move `ConfirmDialogState` to `src/app/confirm_dialog.rs`
- Update all imports across `app/`, `tui/`, `headless/`, `services/`
- Add re-exports in `tui/` for backward compatibility during transition

### Details

#### Part A: Move LogViewState to app/

##### What moves

From `src/tui/widgets/log_view/state.rs`:
- `FocusInfo` struct (line 5-18) + `impl Default` + `impl FocusInfo`
- `LogViewState` struct (line 42-61) + `impl LogViewState` (all methods: `new()`, scroll methods, `visible_range()`, `ensure_auto_scroll()`, `calculate_total_lines()`)

These types have **zero ratatui dependencies**. They use only standard library types (`usize`, `bool`, `Range`), plus `LogEntry` and `CollapseState` from `core/`.

##### Where it goes

Create `src/app/log_view_state.rs`:

```rust
//! Log view state - scroll position, viewport bounds, and focus tracking.
//!
//! This module defines the state types used by both the app handler layer
//! (for scroll commands) and the TUI layer (for rendering the log view).

use std::collections::VecDeque;
use std::ops::Range;

use crate::core::{CollapseState, LogEntry};

// ... (moved types)
```

Add to `src/app/mod.rs`:
```rust
pub mod log_view_state;
```

Re-export from `app/mod.rs` or `app/session.rs`:
```rust
pub use log_view_state::{LogViewState, FocusInfo};
```

##### Update Session import

`src/app/session.rs:14` changes:
```rust
// Before:
use crate::tui::widgets::LogViewState;
// After:
use crate::app::log_view_state::LogViewState;
```

##### Add re-export in tui/ for rendering code

`src/tui/widgets/log_view/state.rs` becomes a thin re-export:
```rust
// Re-export from app layer (authoritative definition)
pub use crate::app::log_view_state::{LogViewState, FocusInfo};
```

Or delete the file and update `tui/widgets/log_view/mod.rs` to import from `app/` directly.

##### Consumer files to update

| File | Current Import | New Import |
|------|---------------|------------|
| `src/app/session.rs:14` | `crate::tui::widgets::LogViewState` | `crate::app::log_view_state::LogViewState` |
| `src/app/handler/scroll.rs` | Uses via `state.session().log_view_state` | No import change needed (field access) |
| `src/tui/widgets/log_view/mod.rs` | `use super::state::*` or `mod state` | `use crate::app::log_view_state::*` |
| `src/tui/render/mod.rs` | May access `LogViewState` via `AppState` | No import change needed (field access) |

---

#### Part B: Move LinkHighlightState to app/

##### What moves

From `src/tui/hyperlinks.rs`:
- `FileReference` struct (line ~20-30) -- file path + line number
- `DetectedLink` struct (line ~40-55) -- file ref + display line + shortcut key
- `LinkHighlightState` struct (line 198-205) + all `impl LinkHighlightState` methods:
  - `new()`, `scan_viewport()`, `has_links()`, `activate()`, `deactivate()`, `link_count()`, `link_by_shortcut()`, `reset()`
- `scan_for_file_references()` function -- used by `scan_viewport()`
- `FILE_REFERENCE_REGEX` lazy_static/const

**Dependencies of these types:**
- `crate::core::{LogEntry, FilterState, CollapseState}` -- all core types
- `regex::Regex` -- external crate
- `std::collections::VecDeque`
- No ratatui dependency

**What stays in `tui/hyperlinks.rs`:**
- Any rendering-specific functions (e.g., `links_on_line()` for display, styled rendering)
- These would import `FileReference`/`DetectedLink` from `app/`

##### Where it goes

Create `src/app/hyperlinks.rs`:

```rust
//! Hyperlink detection and state management.
//!
//! Scans log output for file references (paths with line numbers)
//! and manages the link highlight mode state.

use std::collections::VecDeque;
use regex::Regex;

use crate::core::{CollapseState, FilterState, LogEntry};

// ... (moved types and functions)
```

Add to `src/app/mod.rs`:
```rust
pub mod hyperlinks;
```

##### Update Session import

`src/app/session.rs:13` changes:
```rust
// Before:
use crate::tui::hyperlinks::LinkHighlightState;
// After:
use crate::app::hyperlinks::LinkHighlightState;
```

##### Consumer files to update

| File | Current Import | New Import |
|------|---------------|------------|
| `src/app/session.rs:13` | `crate::tui::hyperlinks::LinkHighlightState` | `crate::app::hyperlinks::LinkHighlightState` |
| `src/app/handler/log_view.rs` | Uses `LinkHighlightState` via `session.link_highlight_state` | No import change (field access) |
| `src/app/handler/log_view.rs` | May import `FileReference` for editor | `crate::app::hyperlinks::FileReference` |
| `src/tui/hyperlinks.rs` | Definition | Becomes re-export + rendering helpers |
| `src/tui/widgets/log_view/mod.rs` | May use `DetectedLink` for rendering | Import from `crate::app::hyperlinks` |
| `src/tui/editor.rs` | Uses `FileReference` | Import from `crate::app::hyperlinks` |

---

#### Part C: Move ConfirmDialogState to app/

##### What moves

From `src/tui/widgets/confirm_dialog.rs`:
- `ConfirmDialogState` struct (line 15-25):
  ```rust
  pub struct ConfirmDialogState {
      pub title: String,
      pub message: String,
      pub session_count: usize,
      pub options: Vec<(String, Message)>,
  }
  ```
- `impl ConfirmDialogState` -- constructors: `new()`, `quit_confirmation()`

**Critical note**: `ConfirmDialogState` has a field `options: Vec<(String, Message)>` which imports `Message` from `app/`. This creates a circular reference when it lives in `tui/`. Moving it to `app/` **resolves** this circular dependency -- both `ConfirmDialogState` and `Message` will be in the same module.

**What stays in `tui/widgets/confirm_dialog.rs`:**
- `ConfirmDialog<'a>` widget struct -- implements `ratatui::widgets::StatefulWidget`
- All rendering logic

##### Where it goes

Create `src/app/confirm_dialog.rs`:

```rust
//! Confirm dialog state.
//!
//! Data model for confirmation dialogs. The rendering widget
//! lives in tui/widgets/confirm_dialog.rs.

use crate::app::message::Message;

#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
    pub title: String,
    pub message: String,
    pub session_count: usize,
    pub options: Vec<(String, Message)>,
}

impl ConfirmDialogState {
    pub fn new(title: impl Into<String>, message: impl Into<String>, options: Vec<(String, Message)>) -> Self { ... }
    pub fn quit_confirmation(session_count: usize) -> Self { ... }
}
```

Add to `src/app/mod.rs`:
```rust
pub mod confirm_dialog;
```

##### Update AppState import

`src/app/state.rs:11` changes:
```rust
// Before:
use crate::tui::widgets::ConfirmDialogState;
// After:
use crate::app::confirm_dialog::ConfirmDialogState;
```

##### Consumer files to update

| File | Current Import | New Import |
|------|---------------|------------|
| `src/app/state.rs:11` | `crate::tui::widgets::ConfirmDialogState` | `crate::app::confirm_dialog::ConfirmDialogState` |
| `src/app/handler/settings_handlers.rs:8` | `crate::tui::widgets::{ConfirmDialogState, ...}` | `crate::app::confirm_dialog::ConfirmDialogState` |
| `src/tui/widgets/confirm_dialog.rs` | (definition) | Import from `crate::app::confirm_dialog::ConfirmDialogState` |
| `src/tui/render/mod.rs` | May access via `state.confirm_dialog_state` | No import change (field access) |
| `src/tui/widgets/mod.rs` | Re-exports `ConfirmDialogState` | Re-export from `crate::app::confirm_dialog` instead |

---

### Acceptance Criteria

1. `src/app/session.rs` has zero `use crate::tui::*` imports
2. `src/app/state.rs` has zero `use crate::tui::*` imports
3. `LogViewState` is defined in `src/app/log_view_state.rs`
4. `LinkHighlightState`, `DetectedLink`, `FileReference` are defined in `src/app/hyperlinks.rs`
5. `ConfirmDialogState` is defined in `src/app/confirm_dialog.rs`
6. TUI widgets import state types from `app/`, not the reverse
7. `cargo build` succeeds
8. `cargo test` passes with no regressions
9. `cargo clippy` is clean

### Testing

```bash
cargo test                    # Full suite
cargo test session            # Session tests
cargo test log_view           # LogView tests (may use state types)
cargo test handler            # Handler tests (heavy user of all these types)
cargo clippy                  # Lint check
```

Pay particular attention to `src/app/handler/tests.rs` and `src/tui/widgets/log_view/tests.rs` -- both extensively use these types.

### Notes

- **Re-exports for gradual migration**: During this task, it's fine to add `pub use crate::app::log_view_state::LogViewState;` in `tui/widgets/log_view/state.rs` so that test files importing from the old path still compile. Task 07 cleans these up.
- **`scan_viewport()` complexity**: The `LinkHighlightState::scan_viewport()` method takes multiple parameters from `core/` types. It is business logic (detecting file references in log output) that was incorrectly placed in the TUI layer. Moving it to `app/` is the right thing to do.
- **`ConfirmDialogState` circular dependency resolution**: Currently `tui/confirm_dialog.rs` imports `Message` from `app/`, and `app/state.rs` imports `ConfirmDialogState` from `tui/`. Moving `ConfirmDialogState` to `app/` eliminates this circular dependency entirely.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/log_view_state.rs` | Created - moved LogViewState and FocusInfo from tui/widgets/log_view/state.rs |
| `src/app/hyperlinks.rs` | Created - moved LinkHighlightState, DetectedLink, FileReference, and all scan logic from tui/hyperlinks.rs |
| `src/app/confirm_dialog.rs` | Created - moved ConfirmDialogState from tui/widgets/confirm_dialog.rs |
| `src/app/mod.rs` | Added new modules: confirm_dialog, hyperlinks, log_view_state |
| `src/app/session.rs` | Updated imports: use app/hyperlinks and app/log_view_state instead of tui/ |
| `src/app/state.rs` | Updated imports: use app/confirm_dialog instead of tui/widgets |
| `src/app/handler/settings_handlers.rs` | Updated imports: use app/confirm_dialog instead of tui/widgets |
| `src/tui/widgets/log_view/state.rs` | Replaced with re-export from app/log_view_state |
| `src/tui/hyperlinks.rs` | Replaced with re-export from app/hyperlinks |
| `src/tui/widgets/confirm_dialog.rs` | Removed state definition, added re-export from app/confirm_dialog, added Message import in tests |

### Notable Decisions/Tradeoffs

1. **Re-export strategy**: Created thin re-export modules in tui/ to maintain backward compatibility during transition. This allows existing code that imports from tui/ to continue working without changes.
2. **Circular dependency resolution**: Moving ConfirmDialogState to app/ resolves the circular dependency where tui/confirm_dialog.rs imported Message from app/, and app/state.rs imported ConfirmDialogState from tui/.
3. **Complete state migration**: All three state types (LogViewState, LinkHighlightState, ConfirmDialogState) are now in app/, eliminating all app/ -> tui/ dependencies for state management.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (clean compilation)
- `cargo test --lib` - Passed (1515 tests passed; 0 failed; 8 ignored)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Backward compatibility via re-exports**: The re-export strategy maintains compatibility but creates temporary dual import paths. These should be cleaned up in a follow-up task to enforce single canonical import locations.
2. **No breaking changes**: All existing imports continue to work through re-exports, ensuring zero disruption to the codebase during this refactor.

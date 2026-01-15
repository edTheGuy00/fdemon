# Task: Remove Old Dialogs

## Summary

Remove the old DeviceSelector and StartupDialog implementations now that NewSessionDialog replaces them.

## Files to Delete

| File | Reason |
|------|--------|
| `src/tui/widgets/device_selector.rs` | Replaced by NewSessionDialog |
| `src/tui/widgets/startup_dialog/mod.rs` | Replaced by NewSessionDialog |
| `src/tui/widgets/startup_dialog/styles.rs` | Replaced by NewSessionDialog styles |

## Files to Modify

| File | Changes |
|------|---------|
| `src/tui/widgets/mod.rs` | Remove old exports |
| `src/app/state.rs` | Remove old state types |
| `src/app/message.rs` | Remove old messages |
| `src/app/handler/mod.rs` | Remove module exports for startup_dialog, device_selector |
| `src/app/handler/keys.rs` | Remove old key handlers |

## Files to Delete (Handler Modules)

| File | Reason |
|------|--------|
| `src/app/handler/startup_dialog.rs` | Replaced by new_session handlers |
| `src/app/handler/device_selector.rs` | Replaced by new_session handlers |

## Implementation

### 1. Update widgets/mod.rs

```rust
// src/tui/widgets/mod.rs

// Remove these:
// pub mod device_selector;
// pub mod startup_dialog;
// pub use device_selector::{DeviceSelector, DeviceSelectorState};
// pub use startup_dialog::StartupDialog;

// Keep new dialog:
pub mod new_session_dialog;
pub use new_session_dialog::{
    NewSessionDialog,
    NewSessionDialogState,
    DialogPane,
    TargetTab,
    LaunchParams,
};

// Keep other widgets:
pub mod log_view;
pub mod help;
pub mod status_bar;
// ... etc
```

### 2. Update app/state.rs

```rust
// src/app/state.rs

// Remove these types:
// pub struct StartupDialogState { ... }
// pub struct DeviceSelectorState { ... }
// pub enum DialogSection { ... }

// Remove from AppState:
// pub startup_dialog: Option<StartupDialogState>,
// pub device_selector: DeviceSelectorState,

// Remove old UiMode variants if not already done:
// UiMode::StartupDialog,
// UiMode::DeviceSelector,
```

### 3. Update app/message.rs

```rust
// src/app/message.rs

// Remove these message variants:
// StartupDialogOpen,
// StartupDialogClose,
// StartupDialogSelectNext,
// StartupDialogSelectPrevious,
// StartupDialogSelectConfig,
// StartupDialogCycleMode,
// StartupDialogSwitchSection,
// StartupDialogSetFlavor { ... },
// StartupDialogSetDartDefines { ... },
// StartupDialogLaunch,
// StartupDialogRefresh,

// DeviceSelectorOpen,
// DeviceSelectorClose,
// DeviceSelectorSelectNext,
// DeviceSelectorSelectPrevious,
// DeviceSelectorConfirm,
// DeviceSelectorRefresh,
// DeviceSelectorDevicesReceived { ... },
// DeviceSelectorError { ... },

// Keep NewSessionDialog messages (added in previous phases)
```

### 4. Remove handler modules

```rust
// src/app/handler/mod.rs

// Remove these module declarations:
// mod startup_dialog;
// mod device_selector;

// Remove these re-exports:
// pub use startup_dialog::*;
// pub use device_selector::*;
```

Then delete the files:
```bash
rm src/app/handler/startup_dialog.rs
rm src/app/handler/device_selector.rs
```

### 5. Update app/handler/keys.rs

```rust
// src/app/handler/keys.rs

// Remove these functions:
// fn handle_startup_dialog_keys(...) { ... }
// fn handle_device_selector_keys(...) { ... }

// Remove old UiMode matching:
// UiMode::StartupDialog => handle_startup_dialog_keys(...),
// UiMode::DeviceSelector => handle_device_selector_keys(...),

// Keep NewSessionDialog key handling
```

### 6. Search for remaining references

```bash
# Find any remaining references to old types
rg "StartupDialog" --type rust
rg "DeviceSelector" --type rust
rg "startup_dialog" --type rust
rg "device_selector" --type rust
```

### 7. Update any remaining imports

```rust
// In any file that imports old types, update to use new types:

// Before:
use crate::tui::widgets::{DeviceSelector, DeviceSelectorState};
use crate::tui::widgets::startup_dialog::StartupDialog;
use crate::app::state::StartupDialogState;

// After:
use crate::tui::widgets::new_session_dialog::{
    NewSessionDialog,
    NewSessionDialogState,
};
```

## Verification Steps

1. **Delete files**
```bash
rm src/tui/widgets/device_selector.rs
rm -rf src/tui/widgets/startup_dialog/
```

2. **Update imports and references**
- Follow compiler errors to find remaining references
- Update each file as needed

3. **Run checks**
```bash
cargo fmt
cargo check  # Fix any errors
cargo test   # Fix any test failures
cargo clippy -- -D warnings
```

4. **Search for stragglers**
```bash
# Should return no results after cleanup
rg "StartupDialog" --type rust
rg "DeviceSelector" --type rust
```

## Common Issues

### Orphaned imports
```rust
// Before (will cause error after deletion):
use crate::tui::widgets::device_selector::truncate_string;

// Solution: Move utility to common module or new_session_dialog
```

### Test fixtures
```rust
// Tests may reference old types
// Update to use NewSessionDialogState instead
```

### Handler match arms
```rust
// Stale match arms will cause non-exhaustive pattern errors
// Remove them from the match statement
```

## Tests

After cleanup, verify:

```bash
# All tests pass
cargo test

# No warnings about unused code
cargo clippy -- -D warnings

# Build succeeds
cargo build
```

## Notes

- Delete files first, then follow compiler errors
- Some utility functions may need to be moved (e.g., `truncate_string`)
- Test files may need significant updates
- Keep a backup branch in case of issues

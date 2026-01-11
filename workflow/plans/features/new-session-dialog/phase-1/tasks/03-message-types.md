## Task: Define NewSessionDialog Message Types

**Objective**: Add all message types needed for NewSessionDialog interactions.

**Depends on**: Task 02 (Dialog state struct)

**Estimated Time**: 20 minutes

### Background

The TEA pattern requires all user interactions and events to be expressed as messages. The new dialog needs messages for:
- Showing/hiding the dialog
- Pane and tab navigation
- Device selection and booting
- Launch context field navigation
- Modal interactions

### Scope

- `src/app/message.rs`: Add new message variants

### Changes Required

**Add to `src/app/message.rs` Message enum:**

```rust
// ─────────────────────────────────────────────────────────
// NewSessionDialog Messages
// ─────────────────────────────────────────────────────────

/// Show the new session dialog
ShowNewSessionDialog,

/// Hide the new session dialog (cancel)
HideNewSessionDialog,

/// Switch focus between left (Target) and right (Launch) panes
NewSessionDialogSwitchPane,

/// Switch between Connected and Bootable tabs (left pane)
NewSessionDialogSwitchTab(TargetTab),

/// Navigate up in current list/field
NewSessionDialogUp,

/// Navigate down in current list/field
NewSessionDialogDown,

/// Select current item / confirm action
/// - On Connected device: launch session
/// - On Bootable device: boot the device
/// - On Config/Flavor field: open fuzzy modal
/// - On DartDefines field: open dart defines modal
/// - On Launch button: launch session
NewSessionDialogConfirm,

/// Boot a specific bootable device
NewSessionDialogBootDevice { device_id: String },

/// Device boot completed - refresh connected list
NewSessionDialogDeviceBooted { device_id: String },

/// Device boot failed
NewSessionDialogBootFailed { device_id: String, error: String },

/// Set connected devices (from flutter devices discovery)
NewSessionDialogSetConnectedDevices { devices: Vec<Device> },

/// Set bootable devices (from native discovery)
NewSessionDialogSetBootableDevices { devices: Vec<BootableDevice> },

/// Set error message
NewSessionDialogSetError { error: String },

/// Clear error message
NewSessionDialogClearError,

// ─────────────────────────────────────────────────────────
// Launch Context Messages
// ─────────────────────────────────────────────────────────

/// Select a configuration by index
NewSessionDialogSelectConfig { index: Option<usize> },

/// Set the build mode
NewSessionDialogSetMode { mode: FlutterMode },

/// Set the flavor string
NewSessionDialogSetFlavor { flavor: String },

/// Set dart defines
NewSessionDialogSetDartDefines { defines: Vec<DartDefine> },

// ─────────────────────────────────────────────────────────
// Fuzzy Modal Messages
// ─────────────────────────────────────────────────────────

/// Open fuzzy search modal
NewSessionDialogOpenFuzzyModal { modal_type: FuzzyModalType },

/// Close fuzzy search modal (cancel)
NewSessionDialogCloseFuzzyModal,

/// Fuzzy modal: input character
NewSessionDialogFuzzyInput { c: char },

/// Fuzzy modal: backspace
NewSessionDialogFuzzyBackspace,

/// Fuzzy modal: navigate up
NewSessionDialogFuzzyUp,

/// Fuzzy modal: navigate down
NewSessionDialogFuzzyDown,

/// Fuzzy modal: select current item
NewSessionDialogFuzzyConfirm,

// ─────────────────────────────────────────────────────────
// Dart Defines Modal Messages
// ─────────────────────────────────────────────────────────

/// Open dart defines modal
NewSessionDialogOpenDartDefinesModal,

/// Close dart defines modal (save and close)
NewSessionDialogCloseDartDefinesModal,

/// Dart defines modal: navigate list
NewSessionDialogDartDefinesUp,
NewSessionDialogDartDefinesDown,

/// Dart defines modal: switch between list/key/value fields
NewSessionDialogDartDefinesSwitchField,

/// Dart defines modal: input character
NewSessionDialogDartDefinesInput { c: char },

/// Dart defines modal: backspace
NewSessionDialogDartDefinesBackspace,

/// Dart defines modal: add new define
NewSessionDialogDartDefinesAdd,

/// Dart defines modal: delete current define
NewSessionDialogDartDefinesDelete,
```

**Add imports at top of message.rs:**

```rust
use crate::core::BootableDevice;
use crate::tui::widgets::new_session_dialog::{
    DartDefine, FuzzyModalType, TargetTab,
};
```

### Acceptance Criteria

1. All message variants added to `Message` enum
2. Messages properly documented with comments
3. Required imports added
4. `cargo check` passes (handlers don't need to be implemented yet - use `_ => {}` catch-all if needed)
5. `cargo clippy -- -D warnings` passes

### Testing

No specific tests needed - messages are tested through handler tests.

### Notes

- Old dialog messages (`ShowDeviceSelector`, `ShowStartupDialog`, etc.) are kept until Phase 7
- Handler implementation comes in Phase 4 (state transitions)
- Some messages carry data (`SetConnectedDevices`, `SetMode`, etc.) for async task results

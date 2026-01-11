# Feature: NewSessionDialog - Unified Session Launch Dialog

## Summary

Consolidate `DeviceSelector` and `StartupDialog` into a single `NewSessionDialog` with a Master-Detail layout featuring tabbed device selection (Connected/Bootable) and fuzzy search modals.

## Problem Statement

1. **Bug:** Second session launch uses `DeviceSelector` which doesn't pass flavor, mode, or config
2. **UX Issues:** Two separate dialogs with different capabilities; long-form input fields are cumbersome
3. **Missing Feature:** Can't see offline/bootable devices (simulators/AVDs) without launching them first

## Solution Overview

### Main Dialog Layout

```
┌── NewSessionDialog ─────────────────────────────────────────────────────┐
│                                                                         │
│  ┌── 🎯 Target Selector ─────────────┐ ┌── ⚙️ Launch Context ───────┐   │
│  │                                   │ │                             │  │
│  │ ╭─────────────╮ ╭─────────────╮   │ │  Configuration:             │  │
│  │ │ 1 Connected │ │ 2 Bootable  │   │ │  [ Development (Default) ▼] │  │
│  │ ╰─────────────╯ ╰─────────────╯   │ │                             │  │
│  │                                   │ │  Mode:                      │  │
│  │  iOS Simulators                   │ │  (●) Debug  (○) Profile     │  │
│  │  ▶ iPhone 15 Pro (iOS 17.2)       │ │  (○) Release                │  │
│  │    iPhone 14 (iOS 16.0)           │ │                             │  │
│  │                                   │ │  Flavor:                    │  │
│  │  Android AVDs                     │ │  [ dev__________________ ▼] │  │
│  │    Pixel_6_API_33                 │ │                             │  │
│  │                                   │ │  Dart Defines:              │  │
│  │                                   │ │  [ 3 items             ▶]   │  │
│  │                                   │ │                             │  │
│  │  [Enter] Boot Device              │ │  [    🚀 LAUNCH (Enter)   ] │  │
│  └───────────────────────────────────┘ └─────────────────────────────┘  │
│                                                                         │
│  [1/2] Switch Tab   [Tab] Switch Pane   [?] Help                        │
└─────────────────────────────────────────────────────────────────────────┘
```

### Fuzzy Search Modal (Config/Flavor Selection)

Triggered when pressing Enter on Configuration or Flavor fields. Provides type-to-filter search with custom input support.

```
┌── Sidecar ──────────────────────────────────────────────────────────────┐
│  ┌── 🎯 Target Selector ─────────────┐ ┌── ⚙️ Launch Context ───────┐  │
│  │                                   │ │                             │  │
│  │  ... (Background Dimmed) ...      │ │  Flavor:                    │  │
│  │                                   │ │  [ dev __________________ ▼]│  │
│  │                                   │ │                             │  │
│  └───────────────────────────────────┘ └─────────────────────────────┘  │
│ ┌─────────────────────────────────────────────────────────────────────┐ │
│ │  🔍 Select Flavor (Type to filter)                                  │ │
│ ├─────────────────────────────────────────────────────────────────────┤ │
│ │ > dev_staging                                                       │ │
│ │   dev_production                                                    │ │
│ │   uat_testing                                                       │ │
│ │   uatb_testing                                                      │ │
│ │   prod_eu                                                           │ │
│ │   prod_us                                                           │ │
│ │   prod_asia                                                         │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
│                                                                         │
│  [↑↓] Navigate  [Enter] Select  [Esc] Cancel  Type to filter/custom     │
└─────────────────────────────────────────────────────────────────────────┘
```

**Fuzzy Modal Behavior:**
- Modal appears as overlay at bottom of dialog
- Background (main dialog) is dimmed but visible
- Type characters to filter list OR enter custom value
- Up/Down arrows navigate filtered results
- Enter selects highlighted item OR uses typed text as custom value
- Esc cancels and returns to main dialog
- Empty query shows all items
- Matching is case-insensitive, supports substring and fuzzy matching

**Modal States:**
- `FuzzyModalType::Config` - Shows launch configurations
- `FuzzyModalType::Flavor` - Shows discovered flavors from project

### Dart Defines Modal (Key-Value Editor)

Triggered when pressing Enter on Dart Defines field. Master-detail layout for managing environment variables.

```
┌── 📝 Manage Dart Defines ───────────────────────────────────────────────┐
│                                                                         │
│  ┌── Active Variables ───────┐  ┌── Edit Variable ───────────────────┐  │
│  │                           │  │                                    │  │
│  │ > API_KEY                 │  │  Key:                              │  │
│  │   BACKEND_URL             │  │  [ API_KEY                  ]      │  │
│  │   DEBUG_MODE              │  │                                    │  │
│  │                           │  │  Value:                            │  │
│  │                           │  │  [ "secret_123_abc"         ]      │  │
│  │                           │  │                                    │  │
│  │                           │  │  [   Save   ]   [  Delete  ]       │  │
│  │                           │  │                                    │  │
│  │  [+] Add New              │  │                                    │  │
│  └───────────────────────────┘  └────────────────────────────────────┘  │
│                                                                         │
│  [Tab] Switch Pane  [↑↓] Navigate  [Enter] Edit/Save  [Esc] Save & Close│
└─────────────────────────────────────────────────────────────────────────┘
```

**Dart Defines Modal Behavior:**
- Full-screen modal (replaces main dialog temporarily)
- Left pane: List of existing dart defines + "Add New" option
- Right pane: Edit form for selected define (Key + Value fields)
- Tab switches focus between left list and right edit form
- When in list (left): Up/Down navigates, Enter selects for editing
- When in edit (right): Tab cycles Key→Value→Save→Delete, Enter activates
- "Add New" creates empty define and focuses Key field
- "Save" commits changes to the define
- "Delete" removes the selected define (with confirmation?)
- Esc saves all changes and returns to main dialog

**Edit Form States:**
- `DartDefineField::List` - Focus on left pane variable list
- `DartDefineField::Key` - Editing key text input
- `DartDefineField::Value` - Editing value text input
- `DartDefineField::Save` - Save button focused
- `DartDefineField::Delete` - Delete button focused

### Key Features

1. **Unified Dialog:** Single entry point for all session launching
2. **Two-Pane Layout:** Target Selector (left) + Launch Context (right)
3. **Tabbed Device List:** Connected devices vs Bootable emulators/simulators
4. **Fuzzy Search Modals:** For config and flavor selection (with custom input)
5. **Dart Defines Modal:** Master-detail for key/value pairs
6. **Native Device Discovery:** Use `xcrun simctl` and `emulator -list-avds` for offline devices

## Design Decisions

1. **Boot behavior:** Two-step process - booting a device switches to Connected tab; user must press Enter again to launch. This allows user to verify the device booted correctly before launching.

2. **Flavor input:** Fuzzy search with custom input support - show known flavors but allow typing custom values.

3. **Tab memory:** Always start on Connected tab when dialog opens.

4. **Tool availability:** Check for `xcrun simctl` and `emulator` command availability at app startup and cache the result. When user switches to Bootable tab, show appropriate message if tools are unavailable (e.g., "Install Android SDK" or "iOS simulators only available on macOS").

5. **Modal layering:** Fuzzy modal overlays main dialog; Dart Defines modal replaces it.

6. **Config editability by source:**
   - **VSCode configs:** All values (flavor, dart-defines, mode) are **read-only**. These fields are disabled since VSCode manages them.
   - **FDemon configs:** Values are editable. When user modifies flavor or dart-defines, **automatically update the `.fdemon/launch.toml` file** to persist changes.
   - **No config selected:** All fields editable as transient values (not persisted).

7. **Platform grouping:** Both Connected and Bootable device lists group devices by platform (iOS Devices, Android Devices, iOS Simulators, Android AVDs) with section headers.

## Implementation Phases

| Phase | Name | Description | Est. |
|-------|------|-------------|------|
| 1 | State Foundation | Create `NewSessionDialogState`, message types, basic transitions | 2h |
| 2 | Fuzzy Search Modal | Reusable fuzzy search widget with filtering and custom input | 2h |
| 3 | Dart Defines Modal | Master-detail modal for key/value editing | 2h |
| 4 | Native Device Discovery | Implement `xcrun simctl` and `emulator -list-avds` discovery | 2h |
| 5 | Target Selector Widget | Tabbed device list (Connected/Bootable) with boot action | 3h |
| 6 | Launch Context Widget | Right pane with config/mode/flavor/dart-defines fields | 2h |
| 7 | Main Dialog Assembly | Combine all widgets, wire up modal triggers | 2h |
| 8 | Integration & Cleanup | Update handlers, remove old dialogs, update tests | 3h |

**Total Estimated Time:** 18 hours

## Phase Details

### Phase 2: Fuzzy Search Modal

**Files:**
- `src/tui/widgets/new_session_dialog/fuzzy_modal.rs`

**State Structure:**
```rust
pub struct FuzzyModalState {
    pub modal_type: FuzzyModalType,
    pub query: String,              // User's typed input
    pub items: Vec<String>,         // All available items
    pub filtered_indices: Vec<usize>, // Indices of matching items
    pub selected_index: usize,      // Currently highlighted
    pub allow_custom: bool,         // Whether custom input is allowed
}

pub enum FuzzyModalType {
    Config,  // Launch configurations
    Flavor,  // Project flavors
}
```

**Key Methods:**
- `filter_items()` - Updates `filtered_indices` based on `query`
- `selected_item()` - Returns highlighted item or custom query
- `navigate_up()` / `navigate_down()` - Move selection
- `input_char()` / `backspace()` - Modify query

**Rendering:**
- Centered modal at bottom 40% of screen
- Header with modal type and "Type to filter" hint
- Scrollable list of filtered items with highlight
- Footer with keybinding hints

### Phase 3: Dart Defines Modal

**Files:**
- `src/tui/widgets/new_session_dialog/dart_defines_modal.rs`

**State Structure:**
```rust
pub struct DartDefinesModalState {
    pub defines: Vec<DartDefine>,
    pub selected_index: usize,      // In left list
    pub active_pane: DefinesPane,   // List or Edit
    pub edit_field: EditField,      // Key, Value, Save, Delete
    pub editing_key: String,        // Current key being edited
    pub editing_value: String,      // Current value being edited
    pub is_new: bool,               // Adding new vs editing existing
}

pub struct DartDefine {
    pub key: String,
    pub value: String,
}

pub enum DefinesPane { List, Edit }
pub enum EditField { Key, Value, Save, Delete }
```

**Key Methods:**
- `select_define()` - Load define into edit fields
- `save_current()` - Commit edit fields to defines list
- `delete_current()` - Remove selected define
- `add_new()` - Create empty define, switch to edit
- `navigate_list()` - Up/down in left pane
- `cycle_edit_field()` - Tab through edit form

**Rendering:**
- Full-screen modal (same size as main dialog)
- Two-column layout (40% list, 60% edit)
- List shows keys with selection indicator
- Edit form with labeled text inputs and buttons
- Footer with context-sensitive keybinding hints

### Phase 4: Native Device Discovery

**Files:**
- `src/daemon/simulators.rs` - iOS simulator discovery
- `src/daemon/avds.rs` - Android AVD discovery
- `src/daemon/tool_availability.rs` - Command availability checking

**Tool Availability Structure:**
```rust
pub struct ToolAvailability {
    pub xcrun_simctl: bool,    // Can run iOS simulator commands
    pub android_emulator: bool, // Can run Android emulator commands
    pub checked: bool,          // Whether check has been performed
}
```

**Key Functions:**
- `check_tool_availability()` - Run at startup, cache results
- `list_ios_simulators()` - Parse `xcrun simctl list devices -j`
- `list_android_avds()` - Parse `emulator -list-avds`
- `boot_simulator(udid)` - Boot iOS simulator
- `boot_avd(name)` - Boot Android AVD

**Bootable Device Structure:**
```rust
pub struct BootableDevice {
    pub id: String,           // UDID for iOS, AVD name for Android
    pub name: String,         // Display name
    pub platform: Platform,   // iOS or Android
    pub runtime: String,      // e.g., "iOS 17.2", "API 33"
    pub state: DeviceState,   // Shutdown, Booted, Booting
}

pub enum Platform { IOS, Android }
pub enum DeviceState { Shutdown, Booted, Booting }
```

### Phase 5: Target Selector Widget

**Files:**
- `src/tui/widgets/new_session_dialog/target_selector.rs`

**Layout:**
```
┌── 🎯 Target Selector ─────────────────┐
│                                       │
│ ╭─────────────╮ ╭─────────────╮       │
│ │ 1 Connected │ │ 2 Bootable  │       │
│ ╰─────────────╯ ╰─────────────╯       │
│                                       │
│  iOS Devices                          │  ← Platform group header
│  ▶ iPhone 15 Pro (physical)           │
│                                       │
│  Android Devices                      │
│    Pixel 8 (physical)                 │
│                                       │
│  [Enter] Select  [r] Refresh          │
└───────────────────────────────────────┘
```

**Features:**
- Tab bar at top (1 Connected, 2 Bootable)
- Platform-grouped device list with section headers
- Selection indicator (▶)
- Loading state with spinner
- Empty state messages
- Unavailable tool messages on Bootable tab

### Phase 6: Launch Context Widget

**Files:**
- `src/tui/widgets/new_session_dialog/launch_context.rs`

**Layout:**
```
┌── ⚙️ Launch Context ─────────────────┐
│                                       │
│  Configuration:                       │
│  [ Development (Default)          ▼]  │  ← Opens fuzzy modal
│                                       │
│  Mode:                                │
│  (●) Debug  (○) Profile  (○) Release  │
│                                       │
│  Flavor:                              │
│  [ dev____________________        ▼]  │  ← Opens fuzzy modal (if editable)
│                                       │
│  Dart Defines:                        │
│  [ 3 items                        ▶]  │  ← Opens dart defines modal
│                                       │
│  [          🚀 LAUNCH (Enter)       ] │
│                                       │
└───────────────────────────────────────┘
```

**Features:**
- Config dropdown (opens fuzzy modal)
- Mode radio buttons (Debug/Profile/Release)
- Flavor field (fuzzy modal or disabled based on config source)
- Dart Defines field (modal or disabled based on config source)
- Launch button
- Visual disabled state for VSCode config fields

### Phase 7: Main Dialog Assembly

**Files:**
- `src/tui/widgets/new_session_dialog/mod.rs`

**Responsibilities:**
- Compose Target Selector (left) + Launch Context (right) into split layout
- Render active modal overlay when present (fuzzy or dart defines)
- Handle pane focus switching (Tab key)
- Delegate rendering to child widgets
- Footer with context-sensitive keybindings

**Layout Composition:**
```
┌── NewSessionDialog ─────────────────────────────────────────────────────┐
│                                                                         │
│  ┌── Target Selector ──────────────┐ ┌── Launch Context ─────────────┐  │
│  │         (50% width)             │ │        (50% width)            │  │
│  │                                 │ │                               │  │
│  └─────────────────────────────────┘ └───────────────────────────────┘  │
│                                                                         │
│  [1/2] Tab   [Tab] Pane   [↑↓] Navigate   [Enter] Select   [Esc] Close  │
└─────────────────────────────────────────────────────────────────────────┘
```

### Phase 8: Integration & Cleanup

**Tasks:**
1. Update `UiMode` enum to use `NewSessionDialog` instead of `StartupDialog`/`DeviceSelector`
2. Update app handlers to route to new dialog
3. Wire up tool availability check at startup
4. Implement config file auto-save for FDemon configs
5. Remove old `DeviceSelectorState` and `StartupDialogState`
6. Remove old widget files
7. Update all references in codebase
8. Update tests
9. Update documentation (KEYBINDINGS.md if exists)

## Files Changed

### Files to Delete

- `src/tui/widgets/device_selector.rs`
- `src/tui/widgets/startup_dialog/` (entire directory)

### Files to Modify

| File | Change |
|------|--------|
| `src/app/state.rs` | Replace `StartupDialogState` + `DeviceSelectorState` |
| `src/app/message.rs` | Update dialog messages |
| `src/app/handler/update.rs` | Update handlers for new dialog |
| `src/app/handler/keys.rs` | Update key mappings |
| `src/tui/render/mod.rs` | Update rendering for new UiMode |
| `src/tui/widgets/mod.rs` | Update widget exports |
| `src/daemon/mod.rs` | Export new discovery modules |

### Files to Create

| File | Purpose |
|------|---------|
| `src/tui/widgets/new_session_dialog/mod.rs` | Main dialog widget |
| `src/tui/widgets/new_session_dialog/state.rs` | `NewSessionDialogState` |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Left pane widget |
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Right pane widget |
| `src/tui/widgets/new_session_dialog/fuzzy_modal.rs` | Fuzzy search modal |
| `src/tui/widgets/new_session_dialog/dart_defines_modal.rs` | Dart defines editor |
| `src/tui/widgets/new_session_dialog/styles.rs` | Style constants |
| `src/daemon/simulators.rs` | iOS simulator discovery |
| `src/daemon/avds.rs` | Android AVD discovery |

## Message Types for Modals

### Fuzzy Modal Messages
```rust
// Open/Close
NewSessionDialogOpenFuzzyModal { modal_type: FuzzyModalType }
NewSessionDialogCloseFuzzyModal

// Navigation
NewSessionDialogFuzzyUp
NewSessionDialogFuzzyDown
NewSessionDialogFuzzyConfirm      // Select item or use custom
NewSessionDialogFuzzyCancel       // Same as close

// Input
NewSessionDialogFuzzyInput { c: char }
NewSessionDialogFuzzyBackspace
NewSessionDialogFuzzyClear        // Clear query
```

### Dart Defines Modal Messages
```rust
// Open/Close
NewSessionDialogOpenDartDefinesModal
NewSessionDialogCloseDartDefinesModal  // Saves and closes

// Navigation
NewSessionDialogDartDefinesUp
NewSessionDialogDartDefinesDown
NewSessionDialogDartDefinesSwitchPane  // Tab between List/Edit
NewSessionDialogDartDefinesCycleField  // Tab within Edit pane

// Actions
NewSessionDialogDartDefinesSelect      // Load into edit form
NewSessionDialogDartDefinesSave        // Commit edit to list
NewSessionDialogDartDefinesDelete      // Remove from list
NewSessionDialogDartDefinesAddNew      // Create new empty

// Input (when in edit mode)
NewSessionDialogDartDefinesInput { c: char }
NewSessionDialogDartDefinesBackspace
```

## Verification

1. `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings`
2. Manual test: Launch session with flavor from startup
3. Manual test: Add second session - verify flavor/config preserved
4. Manual test: Boot offline simulator from dialog
5. Manual test: Fuzzy search config selection
6. Manual test: Fuzzy search flavor with custom input
7. Manual test: Add/edit/delete dart defines via modal

## References

- Approved plan: `/Users/ed/.claude/plans/zesty-waddling-shamir.md`
- Current DeviceSelector: `src/tui/widgets/device_selector.rs`
- Current StartupDialog: `src/tui/widgets/startup_dialog/mod.rs`
- State management: `src/app/state.rs`

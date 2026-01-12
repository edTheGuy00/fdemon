# Phase 3: Dart Defines Modal - Task Index

## Overview

Create a master-detail modal for managing dart define key-value pairs. Full-screen modal with list on left, edit form on right.

**Total Tasks:** 5
**Estimated Time:** 2 hours

## UI Design

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

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-dart-defines-state              │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  02-dart-defines-list-widget        │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  03-dart-defines-edit-widget        │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  04-dart-defines-modal-widget       │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  05-dart-defines-messages           │
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-dart-defines-state](tasks/01-dart-defines-state.md) | Done | Phase 1 | 20m | `new_session_dialog/state.rs` |
| 2 | [02-dart-defines-list-widget](tasks/02-dart-defines-list-widget.md) | Done | 1 | 25m | `new_session_dialog/dart_defines_modal.rs` |
| 3 | [03-dart-defines-edit-widget](tasks/03-dart-defines-edit-widget.md) | Done | 2 | 25m | `new_session_dialog/dart_defines_modal.rs` |
| 4 | [04-dart-defines-modal-widget](tasks/04-dart-defines-modal-widget.md) | Done | 3 | 20m | `new_session_dialog/dart_defines_modal.rs` |
| 5 | [05-dart-defines-messages](tasks/05-dart-defines-messages.md) | Done | 4 | 15m | `app/message.rs`, `app/handler/update.rs` |

## Success Criteria

Phase 3 is complete when:

- [ ] `DartDefinesModalState` struct with defines list, selection, edit fields
- [ ] `DartDefine` struct with key/value
- [ ] Left pane shows list of defines with "[+] Add New" option
- [ ] Right pane shows edit form with Key, Value inputs, Save/Delete buttons
- [ ] Tab switches between left (list) and right (edit) panes
- [ ] Up/Down navigates list; Enter loads item into edit form
- [ ] In edit form: Tab cycles fields, Enter activates buttons
- [ ] Add New creates empty define and focuses Key field
- [ ] Save commits changes, Delete removes item
- [ ] Esc saves all and closes modal
- [ ] Message handlers wired up
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Behavior Specification

### Opening the Modal
- Triggered by pressing Enter on Dart Defines field in Launch Context
- Modal opens full-screen (replaces main dialog visually)
- Copies current `dart_defines` from `NewSessionDialogState` into modal state
- If list is empty, focus goes to "[+] Add New"

### Left Pane (List)
- Shows all define keys in a scrollable list
- "[+] Add New" option at bottom of list
- Up/Down navigates with wrapping
- Enter on a define → loads into edit form, switches to right pane
- Enter on "[+] Add New" → creates empty define, switches to right pane with Key focused

### Right Pane (Edit Form)
- Shows Key and Value text inputs
- Shows Save and Delete buttons
- Tab cycles: Key → Value → Save → Delete → Key
- When Key/Value focused: type to edit, Enter moves to next field
- When Save focused: Enter commits changes to list
- When Delete focused: Enter removes from list (no confirmation)

### Pane Switching
- Tab (when in list) → focus first field in edit form
- Tab (when on Delete) → focus first item in list
- Edit form remembers which field was last focused

### Saving Changes
- "Save" button commits `editing_key` and `editing_value` to the selected define
- If editing a new define, adds to list
- If editing existing, updates in place
- After save, stays on edit form (allows quick edits)

### Deleting Defines
- "Delete" button removes selected define from list
- Selection moves to previous item (or "[+] Add New" if list empty)
- Focus returns to list pane

### Closing the Modal
- Esc saves all changes and closes modal
- Changes are committed to `NewSessionDialogState.dart_defines`
- Focus returns to Dart Defines field in main dialog

## Notes

- Modal state stored in `NewSessionDialogState.dart_defines_modal: Option<DartDefinesModalState>`
- Edit form validates: Key cannot be empty when saving
- Consider visual feedback for unsaved changes in edit form
- Define order is preserved (insertion order)

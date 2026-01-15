## Task: Update Phase 7 and Phase 8 Task Files

**Objective**: Update the individual task files in Phase 7 and Phase 8 to reference the new file paths after the file splitting is complete.

**Depends on**: 05-cleanup-verification

**Estimated Time**: 30 minutes

### Scope

Update file path references in:
- `phase-7/tasks/01-dialog-state.md`
- `phase-7/tasks/04-dialog-messages.md`
- `phase-8/tasks/02-startup-flow.md`
- `phase-8/tasks/03-remove-old-dialogs.md`

### Details

#### Phase 7 Task 01: Dialog State

**File:** `phase-7/tasks/01-dialog-state.md`

Update file references:

| Old Path | New Path |
|----------|----------|
| `src/tui/widgets/new_session_dialog/state.rs` | `src/tui/widgets/new_session_dialog/state/dialog.rs` |

Update the Files table:
```markdown
## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/state/dialog.rs` | Modify (add main state) |
```

Update code comments:
```rust
// Before:
// src/tui/widgets/new_session_dialog/state.rs

// After:
// src/tui/widgets/new_session_dialog/state/dialog.rs
```

Update imports in code examples to use the new module structure:
```rust
// Before:
use super::target_selector::TargetSelectorState;
use super::fuzzy_modal::FuzzyModalState;

// After:
use super::super::target_selector::TargetSelectorState;
// Or if re-exported from state/mod.rs:
use crate::tui::widgets::new_session_dialog::state::{
    FuzzyModalState,
    DartDefinesModalState,
};
```

#### Phase 7 Task 04: Dialog Messages

**File:** `phase-7/tasks/04-dialog-messages.md`

Update file references:

| Old Path | New Path |
|----------|----------|
| `src/app/handler/update.rs` | `src/app/handler/new_session/` (multiple files) |

Update the Files table:
```markdown
## Files

| File | Action |
|------|--------|
| `src/app/message.rs` | Modify (add messages) |
| `src/app/handler/new_session/navigation.rs` | Modify (add pane switching handlers) |
| `src/app/handler/new_session/mod.rs` | Modify (add dialog open/close handlers) |
| `src/app/handler/keys.rs` | Modify (add key routing) |
```

Update code comments:
```rust
// Before:
// src/app/handler/update.rs

// After:
// src/app/handler/new_session/mod.rs (for open/close handlers)
// src/app/handler/new_session/navigation.rs (for pane switching)
```

#### Phase 8 Task 02: Startup Flow

**File:** `phase-8/tasks/02-startup-flow.md`

Update file references:

| Old Path | New Path |
|----------|----------|
| `src/app/handler/update.rs` | `src/app/handler/session.rs` (session handlers) |

Update the Files table:
```markdown
## Files

| File | Action |
|------|--------|
| `src/main.rs` | Modify (startup sequence) |
| `src/app/handler/session.rs` | Modify (add launch success handler) |
| `src/app/handler/new_session/launch_context.rs` | Modify (add auto-launch handler) |
```

Update code comments:
```rust
// Before:
// src/app/handler/update.rs

// After:
// src/app/handler/session.rs (for spawn_tool_availability_check, spawn_device_discovery)
// src/app/handler/new_session/launch_context.rs (for auto-launch logic)
```

#### Phase 8 Task 03: Remove Old Dialogs

**File:** `phase-8/tasks/03-remove-old-dialogs.md`

Update file references to reflect modular handler structure:

| Old Path | New Path |
|----------|----------|
| `src/app/handler/update.rs` | `src/app/handler/startup_dialog.rs` (DELETE) |
| `src/app/handler/update.rs` | `src/app/handler/device_selector.rs` (DELETE) |

Update the Files to Modify table:
```markdown
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
```

Update Implementation section 4:
```markdown
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
```

### Acceptance Criteria

1. Phase 7 Task 01 references `state/dialog.rs` instead of `state.rs`
2. Phase 7 Task 04 references `handler/new_session/` modules instead of `update.rs`
3. Phase 8 Task 02 references `handler/session.rs` instead of `update.rs`
4. Phase 8 Task 03 lists `startup_dialog.rs` and `device_selector.rs` as files to delete
5. All code examples in task files use correct import paths
6. No references to old monolithic file paths remain

### Verification

After updating task files, grep to ensure no stale references:

```bash
# Should return no matches in phase-7 and phase-8 task files
grep -r "handler/update.rs" workflow/plans/features/new-session-dialog/phase-7/
grep -r "handler/update.rs" workflow/plans/features/new-session-dialog/phase-8/
grep -r "new_session_dialog/state.rs" workflow/plans/features/new-session-dialog/phase-7/
```

### Notes

- This task ensures downstream phase tasks are implementable after the split
- Task files are documentation, not code - no cargo commands needed
- Preserves the intent of original tasks while updating file locations
- If any task file has additional stale references, update those too

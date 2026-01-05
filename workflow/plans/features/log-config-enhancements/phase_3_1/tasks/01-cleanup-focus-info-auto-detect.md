## Task: 01-cleanup-focus-info-auto-detect

**Objective**: Remove the automatic file reference detection from `FocusInfo` that happens during render, which is the source of the "wonky" link detection behavior.

**Depends on**: None

### Background

The current implementation in `tui/widgets/log_view.rs` automatically extracts a file reference from the element at the top of the viewport during each render pass. This approach is unreliable because:
1. It only works when a file reference is at the exact top of the viewport
2. It runs on every render, even when not needed
3. Users report "opening links does not work all the time"

### Scope

- `src/tui/widgets/log_view.rs`:
  - Remove `file_ref: Option<FileReference>` field from `FocusInfo` struct
  - Remove `has_file_ref()` method from `FocusInfo` impl
  - Remove file reference extraction logic from `StatefulWidget::render()`
  - Keep `entry_index`, `entry_id`, and `frame_index` fields (needed for stack trace toggle)

### Current Code to Modify

```rust
// In FocusInfo struct (lines ~72-81)
pub struct FocusInfo {
    pub entry_index: Option<usize>,
    pub entry_id: Option<u64>,
    pub frame_index: Option<usize>,
    pub file_ref: Option<FileReference>,  // REMOVE THIS
}

// In FocusInfo impl (lines ~83-93)
impl FocusInfo {
    pub fn new() -> Self { ... }
    pub fn has_file_ref(&self) -> bool { ... }  // REMOVE THIS
}

// In StatefulWidget::render() - multiple places setting file_ref
// Lines ~897-898, ~926-927, ~952-953 (approximately)
state.focus_info.file_ref = extract_file_ref_from_message(&entry.message);
state.focus_info.file_ref = FileReference::from_stack_frame(frame);
```

### Changes Required

1. **Remove `file_ref` field from `FocusInfo`**:
```rust
#[derive(Debug, Default, Clone)]
pub struct FocusInfo {
    /// Index of the focused entry in the log buffer
    pub entry_index: Option<usize>,
    /// ID of the focused entry (for stability across buffer changes)
    pub entry_id: Option<u64>,
    /// Index of the focused frame within a stack trace (if applicable)
    pub frame_index: Option<usize>,
    // file_ref removed - link detection now happens in link highlight mode
}
```

2. **Remove `has_file_ref()` method** (entire method)

3. **Remove file_ref assignments in render()** - search for all occurrences of:
   - `state.focus_info.file_ref =`
   - `extract_file_ref_from_message` calls in render context
   - `FileReference::from_stack_frame` calls in render context

4. **Update imports** - remove unused imports after cleanup

### Acceptance Criteria

1. `FocusInfo` struct no longer has a `file_ref` field
2. `FocusInfo` no longer has a `has_file_ref()` method
3. No file reference extraction happens during render
4. `entry_index`, `entry_id`, `frame_index` tracking still works (for stack trace toggle)
5. All existing tests pass (some may need updating if they check `file_ref`)
6. The `Enter` key stack trace toggle functionality still works
7. No compiler errors or warnings

### Testing

- **Unit Tests**: 
  - Update any tests that reference `FocusInfo.file_ref`
  - Verify `FocusInfo::default()` still works
  
- **Manual Testing**:
  - Verify stack trace toggle with `Enter` key still works
  - Verify scrolling still updates `entry_index` correctly
  - The `o` key functionality will be broken temporarily (fixed in Task 09)

### Notes

- This task intentionally breaks the `o` key functionality temporarily
- Task 09 will restore `o` key with a simpler implementation
- The new link highlight mode (Tasks 03-08) replaces this approach entirely
- Focus tracking for `entry_index`/`entry_id` must remain for the stack trace toggle feature

### Files Changed

| File | Change Type |
|------|-------------|
| `src/tui/widgets/log_view.rs` | Modified - remove file_ref logic |

### Estimated Time

1-2 hours

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/log_view.rs` | Removed `file_ref` field from `FocusInfo` struct, removed `has_file_ref()` method, removed `focused_file_ref()` method from `LogViewState`, removed 3 `file_ref` assignments in render(), removed unused `extract_file_ref_from_message` and `FileReference` imports |
| `src/app/handler/update.rs` | Updated `OpenFileAtCursor` handler to temporarily always return None (with note about Task 09 restoration) |

### Notable Decisions/Tradeoffs

1. **Temporary break of `o` key**: As specified in the task, the `o` key functionality is intentionally broken. The handler now always returns "No file reference at cursor" and logs a helpful message suggesting users try Link Highlight Mode (L key). Task 09 will restore this functionality with on-demand extraction.

2. **Clean removal**: Removed all associated code including the `focused_file_ref()` convenience method that was on `LogViewState`, which was dependent on the removed field.

3. **Preserved focus tracking**: The `entry_index`, `entry_id`, and `frame_index` fields in `FocusInfo` are preserved and still updated during render - these are needed for the stack trace toggle feature (Enter key).

### Testing Performed

- `cargo check` - Passed (no errors)
- `cargo test` - All 988 tests passed
- `cargo clippy` - Only 1 unrelated warning (empty line in doc comments in helpers.rs)

### Risks/Limitations

1. **`o` key non-functional**: Users pressing `o` will see no action until Task 09 is completed
2. **FocusInfo API change**: Any external code depending on `focus_info.file_ref` would break (only `update.rs` had this, now fixed)
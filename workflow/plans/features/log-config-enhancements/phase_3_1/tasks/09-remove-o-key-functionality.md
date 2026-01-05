## Task: 09-remove-o-key-functionality

**Objective**: Remove the `o` key functionality (open file at cursor) since Link Highlight Mode (`L` key) provides a superior and more reliable way to open files from logs.

**Depends on**: 06-key-and-update-handlers

### Background

Phase 3 Task 04 implemented the `o` key to open the file reference at the current cursor position. This relied on `FocusInfo.file_ref` being set during render (Task 03), which was the source of the "wonky" behavior.

With Link Highlight Mode implemented, the `o` key is redundant:
- Link mode shows ALL file references in the viewport
- Users can select exactly which file to open
- No confusion about "which file is at the cursor"

Removing `o` simplifies the codebase and eliminates user confusion.

### Scope

- `src/app/handler/keys.rs`:
  - Remove `o` key binding from `handle_key_normal()`

- `src/app/handler/update.rs`:
  - Remove `Message::OpenFileAtCursor` handler

- `src/app/message.rs`:
  - Remove `OpenFileAtCursor` message variant

### Changes to `src/app/handler/keys.rs`

Remove this section from `handle_key_normal()`:

```rust
// REMOVE THIS ENTIRE BLOCK:
// ─────────────────────────────────────────────────────────
// Editor Actions (Phase 3 Task 4)
// ─────────────────────────────────────────────────────────
// 'o' - Open file at cursor in editor
(KeyCode::Char('o'), KeyModifiers::NONE) => Some(Message::OpenFileAtCursor),
```

### Changes to `src/app/message.rs`

Remove this message variant:

```rust
// REMOVE THIS:
// ─────────────────────────────────────────────────────────
// Editor Actions (Phase 3 Task 4)
// ─────────────────────────────────────────────────────────
/// Open the currently focused file in the configured editor
/// If running in an IDE terminal, opens in that IDE instance
OpenFileAtCursor,
```

### Changes to `src/app/handler/update.rs`

Remove the entire `Message::OpenFileAtCursor` handler block (approximately lines 804-851):

```rust
// REMOVE THIS ENTIRE BLOCK:
Message::OpenFileAtCursor => {
    // Get focused file reference from current session's LogViewState
    // This was set during render by Task 03's focus tracking
    let file_ref = if let Some(handle) = state.session_manager.selected_mut() {
        // focus_info is updated during each render pass
        handle.session.log_view_state.focus_info.file_ref.clone()
    } else {
        None
    };

    let Some(file_ref) = file_ref else {
        // ... rest of handler
    };
    
    // ... sanitize and open logic
}
```

### User Experience Change

| Before (Phase 3) | After (Phase 3.1) |
|------------------|-------------------|
| `o` - Open file at cursor (unreliable) | Removed |
| No link mode | `L` - Enter link highlight mode |
| | `1-9`, `a-z` - Select and open specific file |

### Why This Is Better

1. **Explicit selection**: Users choose exactly which file to open
2. **No guessing**: No confusion about "what's at the cursor"
3. **Visibility**: All available links are shown before selection
4. **Reliability**: Works every time, not dependent on scroll position
5. **Simpler code**: One less message type and handler to maintain

### Migration Path

Users who relied on `o`:
1. Press `L` to enter link mode
2. Press the number/letter of the desired link
3. Two keypresses instead of one, but 100% reliable

### Acceptance Criteria

1. `o` key no longer does anything in normal mode
2. `Message::OpenFileAtCursor` removed from message enum
3. Handler for `OpenFileAtCursor` removed from update.rs
4. No compiler errors (all references removed)
5. No dead code warnings
6. Link Highlight Mode (`L`) is the only way to open files from logs

### Testing

#### Verification Steps

1. Press `o` in normal mode → nothing happens
2. `cargo build` succeeds with no warnings
3. `cargo test` passes
4. Link mode (`L`) still works correctly

#### Code Search

Run these searches to verify complete removal:

```bash
# Should return no results after cleanup
grep -r "OpenFileAtCursor" src/
grep -r "'o'" src/app/handler/keys.rs
```

### Files Changed

| File | Change Type |
|------|-------------|
| `src/app/handler/keys.rs` | Modified - remove o key binding |
| `src/app/message.rs` | Modified - remove message variant |
| `src/app/handler/update.rs` | Modified - remove handler |

### Estimated Time

30 minutes - 1 hour

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

| File | Change |
|------|--------|
| `src/app/handler/keys.rs` | Removed `o` key binding and "Editor Actions (Phase 3 Task 4)" section header |
| `src/app/message.rs` | Removed `OpenFileAtCursor` message variant and "Editor Actions (Phase 3 Task 4)" section |
| `src/app/handler/update.rs` | Removed entire `Message::OpenFileAtCursor` handler block (~45 lines) |

### Verification

1. **Code search** - `grep -r "OpenFileAtCursor" src/` returns no results
2. **Build** - `cargo check` passes with no errors
3. **Tests** - All 950 tests pass

### Notable Decisions/Tradeoffs

- The handler being removed was already non-functional (always returned early with `None` file_ref) since Phase 3.1 Task 01 removed the auto-detection of file references during render
- Link Highlight Mode (`L` key) is now the exclusive method to open files from logs, providing a more reliable and explicit user experience
- Removed ~50 lines of code total across the three files

### Risks/Limitations

- Users accustomed to pressing `o` will need to learn the new `L` -> shortcut key workflow
- The migration path (documented in the plan) involves two keypresses instead of one, but with 100% reliability
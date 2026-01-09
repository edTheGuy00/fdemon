## Task: Update Related Handlers and Comments

**Objective**: Ensure consistency by updating any other places that reference the 'n' keybinding for sessions, and clean up code comments.

**Depends on**: 01-replace-n-with-plus

### Scope

- `src/app/handler/keys.rs`: Update comments throughout
- Check for any other handlers that might need '+' support

### Details

**1. Update comments in handle_key_normal():**

Find the comment block above the Log Search section (around line 227):
```rust
// ─────────────────────────────────────────────────────────
// Log Search (Phase 1 - Task 5)
// ─────────────────────────────────────────────────────────
// '/' - Enter search mode (vim-style)
```

Update the 'n' key comment:
```rust
// 'n' - Next search match (vim-style, only when search active)
(KeyCode::Char('n'), KeyModifiers::NONE) => { ... }
```

**2. Add comment for '+' key handler:**

Near the 'd' key handler section:
```rust
// ─────────────────────────────────────────────────────────
// Session Management
// ─────────────────────────────────────────────────────────
// '+' - Start new session
// If sessions are running: show quick device selector
// If no sessions: show full startup dialog
(KeyCode::Char('+'), KeyModifiers::NONE)
| (KeyCode::Char('+'), KeyModifiers::SHIFT) => { ... }

// 'd' for adding device/session (alternative to '+')
// If sessions are running: show quick device selector
// If no sessions: show full startup dialog
(KeyCode::Char('d'), KeyModifiers::NONE) => { ... }
```

**3. Check other UI modes for '+' key support (optional):**

Consider if '+' should also work from:
- `UiMode::DeviceSelector` - Probably not needed (already showing device list)
- `UiMode::StartupDialog` - No (already in startup flow)
- `UiMode::Settings` - Maybe (to allow quick session start from settings)

For now, '+' only works in `UiMode::Normal`. This can be extended later if needed.

**4. Remove any outdated references:**

Search for any remaining references to 'n' key for session management:
```bash
grep -rn "n.*new session\|n.*device selector" src/
grep -rn "'n' for\|press n to" src/
```

### Acceptance Criteria

1. All comments in keys.rs accurately describe the new keybinding behavior
2. '+' key handler is properly documented
3. 'd' key comment updated to note it's an alternative to '+'
4. No misleading references to 'n' key for session management remain
5. Code is well-organized with clear section comments

### Testing

```bash
# Check for any references to old behavior
grep -rn "n.*session\|n.*device" src/app/handler/

# Run all tests
cargo test --lib
```

### Notes

- This is primarily a code quality/documentation task
- The actual functionality changes are in task 01
- Consider updating the module-level documentation if it exists
- Future enhancement: Add '+' support in Settings mode for quick session start

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**
(To be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy` - Pending
- `cargo test` - Pending

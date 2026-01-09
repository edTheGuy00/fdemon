# Plan: Startup Flow Rework (Non-Connected Default)

## TL;DR

Change the default startup behavior so Flutter Demon enters the TUI immediately in a "Not Connected" state when no auto-start config exists, instead of showing the startup dialog. Replace the "n" keybinding with "+" for starting new sessions. This enables testing most TUI functionality without requiring Flutter devices.

---

## Background

### Current Problems

1. **E2E Test Friction**: Settings page and other E2E tests are blocked or require workarounds because:
   - With `auto_start = false`, the app immediately shows `StartupDialog`
   - Tests must escape from the dialog before testing other UI components
   - Some UI modes (DeviceSelector, StartupDialog) don't handle all key events

2. **Test Dependencies**: The current flow requires either:
   - Flutter devices to be available for auto-start
   - Complex test scaffolding to work around the startup dialog

3. **Keybinding Conflict**: The "n" key is overloaded:
   - Next search match (when search active)
   - New session (when no search)
   - This creates confusion and test complexity

### Desired Behavior

**Non-auto-start flow:**
1. App starts directly in `UiMode::Normal` with no sessions
2. Status bar shows "Not Connected"
3. Center message shows "Press + to start a new session"
4. User presses "+" to show the StartupDialog

**Auto-start flow:** Unchanged - discovers devices and launches session or falls back to dialog.

---

## Affected Modules

- `src/tui/startup.rs` - Modify `show_startup_dialog()` to enter Normal mode instead
- `src/tui/widgets/log_view/mod.rs` - Update empty state message
- `src/tui/widgets/status_bar/mod.rs` - Add "Not Connected" display
- `src/app/handler/keys.rs` - Replace 'n' with '+' for new session
- `src/app/handler/tests.rs` - Update keybinding tests
- `src/tui/render/tests.rs` - Update snapshot tests
- `docs/KEYBINDINGS.md` - Update documentation
- `tests/e2e/pty_utils.rs` - May need updates for new startup behavior

---

## Development Phases

### Phase 1: Core Flow Changes

**Goal**: Enable the app to start in Normal mode without sessions and show appropriate "Not Connected" state.

#### Steps

1. **Modify Startup Logic** (`src/tui/startup.rs`)
   - Change `show_startup_dialog()` to set `UiMode::Normal` instead of `UiMode::StartupDialog`
   - Remove the call to spawn device discovery on startup (defer until user requests)
   - Add comment explaining the manual-start workflow

2. **Update Empty State Display** (`src/tui/widgets/log_view/mod.rs:583-612`)
   - Change "Waiting for Flutter..." to "Not Connected"
   - Change "Make sure you're in a Flutter project directory" to "Press + to start a new session"

3. **Update Status Bar** (`src/tui/widgets/status_bar/mod.rs`)
   - Modify status display logic to show "○ Not Connected" when:
     - No sessions exist (`session_manager.len() == 0`)
     - OR no running sessions (`!has_running_sessions()`)
   - Use gray styling similar to "Stopped" state

**Milestone**: App starts in Normal mode with "Not Connected" status and centered instruction.

---

### Phase 2: Keybinding Changes

**Goal**: Replace "n" with "+" for new session trigger.

#### Steps

1. **Update Normal Mode Keys** (`src/app/handler/keys.rs:233-248`)
   - Remove the 'n' key handler for session creation
   - Add '+' key handler for session creation:
     - If sessions running: `Message::ShowDeviceSelector`
     - If no sessions: `Message::ShowStartupDialog`
   - Keep 'n' for search (now only `NextSearchMatch`, no fallback)

2. **Update Unit Tests** (`src/app/handler/tests.rs`)
   - Update `test_n_key_with_running_sessions_no_search` → `test_plus_key_...`
   - Update `test_n_key_without_sessions` → `test_plus_key_...`
   - Update test for 'n' to only expect `NextSearchMatch` or `None`

3. **Handle '+' Key in Other Modes**
   - Consider if '+' should work from DeviceSelector mode (for consistency)
   - Consider if '+' should work from StartupDialog mode (probably not)

**Milestone**: '+' key opens session dialog, 'n' only does search navigation.

---

### Phase 3: Documentation & Tests

**Goal**: Update all documentation and ensure tests pass.

#### Steps

1. **Update KEYBINDINGS.md** (`docs/KEYBINDINGS.md`)
   - Session Management section: Replace `n` with `+`
   - Log Search section: Remove note about 'n' being context-sensitive
   - Update Tips section
   - Add note about "Not Connected" state

2. **Update Snapshot Tests** (`src/tui/render/tests.rs`)
   - Update tests that show "Waiting for Flutter..." → "Not Connected"
   - Update tests that show help text
   - May need to regenerate snapshots with `cargo test -- --nocapture`

3. **Update E2E Test Utilities** (`tests/e2e/pty_utils.rs`)
   - Update `expect_header()` or add `expect_not_connected()` helper
   - Document new expected startup state
   - Remove workarounds that were needed for StartupDialog

4. **Re-enable Blocked Tests**
   - Settings page tests (`tests/e2e/settings_page.rs`) should work directly
   - Other tests may need minor adjustments

**Milestone**: All tests pass, documentation is accurate.

---

## Edge Cases & Risks

### Key Collision Risk
- **Risk:** '+' requires Shift key, may feel less discoverable
- **Mitigation:** Clear message "Press + to start" on screen; 'd' remains as alternative

### Backward Compatibility
- **Risk:** Users used to pressing 'n' for new session
- **Mitigation:** This is explicitly requested (no backward compatibility for 'n')

### Test Fixture Config
- **Risk:** Test fixtures may have `auto_start = true` unexpectedly
- **Mitigation:** Audit fixtures; ensure `auto_start = false` for most E2E tests

### Search 'n' Key Behavior Change
- **Risk:** Users pressing 'n' without active search will now get nothing
- **Mitigation:** Use '+' for new session or 'd' for device; 'n' is vim-style search-next

---

## Keyboard Shortcuts Summary

### Before
| Key | Action |
|-----|--------|
| `n` | Next search match (if search) OR Show device selector/startup dialog |
| `d` | Show device selector/startup dialog |

### After
| Key | Action |
|-----|--------|
| `n` | Next search match (only when search active) |
| `+` | Start new session (shows startup dialog if no sessions, device selector if sessions exist) |
| `d` | Same as `+` (unchanged) |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] App starts in `UiMode::Normal` when `auto_start = false`
- [ ] Status bar shows "○ Not Connected" when no sessions
- [ ] Log area shows "Press + to start a new session"
- [ ] `cargo check` passes
- [ ] `cargo clippy -- -D warnings` passes

### Phase 2 Complete When:
- [ ] '+' key shows StartupDialog when no sessions
- [ ] '+' key shows DeviceSelector when sessions exist
- [ ] 'n' key only triggers NextSearchMatch (or nothing)
- [ ] All keybinding unit tests pass
- [ ] `cargo test --lib` passes

### Phase 3 Complete When:
- [ ] KEYBINDINGS.md updated with '+' key
- [ ] All snapshot tests updated/pass
- [ ] E2E tests pass without workarounds
- [ ] Settings page tests are unblocked
- [ ] Full verification passes: `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings`

---

## Future Enhancements

1. **Help Overlay**: Add '?' key to show available keybindings
2. **Status Bar Hints**: Show contextual hints like "Press + to start" in status bar
3. **Quick Device Selection**: Allow '+1' to directly select first device without dialog

---

## References

- Bug context: `workflow/plans/bugs/settings-pty-tests/BUG.md`
- Test tasks: `workflow/plans/features/settings-page-testing/phase-1/TASKS.md`
- Existing startup logic: `src/tui/startup.rs`
- Key handlers: `src/app/handler/keys.rs`

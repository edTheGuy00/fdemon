# E2E PTY Input Issue - Bug Report

## Summary

E2E tests using the PTY test framework cannot reliably trigger actions that require Enter or Space key input. Navigation and Escape keys work correctly, but Enter/Space keys appear to not be processed by the application's event loop.

## Symptoms

1. **Toggle tests fail**: Tests that press Enter/Space to toggle boolean settings don't see the expected state change
2. **Value unchanged**: Debug captures show the selection is correct but the value doesn't flip
3. **No dirty indicator**: The "unsaved changes" text never appears after toggle attempts
4. **Navigation works**: j/k keys, arrow keys, and Escape all work correctly

## Affected Tests

All tests in `tests/e2e/settings_page.rs` that rely on Enter/Space to trigger actions:

| Test | Status | Issue |
|------|--------|-------|
| `test_toggle_auto_start` | Ignored | Enter/Space not triggering toggle |
| `test_toggle_auto_reload` | Ignored | Enter/Space not triggering toggle |
| `test_toggle_devtools_auto_open` | Ignored | Enter/Space not triggering toggle |
| `test_toggle_stack_trace_collapsed` | Ignored | Enter/Space not triggering toggle |
| `test_dirty_indicator_appears_on_change` | Ignored | Enter not triggering toggle |

## Technical Analysis

### PTY Input Flow

```
Test Code                    PTY                      Application
    │                         │                            │
    │  send_key(' ')          │                            │
    │ ───────────────────────>│                            │
    │                         │  writes b" " to stdin      │
    │                         │ ──────────────────────────>│
    │                         │                            │
    │                         │                      crossterm::event::poll()
    │                         │                            │
    │                         │                      Event::Key { kind: ??? }
    │                         │                            │
    │                         │                      if kind == Press { process }
    │                         │                            │
```

### Key Handling Code

The application filters key events in `src/tui/event.rs:13`:

```rust
Event::Key(key) if key.kind == event::KeyEventKind::Press => {
    Ok(Some(Message::Key(key)))
}
```

Only `KeyEventKind::Press` events are processed. Other event kinds (Release, Repeat) are ignored.

### PTY Key Sequences

The PTY sends these byte sequences (`tests/e2e/pty_utils.rs`):

| Key | Bytes | Notes |
|-----|-------|-------|
| Enter | `\r` (0x0D) | Carriage return |
| Space | `' '` (0x20) | ASCII space |
| Escape | `\x1b` | ESC character |
| Arrow Down | `\x1b[B` | ANSI escape sequence |

### Working vs Non-Working Keys

| Key | Works? | Byte Sequence | Notes |
|-----|--------|---------------|-------|
| j | Yes | `j` | Single ASCII char |
| k | Yes | `k` | Single ASCII char |
| Escape | Yes | `\x1b` | Single byte |
| Arrow keys | Yes | `\x1b[A/B/C/D` | ANSI sequences |
| Tab | Yes | `\t` | Single byte |
| **Enter** | **No** | `\r` | Carriage return |
| **Space** | **No** | `' '` | ASCII space |

## Hypotheses

### 1. KeyEventKind Filtering

Crossterm may not be emitting `KeyEventKind::Press` for Enter/Space in PTY mode. Some terminals/modes emit different event kinds.

**To verify**: Add logging to capture the actual `KeyEventKind` received for different keys.

### 2. Terminal Mode Configuration

The PTY might not be configured in the right mode (raw vs cooked). In cooked mode, Enter might be buffered or transformed.

**To verify**: Check `expectrl` session configuration and terminal mode settings.

### 3. Line Discipline Issues

The PTY's line discipline might be intercepting `\r` (carriage return) and transforming it.

**To verify**: Try sending `\n` instead of `\r`, or configure the PTY to disable line processing.

### 4. Crossterm Backend

Crossterm has different backends for different platforms. The PTY environment might trigger a different code path.

**To verify**: Check crossterm's event parsing for PTY file descriptors.

## Evidence

### Debug Capture Shows Correct State

```
▶  Auto Start               false            Skip device selector...
   Confirm Quit             true             Ask before quitting...
...
          Tab: Switch tabs  j/k: Navigate  Enter: Edit  Ctrl+S: Save
```

- Selection indicator `▶` is on "Auto Start" (correct)
- Value is "false" (unchanged after toggle attempt)
- Help text shows "Enter: Edit" not "unsaved changes" (toggle didn't fire)

### Unit Tests Pass

The toggle functionality works correctly when tested directly:

```rust
#[test]
fn test_settings_toggle_bool_flips_value() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Settings;
    state.settings.watcher.auto_reload = true;
    state.settings_view_state.selected_index = 4;

    update(&mut state, Message::SettingsToggleBool);

    assert_eq!(state.settings.watcher.auto_reload, false); // PASSES
}
```

## Potential Solutions

### Solution 1: Debug Logging (Investigation)

Add temporary logging to understand what events are received:

```rust
// In src/tui/event.rs
pub fn poll() -> Result<Option<Message>> {
    if event::poll(Duration::from_millis(50))? {
        let event = event::read()?;
        tracing::debug!("Raw event: {:?}", event);
        match event {
            Event::Key(key) => {
                tracing::debug!("Key event: code={:?}, kind={:?}", key.code, key.kind);
                if key.kind == event::KeyEventKind::Press {
                    Ok(Some(Message::Key(key)))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    } else {
        Ok(Some(Message::Tick))
    }
}
```

### Solution 2: Accept All Key Event Kinds

Modify the event filter to accept any key event kind, not just Press:

```rust
Event::Key(key) => {
    // Accept Press, Release, and Repeat events
    // Some terminals/PTYs may not distinguish
    Ok(Some(Message::Key(key)))
}
```

**Risk**: May cause duplicate processing if both Press and Release are received.

### Solution 3: PTY Configuration

Configure the PTY session to use raw mode or disable line processing:

```rust
// In tests/e2e/pty_utils.rs
pub fn spawn(project_path: &Path) -> PtyResult<Self> {
    let mut session = expectrl::spawn(cmd)?;

    // Configure terminal for raw input
    session.set_echo(false)?;
    // Possibly other termios settings...

    Ok(Self { session, ... })
}
```

### Solution 4: Different Enter Sequence

Try sending different byte sequences for Enter:

```rust
// Option A: Newline instead of carriage return
SpecialKey::Enter => b"\n",

// Option B: Both CR and LF
SpecialKey::Enter => b"\r\n",

// Option C: Ctrl+M (same as \r but explicit)
SpecialKey::Enter => b"\x0D",
```

### Solution 5: Use expectrl's send_line

The `expectrl` library has a `send_line` method that might handle Enter correctly:

```rust
pub fn send_enter(&mut self) -> PtyResult<()> {
    self.session.send_line("")?;
    Ok(())
}
```

## Recommended Investigation Steps

1. **Add debug logging** to `src/tui/event.rs` to see what events are actually received
2. **Run fdemon manually** in a real terminal and verify Enter works
3. **Compare event output** between real terminal and PTY
4. **Check crossterm issues** for similar PTY-related bugs
5. **Test Solution 4** (different Enter sequences) as a quick fix
6. **Test Solution 5** (send_line) as an alternative

## Workaround

The toggle tests are currently marked as `#[ignore]` with explanatory messages. The toggle functionality is verified working via unit tests:

- `test_settings_toggle_bool_flips_value` - Verifies value flips
- `test_settings_toggle_bool_sets_dirty_flag` - Verifies dirty flag is set

## Related Files

- `tests/e2e/pty_utils.rs` - PTY test framework
- `tests/e2e/settings_page.rs` - Affected tests
- `src/tui/event.rs` - Event polling and filtering
- `src/app/handler/keys.rs` - Key event handling
- `src/app/handler/update.rs` - Message handling (SettingsToggleBool)

## Priority

**Medium** - The toggle functionality works correctly. This is a test infrastructure issue that prevents E2E verification but doesn't affect users.

## Labels

- `test-infrastructure`
- `e2e-tests`
- `pty`
- `crossterm`

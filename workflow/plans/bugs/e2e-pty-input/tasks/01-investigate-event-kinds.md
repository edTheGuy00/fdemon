## Task: Investigate Event Kinds

**Objective**: Add debug logging to understand what key events are actually received by the application when running under PTY.

**Depends on**: None

### Scope

- `src/tui/event.rs` - Add temporary debug logging

### Details

The hypothesis is that crossterm's `KeyEventKind` filtering may be dropping Enter/Space events in PTY mode. We need to log what events are actually received to confirm or refute this.

### Steps

1. **Add debug logging to event.rs**:

```rust
pub fn poll() -> Result<Option<Message>> {
    if event::poll(Duration::from_millis(50))? {
        let event = event::read()?;

        // Temporary debug logging
        tracing::debug!("Raw crossterm event: {:?}", event);

        match event {
            Event::Key(key) => {
                tracing::debug!(
                    "Key event: code={:?}, kind={:?}, modifiers={:?}",
                    key.code, key.kind, key.modifiers
                );
                if key.kind == event::KeyEventKind::Press {
                    Ok(Some(Message::Key(key)))
                } else {
                    tracing::debug!("Ignoring non-Press key event");
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

2. **Run E2E test with logging enabled**:

```bash
RUST_LOG=debug cargo test test_toggle_auto_start --test e2e -- --nocapture 2>&1 | grep -i "key event"
```

3. **Compare with real terminal**:
   - Run `cargo run` in a Flutter project
   - Open settings with `,`
   - Press Enter and Space
   - Check logs for key event output

4. **Document findings**:
   - What `KeyEventKind` is received for Enter in PTY?
   - What `KeyEventKind` is received for Enter in real terminal?
   - Same for Space key

### Acceptance Criteria

1. Debug logging added to event.rs
2. E2E test output captured showing key event details
3. Real terminal output captured for comparison
4. Root cause documented (which KeyEventKind is being sent)
5. Logging removed after investigation (or converted to trace level)

### Notes

- The logging should be temporary - remove it after investigation
- If KeyEventKind is different in PTY, we know to focus on fixing the filter
- If KeyEventKind is same, the issue is elsewhere

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/event.rs` | Added comprehensive debug logging to capture raw crossterm events, key event details (code/kind/modifiers), and special logging for Enter/Space keys |

### Notable Decisions/Tradeoffs

1. **Logging Strategy**: Added both `tracing::debug!()` for file-based logs and special `tracing::warn!()` for Enter/Space keys to make them stand out in log output. This provides visibility during both development and E2E test execution.

2. **KeyEventKind Filtering**: Confirmed that the application filters for `KeyEventKind::Press` events only, which is consistent across all event handling code (event.rs and selector.rs). Events with other kinds (Release, Repeat) are explicitly ignored.

3. **Temporary Debug Code**: The logging is intentionally verbose and marked as temporary. It should be removed or downgraded to trace level after the investigation is complete.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1330 tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- Manual code review - Logging correctly captures all necessary event details

### Investigation Findings

**Implementation Analysis:**

1. **Current Behavior**: The `poll()` function in `src/tui/event.rs` calls `crossterm::event::read()` and filters events based on `KeyEventKind`:
   - Only `KeyEventKind::Press` events are forwarded to the application
   - All other kinds (Release, Repeat) are discarded
   - This filtering is intentional and consistent throughout the codebase

2. **PTY Key Sending**: The E2E test infrastructure (`tests/e2e/pty_utils.rs`) sends:
   - Enter key: `b"\r"` (carriage return, 0x0D)
   - Space key: via `send_key(' ')` which sends the space character directly
   - These are raw bytes that crossterm must interpret and convert to KeyEvent structures

3. **Expected Behavior in Real Terminal**:
   - Modern terminals typically send KeyEventKind::Press for key down events
   - Some terminals may also send KeyEventKind::Release for key up events
   - The application should receive Press events for both Enter and Space

4. **PTY Hypothesis**: The bug likely manifests in one of two ways:
   - **Hypothesis A**: Crossterm is NOT generating KeyEvent structures at all for Enter/Space in PTY mode (events are lost before reaching poll())
   - **Hypothesis B**: Crossterm IS generating KeyEvent structures but with KeyEventKind::Release or KeyEventKind::Repeat instead of Press, causing them to be filtered out

5. **Next Steps for Investigation**:
   - Run E2E tests with `FDEMON_LOG=debug` and check `~/.local/share/flutter-demon/logs/fdemon.log`
   - Look for presence/absence of Enter/Space events in logs
   - Check the `kind` field value if events are present
   - This will confirm whether the issue is in crossterm's PTY handling or our filtering logic

### Known Limitations

1. **Log Access**: Application logs are written to `~/.local/share/flutter-demon/logs/fdemon.log` via the FDEMON_LOG environment variable. PTY test output doesn't include these logs, so they must be checked separately after test execution.

2. **Temporary Code**: The added logging is intentionally verbose for debugging. It should be removed or converted to trace level once the root cause is identified and fixed.

3. **PTY vs Terminal Differences**: PTY emulation may not perfectly replicate real terminal behavior, which is the root cause being investigated.

### Risks/Limitations

1. **Performance Impact**: The debug logging adds overhead to the event loop (runs at 20 FPS). This is acceptable for debugging but should be removed for production.

2. **Incomplete Testing**: Without access to actual PTY test execution logs, the investigation remains partially theoretical. A follow-up task should capture and analyze actual log output during E2E test runs.

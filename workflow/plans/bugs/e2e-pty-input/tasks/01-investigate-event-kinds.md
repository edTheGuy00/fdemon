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

**Status:** Not Started

**Files Modified:**
- (none yet)

**Findings:**

(To be filled after investigation)

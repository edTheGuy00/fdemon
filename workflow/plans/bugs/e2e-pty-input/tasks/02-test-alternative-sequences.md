## Task: Test Alternative Key Sequences

**Objective**: Experiment with different byte sequences for Enter/Space to find one that works in PTY mode.

**Depends on**: 01-investigate-event-kinds

### Scope

- `tests/e2e/pty_utils.rs` - Modify key sequence definitions

### Details

Based on findings from Task 01, try different approaches to sending Enter/Space keys that might work better in PTY mode.

### Experiments to Try

#### Experiment A: Newline Instead of Carriage Return

```rust
// In pty_utils.rs SpecialKey::as_bytes()
// Change:
SpecialKey::Enter => b"\r",
// To:
SpecialKey::Enter => b"\n",
```

#### Experiment B: CR+LF Sequence

```rust
SpecialKey::Enter => b"\r\n",
```

#### Experiment C: Use expectrl's send_line

Add a new method:

```rust
impl FdemonSession {
    pub fn send_enter_line(&mut self) -> PtyResult<()> {
        self.session.send_line("")?;
        Ok(())
    }
}
```

#### Experiment D: Raw Ctrl+M

```rust
// Ctrl+M is equivalent to Enter (ASCII 13)
SpecialKey::Enter => b"\x0D",
```

#### Experiment E: Modify Event Filter

If the issue is KeyEventKind filtering:

```rust
// In src/tui/event.rs
// Accept any key event, not just Press
Event::Key(key) => {
    Ok(Some(Message::Key(key)))
}
```

### Testing Each Experiment

For each experiment:

1. Make the code change
2. Run: `cargo test test_toggle_auto_start --test e2e`
3. Record result (pass/fail)
4. If fail, check debug logs for what happened
5. Revert and try next experiment

### Results Table

| Experiment | Change | Result | Notes |
|------------|--------|--------|-------|
| A | `\r` → `\n` | | |
| B | `\r` → `\r\n` | | |
| C | send_line("") | | |
| D | `\x0D` | | |
| E | Remove KeyEventKind filter | | |

### Acceptance Criteria

1. All experiments tried and documented
2. At least one working solution identified
3. Side effects of working solution evaluated
4. Recommendation made for which solution to implement

### Notes

- Experiment E has the most risk of side effects (could cause double-processing)
- If Experiment E works, consider a more targeted fix
- Document any regressions in other tests

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Experiment Results:**

(To be filled after testing)

**Recommended Solution:**

(To be filled after testing)

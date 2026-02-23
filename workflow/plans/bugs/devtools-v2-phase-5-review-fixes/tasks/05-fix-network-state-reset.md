## Task: Fix `NetworkState::reset()` to Preserve `recording` Config

**Objective**: Ensure `NetworkState::reset()` preserves the `recording` field (set from `network_auto_record` config) alongside `max_entries`. Also remove redundant field assignments that match `Default`.

**Depends on**: None

**Severity**: MEDIUM — Config silently lost on reset (currently dead code, will affect users when wired in)

### Scope

- `crates/fdemon-app/src/session/network.rs:104-111`: Fix `reset()` method

### Details

**Current code (buggy):**
```rust
pub fn reset(&mut self) {
    *self = Self {
        max_entries: self.max_entries,
        filter_input_active: false,       // redundant — same as Default
        filter_input_buffer: String::new(), // redundant — same as Default
        ..Self::default()                  // Default has recording: true
    };
}
```

**Problems:**
1. `recording` is not preserved — reset overrides it with `Default::recording` which is `true`
2. If user configured `network_auto_record = false`, that setting is lost on reset
3. `filter_input_active: false` and `filter_input_buffer: String::new()` are identical to `Default` — redundant noise

**Fix:**
```rust
pub fn reset(&mut self) {
    *self = Self {
        max_entries: self.max_entries,
        recording: self.recording,
        ..Self::default()
    };
}
```

This preserves both config-derived fields (`max_entries`, `recording`) and resets everything else (entries, selected index, filter state, etc.) to defaults.

### Acceptance Criteria

1. `reset()` preserves `self.recording` alongside `self.max_entries`
2. Redundant `filter_input_active` and `filter_input_buffer` fields removed from the struct literal
3. Existing `test_reset_preserves_max_entries` still passes
4. New test `test_reset_preserves_recording` passes
5. `cargo test -p fdemon-app` passes

### Testing

```rust
#[test]
fn test_reset_preserves_recording() {
    let mut state = NetworkState::default();
    state.recording = false;  // simulate network_auto_record = false
    state.entries.push(/* some entry */);
    state.selected_index = Some(3);

    state.reset();

    assert_eq!(state.recording, false, "recording should be preserved across reset");
    assert!(state.entries.is_empty(), "entries should be cleared");
    assert_eq!(state.selected_index, None, "selected_index should be reset");
}
```

### Notes

- `reset()` is currently dead code — no handler calls it yet. The doc comment says "used on session switch or disconnect" but that wiring hasn't been done. This fix is preventive.
- When `reset()` is eventually wired into session lifecycle handlers, the preserved `recording` field will correctly reflect the user's config rather than silently reverting to `true`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/network.rs` | Fixed `reset()` to preserve `recording` alongside `max_entries`; removed redundant `filter_input_active` and `filter_input_buffer` fields; added doc comment explaining which fields are preserved; added `test_reset_preserves_recording` test |

### Notable Decisions/Tradeoffs

1. **Doc comment on `reset()`**: Added an explicit doc comment explaining that `max_entries` and `recording` are preserved because they are config-derived. This makes the intent obvious to future readers and prevents the bug from being reintroduced when wiring `reset()` into session lifecycle handlers.

2. **No structural changes**: The fix is minimal — only the two-line diff in `reset()` itself, plus the new test. No refactoring of unrelated code.

### Testing Performed

- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app session::network` — Passed (25 tests, including both `test_reset_preserves_max_entries` and new `test_reset_preserves_recording`)
- `cargo clippy -p fdemon-app -- -D warnings` — Passed (no warnings)
- `cargo fmt -p fdemon-app` — Applied (reformatted long assert message); re-ran tests to confirm still passing

### Risks/Limitations

1. **`reset()` is still dead code**: The method is not yet called by any handler. This fix is preventive — it ensures the correct behaviour when the method is eventually wired into session switch or disconnect handlers. No risk of regression since the method is currently unused.

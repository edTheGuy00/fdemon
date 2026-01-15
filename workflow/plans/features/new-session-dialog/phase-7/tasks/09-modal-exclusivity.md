# Task: Add Modal Exclusivity Assertions

## Summary

Add debug assertions to modal open methods to prevent state corruption from multiple modals being open simultaneously.

**Priority:** Major

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/state/dialog.rs` | Modify (add assertions) |

## Problem

The `open_*_modal()` methods don't verify that no other modal is already open. This could lead to state corruption if code accidentally tries to open a modal while another is open.

## Implementation

Add `debug_assert!` to each modal open method:

```rust
pub fn open_config_modal(&mut self) {
    debug_assert!(
        !self.has_modal_open(),
        "Cannot open config modal: another modal is already open"
    );
    // ... rest of implementation
}

pub fn open_flavor_modal(&mut self, known_flavors: Vec<String>) {
    debug_assert!(
        !self.has_modal_open(),
        "Cannot open flavor modal: another modal is already open"
    );
    // ... rest of implementation
}

pub fn open_dart_defines_modal(&mut self) {
    debug_assert!(
        !self.has_modal_open(),
        "Cannot open dart defines modal: another modal is already open"
    );
    // ... rest of implementation
}
```

### Helper method (if not exists)

```rust
pub fn has_modal_open(&self) -> bool {
    self.is_fuzzy_modal_open() || self.is_dart_defines_modal_open()
}
```

## Acceptance Criteria

1. Debug assertions in all `open_*_modal()` methods
2. Assertions catch state corruption during development
3. No runtime overhead in release builds (`debug_assert!` is compiled out)

## Testing

```bash
cargo fmt && cargo check && cargo test --lib && cargo clippy -- -D warnings
```

## Notes

- `debug_assert!` panics in debug builds but compiles to nothing in release
- This catches programming errors early in development
- Consider adding a unit test that verifies the assertion fires

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/new_session_dialog/state.rs` | Added `debug_assert!` to 3 modal open methods |

### Notable Decisions/Tradeoffs

1. **Used existing `has_modal_open()` helper**: The helper method already existed at line 649, so no new method was needed. It checks both `fuzzy_modal` and `dart_defines_modal` for completeness.

2. **Consistent assertion messages**: All three assertions use the same pattern for clarity and maintainability.

3. **Zero runtime overhead**: Using `debug_assert!` ensures these checks are compiled out in release builds, providing development-time safety without production performance impact.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (0.86s)
- `cargo test --lib` - Passed (1562 tests)
- `cargo clippy -- -D warnings` - Passed (8.92s)

### Risks/Limitations

1. **Debug-only protection**: Assertions only fire in debug builds. Production builds won't catch this error, but since the assertions prevent programmer mistakes (not user errors), this is acceptable.

2. **No unit test for assertions**: The task notes suggest adding a test that verifies the assertion fires. This could be added in a follow-up if desired, though testing panic behavior requires `#[should_panic]` which can be fragile.

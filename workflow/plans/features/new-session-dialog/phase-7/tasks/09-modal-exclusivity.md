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

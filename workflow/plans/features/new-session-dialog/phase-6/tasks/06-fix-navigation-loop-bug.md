# Task: Fix Infinite Loop in Field Navigation

## Summary

Fix the potential infinite loop bug in `next_enabled()` and `prev_enabled()` methods in LaunchContextField navigation.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/state.rs` | Modify (fix loop conditions) |

## Background

The code review identified a logic bug in the loop condition that could cause an infinite loop when all fields are disabled.

**Current (buggy):**
```rust
while is_disabled(next) && next.next() != start {  // BUG: checks next.next()
    next = next.next();
}
```

**Problem:** The loop condition `next.next() != start` advances one step ahead before comparing, which means the loop may never terminate if the current `next` equals `start`.

## Implementation

### 1. Fix `next_enabled()` loop condition

Location: `src/tui/widgets/new_session_dialog/state.rs:80-98`

```rust
pub fn next_enabled(self, is_disabled: impl Fn(Self) -> bool) -> Self {
    let mut next = self.next();
    let start = next;
    // FIX: Compare `next` directly with `start`, not `next.next()`
    while is_disabled(next) && next != start {
        next = next.next();
    }
    next
}
```

### 2. Fix `prev_enabled()` loop condition

Location: `src/tui/widgets/new_session_dialog/state.rs:100-113`

```rust
pub fn prev_enabled(self, is_disabled: impl Fn(Self) -> bool) -> Self {
    let mut prev = self.prev();
    let start = prev;
    // FIX: Compare `prev` directly with `start`, not `prev.prev()`
    while is_disabled(prev) && prev != start {
        prev = prev.prev();
    }
    prev
}
```

### 3. Add test for "all fields disabled" scenario

```rust
#[test]
fn test_next_enabled_all_disabled() {
    let field = LaunchContextField::Config;
    // When all fields are disabled, should return the next field (not hang)
    let result = field.next_enabled(|_| true);
    // Should complete without hanging, returning the next field
    assert_eq!(result, LaunchContextField::Mode);
}

#[test]
fn test_prev_enabled_all_disabled() {
    let field = LaunchContextField::Config;
    let result = field.prev_enabled(|_| true);
    // Should complete without hanging
    assert_eq!(result, LaunchContextField::Launch);
}
```

## Acceptance Criteria

1. Loop condition in `next_enabled()` changed from `next.next() != start` to `next != start`
2. Loop condition in `prev_enabled()` changed from `prev.prev() != start` to `prev != start`
3. Test case for "all fields disabled" scenario added and passes
4. `cargo test launch_context` passes
5. No regression in field navigation behavior

## Verification

```bash
cargo fmt && cargo check && cargo test launch_context && cargo clippy -- -D warnings
```

## Notes

- This is a CRITICAL fix - the bug could cause the UI to hang
- The fix maintains the same behavior when some fields are enabled
- When all fields are disabled, navigation should still work (returns next/prev field even if disabled)

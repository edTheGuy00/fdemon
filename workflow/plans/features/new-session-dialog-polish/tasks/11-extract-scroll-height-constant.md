## Task: Extract Scroll Height Magic Number to Constant

**Objective**: Replace hardcoded `10` for estimated visible height with a named constant for clarity and maintainability.

**Depends on**: None

**Estimated Time**: 10m

**Priority**: Major

**Source**: Code Review - Code Quality Inspector

### Scope

- `src/app/handler/new_session/target_selector.rs`: Extract constant and update usages

### Details

The value `10` is used as an estimated visible height for scroll calculations in two places without explanation.

**Current code (lines 18, 28):**
```rust
// In handle_device_up()
state
    .new_session_dialog_state
    .target_selector
    .adjust_scroll(10);  // Magic number

// In handle_device_down()
state
    .new_session_dialog_state
    .target_selector
    .adjust_scroll(10);  // Magic number
```

**Required fix:**
```rust
/// Default estimated visible height for scroll calculations.
/// Used when actual render height is unavailable (TEA pattern constraint).
/// This is an approximation that works well for typical terminal sizes.
const DEFAULT_ESTIMATED_VISIBLE_HEIGHT: usize = 10;

// In handle_device_up()
state
    .new_session_dialog_state
    .target_selector
    .adjust_scroll(DEFAULT_ESTIMATED_VISIBLE_HEIGHT);

// In handle_device_down()
state
    .new_session_dialog_state
    .target_selector
    .adjust_scroll(DEFAULT_ESTIMATED_VISIBLE_HEIGHT);
```

### Acceptance Criteria

1. Named constant `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` defined at module level
2. Doc comment explains the constant's purpose and why it's an estimate
3. Both usages of `10` replaced with the constant
4. Existing scroll tests continue to pass
5. No functional behavior change

### Testing

No new tests needed - this is a refactoring for code clarity. Run existing tests:

```bash
cargo test target_selector
cargo test scroll
```

### Notes

- This is a pure refactoring with no functional change
- The TEA pattern means we don't have render dimensions in the handler layer
- The value `10` is reasonable for most terminal sizes (dialog typically shows 8-12 devices)
- Future enhancement could be to pass actual viewport height from render layer

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending)

**Implementation Details:**
(pending)

**Testing Performed:**
(pending)

**Notable Decisions:**
(pending)

**Risks/Limitations:**
(pending)

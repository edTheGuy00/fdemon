# Action Items: NewSessionDialog Polish

**Review Date:** 2026-01-21
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2

---

## Critical Issues (Must Fix)

### 1. UTF-8 Truncation Panic Risk
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `src/tui/widgets/new_session_dialog/mod.rs`
- **Lines:** 42-49 (`truncate_with_ellipsis`), 53-63 (`truncate_middle`)
- **Problem:** Byte-based string slicing panics on UTF-8 multi-byte character boundaries
- **Required Action:** Replace byte indexing with `.chars()` iteration
- **Acceptance:**
  - Add test: `test_truncate_with_emoji()` using "iPhone 🔥" with max_width=8
  - All truncation tests pass
  - No panics with multi-byte UTF-8 input

**Fix Example:**
```rust
pub fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    if text.chars().count() <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        let truncated: String = text.chars().take(max_width - 3).collect();
        format!("{}...", truncated)
    }
}
```

### 2. Incomplete Selection Preservation
- **Source:** Logic Reasoning Checker, Code Quality Inspector
- **File:** `src/app/handler/update.rs`
- **Lines:** 290-307
- **Problem:** Selection preservation code is commented out with "TODO" despite required methods existing
- **Required Action:** Uncomment the selection preservation logic
- **Acceptance:**
  - Add test: `test_selection_preserved_on_background_refresh()`
  - User's selected device remains selected after background refresh completes

**Fix Example:**
```rust
// Preserve selection if possible (Task 04 - Device Cache Usage)
let previous_selection = state
    .new_session_dialog_state
    .target_selector
    .selected_device_id();

state
    .new_session_dialog_state
    .target_selector
    .set_connected_devices(devices);

// Restore selection if device still exists
if let Some(device_id) = previous_selection {
    state
        .new_session_dialog_state
        .target_selector
        .select_device_by_id(&device_id);
}
```

---

## Major Issues (Should Fix)

### 3. Magic Number for Scroll Adjustment
- **Source:** Code Quality Inspector
- **File:** `src/app/handler/new_session/target_selector.rs`
- **Lines:** 18, 28
- **Problem:** Hardcoded `10` for estimated visible height
- **Suggested Action:** Extract to named constant
- **File to modify:** Same file, add constant at module level

```rust
/// Default estimated visible height for scroll calculations.
/// Used when actual render height is unavailable (TEA pattern constraint).
const DEFAULT_ESTIMATED_VISIBLE_HEIGHT: usize = 10;
```

### 4. Background Refresh Error Handling Mismatch
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `src/tui/actions.rs`
- **Lines:** 61-65
- **Problem:** Comment says "errors are logged only" but implementation sends `DeviceDiscoveryFailed` which may show UI error
- **Suggested Action:** Either update comment OR add `background: bool` to message

---

## Minor Issues (Consider Fixing)

### 5. Missing Doc Comments on Public Utilities
- **File:** `src/tui/widgets/new_session_dialog/mod.rs`
- **Lines:** 42, 53
- **Problem:** `truncate_with_ellipsis` and `truncate_middle` lack `///` docs
- **Suggestion:** Add doc comments explaining behavior and edge cases

### 6. Scroll Indicator Width Threshold
- **File:** `src/tui/widgets/new_session_dialog/device_list.rs`
- **Lines:** 138-142
- **Problem:** Magic number `50` for indicator text switching
- **Suggestion:** Extract to constant `COMPACT_INDICATOR_WIDTH_THRESHOLD`

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] All critical issues resolved (items 1-2)
- [ ] All major issues resolved or justified (items 3-4)
- [ ] New tests added for UTF-8 truncation
- [ ] New test added for selection preservation
- [ ] `cargo fmt` - Code formatted
- [ ] `cargo check` - No compilation errors
- [ ] `cargo test` - All tests pass
- [ ] `cargo clippy -- -D warnings` - No warnings
- [ ] Manual test: Device with emoji name renders without panic
- [ ] Manual test: Selection preserved across background refresh

---

## Verification Commands

```bash
# Full verification suite
cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings

# Specific test patterns
cargo test truncate           # Truncation utilities
cargo test selection          # Selection preservation
cargo test cache              # Device cache
cargo test scroll             # Scroll state

# Run with output for debugging
cargo test -- --nocapture
```

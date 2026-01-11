# Action Items: New Session Dialog (Phase 1 & Phase 2)

**Review Date:** 2026-01-11
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 3

---

## Critical Issues (Must Fix)

### 1. Fix Unchecked Array Access in `selected_value()`

- **Source:** Logic Reasoning Checker
- **File:** `src/tui/widgets/new_session_dialog/state.rs`
- **Line:** 128-134
- **Problem:** Direct array indexing `self.items[idx]` can panic if index is out of bounds
- **Required Action:** Replace with safe access using `.get()`

```rust
// BEFORE (unsafe)
if let Some(&idx) = self.filtered_indices.get(self.selected_index) {
    Some(self.items[idx].clone())
}

// AFTER (safe)
if let Some(&idx) = self.filtered_indices.get(self.selected_index) {
    self.items.get(idx).cloned()
}
```

- **Acceptance:** Unit test added that verifies no panic with invalid indices

---

### 2. Fix Integer Overflow in `render_dim_overlay()`

- **Source:** Code Quality Inspector
- **File:** `src/tui/widgets/new_session_dialog/fuzzy_modal.rs`
- **Line:** 179-188
- **Problem:** `area.y + area.height` can overflow `u16::MAX`
- **Required Action:** Use saturating arithmetic

```rust
// BEFORE (overflow risk)
for y in area.y..area.y + area.height {
    for x in area.x..area.x + area.width {

// AFTER (safe)
for y in area.y..area.y.saturating_add(area.height) {
    for x in area.x..area.x.saturating_add(area.width) {
```

- **Acceptance:** Clippy passes, function handles edge case areas

---

### 3. Add Modal Mutual Exclusion Check

- **Source:** Logic Reasoning Checker
- **File:** `src/app/handler/update.rs`
- **Line:** 1759
- **Problem:** Opening a modal doesn't check if another modal is already open
- **Required Action:** Add guard check before opening modal

```rust
// In Message::NewSessionDialogOpenFuzzyModal handler
Message::NewSessionDialogOpenFuzzyModal { modal_type } => {
    if state.new_session_dialog_state.has_modal_open() {
        tracing::warn!("Cannot open modal while another is open");
        return UpdateResult::none();
    }
    // ... proceed with opening
}
```

- **Acceptance:** Test added that verifies modal cannot be opened when another is active

---

## Major Issues (Should Fix)

### 4. Fix Layer Boundary Violation: TUI → Daemon

- **Source:** Architecture Enforcer
- **File:** `src/tui/widgets/new_session_dialog/state.rs`
- **Line:** 6
- **Problem:** TUI layer imports `Device` from daemon layer
- **Suggested Action:** Move `Device` type to `core/` layer alongside `BootableDevice`

This is a larger refactoring task - recommend creating separate issue to track.

---

### 5. Fix Layer Boundary Violation: Message → TUI Widgets

- **Source:** Architecture Enforcer
- **File:** `src/app/message.rs`
- **Line:** 7
- **Problem:** App layer imports types from TUI layer
- **Suggested Action:** Move `DartDefine`, `FuzzyModalType`, `TargetTab` to `app/` layer

This is a larger refactoring task - recommend creating separate issue to track.

---

### 6. Extract Magic Number to Module Constant

- **Source:** Code Quality Inspector, Risks Analyzer
- **File:** `src/tui/widgets/new_session_dialog/state.rs`
- **Line:** 169
- **Problem:** `VISIBLE_ITEMS = 7` buried inside method
- **Suggested Action:** Move to module-level constant

```rust
// At top of module
/// Number of visible items in the fuzzy modal list.
/// Must match the height calculation in FuzzyModal::render().
pub const FUZZY_MODAL_VISIBLE_ITEMS: usize = 7;

// In adjust_scroll() method
fn adjust_scroll(&mut self) {
    if self.selected_index < self.scroll_offset {
        self.scroll_offset = self.selected_index;
    } else if self.selected_index >= self.scroll_offset + FUZZY_MODAL_VISIBLE_ITEMS {
        self.scroll_offset = self.selected_index - FUZZY_MODAL_VISIBLE_ITEMS + 1;
    }
}
```

---

## Minor Issues (Consider Fixing)

### 7. Add Documentation for Domain Types

- **File:** `src/core/types.rs`
- **Problem:** `Platform`, `DeviceState`, `BootableDevice` lack doc comments
- **Suggested Action:** Add `///` doc comments with usage examples

---

### 8. Track Placeholder Handlers

- **File:** `src/app/handler/update.rs`
- **Line:** 1734-1756
- **Problem:** 26 messages stubbed with no tracking mechanism
- **Suggested Action:** Create checklist in Phase 4 TASKS.md for all placeholder handlers

---

### 9. Document Constructor Differences

- **File:** `src/tui/widgets/new_session_dialog/state.rs`
- **Line:** 302-324
- **Problem:** `new()` vs `with_configs()` difference unclear
- **Suggested Action:** Add doc comments explaining when to use each

---

## Re-review Checklist

After addressing issues, the following must pass:

- [x] Issue #1 resolved: `selected_value()` uses safe array access
- [x] Issue #2 resolved: `render_dim_overlay()` uses saturating arithmetic
- [x] Issue #3 resolved: Modal mutual exclusion check added
- [x] `cargo check` passes
- [x] `cargo test --lib` passes (1388 tests passed)
- [x] `cargo clippy -- -D warnings` passes
- [x] No new warnings introduced

**All blocking issues resolved on 2026-01-11**

---

## Verification Commands

```bash
# Full verification
cargo fmt && cargo check && cargo test --lib && cargo clippy -- -D warnings

# Specific test for fuzzy modal
cargo test fuzzy_modal

# Run only the new session dialog tests
cargo test new_session_dialog
```

---

## Future Work (Track in Phase 3+)

- [ ] Refactor index-based device selection to ID-based
- [ ] Implement flavor discovery from project analysis
- [ ] Add Unicode fallbacks for terminal compatibility
- [ ] Consider splitting Message enum with nested enums
- [ ] Set concrete deadline for removing old dialog systems

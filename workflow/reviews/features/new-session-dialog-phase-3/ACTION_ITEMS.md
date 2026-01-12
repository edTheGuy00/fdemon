# Action Items: Phase 3 - Dart Defines Modal

**Review Date:** 2026-01-12
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2

---

## Critical Issues (Must Fix)

### 1. Delete Operation Selection Logic Flaw

- **Source:** logic_reasoning_checker
- **File:** `src/tui/widgets/new_session_dialog/state.rs`
- **Line:** 423-425
- **Problem:** The selection adjustment condition `self.selected_index > 0 && self.selected_index >= self.defines.len()` is flawed. The `> 0` check is redundant and the logic doesn't match the spec ("Selection moves to previous item"). Currently, deleting a middle item keeps selection at same index (pointing to next item instead of previous).
- **Required Action:** Simplify condition to remove redundant check. If spec requires "previous item" behavior, add that logic explicitly:
  ```rust
  // Current (flawed):
  if self.selected_index > 0 && self.selected_index >= self.defines.len() {
      self.selected_index = self.defines.len().saturating_sub(1);
  }

  // Fixed (minimal):
  if self.selected_index >= self.defines.len() {
      self.selected_index = self.defines.len().saturating_sub(1);
  }

  // Fixed (per spec "previous item"):
  if !self.defines.is_empty() {
      if self.selected_index >= self.defines.len() {
          self.selected_index = self.defines.len() - 1;
      } else if self.selected_index > 0 {
          self.selected_index -= 1;
      }
  }
  ```
- **Acceptance:**
  - Add test: Delete middle item (index 1) in 3-item list, verify selection moves correctly
  - All existing tests still pass
  - `cargo test delete_define` passes

### 2. Confirm Handler Ignores Save Failure

- **Source:** logic_reasoning_checker
- **File:** `src/app/handler/update.rs`
- **Line:** 1911-1913
- **Problem:** `modal.save_edit()` returns `false` when key is empty, but handler ignores return value. User gets no feedback that save failed.
- **Required Action:** Handle the return value. Options:
  1. **Minimal:** Log the failure for debugging
  2. **Better:** Stay in edit mode and keep focus on Key field
  3. **Best:** Add validation message to modal state for UI feedback
  ```rust
  // Option 1 (minimal):
  DartDefinesEditField::Save => {
      if !modal.save_edit() {
          tracing::debug!("Dart define save failed: key cannot be empty");
      }
  }

  // Option 2 (better UX):
  DartDefinesEditField::Save => {
      if !modal.save_edit() {
          modal.edit_field = DartDefinesEditField::Key; // Return focus to key
      }
  }
  ```
- **Acceptance:**
  - Add test: Confirm with empty key stays in edit mode or logs
  - `cargo test` passes

---

## Major Issues (Should Fix)

### 3. Full-Screen Cell Iteration Performance

- **Source:** risks_tradeoffs_analyzer
- **File:** `src/tui/widgets/new_session_dialog/dart_defines_modal.rs`
- **Line:** 616-623, 663-671
- **Problem:** O(width * height) iteration on every render for dimming/clearing
- **Suggested Action:**
  - Add performance test that fails if render > 16ms on 200x60 terminal
  - Or document this as known limitation
  - Or optimize to render only visible portion

### 4. Hardcoded VISIBLE_ITEMS Constant

- **Source:** risks_tradeoffs_analyzer
- **File:** `src/tui/widgets/new_session_dialog/state.rs`
- **Line:** 338
- **Problem:** `VISIBLE_ITEMS = 10` is hardcoded but must match widget height
- **Suggested Action:**
  - Make dynamic by passing available height to scroll calculation
  - Or add validation/assertion in widget render
  - Or document the coupling with a shared constant

### 5. No Input Length Limits

- **Source:** risks_tradeoffs_analyzer
- **File:** `src/tui/widgets/new_session_dialog/state.rs`
- **Line:** 441-447
- **Problem:** Unbounded string input could exhaust memory or break rendering
- **Suggested Action:**
  ```rust
  const MAX_KEY_LENGTH: usize = 256;
  const MAX_VALUE_LENGTH: usize = 4096;

  pub fn input_char(&mut self, c: char) {
      match self.edit_field {
          DartDefinesEditField::Key if self.editing_key.len() < MAX_KEY_LENGTH => {
              self.editing_key.push(c);
          }
          DartDefinesEditField::Value if self.editing_value.len() < MAX_VALUE_LENGTH => {
              self.editing_value.push(c);
          }
          _ => {}
      }
  }
  ```

---

## Minor Issues (Consider Fixing)

### 6. Document Navigation Invariant

- **File:** `src/tui/widgets/new_session_dialog/state.rs:318`
- **Action:** Add comment explaining `list_item_count() >= 1` invariant

### 7. Cursor Position Limitation

- **File:** `src/tui/widgets/new_session_dialog/dart_defines_modal.rs`
- **Action:** Document that cursor is always at end; track as future enhancement

### 8. Clone Performance for Large Lists

- **File:** `src/tui/widgets/new_session_dialog/state.rs:845`
- **Action:** Document expected max defines count (< 50 typical)

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Critical issue #1: Delete selection logic fixed, test added
- [ ] Critical issue #2: Save failure handled or logged
- [ ] `cargo fmt` - Code formatted
- [ ] `cargo check` - No compilation errors
- [ ] `cargo test` - All tests pass (including new ones)
- [ ] `cargo clippy -- -D warnings` - No clippy warnings
- [ ] Major issues tracked for follow-up (can merge after critical fixes)

---

## Issue Tracking

After merging with critical fixes, track these for follow-up:

| Issue | Priority | Tracking |
|-------|----------|----------|
| Performance: full-screen cell iteration | Medium | Performance backlog |
| Hardcoded VISIBLE_ITEMS | Medium | Widget refactoring backlog |
| No input length limits | Medium | Input validation backlog |
| Cursor position support | Low | UX improvements backlog |

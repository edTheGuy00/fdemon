## Task: Fix Dart Defines Modal Overlay Consistency

**Objective**: Make the Dart Defines modal use the same overlay pattern as the Fuzzy modal. Currently the parent uses `Clear.render()` while all other modals use `modal_overlay::dim_background()`, and the widget itself redundantly calls `dim_background()` internally.

**Depends on**: None

**Review Reference**: REVIEW.md #5 (Major), ACTION_ITEMS.md #5

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` line 412: Replace `Clear.render()` with `modal_overlay::dim_background()`
- `crates/fdemon-tui/src/widgets/new_session_dialog/dart_defines_modal.rs` line 655: Remove redundant self-dimming

### Details

**Current behavior** — two inconsistencies:

1. **Parent overlay** (`mod.rs:404-417`):
   ```rust
   fn render_dart_defines_modal(&self, dialog_area: Rect, buf: &mut Buffer) {
       // ...
       Clear.render(dialog_area, buf);  // ← Clears area (no dimming)
       let dart_defines_modal = DartDefinesModal::new(modal_state);
       dart_defines_modal.render(dialog_area, buf);
   }
   ```

   Compare with Fuzzy modal parent (`mod.rs:385-402`):
   ```rust
   fn render_fuzzy_modal_overlay(&self, dialog_area: Rect, buf: &mut Buffer) {
       // ...
       modal_overlay::dim_background(buf, dialog_area);  // ← Dims background
       let fuzzy_modal = FuzzyModal::new(modal_state).loading(is_loading);
       fuzzy_modal.render(dialog_area, buf);
   }
   ```

2. **Widget self-dimming** (`dart_defines_modal.rs:652-656`):
   ```rust
   impl Widget for DartDefinesModal<'_> {
       fn render(self, area: Rect, buf: &mut Buffer) {
           crate::widgets::modal_overlay::dim_background(buf, area);  // ← Widget dims its own bg
           let modal_area = Self::modal_rect(area);
           // ...
       }
   }
   ```
   The Fuzzy modal widget does NOT self-dim — the parent handles it.

**Result**: The Dart Defines modal first clears the area (losing dialog content), then dims the cleared area (dimming nothing). The visual result is a black overlay instead of a dimmed dialog background.

**Fix approach**:

1. In `mod.rs:render_dart_defines_modal()`: Replace `Clear.render(dialog_area, buf)` with `modal_overlay::dim_background(buf, dialog_area)` to match the fuzzy modal pattern
2. In `dart_defines_modal.rs`: Remove the `dim_background()` call inside the widget's `render()` method, since the parent now handles it (matching the fuzzy modal widget pattern)

This ensures:
- Parent handles overlay (consistent across all modals)
- Widget only renders its own content (single responsibility)
- User sees dimmed dialog content behind the modal (not a black area)

### Acceptance Criteria

1. Dart Defines modal shows dimmed dialog content behind it (not blank/black)
2. Overlay behavior matches the Fuzzy modal pattern
3. No double-dimming artifacts
4. `cargo check -p fdemon-tui` passes

### Testing

- Verify existing dart_defines_modal tests pass
- Manual verification that the modal overlay looks correct

### Notes

- `dim_background()` is defined in `crates/fdemon-tui/src/widgets/modal_overlay.rs:99-113`. It sets all cells in the area to `TEXT_MUTED` fg and `DEEPEST_BG` bg.
- The `Clear` widget from ratatui simply resets all cells to the default style, losing any rendered content underneath.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Line 412: Replaced `Clear.render(dialog_area, buf)` with `modal_overlay::dim_background(buf, dialog_area)` to match fuzzy modal pattern |
| `crates/fdemon-tui/src/widgets/new_session_dialog/dart_defines_modal.rs` | Line 653-655: Removed redundant `dim_background()` call from widget's render method |

### Notable Decisions/Tradeoffs

1. **Parent-handles-overlay pattern**: The parent component (`mod.rs`) now handles the overlay dimming for the Dart Defines modal, matching the pattern used by the Fuzzy modal. This ensures consistent behavior across all modals and follows the single responsibility principle.
2. **Widget simplification**: The `DartDefinesModal` widget now only renders its own content without self-dimming, making it consistent with other modal widgets like `FuzzyModal`.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed (0.67s)
- `cargo test -p fdemon-tui --lib` - Passed (430 tests)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed

### Risks/Limitations

None identified. The changes align the Dart Defines modal with the established pattern used by other modals in the codebase, fixing the visual bug where a black overlay appeared instead of dimmed dialog content.

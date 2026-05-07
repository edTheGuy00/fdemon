# Task 02: FuzzyModal Scroll Underflow Guard

## Goal

Fix the `usize` underflow panic in `widgets/new_session_dialog/fuzzy_modal.rs` that triggers when the user types a no-match query while the list is scrolled past page 1.

## Background

`crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs:230-235` (current code):

```rust
let list_area = chunks[2];
let visible_height = list_area.height as usize;
let start = modal.state.scroll_offset;
let end = (start + visible_height).min(modal.state.filtered_indices.len());

for screen_row in 0..(end - start) {
    let abs_index = start + screen_row;
    ...
}
```

When `scroll_offset > filtered_indices.len()` — possible if the user scrolls to row 30 and then types a query that filters all results out — `end = filtered_indices.len() < start`. In debug, `end - start` underflows `usize` and panics. In release, it wraps to `~usize::MAX` and the loop runs effectively forever, freezing the TUI.

The current Phase-5 production code does not reset `scroll_offset` when the query changes (verified by reading `crates/fdemon-app/src/new_session_dialog/fuzzy_modal.rs::FuzzyModalState::set_query` and similar). Fixing the scroll-reset on query change is a separate concern; this task fixes the panic risk regardless.

## Files

**Modify:**
- `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs`

**Read (reference):**
- `crates/fdemon-app/src/new_session_dialog/fuzzy_modal.rs` — `FuzzyModalState` shape (no writes)

## Plan

1. **Locate the underflow site** at lines 230-235 (per current diff). Replace with:

   ```rust
   let list_area = chunks[2];
   let visible_height = list_area.height as usize;
   let total = modal.state.filtered_indices.len();
   // `scroll_offset` may exceed `total` if a previously-scrolled list is filtered
   // down by a query change. Clamp `start` to `total` so the loop bound is non-negative.
   let start = modal.state.scroll_offset.min(total);
   let end = (start + visible_height).min(total);

   for screen_row in 0..(end - start) {
       let abs_index = start + screen_row;
       ...
   }
   ```

   Or more conservatively, guard before the loop:

   ```rust
   if end <= start {
       return; // or: skip the loop body
   }
   ```

   Prefer the first form (clamp `start`) so the function still runs the post-loop logic if any. Audit the surrounding code to choose the safer form.

2. **Add a regression test** in `fuzzy_modal.rs::tests`:

   ```rust
   #[test]
   fn render_with_regions_no_panic_when_filter_clears_results_while_scrolled() {
       use crate::widgets::MouseCtx;
       use fdemon_app::{MouseRegions, MouseRegionsBuilder};

       let mut state = FuzzyModalState::new(/* ... */);
       state.scroll_offset = 30;          // Scroll well past the eventual filtered count
       state.filtered_indices.clear();    // Simulate "no matches"

       let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
       let mut regions = MouseRegions::new();
       let mut builder = regions.builder();
       let mut ctx = MouseCtx::new(builder);
       let modal = FuzzyModal::new(&state, /* ... */);

       // Must not panic.
       fuzzy_modal_render_with_regions(
           Rect::new(0, 0, 80, 24),
           &mut buf,
           modal,
           Some(&mut ctx),
       );

       // No regions registered when filtered_indices is empty.
       assert_eq!(regions.entries().len(), 0);
   }
   ```

   Adjust `FuzzyModalState::new` and `FuzzyModal::new` calls to match the actual constructors (read the file to verify). The key invariant under test: `(scroll_offset, filtered_indices.len()) = (30, 0)` does not panic.

3. **(Optional, only if scope-creep is approved)**: also reset `scroll_offset` to `0` whenever `filtered_indices` shrinks below the current offset. This is a UX improvement orthogonal to the panic fix and may be deferred to a separate task — leave for now.

4. **Run quality gates**:
   ```bash
   cargo test -p fdemon-tui fuzzy_modal::tests
   cargo test --workspace
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo check --workspace --all-targets
   ```

## Acceptance Criteria

- [ ] Underflow guard added in `fuzzy_modal_render_with_regions`. The body must not panic when `scroll_offset > filtered_indices.len()`.
- [ ] Regression test added that exercises the `(scroll_offset = 30, filtered_indices.len() = 0)` case and asserts no panic + zero regions registered.
- [ ] All quality gates pass.

## Notes

- This is a 2-3-line production fix plus 1 regression test. Total task scope is intentionally narrow.
- T10 modifies the *call site* of `fuzzy_modal_render_with_regions` in `new_session_dialog/mod.rs` (single-pass refactor, Minor #14). T10 does NOT modify `fuzzy_modal.rs` itself. Parallel-safe.
- If, while reading the surrounding code, you discover that `set_query` *should* reset `scroll_offset` (root-cause fix vs. defensive guard), surface that observation in the Completion Summary but do not act on it in this task — file a follow-up.

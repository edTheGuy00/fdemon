# Task 09: Inspector Tree Glyph + Network Details Polish

## Goal

Two unrelated minor cleanups in two files:
1. Harden the inspector tree glyph X-coordinate calculation against extreme depths to avoid silent region discard (Minor #18 — security medium).
2. Move the `LABEL_COL_WIDTH` constant in network/request_details.rs to the proper position (after `use` blocks) per Rust conventions (Minor #24).

## Background

- **Glyph overflow**: In `widgets/devtools/inspector/tree_panel.rs:138-149`, the glyph X coordinate is `tree_inner.x.saturating_add((*depth as u16).saturating_mul(2))`. At extreme depths (e.g., depth 1000 → `depth * 2 = 2000`), this places the glyph rect far past `tree_inner.right()`. The current `if glyph_x < tree_inner.right()` guard catches this and discards silently — no panic, but a deep node's glyph click silently produces a row-select instead of a toggle. The security review flagged this as a defense-in-depth concern.

- **Const placement**: In `widgets/devtools/network/request_details.rs:14-15`:
  ```rust
  /// Width of the label column in the General tab layout (characters).
  const LABEL_COL_WIDTH: u16 = 18;
  use ratatui::{...};
  ```
  A `const` declaration appears between two `use` statements. `cargo fmt` accepts this but the convention is `use` blocks first, then `const` / `static` items.

## Files

**Modify:**
- `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`
- `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs`

## Plan

### Part A: Glyph X-coordinate hardening

1. **Locate the glyph rect calculation** in `tree_panel.rs` (around lines 138–149 per the review). The current code is approximately:
   ```rust
   let glyph_x = tree_inner.x.saturating_add((*depth as u16).saturating_mul(2));
   if glyph_x < tree_inner.right() {
       let glyph_rect = MouseRect::new(glyph_x, y, 1, 1);
       // builder.click(glyph_rect, MouseAction::emit(Message::DevToolsInspectorToggleNode { index }), 0);
   }
   ```

2. **Replace `saturating_mul` + silent discard** with `checked_mul` + early return:
   ```rust
   // depth-to-x conversion. checked_mul guarantees we don't silently saturate
   // and place the glyph far off-screen. If the multiplication overflows u16
   // (depth > ~32k), the tree is pathological — skip glyph registration.
   let Some(indent) = (*depth as u16).checked_mul(2) else {
       continue; // skip glyph for impossibly deep node
   };
   let Some(glyph_x) = tree_inner.x.checked_add(indent) else {
       continue; // skip glyph if rect would land past u16 bounds
   };
   if glyph_x >= tree_inner.right() {
       continue; // skip glyph clipped past the right edge (normal case)
   }
   let glyph_rect = MouseRect::new(glyph_x, y, 1, 1);
   builder.click(glyph_rect, /* ... */, 0);
   ```

   Or, if the existing structure pushes the row before the glyph (which is required for last-pushed-wins), use a labeled break or explicit guard chain — adapt to the actual code shape.

3. **No new test required for this change.** The original behavior on extreme depths was "silent fallback to row-select," and the new behavior is "skip glyph registration entirely" — both are no-region outcomes. The only observable difference is at depths between 32,768 and 65,535 (`u16::MAX`), where checked arithmetic now skips and saturating arithmetic would have placed the glyph at u16::MAX (still off-screen, still skipped by the right-edge guard). So this is a correctness hardening with no observable behavior change at realistic depths.

### Part B: Const placement

1. **Move `LABEL_COL_WIDTH`** in `widgets/devtools/network/request_details.rs` from between the two `use` blocks to after all `use` statements:
   ```rust
   use ratatui::{...};
   use ratatui::{...};

   /// Width of the label column in the General tab layout (characters).
   const LABEL_COL_WIDTH: u16 = 18;
   ```

2. **Run `cargo fmt`** to normalize. No behavioral change.

## Acceptance Criteria

- [ ] `tree_panel.rs` glyph X-coordinate uses `checked_mul` and `checked_add` (or equivalent overflow-safe arithmetic).
- [ ] `LABEL_COL_WIDTH` const is placed after all `use` blocks in `request_details.rs`.
- [ ] All existing inspector tree tests pass.
- [ ] All existing network request-details tests pass.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets` pass.

## Notes

- **Do not touch** `widgets/devtools/inspector/mod.rs` or `widgets/devtools/inspector/tests.rs` — those are owned by Task 03.
- **Do not touch** `widgets/devtools/network/mod.rs` or `widgets/devtools/network/request_table.rs` — same reasoning.
- The two files in this task are unrelated, but bundling them avoids a 1-line fix in its own task. Both are pure mechanical changes.
- The glyph overflow change is a hardening change, not a bug fix — no real-world inspector tree reaches `u16::MAX / 2` depth. The security reviewer flagged it as defense-in-depth. Treat the work effort accordingly: a few lines of refactoring, no new test.

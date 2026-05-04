# Task 02: Delete dead `to_mouse_rect` helper

**Status:** Not Started
**Estimated Hours:** 0.1h
**Depends On:** —
**Crate / Area:** `fdemon-tui`

## Goal

Remove the dead `to_mouse_rect` helper from `crates/fdemon-tui/src/widgets/mod.rs` (review item 2).

The helper is currently kept alive by `#[allow(dead_code)]` with a stale comment claiming "Task 07 will add the call site from tabs.rs." Task 07 has shipped — both the multi-session tab regions and the single-session device pill construct `MouseRect::new(x, y, w, h)` directly rather than going through the helper. The `#[allow(dead_code)]` is therefore suppressing a legitimate dead-code warning. Per `docs/CODE_STANDARDS.md`, helpers without consumers should be deleted, not preserved speculatively.

If a future phase needs the helper, it can be re-added in five lines.

## Files Modified (Write)

- `crates/fdemon-tui/src/widgets/mod.rs`

## Files Read

- (none required — change is local)

## Implementation Steps

1. **Locate the helper** at `crates/fdemon-tui/src/widgets/mod.rs:34-47`:
   ```rust
   /// Convert a `ratatui::layout::Rect` to a `fdemon_app::MouseRect`.
   /// ...
   // Task 07 (tab/device-pill regions) will add the call site from tabs.rs.
   #[allow(dead_code)]
   pub(crate) fn to_mouse_rect(r: ratatui::layout::Rect) -> fdemon_app::MouseRect {
       fdemon_app::MouseRect::new(r.x, r.y, r.width, r.height)
   }
   ```

2. **Delete the doc comment, the `// Task 07 …` line, the `#[allow(dead_code)]` attribute, and the function body.**

3. **Re-check the surrounding `pub use` block** to confirm nothing else in `widgets/mod.rs` referred to `to_mouse_rect`. (Grep already confirms: only the function's own definition appears.)

## Acceptance Criteria

- [ ] `to_mouse_rect` does not appear in `crates/fdemon-tui/src/widgets/mod.rs`
- [ ] No `#[allow(dead_code)]` attribute remains in `widgets/mod.rs` related to mouse helpers
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes (no dead-code warning resurfaces)
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo test --workspace` passes

## Notes

- This is a 5-line deletion. No callers exist; verified by grep `to_mouse_rect` across `crates/fdemon-tui/src/`.
- If Phase 4 later wants this helper for log row / frame bar / network row click registration, the implementor for that task can re-add it. The conversion body is one line.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/mod.rs` | Deleted `to_mouse_rect` function (doc comment, stale task comment, `#[allow(dead_code)]` attribute, and function body — 14 lines removed) |

### Notable Decisions/Tradeoffs

1. **Full block deletion**: Removed the entire function including its multi-line doc comment and the stale `// Task 07 ...` comment, leaving the file clean with no dead-code annotations or forward-references.

### Testing Performed

- `cargo check --workspace --all-targets` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no dead-code warning)
- `cargo fmt --all -- --check` - Passed
- `cargo test --workspace` - Passed (4,133 tests passed, 0 failed)

### Risks/Limitations

1. **None**: Pure deletion with no callers. The conversion logic remains trivially re-creatable as a one-liner if needed in a future phase.

## Task: fdemon-tui pill click fix + cleanups (M1, m2, m3, m4, n1, n3)

**Objective**: Fix the pill mouse-click so it actually emits `Message::ScrollToBottom` (M1 — currently shadowed by the log-row click region), and clean up the surrounding minor review items: builder `///` doc (m2), saturating coordinate arithmetic (m3), literal glyphs (m4), plus pill+scrollbar and narrow-terminal boundary tests (n1, n3).

**Depends on**: None. Runs in parallel with task 01 (different crate, disjoint files). The pill render/click code and its tests construct `LogView` with explicit `unseen_log_count` values, so they are independent of task 01's runtime increment changes.

**Estimated Time**: 1–1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: register the pill click region at `z=1`; add the builder `///` doc; switch pill coordinate math to saturating arithmetic; use literal glyphs in the pill constants.
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: strengthen the click test to assert hit-test precedence; add pill+scrollbar co-render test; add narrow-terminal boundary tests.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/render/mod.rs`: `MouseCtx::click_at_z(rect, action, z)` at line 51 — the z-aware registration entry point.
- `crates/fdemon-app/src/mouse_regions.rs`: `hit_test` at lines 187-199 (`.max_by_key(|(i, e)| (e.z_index, *i))`) — confirms higher `z_index` wins, and last-pushed wins only at equal z.
- `crates/fdemon-tui/src/widgets/log_view/styles.rs`: pill style tokens (`JUMP_HINT_FG`/`JUMP_HINT_BG`), read-only.

### Details

#### 1. M1 — register the pill click region at a higher z-index

Root cause: `hit_test` picks `max_by_key((z_index, index))`. The pill region is pushed from
`render_jump_to_latest_pill` (called at `mod.rs:1643`, registers at `:1870`) **before** the per-row
`ClickLogRow` regions (`:1712`). Both at `z=0`, so the later-pushed row region wins on the pill's cell.

Fix: register the pill at `z=1` so it wins regardless of push order. In `render_jump_to_latest_pill`:

```rust
// Mouse routing: clicking the pill emits Message::ScrollToBottom.
// Registered at z=1 so it wins over the z=0 per-row ClickLogRow region that
// also covers the pill's cell (hit_test is max-by (z_index, push_index)).
if let Some(ctx) = mouse_ctx {
    let rect = Rect { x, y, width: pill_width, height: 1 };
    ctx.click_at_z(rect, MouseAction::emit(Message::ScrollToBottom), 1);
}
```

Confirm `MouseCtx` exposes `click_at_z` (it does — `render/mod.rs:51`). The `MouseRect`/`Rect`
conversion already used by the existing `ctx.click` call applies identically.

#### 2. m2 — `///` doc on the builder method

The `LogView::unseen_log_count(count)` builder (`mod.rs:~181`) has a doc on the struct field but not
on the `impl` method. Add a `///` matching the sibling builders (`filter_state`, `wrap_mode`, …):

```rust
/// Set the count of log entries that arrived while the view was scrolled away
/// from the tail. Drives the jump-to-latest pill. Default 0 (no pill drawn).
pub fn unseen_log_count(mut self, count: usize) -> Self {
    self.unseen_log_count = count;
    self
}
```

#### 3. m3 — saturating coordinate arithmetic

In `render_jump_to_latest_pill` (`mod.rs:1857-1858`), replace bare `u16` arithmetic with saturating
ops to match the surrounding style (the row-clip logic just above already uses `saturating_*`):

```rust
let y = content_area.y.saturating_add(content_area.height).saturating_sub(1);
let x = content_area
    .x
    .saturating_add(content_area.width)
    .saturating_sub(pill_width)
    .saturating_sub(1);
```

The existing guards (`content_area.height == 0` early-return, `content_area.width < pill_width + 1`
suppression) already make these safe; this is explicitness/consistency, and must not change the
computed coordinates for valid inputs.

#### 4. m4 — literal glyphs in pill constants

Replace the `\u{...}` escapes with the literal characters used elsewhere in the module:

```rust
const JUMP_HINT_PREFIX: &str = "↓ ";
const JUMP_HINT_SUFFIX: &str = " · G to jump";
```

Purely cosmetic; the rendered label and `pill_width` (`label.chars().count()`) are unchanged.

#### 5. n1 — pill + scrollbar co-render test

Add a test that renders at a width where both the pill and the scrollbar draw simultaneously (scrolled
up, `total_lines > visible_lines`, `auto_scroll == false`, `unseen_log_count > 0`), and assert: the
keybind text `G to jump` is fully intact on the bottom row, and the scrollbar end-cap renders on its
column. This locks down the right-margin vs scrollbar-column relationship.

#### 6. n3 — narrow-terminal boundary tests

Add two tests pinning the exact inclusive suppression boundary from `content_area.width < pill_width + 1`:
- width `== pill_width` → pill **suppressed** (no `↓`, no `G to jump`).
- width `== pill_width + 1` → pill **rendered**.

Compute `pill_width` for a known count (e.g. derive from the same label format) so the test does not
hardcode a brittle literal; or assert against `JUMP_HINT_PREFIX`/`JUMP_HINT_SUFFIX` lengths.

### Acceptance Criteria

1. The pill click region is registered at `z=1`; a hit-test at the pill cell resolves to
   `Message::ScrollToBottom` even when a `ClickLogRow` region covers the same cell. (Strengthened test
   calls `regions.hit_test(pill_x, pill_y, MouseButton::Left)` and asserts the `ScrollToBottom` action,
   not mere region existence.)
2. `LogView::unseen_log_count` builder has a `///` doc comment.
3. Pill `x`/`y` use saturating arithmetic; rendered position is unchanged for valid inputs.
4. Pill constants use literal `↓` and `·` glyphs; label text and width unchanged.
5. A test renders pill + scrollbar together and asserts the keybind text is intact and the scrollbar
   end-cap is present (n1).
6. Boundary tests assert suppression at `width == pill_width` and rendering at `width == pill_width + 1` (n3).
7. No regression in existing `log_view` snapshot/render tests (the pill still only renders when
   `!auto_scroll && unseen_log_count > 0`).
8. `cargo test -p fdemon-tui`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Testing

Strengthen the existing click test (replace the existence-only assertion):

```rust
#[test]
fn jump_hint_click_emits_scroll_to_bottom() {
    let mut buf = make_buffer(60, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(10); // ensure a log row covers the pill's bottom row
    let mut regions = MouseRegions::default();
    let mut builder = regions.builder();
    let mut ctx = MouseCtx::new(&mut builder);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(3);

    render_with_regions(Rect::new(0, 0, 60, 10), &mut buf, &mut state, view, Some(&mut ctx));
    drop(builder); // release the borrow before hit_test, per existing test pattern

    // Pick a cell known to be inside the pill on the bottom row.
    let pill_y = 9;
    let pill_x = /* compute right-aligned pill cell, e.g. 60 - 2 */ 58;
    let entry = regions.hit_test(pill_x, pill_y, MouseButton::Left);
    assert!(matches!(
        entry.map(|e| e.on_left.as_ref()),
        Some(Some(MouseAction::Emit(Message::ScrollToBottom)))
    ));
}
```

Adjust the `hit_test` call and action-matching to the actual `MouseRegionEntry` API (see how existing
`ClickLogRow` tests in this file assert hit-test results) — the shape above is illustrative. Mirror
existing helpers (`make_buffer`, `make_logs`, `default_icons`, `read_row`) for the n1/n3 tests.

### Notes

- **Do not edit any `fdemon-app` file** — `clear_logs`, `handle_page_down`, and the filter-gated
  increment are task 01.
- **Do not add a new `Message` variant** — `Message::ScrollToBottom` already exists.
- **z=1 rationale:** modal overlays pass `None` as `MouseCtx` to base-UI widgets, so the log view (and
  its pill) only register regions when no modal is active. z=1 here orders the pill above sibling
  log-view regions only; it does not interfere with modal precedence.
- **Out of scope:** `unicode-width` for `pill_width` (n2), test-helper alias cleanup (n4), test-module
  relocation (n5).

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | M1: pill click at z=1; m2: updated builder doc; m3: saturating arithmetic on x/y coords; m4: literal ↓ and · glyphs in constants |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Strengthened click test to use hit_test (M1 verification); added n1 pill+scrollbar co-render test; added n3 boundary tests |

### Notable Decisions/Tradeoffs

1. **hit_test in click test**: The strengthened test uses `make_logs(10)` so that a `ClickLogRow` region actually covers the pill's cell at y=8, making the z=1 assertion meaningful. With only 2 entries, no row region would cover that cell and the test would pass trivially.

2. **n3 test uses count=1**: Using the single-digit count means pill_width is derived directly from `JUMP_HINT_PREFIX.chars().count() + 1 + " new".len() + JUMP_HINT_SUFFIX.chars().count()` via the format string, avoiding brittle hardcoded literals. The label computation mirrors the production code exactly.

3. **n1 scrollbar verification**: Rather than checking the exact scrollbar column char (which depends on ratatui internals for the thumb position), the test checks any row in the scrollbar column (x=59) for `▼` (the end-cap). This is stable regardless of offset/thumb position.

4. **Saturating arithmetic format**: The `y` computation was reformatted as a multi-line chain to pass `cargo fmt --check` (the single-line version exceeded the line width limit).

### Testing Performed

- `cargo test -p fdemon-tui -- jump_hint` — 9 tests, all pass
- `cargo test -p fdemon-tui` — 1340 tests pass, 0 failed
- `cargo fmt --all -- --check` — pass
- `cargo clippy --workspace --all-targets -- -D warnings` — pass

### Risks/Limitations

1. **n1 scrollbar column**: If Ratatui changes the scrollbar rendering to use a different column, the test would need updating. The column x=59 is `area.width - 1` for a 60-wide area; this is unlikely to change.

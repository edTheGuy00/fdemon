## Task: Fix Flex Explorer Visual Bugs

**Objective**: Resolve the user-reported vertical MainAxis label readability issue (C1) plus the two associated quality bugs in the same file (`buf.area` placement bug C3 and dead-parameter anti-pattern M1). Two minor bundled cleanups (m6 vacuous match, m7 wrong constant) are folded in since they share the file.

**Depends on**: None

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs`

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` — palette + tab block plumbing (read for `Block::title` patterns used by sibling tabs)
- `crates/fdemon-core/src/widget_tree.rs` — `Axis`, `MainAxisAlignment`, `CrossAxisAlignment`, `LayoutInfo` (read-only for type signatures)
- `workflow/reviews/features/devtools-inspector-parity/phase-2/REVIEW.md` — C1, C3, M1 findings

### Details

This task addresses four issues in `flex_explorer_tab.rs`, all touching the same file. Apply in order.

#### 1. Redesign vertical main-axis label presentation (C1 — USER-REPORTED)

**Current behavior** (`render_main_axis_strip_vertical`, lines 430–473):
- 3-column strip on the right side of the panel.
- Column 0 (lines 453–461): renders `▲` arrow, then "MainAxis" letters one-per-row, then `▼` arrow.
- Column 1 (lines 463–472): renders the alignment value (e.g. "start") letters one-per-row.
- Column 2: unused.
- Result: cluster of stacked single-character text at the far right edge, unreadable. Longer values like "spaceBetween" (12 chars) won't fit when content height is small.

**Required change:** Move the textual labels into the outer block title. The block currently shows `" Cross Axis: stretch "` (built by `cross_axis_label`, line ~736). Extend the title to include the main-axis label and alignment value, in this format:

```
" Main ↕ start  │  Cross Axis: stretch "
```

For horizontal flex (Row): use `↔` instead of `↕`. The strip then carries ONLY the `▲` / `▼` arrows centred between header and footer rows — no text. This keeps the strip simple, eliminates the readability problem entirely, and matches the existing pattern where `cross_axis_label` already lives in the title.

**Implementation steps:**

1. Rename `cross_axis_label(direction, alignment)` to `flex_axis_title(direction, main_align, cross_align)` and have it produce the combined title string. Signature:
   ```rust
   fn flex_axis_title(
       direction: Axis,
       main_align: MainAxisAlignment,
       cross_align: CrossAxisAlignment,
   ) -> String
   ```
   Returns `format!(" Main {arrow} {main} │ Cross Axis: {cross} ", arrow = ..., main = ..., cross = ...)`. The leading and trailing spaces preserve the current title style (e.g. compare `" Cross Axis: stretch "`).

2. Update the two call sites (`render_vertical_flex` and `render_horizontal_flex`) to construct the title via `flex_axis_title(direction, main_align, cross_align)` instead of the current `cross_axis_label(direction, cross_align)`.

3. Strip the textual rendering from `render_main_axis_strip_vertical` and `render_main_axis_strip_horizontal`. The vertical strip then becomes:
   - `▲` at row 0
   - blank cells between (or optionally a single `│` divider column if it improves visual flow — implementor's call)
   - `▼` at last row
   Three columns is now more than enough for just two arrow rows.

4. Update `main_axis_value()` and `cross_axis_value()` (helpers that map enum → display string) — these are still needed by the new `flex_axis_title` builder; no changes to their bodies.

**Acceptance:** Open the Flex Explorer tab on a `Column` widget. The title bar shows both main-axis and cross-axis labels readable left-to-right. The right-side strip shows only the `▲` and `▼` arrows. No vertical letter-stacks anywhere.

#### 2. Fix `buf.area` → `area` in size-guard fallback (C3)

**Location:** `flex_explorer_tab.rs:91` (top-level `render()`):

```rust
// CURRENT (wrong):
if area.height < MIN_FLEX_VIZ_HEIGHT || area.width < MIN_FLEX_VIZ_WIDTH {
    render_muted_centered(buf.area, buf, "Terminal too small for flex visualization.");
    return;
}
```

Change to `area`:

```rust
if area.height < MIN_FLEX_VIZ_HEIGHT || area.width < MIN_FLEX_VIZ_WIDTH {
    render_muted_centered(area, buf, "Terminal too small for flex visualization.");
    return;
}
```

One character. The inner fallback inside `render_flex_viz` already uses `inner` (the post-block-render rect), so this is the only site with the bug.

**Acceptance:** A regression test in the file's test module sets up a `Rect { x: 5, y: 5, width: 8, height: 6 }` (smaller than `MIN_FLEX_VIZ_HEIGHT` / `MIN_FLEX_VIZ_WIDTH`) inside a larger buffer (e.g. `Buffer::empty(Rect::new(0, 0, 100, 50))`), invokes `render`, and verifies the message text appears within `area` and NOT at the centre of the full buffer.

#### 3. Remove dead `inspector_state` parameter from `render_flex_viz` (M1)

**Location:** `flex_explorer_tab.rs:101` (signature) and line ~170 (`let _ = inspector_state;` body silence).

**Change:**

1. Remove `inspector_state: &InspectorState` from the `render_flex_viz` signature.
2. Remove the `let _ = inspector_state;` line from the function body.
3. Update the single call site in `render()` (line ~96) to drop the argument.
4. Remove the now-unused `InspectorState` import if no other reference remains.

**Acceptance:** `cargo clippy --workspace --all-targets -- -D warnings` passes with no `unused_variables` or `unused_imports` warnings. The function signature has no parameter that requires lint-silencing.

#### 4. Bundled minor: simplify `cross_axis_label` vacuous match (m6)

Since `cross_axis_label` is being renamed and rewritten by step 1, the vacuous match (`Axis::Vertical | Axis::Horizontal => "Cross Axis"`) is naturally eliminated. Confirm: the new `flex_axis_title` should always use the literal `"Cross Axis"` for the cross-axis portion — no match needed.

#### 5. Bundled minor: introduce `MIN_HORIZONTAL_FLEX_HEIGHT` constant (m7)

**Location:** `flex_explorer_tab.rs:526` inside `render_horizontal_flex`:

```rust
// CURRENT (semantically wrong — MAIN_AXIS_STRIP_WIDTH is a column count):
if area.height <= MAIN_AXIS_STRIP_WIDTH.min(3) {
    render_muted_centered(area, buf, "Terminal too small for flex visualization.");
    return;
}
```

Add a new constant near the existing layout constants (top of the file, near `MAIN_AXIS_STRIP_WIDTH`):

```rust
/// Minimum height (rows) required to render a horizontal flex visualization.
/// Composed of: 1 header row + 1 child row + 1 strip row + 1 footer row.
const MIN_HORIZONTAL_FLEX_HEIGHT: u16 = 4;
```

Replace the guard:

```rust
if area.height < MIN_HORIZONTAL_FLEX_HEIGHT {
    render_muted_centered(area, buf, "Terminal too small for flex visualization.");
    return;
}
```

(Note `<` not `<=` — if `area.height == MIN_HORIZONTAL_FLEX_HEIGHT` exactly, rendering should succeed.)

### Acceptance Criteria

1. **User verification (C1):** Opening Flex Explorer on a `Column` widget shows main-axis and cross-axis labels readable left-to-right in the block title. Strip shows only `▲` / `▼`.
2. **Fallback placement (C3):** "Terminal too small for flex visualization." renders centred in the tab pane, not the full buffer. Regression test in place.
3. **Dead-param removal (M1):** `render_flex_viz` signature has no `inspector_state` parameter; no `let _ =` silence in the body.
4. **Quality gate:** `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.
5. **No test regressions:** All existing `flex_explorer_tab.rs` tests still pass after the title format change. (The 11 existing Phase 2 tests will need their title-string assertions updated; the test for cross-axis-only title at line ~880 should now assert the combined `" Main ... │ Cross ... "` format. This is expected and intentional.)

### Testing

Add or update these tests in the existing `#[cfg(test)] mod tests` block at the bottom of the file:

```rust
#[test]
fn render_centers_too_small_message_in_panel_not_buffer() {
    // Buffer is much larger than the tab pane; pane is smaller than min dims
    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 50));
    let area = Rect::new(20, 10, 8, 4); // below MIN_FLEX_VIZ_WIDTH / HEIGHT
    let state = InspectorState::default();
    let layout = LayoutInfo::default();
    render(area, &mut buf, &state, &layout, "Column");

    // The "Terminal too small" message must land within `area`, not buffer centre
    let mut found_in_area = false;
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let ch = buf.get(x, y).symbol();
            if ch.contains("T") || ch.contains("e") { found_in_area = true; }
        }
    }
    assert!(found_in_area, "message must render inside `area`, not the full buffer");

    // And NOT outside area in the buffer centre (where buf.area centre would be ~50,25)
    let buf_centre_ch = buf.get(50, 25).symbol();
    assert!(buf_centre_ch.trim().is_empty(),
        "no message should render at the full-buffer centre");
}

#[test]
fn vertical_flex_title_contains_main_and_cross_axis_labels() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
    let area = Rect::new(0, 0, 80, 20);
    let state = InspectorState::default();
    let layout = LayoutInfo {
        direction: Some(Axis::Vertical),
        main_axis_alignment: Some(MainAxisAlignment::SpaceBetween),
        cross_axis_alignment: Some(CrossAxisAlignment::Stretch),
        main_axis_size: Some(MainAxisSize::Max),
        ..LayoutInfo::default()
    };
    render(area, &mut buf, &state, &layout, "Column");

    let text: String = (0..buf.area.width)
        .map(|x| buf.get(x, 0).symbol().to_string())
        .collect();
    assert!(text.contains("Main") && text.contains("spaceBetween"),
        "title must include main-axis label: got `{text}`");
    assert!(text.contains("Cross") && text.contains("stretch"),
        "title must include cross-axis label: got `{text}`");
}

#[test]
fn vertical_main_axis_strip_no_longer_renders_letter_stacks() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
    let area = Rect::new(0, 0, 80, 20);
    let state = InspectorState::default();
    let layout = LayoutInfo {
        direction: Some(Axis::Vertical),
        main_axis_alignment: Some(MainAxisAlignment::Start),
        cross_axis_alignment: Some(CrossAxisAlignment::Center),
        ..LayoutInfo::default()
    };
    render(area, &mut buf, &state, &layout, "Column");

    // Strip is at right edge. Read the rightmost 3 cols. Should contain ▲ and ▼
    // but NOT the letters "M", "a", "i", "n" stacked vertically.
    let strip_x_start = area.right() - MAIN_AXIS_STRIP_WIDTH;
    let mut letters_in_strip = 0;
    for y in (area.y + 1)..(area.bottom() - 1) {
        for x in strip_x_start..area.right() {
            let s = buf.get(x, y).symbol();
            if s.chars().any(|c| c.is_ascii_alphabetic()) { letters_in_strip += 1; }
        }
    }
    assert_eq!(letters_in_strip, 0, "no letters should appear in the side strip");
}
```

Also update the existing tests that assert on `" Cross Axis: stretch "` literal title — change them to assert on the new combined title format (or use `.contains("Cross")` checks instead of exact matches).

### Notes

- Per cross-cutting constraint #2 in `TASKS.md`, the title-based redesign is the chosen approach. Do NOT widen `MAIN_AXIS_STRIP_WIDTH` — that takes horizontal space away from child boxes which are already cramped at typical terminal widths.
- The arrows `↕` / `↔` in the title are Unicode glyphs that render in most monospace fonts. If a future review finds rendering issues in specific terminals, the implementor of a later task can swap to ASCII `^v` / `<>` — but for now Unicode is consistent with the existing `▲ / ▼ / ◀ / ▶` strip arrows.
- The `flex_axis_title` builder MUST return a `String` (not `&'static str`) since it interpolates the runtime alignment values.
- Removing `inspector_state` from `render_flex_viz` may surface that other functions in the file also accept it unused — check the call chain. Currently only `render_flex_viz` has the dead param per the review.

---

## Completion Summary

**Status:** Not Started

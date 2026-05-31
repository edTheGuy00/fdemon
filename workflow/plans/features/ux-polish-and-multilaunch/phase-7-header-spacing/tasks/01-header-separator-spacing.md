## Task: Multi-session header — separator rule + remove bottom dead space

**Objective**: Re-layout the multi-session header's inner area so it reads
**title → dim separator rule → device tabs**, eliminating the empty `CARD_BG` row that
currently sits at the bottom of the bordered block and giving the title/tabs visual
separation. `header_height` stays 5 — this is a pure re-distribution of the 3 inner rows
already allocated, plus one new dim `─` rule.

**Depends on**: none (sequence after Phase 6 if its reload-flash `bg` change is not yet
merged — this task threads that same `bg` into the new rule).

**Estimated Time**: 1–1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/header.rs`: rework the multi-session branch of
  `render_main_header` (lines 128-153); add a private `render_separator_row` helper; add
  render tests.
- `crates/fdemon-tui/src/layout.rs`: reword the `header_height` comment only (no code
  change).

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/theme/palette.rs`: `BORDER_DIM` (`Color::Rgb(45, 51, 59)`) for
  the rule fg.
- `crates/fdemon-tui/src/theme/styles.rs`: `glass_block` (border/inner geometry —
  understand, do **not** modify).
- `crates/fdemon-tui/src/widgets/tabs.rs`: `render_session_tabs` already pads
  `x+1 / width-2` (lines 118-122) — the rule's inset must match this.

### Background

`layout::create_with_sessions` (`layout.rs:23-46`) gives the header
`Constraint::Length(5)` when `session_count > 1`. After the rounded `glass_block` border
(`theme/styles.rs:102`, no `Padding`), that leaves **3 inner rows**. But the multi-session
branch of `render_main_header` only paints two of them:

```rust
// crates/fdemon-tui/src/widgets/header.rs:128-149 (current)
if has_multiple_sessions {
    if inner.height >= 2 {
        let title_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };
        header.render_title_row(title_area, buf, false, None);

        let tabs_area = Rect {
            x: inner.x,
            y: inner.y + 1,                                  // tabs adjacent to title
            width: inner.width,
            height: inner.height.saturating_sub(1),          // claims 2 rows, paints 1
        };
        if let Some(session_manager) = header.session_manager {
            render_session_tabs(tabs_area, buf, session_manager, header.icons, ctx);
        }
    } else {
        header.render_title_row(inner, buf, false, None);
    }
}
```

`render_session_tabs` renders a single-row ratatui `Tabs`; the leftover inner row
(`inner.y + 2`) is never written and shows as blank `CARD_BG` inside the border — the
"too much empty space at the bottom" in the report. The title and tabs also sit on
adjacent rows with no separation.

The `bg` for the whole header is computed once at `header.rs:104` (reload-flash blend from
`CARD_BG` toward `STATUS_GREEN`). The new rule must paint on that same `bg`, not a
hard-coded `CARD_BG`, so a reload flash tints the block uniformly.

### Details

**1. Re-layout the multi-session branch** (`header.rs:128-153`)

Replace the inner-area split so the normal multi-session case (`inner.height >= 3`, which
is `== 3` given `header_height == 5`) places: title at `inner.y`, the rule at
`inner.y + 1`, tabs at `inner.y + 2`. Keep the existing 2-row layout as a squeezed-terminal
fallback and the 1-row title-only path unchanged.

```rust
if has_multiple_sessions {
    let title_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };

    if inner.height >= 3 {
        // Title → dim separator rule → tabs
        header.render_title_row(title_area, buf, false, None);

        let sep_area = Rect { x: inner.x, y: inner.y + 1, width: inner.width, height: 1 };
        render_separator_row(sep_area, buf, bg);

        let tabs_area = Rect {
            x: inner.x,
            y: inner.y + 2,
            width: inner.width,
            height: inner.height.saturating_sub(2),
        };
        if let Some(session_manager) = header.session_manager {
            render_session_tabs(tabs_area, buf, session_manager, header.icons, ctx);
        }
    } else if inner.height == 2 {
        // Squeezed terminal: title + tabs adjacent, no separator (prior behaviour)
        header.render_title_row(title_area, buf, false, None);
        let tabs_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        };
        if let Some(session_manager) = header.session_manager {
            render_session_tabs(tabs_area, buf, session_manager, header.icons, ctx);
        }
    } else {
        header.render_title_row(inner, buf, false, None);
    }
}
```

- `bg` is already in scope (computed at `header.rs:104`); pass it into the helper.
- The single-session / no-session `else` branch (`header.rs:154-157`) is untouched.

**2. Add the `render_separator_row` helper** (private fn in `header.rs`)

Paint a `─` rule inset 1 cell on each side to align with the tabs' `x+1 / width-2`
padding, `BORDER_DIM` fg on the supplied `bg`:

```rust
/// Render a dim horizontal rule separating the title row from the device tabs
/// in the multi-session header. Inset by one cell on each side to align with the
/// tabs' left/right padding; painted on `bg` so it tints with the reload flash.
fn render_separator_row(area: Rect, buf: &mut Buffer, bg: ratatui::style::Color) {
    if area.width <= 2 || area.height == 0 {
        return;
    }
    let rule_width = area.width.saturating_sub(2) as usize;
    let line = Line::from(Span::styled(
        "─".repeat(rule_width),
        Style::default().fg(palette::BORDER_DIM).bg(bg),
    ));
    let rule_area = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    line.render(rule_area, buf); // `Line: Widget` via the existing `widgets::Widget` import
}
```

- Use the box-drawing `─` (U+2500), matching the rounded glass border family.
- `Line`/`Span`/`Style`/`Widget` are already imported at the top of `header.rs`; add no new
  imports beyond `palette::BORDER_DIM` (the `palette` module is already imported).

**3. Reword the layout comment** (`layout.rs:24-31`)

Change the multi-session arm comment so it no longer calls the extra row "breathing room"
at the bottom:

```rust
let header_height = if session_count > 1 {
    5 // Top border + title row + separator row + tabs row + bottom border
} else {
    3 // Top border + title row + bottom border
};
```

Leave the value `5` and all `layout.rs` tests unchanged.

### Acceptance Criteria

1. With ≥2 sessions and `inner.height >= 3`, the header renders the title on `inner.y`, a
   `─` rule (`BORDER_DIM` fg) on `inner.y + 1`, and the device tabs on `inner.y + 2`; the
   bottom inner row is no longer a blank painted band.
2. The rule is inset 1 cell on each side (starts at `inner.x + 1`, width `inner.width - 2`)
   so it lines up with the tab labels.
3. The rule's `bg` is the same flash-blended `bg` used for the rest of the header (paints
   `palette::CARD_BG` when `reload_flash == 0`).
4. `header_height` is still 5; `layout.rs` tests pass unchanged and only the comment is
   reworded.
5. Squeezed-terminal fallback: with `inner.height == 2` the header renders title + tabs
   (no separator) without clipping; with `inner.height < 2` it renders title only.
6. Single-session and no-session headers are visually unchanged (the `else` branch is
   untouched).
7. `cargo test -p fdemon-tui --lib`, `cargo fmt --all -- --check`, and
   `cargo clippy -p fdemon-tui --all-targets -- -D warnings` are clean.

### Testing

Add render tests in `header.rs`'s test module (model after existing `render_main_header`
tests that build a `MainHeader` with a multi-session `SessionManager` and render into a
`Buffer`). Use a header `Rect` of height 5 (→ `inner.height == 3`):

```rust
#[test]
fn multi_session_header_renders_separator_between_title_and_tabs() {
    // Build a SessionManager with >= 2 sessions; render_main_header into a height-5 Rect.
    // Assert: a cell on row inner.y + 1 (x >= inner.x + 1) holds '─'.
    // Assert: a device-name glyph from the tabs lands on row inner.y + 2, not inner.y + 1.
}

#[test]
fn multi_session_header_has_no_trailing_empty_inner_row() {
    // With inner.height == 3, the separator occupies the middle row; assert the
    // previously-empty bottom inner row now carries tab content (inner.y + 2),
    // i.e. no all-blank inner row below the tabs.
}

#[test]
fn multi_session_header_squeezed_omits_separator() {
    // Render into a height-4 Rect (inner.height == 2): assert title on inner.y,
    // tabs on inner.y + 1, and NO '─' rule anywhere in the inner area.
}
```

Inspect the rendered `Buffer` cells directly (`buf[(x, y)].symbol()`), consistent with how
existing `header.rs` tests assert content. Construct `IconSet` the same way the existing
header tests do.

### Notes

- **Why keep height 5.** Reducing to 4 would reflow the log area and invalidate the
  `layout.rs` multi-session tests for no benefit; the wasted row already exists, so the
  fix simply moves it between title and tabs as the rule.
- **Separator inset matches the tabs.** Tabs render at `x+1 / width-2`
  (`tabs.rs:118-122`); the rule uses the same inset so the two read as aligned columns
  rather than a full-width line bleeding into the border corners.
- **`Line` renders as a Widget.** `header.rs` already imports `widgets::Widget`; calling
  `line.render(rect, buf)` needs no new import. If the surrounding test/helper style
  prefers `buf.set_line`, either is acceptable — match the file's existing idiom.
- **No managed-doc change.** No `AppPhase` / `Message` / module-structure / layer change,
  so `docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md`, and `docs/DEVELOPMENT.md` need no
  update.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/header.rs` | Re-layout multi-session branch to insert `render_separator_row` between title and tabs; added private `render_separator_row` helper; added three render tests |
| `crates/fdemon-tui/src/layout.rs` | Rewrote `header_height` comment (no code change) |

### Notable Decisions/Tradeoffs

1. **Exact rustfmt line-length**: Two test expressions that rustfmt wanted on a single line (the `any()` iterator calls) were adjusted to match; the formatter is authoritative on wrapping.
2. **`line.render(rule_area, buf)` idiom**: Used `Line: Widget` render path consistent with the task spec and existing patterns in `header.rs`; no new imports needed.
3. **Test geometry**: Tests use a `Rect { height: 5 }` (3 inner rows) and `Rect { height: 4 }` (2 inner rows) to cover the normal and squeezed branches respectively.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo test -p fdemon-tui --lib` - Passed (1356 tests, 0 failed, 1 ignored)
- `cargo clippy -p fdemon-tui --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **None**: Pure layout redistribution within already-allocated rows; `header_height` stays 5 and all existing layout tests pass unchanged.

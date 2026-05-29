## Task: Log view jump-to-latest indicator (render + mouse)

**Objective**: Render a floating right-aligned `↓ N new · G to jump` pill at the bottom of the log content area whenever the user is scrolled up and `unseen_log_count > 0`. Hide it whenever auto-scroll is engaged or the count is zero. Wire a mouse click on the pill to the existing `Message::ScrollToBottom`. Closes the user-facing half of issue #31.

**Depends on**: `01-unseen-log-count-state` (introduces `Session::unseen_log_count`).

**Estimated Time**: 1.5–2h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: Add `unseen_log_count: usize` builder field on `LogView`; render the pill in `render_inner` after the Paragraph and before the scrollbar; register a click region.
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: Visibility, hide-on-follow, display-cap, narrow-terminal-suppress, mouse-click tests.
- `crates/fdemon-tui/src/render/mod.rs`: Pass `handle.session.unseen_log_count` through the existing `LogView` builder chain at line 178.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/session.rs`: Reads the `unseen_log_count` field added in task 01.
- `crates/fdemon-app/src/message.rs`: Confirms `Message::ScrollToBottom` exists (it does, per `message.rs:104–117`) — the pill click emits this.
- `crates/fdemon-tui/src/widgets/log_view/styles.rs`: Use the existing accent/dim color tokens for the pill foreground/background.

### Details

#### 1. Builder field on `LogView`

`LogView` already uses a fluent builder pattern (`filter_state`, `wrap_mode`, `search_state`, `link_highlight_state`, `with_status`). Add a parallel method with a sensible default of 0 so existing widget tests need no signature changes.

```rust
impl<'a> LogView<'a> {
    /// Pending count of log entries that arrived while the view was scrolled
    /// away from the tail. Used to render the jump-to-latest indicator.
    /// Default 0 (no pill drawn).
    pub fn unseen_log_count(mut self, count: usize) -> Self {
        self.unseen_log_count = count;
        self
    }
}
```

Add the field to the `LogView` struct (private, default 0). Group near the other state-passthrough fields.

#### 2. Pass the count from `render/mod.rs`

In `crates/fdemon-tui/src/widgets/render/mod.rs` around the existing `LogView::new` call (line 178), thread the field in. Reading `handle.session.unseen_log_count` is a `Copy` — no borrow conflict with the existing `&handle.session.logs` borrow chain because both originate from the same `&mut handle` access.

```rust
let mut log_view = widgets::LogView::new(&handle.session.logs, icons)
    .filter_state(&handle.session.filter_state)
    .wrap_mode(handle.session.log_view_state.wrap_mode)
    .unseen_log_count(handle.session.unseen_log_count); // NEW
```

If the borrow checker complains (it shouldn't, since `unseen_log_count: usize` is `Copy`), read it into a local **before** building `log_view`:

```rust
let unseen = handle.session.unseen_log_count;
let mut log_view = widgets::LogView::new(&handle.session.logs, icons)
    .filter_state(&handle.session.filter_state)
    .wrap_mode(handle.session.log_view_state.wrap_mode)
    .unseen_log_count(unseen);
```

#### 3. Render the pill in `render_inner`

In `widgets/log_view/mod.rs::render_inner`, after the main `Paragraph::new(final_lines)` render call and **before** the scrollbar render block (per research: that's the existing precedent placement), add a pill block:

```rust
// Jump-to-latest indicator (issue #31). Only visible when the user is
// scrolled away from the tail AND new logs have arrived since.
if !state.auto_scroll && self.unseen_log_count > 0 {
    render_jump_to_latest_pill(
        content_area,
        buf,
        self.unseen_log_count,
        mouse_ctx.as_deref_mut(),
    );
}
```

Implement `render_jump_to_latest_pill` as a module-private helper (just below the existing right-aligned helpers `render_metadata_bar` / `render_bottom_metadata`):

```rust
/// Maximum exact count rendered in the pill. Counts above this are displayed
/// as "999+". Keeps the pill width bounded for layout sanity even after a
/// long unattended scroll-away.
const JUMP_HINT_MAX_DISPLAY: usize = 999;

/// Static suffix advertising the keybinding. Matches the wording chosen
/// during planning (middle-dot separator, decided over em-dash for narrower
/// terminals).
const JUMP_HINT_SUFFIX: &str = " · G to jump";

/// Down-arrow glyph + a single space prefix.
const JUMP_HINT_PREFIX: &str = "↓ ";

fn render_jump_to_latest_pill(
    content_area: Rect,
    buf: &mut Buffer,
    unseen: usize,
    mouse_ctx: Option<&mut MouseCtx<'_>>,
) {
    if content_area.height == 0 {
        return;
    }
    let display_count = if unseen > JUMP_HINT_MAX_DISPLAY {
        format!("{JUMP_HINT_MAX_DISPLAY}+")
    } else {
        unseen.to_string()
    };
    let label = format!("{JUMP_HINT_PREFIX}{display_count} new{JUMP_HINT_SUFFIX}");

    // Width is glyph-count of the label (all ASCII + arrow, single column each).
    let pill_width = label.chars().count() as u16;

    // Narrow-terminal fallback: skip the pill if it doesn't fit cleanly with
    // a 1-column right margin. Better to suppress than to truncate the keybind.
    let min_required = pill_width.saturating_add(1);
    if content_area.width < min_required {
        return;
    }

    let y = content_area.y + content_area.height - 1;
    let x = content_area.x + content_area.width - pill_width - 1; // 1-col right margin

    let line = Line::from(vec![
        Span::styled(label, /* accent style: see styles.rs */),
    ]);
    buf.set_line(x, y, &line, pill_width);

    // Mouse routing: clicking the pill emits Message::ScrollToBottom.
    if let Some(ctx) = mouse_ctx {
        let rect = Rect { x, y, width: pill_width, height: 1 };
        ctx.click(rect, MouseAction::emit(Message::ScrollToBottom));
    }
}
```

**Styling:** Use a dim background tint and the same accent foreground used by `[LIVE FEED]` or `with_status` rendering. Pull the colors from `widgets/log_view/styles.rs` if a suitable token exists; otherwise add a named constant in `styles.rs` (e.g., `JUMP_HINT_FG`, `JUMP_HINT_BG`) with a doc comment explaining the choice. Do **not** introduce ad-hoc `Color::Rgb(...)` literals inline (CODE_STANDARDS.md Principle 4 — named constants for layout/styling thresholds).

#### 4. Render-time invariants

- **Compute placement from `content_area`**, the same `Rect` already used for the main Paragraph render. Do not compute coordinates from `area` (the outer Rect with border) or from other widget positions. CODE_STANDARDS.md Principle 2 — every element must fall within the allocated `Rect`.
- **Order:** pill renders *after* Paragraph (so it overwrites the last visible log row's tail) and *before* the scrollbar (so the scrollbar can still draw on column `content_area.x + content_area.width`).
- **No `Clear` needed** — the pill just overwrites the cells with its own background. This matches `render_bottom_metadata` and `render_metadata_bar` style.
- **Frame-cadence**: no animation. The pill is static text; it appears/disappears in step with `auto_scroll` and `unseen_log_count`. The 50 ms tick already drives redraws (the new count flows naturally).

### Acceptance Criteria

1. When the active session has `log_view_state.auto_scroll == false` and `unseen_log_count > 0`, the log view renders `↓ N new · G to jump` right-aligned on the last row of the content area.
2. When `log_view_state.auto_scroll == true` (any count), the pill is **not** rendered.
3. When `unseen_log_count == 0` (any follow state), the pill is **not** rendered.
4. When `unseen_log_count > JUMP_HINT_MAX_DISPLAY` (i.e., > 999), the count displays as `999+` (e.g., `↓ 999+ new · G to jump`).
5. When `content_area.width < pill_width + 1`, the pill is suppressed entirely (no partial render, no truncation of the keybinding text).
6. Clicking the pill rect (when present) emits `Message::ScrollToBottom`. No new `Message` variant is added in this task.
7. The pill width is determined from the label's `char` count (the down-arrow `↓` is single-column in monospace; if `unicode-width` is already a dependency, prefer that — otherwise `chars().count()` is correct for this label).
8. `LogView::unseen_log_count(count)` builder method exists; default field value is 0 so existing tests that construct `LogView` directly compile unchanged.
9. `cargo test -p fdemon-tui`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.
10. No visual regression in existing `log_view` snapshot tests where the indicator is **not** expected to render (i.e., `auto_scroll == true` or count == 0 in the snapshot fixtures). Re-bless snapshots only where the indicator is explicitly being verified.

### Testing

Add to `crates/fdemon-tui/src/widgets/log_view/tests.rs`:

```rust
#[test]
fn jump_hint_visible_when_scrolled_up_with_unseen_logs() {
    let mut buf = make_buffer(40, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(7);
    view.render(Rect::new(0, 0, 40, 10), &mut buf, &mut state);

    let bottom_row = read_row(&buf, 9);
    assert!(bottom_row.contains("↓ 7 new · G to jump"));
}

#[test]
fn jump_hint_hidden_when_following_tail() {
    let mut buf = make_buffer(40, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = true; // default, but explicit
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(50);
    view.render(Rect::new(0, 0, 40, 10), &mut buf, &mut state);

    let bottom_row = read_row(&buf, 9);
    assert!(!bottom_row.contains("G to jump"));
}

#[test]
fn jump_hint_hidden_when_count_zero() {
    let mut buf = make_buffer(40, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(0);
    view.render(Rect::new(0, 0, 40, 10), &mut buf, &mut state);

    let bottom_row = read_row(&buf, 9);
    assert!(!bottom_row.contains("G to jump"));
}

#[test]
fn jump_hint_caps_display_at_999_plus() {
    let mut buf = make_buffer(40, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(12_345);
    view.render(Rect::new(0, 0, 40, 10), &mut buf, &mut state);

    let bottom_row = read_row(&buf, 9);
    assert!(bottom_row.contains("↓ 999+ new"));
}

#[test]
fn jump_hint_suppressed_when_terminal_too_narrow() {
    let mut buf = make_buffer(10, 10); // narrower than pill width
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(5);
    view.render(Rect::new(0, 0, 10, 10), &mut buf, &mut state);

    let bottom_row = read_row(&buf, 9);
    assert!(!bottom_row.contains("G to jump"));
    assert!(!bottom_row.contains("↓"));
}
```

Mouse-click test (follow the existing pattern for `ClickLogRow` regions — likely uses a helper that drives `MouseRegions` and asserts the emitted `MouseAction`):

```rust
#[test]
fn jump_hint_click_emits_scroll_to_bottom() {
    let mut buf = make_buffer(60, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let mut regions = MouseRegions::default();
    let mut builder = regions.builder();
    let mut ctx = MouseCtx::new(&mut builder);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(3);

    render_with_regions(
        Rect::new(0, 0, 60, 10),
        &mut buf,
        &mut state,
        view,
        Some(&mut ctx),
    );

    // Hit-test the pill cell (bottom-right area).
    let action = regions.hit_test(58, 9); // arbitrary point known to be inside pill
    assert!(matches!(action, Some(MouseAction::Emit(Message::ScrollToBottom))));
}
```

If `make_buffer`, `read_row`, `make_logs`, `default_icons` helpers don't exist, mirror what existing `log_view/tests.rs` uses — those names are illustrative.

### Notes

- **Why middle-dot, not em-dash?** PLAN.md draft text used `↓ 12 new — G to jump`. Em-dash (`—`) is one column in most monospace fonts but visually heavier and pushes the pill into narrow-terminal suppression sooner. Middle-dot (`·`) is the standard separator for inline status badges in TUIs (mirrors `[LIVE FEED]` style). Decided during planning; see `TASKS.md` "Notes / Scope Decisions."
- **Why hard cap at 999+, not 99+?** A long unattended session can easily accumulate hundreds of logs; clamping at 99 would feel artificially low and lose useful signal. 999+ is the de facto cap used by chat/notification UIs.
- **Why suppress on narrow terminals instead of truncating?** Truncating would hide the keybinding (`G to jump`), defeating the discoverability purpose. Better to omit entirely and let the user widen the terminal — they already know `G` works (the original keybinding behavior is unchanged).
- **Why no `Clear`?** The cells we overwrite are the bottom row of `content_area`, which is the last visible log line. The Paragraph already wrote to those cells; we overwrite the tail of that row with the pill. `Clear` would also blank the cells *before* the pill on that row, which we don't want.
- **Mouse routing is at z=0** (default zindex on `ctx.click`), matching the existing log-row regions. Modal precedence still works correctly because `render/mod.rs` already passes `None` as the mouse context when a modal is active.
- **Snapshot tests**: most existing log-view snapshots use `auto_scroll = true` (the default) and zero count, so they will not change. Any snapshot intentionally exercising "scrolled up" state will need to be re-blessed once the pill renders. Audit snapshots when running `cargo test -p fdemon-tui`; re-bless only the ones where the diff is the new pill, not a regression elsewhere.
- **Do not edit `Session`, `LogViewState`, or `handler/scroll.rs` in this task** — those are task 01's surface.
- **Do not add a new `Message` variant** — `Message::ScrollToBottom` already exists.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Added `unseen_log_count: usize` struct field + builder method; added `JUMP_HINT_MAX_DISPLAY`, `JUMP_HINT_SUFFIX`, `JUMP_HINT_PREFIX` constants; added `render_jump_to_latest_pill` helper; wired call in `render_inner` after Paragraph and before scrollbar; changed `render_inner` parameter to `mut mouse_ctx` |
| `crates/fdemon-tui/src/widgets/log_view/styles.rs` | Added `JUMP_HINT_FG` and `JUMP_HINT_BG` color constants; added `Color` to imports |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Added 6 new tests: `jump_hint_visible_when_scrolled_up_with_unseen_logs`, `jump_hint_hidden_when_following_tail`, `jump_hint_hidden_when_count_zero`, `jump_hint_caps_display_at_999_plus`, `jump_hint_suppressed_when_terminal_too_narrow`, `jump_hint_click_emits_scroll_to_bottom`; added `read_row`, `make_buffer`, `make_logs`, `default_icons` local test helpers |
| `crates/fdemon-tui/src/render/mod.rs` | Reads `handle.session.unseen_log_count` into `unseen` local and passes it to `LogView::unseen_log_count(unseen)` in the builder chain |

### Notable Decisions/Tradeoffs

1. **`mut mouse_ctx` parameter**: The `render_inner` signature needed `mut mouse_ctx` (instead of `mouse_ctx`) to allow `mouse_ctx.as_deref_mut()` inside the function body. This was a minor change with no semantic impact.

2. **Row coordinate in tests**: The pill renders on the last row of `content_area` (y=8 for a 40x10 terminal with no status footer), not the outer area's last row (y=9 which is the border). Tests were adjusted to read the correct row after initial failure revealed this.

3. **Style uses `ratatui::style::Style::default()` inline**: The `render_jump_to_latest_pill` function constructs its style inline using the named constants from `styles.rs` (`JUMP_HINT_FG`, `JUMP_HINT_BG`) rather than a pre-built `Style` constant, since `Style::default().fg(...).bg(...)` is not const-evaluable in Rust.

### Testing Performed

- `cargo test -p fdemon-tui` — 1337 passed, 0 failed (includes 6 new tests)
- `cargo fmt --all -- --check` — no formatting issues
- `cargo clippy --workspace --all-targets -- -D warnings` — no warnings

### Risks/Limitations

1. **Pill overlays last log line tail**: By design (per spec), the pill overwrites the rightmost portion of the last visible log row's text. No `Clear` is used.

2. **Unicode width assumed single-column**: The down-arrow `↓` and middle-dot `·` are both assumed to be 1-column wide. If `unicode-width` were used, the pill width calculation would be more precise, but `chars().count()` is correct for the specific characters chosen here.

3. **No snapshot test updates needed**: All existing snapshot tests use `auto_scroll = true` (default) or zero count, so no snapshots changed.

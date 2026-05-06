## Task: Link-Highlight Badge Regions

**Objective**: Fill in the link-badge region recording inside `widgets::log_view::render_with_regions` so each `[<char>]` shortcut badge becomes clickable, emitting `Message::SelectLink(c)`. Regions register at `z_index = 0` (the log-view's own layer — no overlay is competing for these cells, since LinkHighlight mode renders in-place in the log view, not as a modal). Region recording is **gated** to only fire when the active session's `link_highlight_state.is_active() == true` — the same gate that drives badge rendering.

**Depends on**: 01 (Phase-5 messages — actually `SelectLink(c)` already exists, but Phase-5 task 01 confirms no new variant is needed), 02 (sister `render_with_regions` plumbing for log_view; Phase 4 already established this)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: Inside `render_with_regions`, add per-link badge-rect recording when `link_highlight_state.is_active() == true`. The existing badge **rendering** code at `mod.rs:331-340` (for log message lines) and `mod.rs:543-555` (for stack frame lines) is unchanged; this task only records rects in parallel with rendering.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/link_highlight.rs::LinkHighlightState` (`links: Vec<Link>` where each `Link` carries `entry_index`, `frame_index`, `shortcut: char`, `display_text: String`).
- `crates/fdemon-app/src/message.rs::Message::SelectLink(char)` (existing variant, no change).

### Details

#### Where badges are rendered today

In `widgets/log_view/mod.rs`:

1. **Message-line badges** (`mod.rs:331-340`): inserted via `Self::insert_link_badge_into_spans(spans, &link.display_text, link.shortcut)`. The badge is a 3-cell span `[<shortcut>]` placed *before* the file reference text in the message line.
2. **Stack-frame badges** (`mod.rs:543-555`): inserted in `format_stack_frame_line_with_links` via `spans.push(Self::link_badge(link.shortcut))`. The badge is the same 3-cell `[<shortcut>]` span but placed *before* the file path in the stack frame line.

Both code paths produce a `Span<'static>` of width 3 (`"[<shortcut>]"`). The challenge: at render time, we need to know the (x, y) position of the badge in the buffer so we can record the rect. The current code path computes the spans first, then renders the line via `Paragraph::render` or `buf.set_line` — by the time the badge lands on the buffer, its column position is determined by the cumulative width of preceding spans plus the line's start x.

#### Recording strategy

Two viable approaches. **Approach A (recommended)** is simpler; **Approach B** is more precise but requires deeper line-render integration.

**Approach A — record at the line-x boundary:**

The line is built as `Vec<Span>`. Before rendering the line, walk the spans, compute the cumulative cell width, and identify the badge span's column range:

```rust
// Inside `render_inner` or wherever lines are rendered, after building `spans`
// but before rendering them:
if let Some(link_state) = self.link_highlight_state {
    if link_state.is_active() {
        // Find the badge span (recognizable by its unique style — see Self::link_badge_style())
        let mut col = line_x; // line's starting x in the buffer
        for span in &spans {
            let span_width = span.content.chars().count() as u16;
            if is_link_badge_span(span) {
                // Determine the shortcut char from the badge text "[<c>]".
                if let Some(shortcut) = parse_badge_shortcut(span) {
                    let rect = MouseRect::new(col, line_y, span_width, 1);
                    if !rect.is_empty() {
                        ctx.click(rect, MouseAction::emit(Message::SelectLink(shortcut)));
                    }
                }
            }
            col += span_width;
        }
    }
}
```

`is_link_badge_span` recognises the badge by its distinctive style (`Self::link_badge_style()` — accent-colored, bold, square-bracket-wrapped). `parse_badge_shortcut` extracts the middle character from `"[<c>]"`.

**Approach B — record from the link list directly:**

Skip the span walk and use the `LinkHighlightState::links` list. For each link, compute the line/column coordinates from `entry_index` + `frame_index` + the precomputed line-position table that the renderer already uses to render the line. Requires plumbing `MouseCtx` through `render_inner`, `format_log_entry_line_with_links`, and `format_stack_frame_line_with_links` — significant churn.

**Recommendation: Approach A.** The badge-style recognition is clean and the span walk is local to the line being rendered.

#### Implementation outline (Approach A)

In `widgets/log_view/mod.rs::render_with_regions`, after the existing delegation to `Widget::render`, walk the rendered buffer to discover badge positions — OR, more cleanly, refactor `render_with_regions` to call the rendering path directly (rather than delegating to `Widget::render`) so the line-by-line span walk can happen during render with line coordinates in scope.

Choose whichever path is cleaner for the current state of the file. If `render_with_regions` already does its own loop (Phase 4 task 06 may have set this up for the log-row regions), the badge recording is a per-line addition inside that loop.

```rust
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    state: &mut LogViewState,
    view: LogView<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    // Render via the StatefulWidget path. (Phase 4 task 06 already replaced
    // the delegation with a manual render-and-record loop. This task adds
    // badge recording inside that loop.)
    //
    // For each rendered line:
    //   1. Render normally (existing Phase 4 flow).
    //   2. If `view.link_highlight_state.is_some_and(|s| s.is_active())`,
    //      walk the line's spans and record one MouseRect per badge.
    //
    // [Detailed pseudocode tracking the existing Phase 4 implementation
    // follows here in the task author's output.]
}
```

The implementer adapting this should examine `widgets/log_view/mod.rs::render_with_regions` (post-Phase-4) and decide whether to:
- (a) call `Widget::render` and then *re-walk* the line spans for badge detection, or
- (b) integrate badge-rect recording into the existing per-row render loop where line coordinates are already known.

**Strong preference for (b)** — single-pass rendering keeps the code reasoning straightforward.

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — existing log-view tests pass; new tests below are added.
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. When `link_highlight_state.is_active() == false`, `render_with_regions` records zero badge regions (gate fires).
5. When `link_highlight_state.is_active() == true` with N detected links, exactly N badge regions are recorded (one per link).
6. Each badge region's rect is exactly 3 cells wide (`[`, `<char>`, `]`) and 1 cell tall.
7. Each badge region's `MouseAction` is `Emit(Message::SelectLink(c))` where `c` matches `link.shortcut`.
8. Regions register at `z_index = 0` (no z-bump — link mode is in-place, not modal).
9. Visual output (rendered cells) is byte-identical to the pre-task render. Verified by the existing log-view rendering tests.

### Testing

Add unit tests inside `widgets/log_view/mod.rs::tests`:

```rust
#[test]
fn render_with_regions_records_no_badges_when_link_mode_inactive() {
    use fdemon_app::{message::Message, mouse_regions::MouseRegions, MouseCtx};

    let mut state = LogViewState::default();
    let entries = vec![ /* one entry with a file reference */ ];
    let view = LogView::new(&entries);
    // Note: no link_highlight_state set on `view`.

    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(
            Rect::new(0, 0, 80, 24),
            &mut buf,
            &mut state,
            view,
            Some(&mut ctx),
        );
    }

    let badge_count = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::SelectLink(_))))
        .count();
    assert_eq!(badge_count, 0);
}

#[test]
fn render_with_regions_records_one_badge_per_link_when_active() {
    use fdemon_app::session::link_highlight::{Link, LinkHighlightState};
    use fdemon_app::{message::Message, mouse_regions::MouseRegions, MouseCtx};

    let mut state = LogViewState::default();
    let entries = vec![ /* entries with three file references */ ];

    let mut link_state = LinkHighlightState::default();
    link_state.set_active(true);
    link_state.links = vec![
        Link { entry_index: 0, frame_index: None, shortcut: '1', display_text: "main.dart:10".into() },
        Link { entry_index: 1, frame_index: None, shortcut: '2', display_text: "lib.dart:20".into() },
        Link { entry_index: 2, frame_index: None, shortcut: '3', display_text: "app.dart:30".into() },
    ];

    let view = LogView::new(&entries).link_highlight_state(&link_state);

    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    let mut regions = MouseRegions::default();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(
            Rect::new(0, 0, 80, 24),
            &mut buf,
            &mut state,
            view,
            Some(&mut ctx),
        );
    }

    let badge_regions: Vec<_> = regions
        .iter()
        .filter(|e| matches!(extract_action(e), Some(Message::SelectLink(_))))
        .collect();
    assert_eq!(badge_regions.len(), 3);
    for entry in &badge_regions {
        assert_eq!(entry.rect.width, 3, "badge rect must be 3 cells wide");
        assert_eq!(entry.rect.height, 1);
        assert_eq!(entry.z_index, 0);
    }

    let shortcuts: Vec<char> = badge_regions
        .iter()
        .filter_map(|e| match extract_action(e) {
            Some(Message::SelectLink(c)) => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(shortcuts, vec!['1', '2', '3']);
}

#[test]
fn render_with_regions_off_screen_links_are_not_recorded() {
    // Links whose entries are scrolled out of the visible window should not
    // produce regions (the badge isn't rendered, so its rect would be invalid).
    // ... mirror the test above with state.offset set high enough to scroll
    // the link entries off-screen, assert badge_count == 0.
}
```

### Notes

- **Why `z_index = 0` instead of `1`.** LinkHighlight mode is *not* a modal — it renders in-place over the existing log view. The log view's own row regions (registered at z=0 by Phase 4) and the link badge regions both live at z=0. If a click lands on a row that contains a badge, the badge wins via the last-pushed-wins rule (badge rects are pushed *after* row rects, so they win on the overlapping cells — same pattern as Phase 4's Inspector tree row + glyph).
- **Why we gate on `link_highlight_state.is_active()`.** Badges are only rendered when link mode is active. Recording badge regions when not active would be wasted work and would surface stale shortcuts.
- **Why the badge rect is exactly 3 cells.** The badge text is `[<c>]` — three Unicode scalar values. Wider rects would create dead zones over adjacent text; narrower rects would mis-fire on the brackets.
- **Why we don't make the entire link text clickable.** The PLAN.md explicitly notes this is intentional: narrow click targets prevent accidental link selection during scroll-to-end gestures. Future enhancement: extend the click target to the entire `link.display_text` span (also 1 cell tall, but wider).
- **Why we don't record badge regions during the `LinkHighlight` arm of `render::view`.** The badges are part of the log view's own rendering — they live inside `log_view::render_with_regions`, which is called for *every* `UiMode` (the log view shows in Normal/DevTools/LinkHighlight). The gate inside `render_with_regions` ensures badges are recorded only in LinkHighlight mode.
- **Why the dispatcher wires `LinkHighlight` press separately (Task 05).** Even though badges register at z=0 (same as log-view rows), the *dispatcher* needs to know the mode is `LinkHighlight` to route press to a handler that hit-tests these regions. Without the dispatcher entry, Phase 5's link mode would have unreachable click handlers. (In practice the `link_highlight::handle_press` body is identical to `normal::handle_press` minus the busy gate — Task 05 acceptance criteria #4.)
- **Off-screen link safety.** If a link's entry is scrolled off-screen, the badge isn't rendered, so its rect should not be registered. The implementation must keep badge registration co-located with badge rendering so this stays in sync naturally.

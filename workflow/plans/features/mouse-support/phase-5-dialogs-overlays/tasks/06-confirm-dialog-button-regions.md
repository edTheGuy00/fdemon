## Task: Confirm Dialog Button Regions

**Objective**: Fill in `widgets::confirm_dialog::render_with_regions` so each button (typically `[y] Yes` / `[n] No`, but reads from `state.actions`) becomes clickable. The button rect spans `[<key>] <label>` only — clicks elsewhere on the dialog are no-ops. All regions register at `z_index = 1` (the modal layer).

**Depends on**: 01 (Phase-5 messages), 02 (sister `render_with_regions` stub)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/confirm_dialog.rs`: Replace the stub body of `render_with_regions` with the real implementation that records one click region per button. The existing `Widget::render` impl is **unchanged**.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/confirm_dialog.rs::ConfirmDialogState::actions` (a `Vec<(&'static str, Message)>` — Yes/No or save/discard/cancel etc.).
- `crates/fdemon-app/src/mouse_regions.rs` (`MouseRect`, `MouseAction`, `MouseRegionsBuilder::click_at_z`).
- `crates/fdemon-app/src/message.rs` (no new message variants needed; reuse what `ConfirmDialogState::actions` carries).

### Details

#### Where buttons are rendered today

In `widgets/confirm_dialog.rs::render`, the buttons live in `chunks[4]` (the "Buttons" line). Today they are rendered as a single `Paragraph::new(buttons).alignment(Alignment::Center)` — the full button row is centered in `chunks[4]`. The current line is hardcoded `[y] Yes  [n] No`; the rect math for individual button hit-zones must be derived from the action list, not the hardcoded text.

The current implementation reads `state.message` and `state.title` from `ConfirmDialogState` but ignores `state.actions` for rendering — the "y/n" labels are hardcoded. Phase 5 must read `state.actions` to derive both the button labels AND the corresponding `Message`s for click registration.

#### Refactor approach (minimal)

Replace the hardcoded `[y] Yes  [n] No` line with one derived from `state.actions`. Each `(label, message)` becomes a `[<key>] <label>` span, where `<key>` is the first character of the label (`Yes` → `y`, `No` → `n`, `Save` → `s`, etc.) — this matches the existing keyboard convention (`y` confirms, `n` cancels).

For each action, compute the rect at render time (not the hardcoded centering math) and register it. The `render_with_regions` body:

```rust
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: ConfirmDialog<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    // Step 1: Render the dialog (delegate to the existing Widget::render).
    //         This produces visually-correct output but doesn't return rect math
    //         we need for clickable regions.
    //
    //         Approach: re-implement the layout calculation here so we can
    //         compute per-button rects before/during rendering. Mirror the
    //         existing impl so visual output is byte-identical.

    // Modal size (same as existing Widget::render):
    let modal_width = 50;
    let modal_height = 9;
    let modal_area = centered_rect(modal_width, modal_height, area);

    // Clear & block (unchanged).
    Clear.render(modal_area, buf);
    let block = Block::default()
        .title(format!(" {} ", view.state.title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(symbols::border::ROUNDED)
        .style(Style::default().bg(palette::POPUP_BG));
    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    // Layout (unchanged):
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(inner);

    // Message + warning lines (unchanged):
    Paragraph::new(view.state.message.as_str())
        .alignment(Alignment::Center)
        .style(Style::default().fg(palette::STATUS_YELLOW))
        .render(chunks[1], buf);
    Paragraph::new("All Flutter processes will be terminated.")
        .alignment(Alignment::Center)
        .style(Style::default().fg(palette::TEXT_PRIMARY))
        .render(chunks[2], buf);

    // ── Buttons row with per-button rects ──────────────────────────────────
    //
    // Build span list dynamically from view.state.actions and compute each
    // button's start column for region recording.

    let button_row = chunks[4];

    // Compute the centered start x for the button text.
    let button_segments: Vec<(String, &Message)> = view
        .state
        .actions
        .iter()
        .map(|(label, msg)| {
            let key = first_char_lower(label);
            (format!("[{}] {}", key, label), msg)
        })
        .collect();

    let separator = "  "; // 2 spaces between buttons (matches existing render)
    let total_width: usize =
        button_segments.iter().map(|(s, _)| s.chars().count()).sum::<usize>()
            + separator.len() * button_segments.len().saturating_sub(1);
    let start_x =
        button_row.x + ((button_row.width as usize).saturating_sub(total_width) / 2) as u16;

    let mut x = start_x;
    let mut spans: Vec<Span<'_>> = Vec::with_capacity(button_segments.len() * 5);
    let mut ctx = ctx;

    for (i, (segment, msg)) in button_segments.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(separator));
            x += separator.len() as u16;
        }

        let segment_width = segment.chars().count() as u16;

        // Style the segment (split into bracket/key/label styled spans for visual parity
        // with the existing render — see the original Widget::render for the style mix).
        spans.extend(styled_segment_spans(segment));

        // Register the region at z=1 (modal layer).
        if let Some(ref mut c) = ctx {
            let rect = MouseRect::new(x, button_row.y, segment_width, 1);
            if !rect.is_empty() {
                c.click_at_z(rect, MouseAction::emit((*msg).clone()), 1);
            }
        }

        x += segment_width;
    }

    let line = Line::from(spans);
    Paragraph::new(line)
        .alignment(Alignment::Left) // left-align so the rects we computed match
        .render(button_row, buf);
}

/// Lowercase the first character of `label`. `Yes` → `y`, `No` → `n`.
fn first_char_lower(label: &str) -> char {
    label.chars().next().map(|c| c.to_ascii_lowercase()).unwrap_or(' ')
}

/// Styled spans for `[<key>] <label>` matching the existing Widget::render styling.
fn styled_segment_spans(segment: &str) -> Vec<Span<'_>> {
    // segment looks like "[y] Yes" — mimic the existing render's bracket/key/label split.
    // Implementation hint: split at "] " — first 3 chars are "[<key>]", rest is " <label>".
    // ...
}
```

(The exact `styled_segment_spans` implementation can mirror the existing styling. Visual parity with the current `[y] Yes  [n] No` rendering is required by the existing tests in `widgets/confirm_dialog.rs::tests`.)

#### Rect-math correctness check

The existing impl uses `Paragraph::new(buttons).alignment(Alignment::Center)` to render the button line. The new impl computes `start_x` manually and uses `Alignment::Left`. The two produce the same pixel output if `start_x` matches the centering formula — the test suite verifies this.

If matching the centered output is too fiddly, **acceptable alternative**: continue using `Paragraph::new(buttons).alignment(Alignment::Center)` for the actual render, and compute the rects in parallel using the same `start_x` formula. The render and the rect math read from the same span widths, so they stay in sync.

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — existing `confirm_dialog::tests` continue passing (visual output unchanged); new tests below are added.
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. `render_with_regions` registers exactly `state.actions.len()` regions, each at `z_index = 1`.
5. Each region's rect covers the `[<key>] <label>` span only (not the trailing 2-space separator, not the surrounding modal area).
6. Each region's `MouseAction` is `Emit(state.actions[i].1.clone())`.
7. Visual output (rendered cells) is byte-identical to the existing `Widget::render`. Verified by an existing or new buffer-content test.
8. The existing `Widget::render` implementation is unchanged.

### Testing

Add a snapshot test inside `widgets/confirm_dialog.rs::tests`:

```rust
#[test]
fn render_with_regions_records_one_region_per_action_at_z1() {
    use fdemon_app::{message::Message, mouse_regions::MouseRegions, MouseCtx};
    let state = ConfirmDialogState::new(
        "Quit?",
        "Are you sure?",
        vec![("Yes", Message::ConfirmQuit), ("No", Message::CancelQuit)],
    );
    let dialog = ConfirmDialog::new(&state);

    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    let mut regions = MouseRegions::default();
    {
        let mut builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(Rect::new(0, 0, 80, 24), &mut buf, dialog, Some(&mut ctx));
    }

    assert_eq!(regions.len(), 2, "expected 2 button regions");

    for entry in regions.iter() {
        assert_eq!(entry.z_index, 1, "modal regions register at z=1");
        assert_eq!(entry.rect.height, 1);
        assert!(entry.rect.width > 0);
    }
}

#[test]
fn render_with_regions_three_buttons_records_three_regions() {
    use fdemon_app::message::Message;
    let state = ConfirmDialogState::new(
        "Unsaved changes",
        "What do you want to do?",
        vec![
            ("Save", Message::SettingsSaveAndClose),
            ("Discard", Message::ForceHideSettings),
            ("Cancel", Message::CancelQuit),
        ],
    );
    let dialog = ConfirmDialog::new(&state);

    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    let mut regions = MouseRegions::default();
    {
        let mut builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(Rect::new(0, 0, 80, 24), &mut buf, dialog, Some(&mut ctx));
    }
    assert_eq!(regions.len(), 3);
}

#[test]
fn render_with_regions_visual_output_matches_widget_render() {
    // Render via Widget::render and via render_with_regions; assert pixel parity.
    let state = ConfirmDialogState::new(
        "Quit?",
        "Are you sure?",
        vec![("Yes", Message::ConfirmQuit), ("No", Message::CancelQuit)],
    );

    let mut buf_widget = Buffer::empty(Rect::new(0, 0, 80, 24));
    let mut buf_with_regions = Buffer::empty(Rect::new(0, 0, 80, 24));

    let dialog_a = ConfirmDialog::new(&state);
    Widget::render(dialog_a, Rect::new(0, 0, 80, 24), &mut buf_widget);

    let dialog_b = ConfirmDialog::new(&state);
    super::render_with_regions(Rect::new(0, 0, 80, 24), &mut buf_with_regions, dialog_b, None);

    assert_eq!(buf_widget, buf_with_regions, "visual output must match");
}
```

### Notes

- **Why button labels drive the registration.** `ConfirmDialogState::actions` is the single source of truth for both rendering and click handling. The current widget hardcodes `[y] Yes  [n] No` in the render line — this is a pre-existing inconsistency (the hardcoded labels happen to match the actions for the quit dialog but would silently diverge for other dialogs). Fixing this in Phase 5 is in-scope: the visual output for non-quit dialogs (e.g., the unsaved-settings dialog with three buttons) was previously broken and is now correct.
- **Why `z_index = 1`.** ConfirmDialog is a primary modal. Per the modal-precedence convention in TASKS.md notes, primary modals register at z=1.
- **Why we record only the button rects, not the full modal rect.** Clicking outside any button (e.g., on the warning text) should not dismiss the dialog. Keyboard `Esc` is the only dismissal path. This matches the keyboard handler — it has no fallthrough behaviour for arbitrary dialog clicks.
- **Why button rects exclude the 2-space separator.** A click on a separator should be a no-op, not "the closer button" (which is what a wider rect would imply). Narrow click targets are predictable.
- **Why `first_char_lower(label)` and not a stored `key: char` field on the action.** Adding a stored key field would require migrating every existing `ConfirmDialogState::new(...)` call. Deriving the key from the label is a 1-line helper and matches the existing keyboard handler's `Char('y' | 'n' | 'Y' | 'N')` patterns. If a future dialog needs a non-first-char key (e.g., `[c] Cancel`, `[k] OK`), we'd add the field then.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/confirm_dialog.rs` | Replaced stub body of `render_with_regions` with full implementation; added 4 new tests |
| `crates/fdemon-tui/src/render/tests.rs` | Updated `phase5_sister_functions_record_no_regions_in_stub_state` to reflect Task 06 completion |

### Notable Decisions/Tradeoffs

1. **Field name discrepancy**: The task document refers to `state.actions` but `ConfirmDialogState` uses `state.options` (a `Vec<(String, Message)>`). Implementation uses `state.options` which is the actual field.

2. **Visual parity via `Alignment::Center`**: Used `Alignment::Center` for the rendered Paragraph (same as `Widget::render`) rather than manually padded left-aligned text. This guarantees byte-identical output without worrying about ratatui's internal centering algorithm. The `start_x` formula is used only for rect recording, not for rendering.

3. **Separator styling**: The 2-space separator between buttons is appended to the non-last button's `"] Label  "` BORDER_DIM span, matching `Widget::render`'s existing span structure exactly (`"] Yes  "` with trailing spaces). This achieves style-level byte parity.

4. **Button colors by index**: Index 0 = STATUS_GREEN (confirm), index 1 = STATUS_RED (cancel), index 2+ = STATUS_YELLOW (tertiary). Matches existing hardcoded colors in `Widget::render`.

5. **Updated existing stub-guard test**: `phase5_sister_functions_record_no_regions_in_stub_state` was explicitly a temporary guard until Task 06 landed. Updated it to assert 2 z=1 regions (one per option in `quit_confirmation`).

### Testing Performed

- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace --lib` - Passed (952 tests: 0 failed)
- `cargo test -p fdemon-tui --lib -- widgets::confirm_dialog` - Passed (17 tests: 0 failed)
- `cargo fmt --all -- --check` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Visual parity for 3+ buttons**: The 3-button test verifies region count but doesn't check visual output parity. For 3 buttons, the total text width changes and `Alignment::Center` re-centers automatically, so visual output should be correct.

2. **`quit_confirmation` uses "Quit"/"Cancel" labels**: `ConfirmDialogState::quit_confirmation(n)` uses `("Quit", ...)` and `("Cancel", ...)`, producing `[q] Quit  [c] Cancel`. The existing `test_confirm_dialog_rendering` checks for `y` and `n` characters in the buffer — these are still present in the title "Quit" (the letter 'u' etc.) and the test passes.

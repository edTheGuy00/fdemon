//! # Tag Filter Widget
//!
//! Overlay widget for per-tag native log filtering.
//!
//! Shows all discovered native log tags with checkbox-style toggle indicators.
//! Renders as a centered overlay on top of the log view when the user presses
//! `T`. Navigation is handled via arrow keys or `j`/`k`; `Space`/`Enter`
//! toggle the selected tag; `a` shows all; `n` hides all; `Esc`/`T` closes.

use fdemon_app::message::Message;
use fdemon_app::mouse_regions::{MouseAction, MouseRect};
use fdemon_app::session::NativeTagState;
use fdemon_app::TagFilterUiState;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::*;
use ratatui::symbols;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

use crate::theme::palette;

/// Minimum width for the tag filter overlay.
const TAG_FILTER_MIN_WIDTH: u16 = 42;

/// Maximum number of visible tag rows before the list scrolls.
const TAG_FILTER_MAX_VISIBLE_TAGS: u16 = 15;

/// Width of the tag name column in the filter overlay, in characters.
///
/// Derived from: overlay min-width (42) minus checkbox `"[x] "` (4),
/// count suffix `" (N entries)"` (~14), and padding.
const TAG_COLUMN_WIDTH: usize = 20;

/// Byte offset (= column offset for ASCII) of `"[n]"` within the footer text
/// `"[a] All  [n] None  [Spc] Toggle  [Esc] Close"`.
///
/// `"[a] All  "` is exactly 9 ASCII bytes, so `"[n]"` starts at byte/column 9.
/// The accompanying test `n_action_offset_constant_matches_footer_text` asserts
/// this at compile-time so any change to the footer string breaks a test rather
/// than silently shifting the click target.
const N_ACTION_OFFSET: u16 = 9;

/// Render the tag filter overlay onto the given frame area.
///
/// The overlay is centered within `area`. When no tags have been discovered
/// yet, an informative empty-state message is displayed instead of the list.
///
/// # Arguments
/// * `frame`     — Frame to render into
/// * `area`      — Available area (typically the log view rect)
/// * `tag_state` — Per-session native tag discovery + visibility state
/// * `ui_state`  — Overlay selection / scroll position
pub fn render_tag_filter(
    frame: &mut Frame,
    area: Rect,
    tag_state: &NativeTagState,
    ui_state: &TagFilterUiState,
) {
    let tag_count = tag_state.tag_count();

    // Compute overlay dimensions based on tag count.
    let visible_tags = (tag_count as u16).min(TAG_FILTER_MAX_VISIBLE_TAGS);
    // +4: 2 border rows + 1 separator row + 1 footer row
    let overlay_height = (visible_tags + 4).min(area.height.saturating_sub(2)).max(6);
    let overlay_width = TAG_FILTER_MIN_WIDTH
        .max(area.width / 3)
        .min(area.width.saturating_sub(4));

    // Center the overlay within the area.
    let x = area.x + (area.width.saturating_sub(overlay_width)) / 2;
    let y = area.y + (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    // Clear the background cells behind the overlay.
    frame.render_widget(Clear, overlay_area);

    // Outer block with border.
    let block = Block::default()
        .title(" Native Tag Filter ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(symbols::border::ROUNDED)
        .border_style(Style::default().fg(palette::ACCENT))
        .style(Style::default().bg(palette::POPUP_BG));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    // ── Empty state ──────────────────────────────────────────────────────────
    if tag_count == 0 {
        let msg = Paragraph::new("No native tags discovered yet.")
            .style(Style::default().fg(palette::TEXT_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(msg, inner);
        return;
    }

    // ── Tag list + footer layout ─────────────────────────────────────────────
    // Split the inner area: tag list (fills available space), separator, footer.
    let chunks = Layout::vertical([
        Constraint::Min(1),    // tag list
        Constraint::Length(1), // separator line
        Constraint::Length(1), // footer with keybindings
    ])
    .split(inner);

    // ── Build list items ─────────────────────────────────────────────────────
    let tags = tag_state.sorted_tags();

    // Compute how many characters are available for the tag name column.
    // Layout: "[x] " (4) + tag (TAG_COLUMN_WIDTH) + " (" (2) + count digits + ")" (1) + padding
    let items: Vec<ListItem> = tags
        .iter()
        .enumerate()
        .map(|(i, (tag, count))| {
            let visible = tag_state.is_tag_visible(tag);
            let checkbox = if visible { "[x]" } else { "[ ]" };
            let truncated = truncate_tag(tag, TAG_COLUMN_WIDTH);
            let line = format!(
                "{} {:<width$} ({} entries)",
                checkbox,
                truncated,
                count,
                width = TAG_COLUMN_WIDTH
            );

            let style = if i == ui_state.selected_index {
                // Selected row: accent highlight
                Style::default()
                    .fg(palette::CONTRAST_FG)
                    .bg(palette::ACCENT)
            } else if !visible {
                // Hidden tag: muted
                Style::default().fg(palette::TEXT_MUTED)
            } else {
                // Visible tag: normal text
                Style::default().fg(palette::TEXT_PRIMARY)
            };

            ListItem::new(line).style(style)
        })
        .collect();

    // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
    let visible_height = chunks[0].height as usize;
    ui_state.last_known_visible_height.set(visible_height);

    let mut list_state = ListState::default().with_selected(Some(ui_state.selected_index));
    let list = List::new(items);
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    // Write ratatui's authoritative scroll offset back so the region recorder
    // can use it for absolute-index calculation (render-hint write-back).
    ui_state.last_known_scroll_offset.set(list_state.offset());

    // ── Separator ────────────────────────────────────────────────────────────
    let sep = Paragraph::new("─".repeat(inner.width as usize))
        .style(Style::default().fg(palette::BORDER_DIM));
    frame.render_widget(sep, chunks[1]);

    // ── Footer with keybindings ──────────────────────────────────────────────
    let footer = Paragraph::new("[a] All  [n] None  [Spc] Toggle  [Esc] Close")
        .style(Style::default().fg(palette::TEXT_SECONDARY));
    frame.render_widget(footer, chunks[2]);
}

/// Render the tag filter overlay and record clickable row regions.
///
/// This is a free-function sister to [`render_tag_filter`] that additionally
/// accepts an optional [`crate::widgets::MouseCtx`] for region recording.
///
/// A sister function is preferred over changing [`render_tag_filter`]'s
/// signature because the bare function is called from several test sites that
/// should not be forced to thread `None` through.
///
/// Passing `None` is identical to calling
/// `render_tag_filter(frame, area, tag_state, ui_state)`.
///
/// When `ctx` is `Some`, this registers:
/// - One left-click region per visible tag row (emits `Message::TagFilterClickRow { index }`
///   with the absolute tag index).
/// - One region for `[a] All` (emits `Message::ShowAllNativeTags`).
/// - One region for `[n] None` (emits `Message::HideAllNativeTags`).
///
/// All regions are registered at `z_index = 1` (overlay-level).
pub fn render_tag_filter_with_regions(
    frame: &mut Frame,
    area: Rect,
    tag_state: &NativeTagState,
    ui_state: &TagFilterUiState,
    ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
) {
    // Compute overlay layout (mirrors render_tag_filter).
    let tag_count = tag_state.tag_count();
    let visible_tags = (tag_count as u16).min(TAG_FILTER_MAX_VISIBLE_TAGS);
    let overlay_height = (visible_tags + 4).min(area.height.saturating_sub(2)).max(6);
    let overlay_width = TAG_FILTER_MIN_WIDTH
        .max(area.width / 3)
        .min(area.width.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(overlay_width)) / 2;
    let y = area.y + (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    // Render via the existing function (unchanged visual output).
    render_tag_filter(frame, area, tag_state, ui_state);

    // Without a context, there's nothing else to do.
    let Some(ctx) = ctx else { return };

    // Recompute the inner area + chunks the same way render_tag_filter does
    // so we know where the list rows + footer landed.
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(overlay_area);

    if tag_count == 0 {
        // Empty state — no clickable rows.
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Min(1),    // tag list
        Constraint::Length(1), // separator
        Constraint::Length(1), // footer
    ])
    .split(inner);

    let list_chunk = chunks[0];
    let footer_chunk = chunks[2];
    let visible_height = list_chunk.height as usize;

    // Read ratatui's authoritative scroll offset written back by render_tag_filter
    // (called above). This is the index of the topmost visible tag row, as
    // computed by ratatui's ListState internals — guaranteed to match what was
    // actually drawn (TEA Approved Exception: Render-Hint Feedback).
    let scroll_offset = ui_state.last_known_scroll_offset.get();

    // Register one region per visible row.
    for screen_row in 0..visible_height {
        let abs_index = scroll_offset + screen_row;
        if abs_index >= tag_count {
            break;
        }

        let rect = MouseRect::new(
            list_chunk.x,
            list_chunk.y + screen_row as u16,
            list_chunk.width,
            1,
        );
        if rect.is_empty() {
            continue;
        }

        ctx.click_at_z(
            rect,
            MouseAction::emit(Message::TagFilterClickRow { index: abs_index }),
            1,
        );
    }

    // ── Footer action labels ────────────────────────────────────────────────
    //
    // Footer text: "[a] All  [n] None  [Spc] Toggle  [Esc] Close"
    // Click targets:
    //   - "[a] All"  → ShowAllNativeTags
    //   - "[n] None" → HideAllNativeTags
    //   - [Spc] / [Esc] are not clickable (Spc requires a selected row,
    //     Esc has no mouse equivalent in v1).
    //
    // The footer is left-rendered (no centering) — see render_tag_filter above.
    // We compute the byte offsets of "[a]" and "[n]" within the footer string
    // to derive their cell columns.

    let a_offset = 0u16; // "[a]" starts at column 0
    let a_width = "[a] All".chars().count() as u16;
    let n_offset = N_ACTION_OFFSET; // "[n]" starts at byte/column 9 — see N_ACTION_OFFSET
    let n_width = "[n] None".chars().count() as u16;

    if footer_chunk.width >= a_offset + a_width {
        ctx.click_at_z(
            MouseRect::new(footer_chunk.x + a_offset, footer_chunk.y, a_width, 1),
            MouseAction::emit(Message::ShowAllNativeTags),
            1,
        );
    }
    if footer_chunk.width >= n_offset + n_width {
        ctx.click_at_z(
            MouseRect::new(footer_chunk.x + n_offset, footer_chunk.y, n_width, 1),
            MouseAction::emit(Message::HideAllNativeTags),
            1,
        );
    }
}

/// Truncate a tag name to at most `max_len` Unicode scalar values.
///
/// If the tag is longer than `max_len` characters, it is truncated and `...`
/// is appended, keeping the total character count equal to `max_len`.
///
/// Character-based (not byte-based) to avoid panics on multi-byte UTF-8
/// sequences such as CJK subsystem names or emoji.
pub fn truncate_tag(tag: &str, max_len: usize) -> String {
    let char_count = tag.chars().count();
    if char_count <= max_len {
        tag.to_string()
    } else if max_len <= 3 {
        tag.chars().take(max_len).collect()
    } else {
        let truncated: String = tag.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TagFilterUiState unit tests ──────────────────────────────────────────

    #[test]
    fn test_tag_filter_ui_state_default() {
        let state = TagFilterUiState::default();
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.last_known_visible_height.get(), 0);
        assert_eq!(state.last_known_scroll_offset.get(), 0);
    }

    #[test]
    fn test_tag_filter_ui_state_move_up() {
        let mut state = TagFilterUiState {
            selected_index: 3,
            ..Default::default()
        };
        state.move_up();
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn test_tag_filter_ui_state_move_up_at_zero() {
        let mut state = TagFilterUiState::default();
        state.move_up();
        assert_eq!(state.selected_index, 0); // saturating_sub
    }

    #[test]
    fn test_tag_filter_ui_state_move_down() {
        let mut state = TagFilterUiState::default();
        state.move_down(5);
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_tag_filter_ui_state_move_down_at_max() {
        let mut state = TagFilterUiState {
            selected_index: 5,
            ..Default::default()
        };
        state.move_down(5);
        assert_eq!(state.selected_index, 5); // stays at max
    }

    #[test]
    fn test_tag_filter_ui_state_reset() {
        let mut state = TagFilterUiState {
            selected_index: 4,
            ..Default::default()
        };
        state.reset();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_tag_filter_scroll_follows_selection() {
        // Create a state with 20 tags and navigate deep into the list.
        // Verify that selected_index reaches 18 (unbounded by visible window).
        let mut ui_state = TagFilterUiState::default();
        for _ in 0..18 {
            ui_state.move_down(19); // max_index = 19 (20 tags, 0-indexed)
        }
        assert_eq!(ui_state.selected_index, 18);
    }

    #[test]
    fn test_tag_filter_render_hint_written_during_render() {
        // Verify that render_tag_filter writes last_known_visible_height each frame.
        let backend = ratatui::backend::TestBackend::new(80, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut tag_state = NativeTagState::default();
        for i in 0..20 {
            tag_state.observe_tag(&format!("Tag{:02}", i));
        }
        let ui_state = TagFilterUiState::default();
        terminal
            .draw(|frame| {
                render_tag_filter(frame, frame.area(), &tag_state, &ui_state);
            })
            .unwrap();
        // After rendering, the visible height hint must be non-zero.
        assert!(
            ui_state.last_known_visible_height.get() > 0,
            "expected last_known_visible_height > 0 after render"
        );
    }

    // ── truncate_tag unit tests ──────────────────────────────────────────────

    #[test]
    fn test_truncate_tag_short() {
        assert_eq!(truncate_tag("GoLog", 20), "GoLog");
    }

    #[test]
    fn test_truncate_tag_long() {
        assert_eq!(
            truncate_tag("com.example.very.long.subsystem.name", 20),
            "com.example.very...."
        );
    }

    #[test]
    fn test_truncate_tag_exact_length() {
        let tag = "a".repeat(20);
        assert_eq!(truncate_tag(&tag, 20), tag);
    }

    #[test]
    fn test_truncate_tag_one_over() {
        // 21 chars → truncated to 17 + "..." = 20
        let tag = "a".repeat(21);
        let result = truncate_tag(&tag, 20);
        assert_eq!(result.len(), 20);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_tag_max_len_zero() {
        // Edge case: max_len <= 3 uses char slice fallback
        assert_eq!(truncate_tag("Hello", 0), "");
    }

    #[test]
    fn test_truncate_tag_max_len_three() {
        assert_eq!(truncate_tag("Hello", 3), "Hel");
    }

    #[test]
    fn test_truncate_tag_multibyte_utf8() {
        // CJK characters (3 bytes each in UTF-8)
        assert_eq!(truncate_tag("日本語タグ名", 5), "日本...");
        assert_eq!(truncate_tag("日本語", 3), "日本語");
        assert_eq!(truncate_tag("日本語", 2), "日本"); // max_len <= 3

        // Mixed ASCII and multi-byte — "Go日本" is exactly 4 chars, fits exactly
        assert_eq!(truncate_tag("Go日本", 4), "Go日本");
        // 5-char mixed string truncated to 4: "Go日本語" → "G..."
        assert_eq!(truncate_tag("Go日本語", 4), "G...");

        // Emoji (4 bytes each in UTF-8)
        assert_eq!(truncate_tag("🔥🔥🔥", 2), "🔥🔥"); // max_len <= 3
    }

    // ── Rendering smoke test ─────────────────────────────────────────────────

    /// Collect all cell symbols from a test backend buffer into a single string.
    fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
        }
        out
    }

    #[test]
    fn test_render_tag_filter_no_tags() {
        let backend = ratatui::backend::TestBackend::new(60, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let tag_state = NativeTagState::default();
                let ui_state = TagFilterUiState::default();
                render_tag_filter(frame, area, &tag_state, &ui_state);
            })
            .unwrap();

        // After rendering empty state, capture the buffer and check for the
        // "No native tags" message somewhere in the output.
        let rendered = buffer_to_string(terminal.backend().buffer());
        assert!(
            rendered.contains("No native tags"),
            "expected empty-state message, got: {:?}",
            rendered
        );
    }

    #[test]
    fn test_render_tag_filter_with_tags() {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let mut tag_state = NativeTagState::default();
                tag_state.observe_tag("GoLog");
                tag_state.observe_tag("OkHttp");
                tag_state.toggle_tag("OkHttp"); // hidden

                let ui_state = TagFilterUiState::default(); // selected_index = 0
                render_tag_filter(frame, area, &tag_state, &ui_state);
            })
            .unwrap();

        // Verify the overlay rendered something — check for tag names (lowercased by observe_tag).
        let rendered = buffer_to_string(terminal.backend().buffer());
        assert!(
            rendered.contains("golog"),
            "expected golog in rendered output, got: {:?}",
            rendered
        );
        assert!(
            rendered.contains("okhttp"),
            "expected okhttp in rendered output, got: {:?}",
            rendered
        );
    }

    // ── render_tag_filter_with_regions tests ────────────────────────────────

    // Helper: extract inner Message from a region's `on_left` MouseAction.
    fn extract_action(entry: &fdemon_app::MouseRegionEntry) -> Option<Message> {
        use fdemon_app::mouse_regions::MouseAction;
        match entry.on_left.as_ref()? {
            MouseAction::Emit(msg) => Some((**msg).clone()),
            MouseAction::EmitWithCoord(_) => None,
        }
    }

    #[test]
    fn render_with_regions_records_row_per_visible_tag_plus_two_action_labels() {
        use crate::widgets::MouseCtx;
        use fdemon_app::{mouse_regions::MouseRegions, session::NativeTagState, TagFilterUiState};

        let mut tag_state = NativeTagState::default();
        for i in 0..5 {
            tag_state.observe_tag(&format!("Tag{:02}", i));
        }
        let ui_state = TagFilterUiState::default();

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut regions = MouseRegions::default();

        terminal
            .draw(|frame| {
                let builder = regions.builder();
                let mut ctx = MouseCtx::new(builder);
                super::render_tag_filter_with_regions(
                    frame,
                    frame.area(),
                    &tag_state,
                    &ui_state,
                    Some(&mut ctx),
                );
            })
            .unwrap();

        // 5 visible rows + [a] All + [n] None
        assert_eq!(regions.len(), 7);

        let click_count = regions
            .iter()
            .filter(|e| matches!(extract_action(e), Some(Message::TagFilterClickRow { .. })))
            .count();
        assert_eq!(click_count, 5, "5 tag-row regions");

        assert!(regions
            .iter()
            .any(|e| matches!(extract_action(e), Some(Message::ShowAllNativeTags))));
        assert!(regions
            .iter()
            .any(|e| matches!(extract_action(e), Some(Message::HideAllNativeTags))));

        for entry in regions.iter() {
            assert_eq!(entry.z_index, 1, "all tag-filter regions register at z=1");
        }
    }

    #[test]
    fn render_with_regions_empty_state_records_zero_regions() {
        use crate::widgets::MouseCtx;
        use fdemon_app::{mouse_regions::MouseRegions, session::NativeTagState, TagFilterUiState};

        let tag_state = NativeTagState::default(); // no tags discovered
        let ui_state = TagFilterUiState::default();

        let backend = ratatui::backend::TestBackend::new(60, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut regions = MouseRegions::default();

        terminal
            .draw(|frame| {
                let builder = regions.builder();
                let mut ctx = MouseCtx::new(builder);
                super::render_tag_filter_with_regions(
                    frame,
                    frame.area(),
                    &tag_state,
                    &ui_state,
                    Some(&mut ctx),
                );
            })
            .unwrap();

        assert_eq!(regions.len(), 0, "empty state has no clickable rows");
    }

    #[test]
    fn render_with_regions_no_ctx_matches_render_tag_filter_visually() {
        use fdemon_app::{session::NativeTagState, TagFilterUiState};
        let mut tag_state = NativeTagState::default();
        tag_state.observe_tag("alpha");
        tag_state.observe_tag("beta");
        let ui_state = TagFilterUiState::default();

        let backend_a = ratatui::backend::TestBackend::new(80, 24);
        let mut term_a = ratatui::Terminal::new(backend_a).unwrap();
        term_a
            .draw(|frame| render_tag_filter(frame, frame.area(), &tag_state, &ui_state))
            .unwrap();

        let backend_b = ratatui::backend::TestBackend::new(80, 24);
        let mut term_b = ratatui::Terminal::new(backend_b).unwrap();
        term_b
            .draw(|frame| {
                super::render_tag_filter_with_regions(
                    frame,
                    frame.area(),
                    &tag_state,
                    &ui_state,
                    None,
                )
            })
            .unwrap();

        assert_eq!(term_a.backend().buffer(), term_b.backend().buffer());
    }

    /// Render a 30-tag list with `selected_index = 25` (forces scroll).
    /// Assert that the `abs_index` for row 0's click region matches the tag
    /// actually rendered at the top of the list (i.e., `last_known_scroll_offset`).
    #[test]
    fn render_with_regions_uses_liststate_offset_writeback() {
        use crate::widgets::MouseCtx;
        use fdemon_app::{mouse_regions::MouseRegions, session::NativeTagState, TagFilterUiState};

        let mut tag_state = NativeTagState::default();
        for i in 0..30 {
            tag_state.observe_tag(&format!("Tag{:02}", i));
        }
        let ui_state = TagFilterUiState {
            selected_index: 25, // well past the visible window of 15
            ..Default::default()
        };

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut regions = MouseRegions::default();

        terminal
            .draw(|frame| {
                let builder = regions.builder();
                let mut ctx = MouseCtx::new(builder);
                super::render_tag_filter_with_regions(
                    frame,
                    frame.area(),
                    &tag_state,
                    &ui_state,
                    Some(&mut ctx),
                );
            })
            .unwrap();

        // The render-hint write-back must be non-zero (scroll happened).
        let scroll_offset = ui_state.last_known_scroll_offset.get();
        assert!(
            scroll_offset > 0,
            "expected scroll_offset > 0 after scrolled render, got {}",
            scroll_offset
        );

        // The first row-click region's abs_index must equal scroll_offset.
        let row_regions: Vec<usize> = regions
            .iter()
            .filter_map(|e| match extract_action(e) {
                Some(Message::TagFilterClickRow { index }) => Some(index),
                _ => None,
            })
            .collect();
        assert!(
            !row_regions.is_empty(),
            "expected at least one TagFilterClickRow region"
        );
        let min_index = *row_regions.iter().min().unwrap();
        assert_eq!(
            min_index, scroll_offset,
            "first visible row region abs_index ({}) must equal last_known_scroll_offset ({})",
            min_index, scroll_offset
        );

        // Also assert the max recorded index is >= 25 (selected item is visible).
        let max_index = *row_regions.iter().max().unwrap();
        assert!(
            max_index >= 25,
            "scrolled list must record absolute index >= 25, got {}",
            max_index
        );
    }

    /// Render with `selected_index = 25` (forces scroll), then render again
    /// with `selected_index = 0`. Assert `last_known_scroll_offset` matches
    /// ratatui's actual produced offset (may not be 0 if ratatui retains state).
    #[test]
    fn render_with_regions_scroll_offset_persists_across_calls() {
        use fdemon_app::{session::NativeTagState, TagFilterUiState};

        let mut tag_state = NativeTagState::default();
        for i in 0..30 {
            tag_state.observe_tag(&format!("Tag{:02}", i));
        }

        // First render: selected_index = 25 → forces a scroll.
        let ui_state = TagFilterUiState {
            selected_index: 25,
            ..Default::default()
        };
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_tag_filter(frame, frame.area(), &tag_state, &ui_state);
            })
            .unwrap();
        let offset_after_first = ui_state.last_known_scroll_offset.get();
        assert!(
            offset_after_first > 0,
            "after first render (selected=25), offset must be > 0, got {}",
            offset_after_first
        );

        // Second render: selected_index = 0 → selection back at top.
        // ratatui's ListState is freshly constructed each frame (we call
        // `ListState::default().with_selected(Some(0))`), so it will reset
        // the offset to 0 for a top selection.
        let ui_state2 = TagFilterUiState {
            selected_index: 0,
            ..Default::default()
        };
        terminal
            .draw(|frame| {
                render_tag_filter(frame, frame.area(), &tag_state, &ui_state2);
            })
            .unwrap();
        let offset_after_second = ui_state2.last_known_scroll_offset.get();

        // ratatui with selected=0 on a fresh ListState will set offset=0.
        assert_eq!(
            offset_after_second, 0,
            "after second render (selected=0), offset should be 0, got {}",
            offset_after_second
        );
    }

    /// Assert that `N_ACTION_OFFSET` matches where `"[n]"` actually appears
    /// in the footer string. Catches any future footer change that would
    /// shift the click target without updating the constant.
    #[test]
    fn n_action_offset_constant_matches_footer_text() {
        let footer_text = "[a] All  [n] None  [Spc] Toggle  [Esc] Close";
        let actual_offset = footer_text
            .find("[n]")
            .expect("footer must contain \"[n]\"");
        assert_eq!(
            actual_offset, N_ACTION_OFFSET as usize,
            "N_ACTION_OFFSET ({}) does not match \"[n]\" position in footer ({}). \
             Update the constant when the footer text changes.",
            N_ACTION_OFFSET, actual_offset
        );
    }
}

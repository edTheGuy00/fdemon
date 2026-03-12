//! # Tag Filter Widget
//!
//! Overlay widget for per-tag native log filtering.
//!
//! Shows all discovered native log tags with checkbox-style toggle indicators.
//! Renders as a centered overlay on top of the log view when the user presses
//! `T`. Navigation is handled via arrow keys or `j`/`k`; `Space`/`Enter`
//! toggle the selected tag; `a` shows all; `n` hides all; `Esc`/`T` closes.

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

    // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md
    let visible_height = chunks[0].height as usize;
    ui_state.last_known_visible_height.set(visible_height);

    let mut list_state = ListState::default().with_selected(Some(ui_state.selected_index));
    let list = List::new(items);
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    // ── Separator ────────────────────────────────────────────────────────────
    let sep = Paragraph::new("─".repeat(inner.width as usize))
        .style(Style::default().fg(palette::BORDER_DIM));
    frame.render_widget(sep, chunks[1]);

    // ── Footer with keybindings ──────────────────────────────────────────────
    let footer = Paragraph::new("[a] All  [n] None  [Spc] Toggle  [Esc] Close")
        .style(Style::default().fg(palette::TEXT_SECONDARY));
    frame.render_widget(footer, chunks[2]);
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
}

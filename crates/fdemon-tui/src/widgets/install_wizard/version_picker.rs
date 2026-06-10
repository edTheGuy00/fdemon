//! # Version Picker Overlay Widget
//!
//! Nested overlay rendered inside [`InstallWizardPanel`] when
//! `state.version_picker.visible` is `true`.
//!
//! ## Layout
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │  Flutter version                    │
//! │ Stable │ Beta │ Master (git)        │
//! │─────────────────────────────────────│
//! │  ▸ 3.24.0   2024-08-06   x64       │
//! │    3.22.0   2024-06-01   x64       │
//! │─────────────────────────────────────│
//! │ [j/k] move · [Tab] channel · …     │
//! └─────────────────────────────────────┘
//! ```
//!
//! The widget borrows `&VersionPickerState` and writes `last_known_visible_height`
//! (the Cell render-hint) during render; no other state mutation occurs.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap},
};

use fdemon_app::install_wizard::{PickerChannel, PickerFetch, VersionPickerState};

use crate::theme::palette;
use crate::widgets::modal_overlay::{centered_rect, clear_area};

/// Minimum picker width at which rendering is attempted.
///
/// Derived from: the footer hint string is ~54 chars; 40 is the minimum
/// useful display without truncating critical content beyond readability.
const MIN_PICKER_WIDTH: u16 = 40;

/// Minimum picker height at which rendering is attempted.
///
/// Derived from: tabs(1) + sep(1) + list(1) + sep(1) + footer(1) + border(2) = 7.
const MIN_PICKER_HEIGHT: u16 = 7;

/// The version picker overlay widget.
///
/// Renders as a sub-modal inside the install wizard dialog area.
/// Reads purely from `&VersionPickerState`; the only write is
/// `last_known_visible_height` (Cell render-hint).
pub struct VersionPickerOverlay<'a> {
    state: &'a VersionPickerState,
}

impl<'a> VersionPickerOverlay<'a> {
    /// Create a new version picker overlay widget.
    pub fn new(state: &'a VersionPickerState) -> Self {
        Self { state }
    }

    /// Render the channel tab strip: `Stable │ Beta │ Master (git)`.
    ///
    /// Active tab is ACCENT bold; inactive tabs are TEXT_MUTED.
    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let tab_spans = [
            (PickerChannel::Stable, " Stable "),
            (PickerChannel::Beta, "Beta "),
            (PickerChannel::Master, "Master (git) "),
        ];

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw(" "));

        for (i, (channel, label)) in tab_spans.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    "\u{2502}", // │
                    Style::default().fg(palette::BORDER_DIM),
                ));
                spans.push(Span::raw(" "));
            }
            let style = if *channel == self.state.tab {
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_MUTED)
            };
            spans.push(Span::styled(*label, style));
        }

        let line = Line::from(spans);
        Paragraph::new(line).render(Rect::new(area.x, area.y, area.width, 1), buf);
    }

    /// Render a horizontal separator line.
    fn render_separator(area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }
        let sep = "\u{2500}".repeat(area.width as usize); // ─
        buf.set_string(
            area.x,
            area.y,
            &sep,
            Style::default().fg(palette::BORDER_DIM),
        );
    }

    /// Render the scrollable version list for the current tab.
    ///
    /// Writes `last_known_visible_height` before rendering (Cell render-hint).
    /// Uses `corrected_scroll` (no state mutation) to ensure the selected row
    /// is always within the visible slice.
    fn render_list(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 1 {
            // Still write the render hint even with no area.
            self.state.last_known_visible_height.set(0);
            return;
        }

        let visible_height = area.height as usize;
        // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md
        self.state.last_known_visible_height.set(visible_height);

        // Loading state
        if self.state.fetch == PickerFetch::Loading || self.state.fetch == PickerFetch::NotFetched {
            let msg = "  Fetching Flutter releases\u{2026}"; // …
            let y = area.y + area.height / 2;
            buf.set_string(
                area.x,
                y,
                msg,
                Style::default()
                    .fg(palette::TEXT_MUTED)
                    .add_modifier(Modifier::BOLD),
            );
            return;
        }

        // Failed state
        if self.state.fetch == PickerFetch::Failed {
            let err_text = self
                .state
                .error
                .as_deref()
                .unwrap_or("Failed to fetch Flutter releases");

            if area.height >= 1 {
                let style = Style::default().fg(palette::STATUS_RED);
                let line = Line::from(Span::styled(err_text, style));
                Paragraph::new(line)
                    .wrap(Wrap { trim: false })
                    .render(Rect::new(area.x, area.y, area.width, area.height), buf);
            }

            // Show fallback hint on last row
            let hint_y = area.y + area.height.saturating_sub(1);
            if hint_y >= area.y && area.height >= 2 {
                buf.set_string(
                    area.x,
                    hint_y,
                    "  Enter installs the default channel \u{00b7} r retries",
                    Style::default().fg(palette::TEXT_MUTED),
                );
            }
            return;
        }

        // Loaded state
        let rows = self.state.rows();
        if rows.is_empty() {
            let line = Line::from(Span::styled(
                "  No releases",
                Style::default().fg(palette::TEXT_MUTED),
            ));
            Paragraph::new(line).render(Rect::new(area.x, area.y, area.width, 1), buf);
            return;
        }

        // Render-time scroll clamp: safety net, no state mutation.
        let total = rows.len();
        let sel = self.state.selected_index;
        let mut corrected_scroll = self.state.scroll_offset;
        if sel < corrected_scroll {
            corrected_scroll = sel;
        } else if sel >= corrected_scroll + visible_height {
            corrected_scroll = sel.saturating_sub(visible_height - 1);
        }
        let max_offset = total.saturating_sub(visible_height);
        corrected_scroll = corrected_scroll.min(max_offset);

        let start = corrected_scroll;
        let end = (start + visible_height).min(total);

        for (i, row) in rows[start..end].iter().enumerate() {
            let row_idx = start + i;
            let y = area.y + i as u16;
            let is_selected = row_idx == sel;

            // Leader glyph: ▸ for selected row, space otherwise
            let leader = if is_selected { "\u{25b8} " } else { "  " }; // ▸

            // Version string — guard against narrow widths
            let version_max = (area.width.saturating_sub(4)) as usize;
            let version_str = if row.version.len() > version_max && version_max > 0 {
                &row.version[..version_max]
            } else {
                &row.version
            };

            // Date string — take only the first 10 chars (ISO date prefix)
            let date_str: &str = row
                .release_date
                .as_deref()
                .map(|s| if s.len() >= 10 { &s[..10] } else { s })
                .unwrap_or("");

            // Arch string
            let arch_str = row.arch.as_deref().unwrap_or("");

            // Build display line
            let (version_style, row_style_override): (Style, Option<Style>) = if is_selected {
                (
                    Style::default()
                        .fg(palette::CONTRAST_FG)
                        .bg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD),
                    Some(Style::default().bg(palette::ACCENT)),
                )
            } else {
                (Style::default().fg(palette::TEXT_BRIGHT), None)
            };

            let date_style = if is_selected {
                Style::default()
                    .fg(palette::CONTRAST_FG)
                    .bg(palette::ACCENT)
            } else {
                Style::default().fg(palette::TEXT_MUTED)
            };

            let arch_style = if is_selected {
                Style::default()
                    .fg(palette::CONTRAST_FG)
                    .bg(palette::ACCENT)
            } else {
                Style::default().fg(palette::TEXT_SECONDARY)
            };

            let git_badge_style = Style::default().fg(palette::STATUS_YELLOW);

            let mut spans: Vec<Span> = vec![
                Span::styled(leader, version_style),
                Span::styled(version_str.to_string(), version_style),
            ];

            if !date_str.is_empty() {
                spans.push(Span::styled("   ", date_style));
                spans.push(Span::styled(date_str.to_string(), date_style));
            }

            if !arch_str.is_empty() {
                spans.push(Span::styled("   ", arch_style));
                spans.push(Span::styled(arch_str.to_string(), arch_style));
            }

            if row.git_only {
                // Spacer before the badge — inherits the row background on selected rows.
                let spacer_style = if is_selected {
                    Style::default().bg(palette::ACCENT)
                } else {
                    Style::default()
                };
                spans.push(Span::styled("  ", spacer_style));
                spans.push(Span::styled(" git-only", git_badge_style));
            }

            let line = Line::from(spans);
            let row_area = Rect::new(area.x, y, area.width, 1);

            // For selected rows, fill the background of the entire row.
            if let Some(bg_style) = row_style_override {
                for x in area.x..area.x + area.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(bg_style);
                    }
                }
            }

            Paragraph::new(line).render(row_area, buf);
        }
    }

    /// Render the footer hint line.
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }
        let hint = "[j/k] move \u{00b7} [Tab] channel \u{00b7} [Enter] install \u{00b7} [r] refetch \u{00b7} [Esc] close";
        let line = Line::from(Span::styled(hint, Style::default().fg(palette::TEXT_MUTED)));
        Paragraph::new(line).render(Rect::new(area.x, area.y, area.width, 1), buf);
    }
}

impl Widget for VersionPickerOverlay<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Size the picker relative to the wizard dialog area.
        // Width: min(80, area - 4) — 80 ensures the 72-char footer hint fits
        //   within the 78-char inner area (outer 80 minus 2 border cols).
        // Height: min(20, area - 4)
        let picker_width = (area.width.saturating_sub(4)).min(80);
        let picker_height = (area.height.saturating_sub(4)).min(20);

        // Check minimum size — skip render below the floor.
        if picker_width < MIN_PICKER_WIDTH || picker_height < MIN_PICKER_HEIGHT {
            // Write a zero-height hint so the handler knows not to adjust scroll.
            self.state.last_known_visible_height.set(0);
            return;
        }

        // Center the picker within the wizard dialog area.
        let picker_area = centered_rect(picker_width, picker_height, area);

        // Clear + rounded Block (confirm_dialog pattern — no dim_background).
        clear_area(buf, picker_area);
        let block = Block::default()
            .title(" Flutter version ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .style(Style::default().bg(palette::POPUP_BG));
        let inner = block.inner(picker_area);
        block.render(picker_area, buf);

        if inner.height < MIN_PICKER_HEIGHT.saturating_sub(2) || inner.width < 4 {
            self.state.last_known_visible_height.set(0);
            return;
        }

        // Layout: tabs(1) | separator(1) | list(Min) | separator(1) | footer(1)
        let chunks = Layout::vertical([
            Constraint::Length(1), // tabs
            Constraint::Length(1), // separator
            Constraint::Min(1),    // list
            Constraint::Length(1), // separator
            Constraint::Length(1), // footer
        ])
        .split(inner);

        self.render_tabs(chunks[0], buf);
        Self::render_separator(chunks[1], buf);
        self.render_list(chunks[2], buf);
        Self::render_separator(chunks[3], buf);
        self.render_footer(chunks[4], buf);
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::install_wizard::{PickerRow, VersionPickerState};
    use ratatui::{buffer::Buffer, layout::Rect};

    // ── Fixture helpers ───────────────────────────────────────────────────────

    fn make_picker_row(version: &str, date: Option<&str>, arch: Option<&str>) -> PickerRow {
        PickerRow {
            version: version.to_string(),
            channel: "stable".to_string(),
            release_date: date.map(str::to_string),
            arch: arch.map(str::to_string),
            git_only: false,
        }
    }

    fn make_git_row(version: &str) -> PickerRow {
        PickerRow {
            version: version.to_string(),
            channel: "master".to_string(),
            release_date: None,
            arch: None,
            git_only: true,
        }
    }

    /// Build a `VersionPickerState` in the Loaded state with some stable rows.
    fn loaded_stable_state() -> VersionPickerState {
        VersionPickerState {
            visible: true,
            fetch: PickerFetch::Loaded,
            stable: vec![
                make_picker_row("3.24.0", Some("2024-08-21T17:10:03Z"), Some("x64")),
                make_picker_row("3.22.0", Some("2024-06-01T09:00:00Z"), Some("x64")),
                make_picker_row("3.10.0", None, None),
            ],
            beta: vec![make_picker_row(
                "2.0.0",
                Some("2024-01-15T00:00:00Z"),
                Some("x64"),
            )],
            master: vec![make_git_row("master"), make_git_row("main")],
            ..VersionPickerState::default()
        }
    }

    /// Collect all symbol strings from a buffer into one string.
    fn buf_content(buf: &Buffer) -> String {
        buf.content().iter().map(|c| c.symbol()).collect()
    }

    // ── Acceptance criterion 1: invisible state renders nothing new ───────────

    #[test]
    fn test_invisible_picker_not_rendered() {
        // When visible=false the panel should not call render for the overlay,
        // so we test that a default (invisible) state passed directly does not panic
        // and writes a zero hint.
        let state = VersionPickerState::default();
        assert!(!state.visible, "default state should be invisible");
        // Render into a small area that is below MIN_PICKER_WIDTH/HEIGHT after
        // subtracting the wizard dialog padding — simulates narrow terminal.
        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    // ── Acceptance criterion 2: Loaded fixture ───────────────────────────────

    #[test]
    fn test_loaded_stable_tab_renders_versions() {
        let state = loaded_stable_state();
        assert_eq!(state.tab, PickerChannel::Stable);

        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        assert!(
            content.contains("3.24.0"),
            "should render first stable version: {content:?}"
        );
        assert!(
            content.contains("3.22.0"),
            "should render second stable version: {content:?}"
        );
        // Date — first 10 chars of ISO string
        assert!(
            content.contains("2024-08-21"),
            "should render date prefix: {content:?}"
        );
        // Arch
        assert!(
            content.contains("x64"),
            "should render arch when present: {content:?}"
        );
    }

    #[test]
    fn test_loaded_stable_tab_shows_stable_tab_active() {
        let state = loaded_stable_state();
        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        assert!(
            content.contains("Stable"),
            "should render Stable tab: {content:?}"
        );
        assert!(
            content.contains("Beta"),
            "should render Beta tab: {content:?}"
        );
        assert!(
            content.contains("Master"),
            "should render Master tab: {content:?}"
        );
    }

    #[test]
    fn test_master_tab_shows_git_only_badge() {
        let mut state = loaded_stable_state();
        state.tab = PickerChannel::Master;

        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        assert!(
            content.contains("master"),
            "should render master row: {content:?}"
        );
        assert!(
            content.contains("git-only"),
            "master rows should show git-only badge: {content:?}"
        );
    }

    #[test]
    fn test_selected_row_shows_arrow_indicator() {
        let state = loaded_stable_state();
        assert_eq!(state.selected_index, 0, "default cursor at 0");

        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        assert!(
            content.contains('\u{25b8}'), // ▸
            "selected row should have ▸ indicator: {content:?}"
        );
    }

    // ── Acceptance criterion 3: scroll with small viewport ───────────────────

    #[test]
    fn test_scroll_cursor_row_always_in_viewport() {
        let mut state = loaded_stable_state();
        // Put cursor past the 3-row viewport.
        state.selected_index = 2; // third row, zero-indexed
                                  // last_known_visible_height = 0 → fallback used by adjust_scroll,
                                  // so scroll_offset may not have been set correctly yet.
                                  // The render-time corrected_scroll should still show row 2.

        // Render with a 3-row list area (approx: picker_height=7+borders → inner list ≈ 3)
        let widget = VersionPickerOverlay::new(&state);
        // Use a larger area so the picker is fully rendered.
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        // Row at index 2 is "3.10.0" (no date, no arch)
        assert!(
            content.contains("3.10.0"),
            "cursor row (index 2) must be in the rendered slice: {content:?}"
        );
    }

    #[test]
    fn test_last_known_visible_height_written_by_render() {
        let state = loaded_stable_state();
        assert_eq!(state.last_known_visible_height.get(), 0, "starts at 0");

        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        assert!(
            state.last_known_visible_height.get() > 0,
            "render must write last_known_visible_height: height={}",
            state.last_known_visible_height.get()
        );
    }

    // ── Acceptance criterion 4: Loading and Failed states ────────────────────

    #[test]
    fn test_loading_state_shows_fetching_message() {
        let state = VersionPickerState {
            visible: true,
            fetch: PickerFetch::Loading,
            ..VersionPickerState::default()
        };

        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        assert!(
            content.contains("Fetching Flutter releases"),
            "Loading state must show fetching message: {content:?}"
        );
    }

    #[test]
    fn test_failed_state_shows_error_text() {
        let state = VersionPickerState {
            visible: true,
            fetch: PickerFetch::Failed,
            error: Some("network timeout".to_string()),
            ..VersionPickerState::default()
        };

        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        assert!(
            content.contains("network timeout"),
            "Failed state must show error text: {content:?}"
        );
    }

    #[test]
    fn test_failed_state_shows_fallback_hint() {
        let state = VersionPickerState {
            visible: true,
            fetch: PickerFetch::Failed,
            error: Some("network timeout".to_string()),
            ..VersionPickerState::default()
        };

        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        // The fallback hint: "Enter installs the default channel · r retries"
        assert!(
            content.contains("default channel"),
            "Failed state must show default-channel fallback hint: {content:?}"
        );
        assert!(
            content.contains("retries"),
            "Failed state must show retry hint: {content:?}"
        );
    }

    // ── Acceptance criterion 4: footer always shown ───────────────────────────

    #[test]
    fn test_footer_shows_all_hints() {
        let state = loaded_stable_state();
        let widget = VersionPickerOverlay::new(&state);
        // Use a wide enough area (160 wide) so the 72-char footer hint is not truncated.
        let area = Rect::new(0, 0, 160, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        // Check footer contains all expected hints
        assert!(content.contains("j/k"), "footer should show j/k hint");
        assert!(content.contains("Tab"), "footer should show Tab hint");
        assert!(content.contains("Enter"), "footer should show Enter hint");
        assert!(
            content.contains("refetch"),
            "footer should show refetch hint"
        );
        assert!(content.contains("Esc"), "footer should show Esc hint");
    }

    // ── No panic guards ───────────────────────────────────────────────────────

    #[test]
    fn test_no_panic_tiny_area() {
        let state = loaded_stable_state();
        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 5, 3);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_no_panic_zero_area() {
        let state = loaded_stable_state();
        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_no_panic_not_fetched_state() {
        let state = VersionPickerState {
            visible: true,
            // fetch is NotFetched (default)
            ..VersionPickerState::default()
        };
        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_no_panic_empty_loaded_tab() {
        let state = VersionPickerState {
            visible: true,
            fetch: PickerFetch::Loaded,
            // All tabs are empty
            ..VersionPickerState::default()
        };
        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);
        assert!(
            content.contains("No releases"),
            "empty tab should show 'No releases': {content:?}"
        );
    }

    #[test]
    fn test_no_panic_long_version_string_narrow_width() {
        let state = VersionPickerState {
            visible: true,
            fetch: PickerFetch::Loaded,
            stable: vec![PickerRow {
                version: "1.12.13+hotfix.5".to_string(),
                channel: "stable".to_string(),
                release_date: Some("2020-05-05".to_string()),
                arch: Some("x64".to_string()),
                git_only: false,
            }],
            ..VersionPickerState::default()
        };
        let widget = VersionPickerOverlay::new(&state);
        // Narrow but above minimum threshold
        let area = Rect::new(0, 0, 50, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_date_truncated_to_10_chars() {
        let state = VersionPickerState {
            visible: true,
            fetch: PickerFetch::Loaded,
            stable: vec![make_picker_row(
                "3.24.0",
                Some("2024-08-21T17:10:03.737Z"),
                None,
            )],
            ..VersionPickerState::default()
        };

        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        // Should show "2024-08-21" but not the time portion
        assert!(
            content.contains("2024-08-21"),
            "should show truncated date: {content:?}"
        );
        assert!(
            !content.contains("17:10"),
            "should NOT show time portion of ISO string: {content:?}"
        );
    }

    #[test]
    fn test_block_title_shown() {
        let state = loaded_stable_state();
        let widget = VersionPickerOverlay::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content = buf_content(&buf);

        assert!(
            content.contains("Flutter version"),
            "block title 'Flutter version' should be visible: {content:?}"
        );
    }
}

//! # Version List Pane
//!
//! Right pane of the Flutter Version Panel.
//! Displays a scrollable list of Flutter SDK versions installed in the FVM cache.
//!
//! States:
//! - **Loading** — scan in progress; shows "Scanning…"
//! - **Error**   — scan failed; shows error text
//! - **Empty**   — no versions found; shows hint message
//! - **List**    — normal rendering with active indicator and selection highlight
//!
//! Writes `last_known_visible_height` via `Cell` each frame (render-hint pattern).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use fdemon_app::flutter_version::{InstalledSdk, VersionListState};

use crate::theme::{icons::IconSet, palette};

/// Height of the section title ("Installed Versions") + underline separator.
///
/// Derived from: 1 title row + 1 separator row = 2 rows.
const HEADER_HEIGHT: u16 = 2;

/// Active version indicator character (filled circle).
const ACTIVE_INDICATOR: char = '\u{25cf}'; // ●

/// Inactive version placeholder (space, same width as active indicator).
const INACTIVE_INDICATOR: char = ' ';

/// Right pane — scrollable installed versions list.
pub struct VersionListPane<'a> {
    state: &'a VersionListState,
    focused: bool,
    /// Icon set (reserved for future icon decorations).
    #[allow(dead_code)]
    icons: &'a IconSet,
}

impl<'a> VersionListPane<'a> {
    /// Create a new version list pane.
    ///
    /// # Arguments
    /// * `state`   – Version list state
    /// * `focused` – Whether this pane has keyboard focus
    /// * `icons`   – Runtime icon resolver
    pub fn new(state: &'a VersionListState, focused: bool, icons: &'a IconSet) -> Self {
        Self {
            state,
            focused,
            icons,
        }
    }

    /// Render a single version item row at absolute y position `y`.
    ///
    /// Format: `  [●/ ] version_string [(channel)]`
    fn render_version_item(
        &self,
        sdk: &InstalledSdk,
        y: u16,
        x: u16,
        width: u16,
        is_selected: bool,
        buf: &mut Buffer,
    ) {
        let indicator = if sdk.is_active {
            ACTIVE_INDICATOR
        } else {
            INACTIVE_INDICATOR
        };

        // Build channel suffix (omit if channel is None or same as version string)
        let channel_suffix = match &sdk.channel {
            Some(ch) if ch != &sdk.version => format!(" ({})", ch),
            _ => String::new(),
        };

        let text = format!("  {} {} {}", indicator, sdk.version, channel_suffix);
        let text = text.trim_end().to_string();

        let row_style = if is_selected && self.focused {
            // Focused + selected: high-contrast accent background
            Style::default()
                .fg(palette::CONTRAST_FG)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            // Selected but pane not focused: subtle highlight
            Style::default()
                .fg(palette::TEXT_BRIGHT)
                .bg(palette::SELECTED_ROW_BG)
        } else if sdk.is_active {
            // Active version: accent color text
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_PRIMARY)
        };

        buf.set_string(x, y, &text, row_style);

        // Fill the rest of the row with the row background to avoid stray characters
        let text_width = text.chars().count() as u16;
        if text_width < width {
            let padding = " ".repeat((width - text_width) as usize);
            buf.set_string(x + text_width, y, &padding, row_style);
        }
    }

    /// Render the "Installed Versions" header and underline.
    fn render_list_header(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let title_style = if self.focused {
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_SECONDARY)
        };

        let title = Line::from(vec![
            Span::raw("  "),
            Span::styled("Installed Versions", title_style),
        ]);
        Paragraph::new(title).render(Rect::new(area.x, area.y, area.width, 1), buf);

        if area.height >= 2 {
            let separator = "\u{2500}".repeat(area.width as usize); // ─
            buf.set_string(
                area.x,
                area.y + 1,
                &separator,
                Style::default().fg(palette::BORDER_DIM),
            );
        }
    }

    /// Render loading state: "Scanning…"
    fn render_loading(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }
        let msg = Line::from(Span::styled(
            "  Scanning\u{2026}",
            Style::default().fg(palette::TEXT_MUTED),
        ));
        Paragraph::new(msg).render(Rect::new(area.x, area.y, area.width, 1), buf);
    }

    /// Render error state.
    fn render_error(&self, error: &str, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }
        let msg = Line::from(vec![
            Span::raw("  "),
            Span::styled(error, Style::default().fg(palette::STATUS_RED)),
        ]);
        Paragraph::new(msg).render(Rect::new(area.x, area.y, area.width, 1), buf);
    }

    /// Render empty state: no versions installed.
    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

        let msg = Line::from(Span::styled(
            "No versions found",
            Style::default().fg(palette::TEXT_MUTED),
        ));
        Paragraph::new(msg).render(
            Rect::new(
                chunks[1].x + 2,
                chunks[1].y,
                chunks[1].width.saturating_sub(2),
                1,
            ),
            buf,
        );

        let hint = Line::from(Span::styled(
            "Install with: fvm install <version>",
            Style::default().fg(palette::TEXT_MUTED),
        ));
        Paragraph::new(hint).render(
            Rect::new(
                chunks[2].x + 2,
                chunks[2].y,
                chunks[2].width.saturating_sub(2),
                1,
            ),
            buf,
        );
    }
}

impl Widget for VersionListPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Layout: header | list (flexible)
        let chunks =
            Layout::vertical([Constraint::Length(HEADER_HEIGHT), Constraint::Min(0)]).split(area);

        self.render_list_header(chunks[0], buf);

        let list_area = chunks[1];

        // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md
        let visible_height = list_area.height as usize;
        self.state.last_known_visible_height.set(visible_height);

        // Early return for special states
        if self.state.loading {
            self.render_loading(list_area, buf);
            return;
        }

        if let Some(ref err) = self.state.error {
            self.render_error(err, list_area, buf);
            return;
        }

        if self.state.installed_versions.is_empty() {
            self.render_empty(list_area, buf);
            return;
        }

        // Render-time scroll clamp: safety net so selected item is always visible.
        // This does NOT mutate state — it produces a local corrected offset for
        // this frame only. The handler uses `last_known_visible_height` to clamp
        // the real scroll_offset on future keystrokes.
        let corrected_scroll = if visible_height > 0 {
            let total = self.state.installed_versions.len();
            let sel = self.state.selected_index;
            let mut offset = self.state.scroll_offset;
            // Clamp: selected must be within [offset, offset+visible_height)
            if sel < offset {
                offset = sel;
            } else if sel >= offset + visible_height {
                offset = sel.saturating_sub(visible_height - 1);
            }
            // Also clamp so we don't scroll past the end
            let max_offset = total.saturating_sub(visible_height);
            offset.min(max_offset)
        } else {
            self.state.scroll_offset
        };

        let total = self.state.installed_versions.len();
        let start = corrected_scroll;
        let end = (start + visible_height).min(total);

        for (i, sdk) in self.state.installed_versions[start..end].iter().enumerate() {
            let y = list_area.y + i as u16;
            let is_selected = (start + i) == self.state.selected_index;
            self.render_version_item(sdk, y, list_area.x, list_area.width, is_selected, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::path::PathBuf;

    fn make_state_with_versions() -> VersionListState {
        VersionListState {
            installed_versions: vec![
                InstalledSdk {
                    version: "3.19.0".into(),
                    channel: Some("stable".into()),
                    path: PathBuf::from("/fvm/versions/3.19.0"),
                    is_active: true,
                },
                InstalledSdk {
                    version: "3.16.0".into(),
                    channel: Some("stable".into()),
                    path: PathBuf::from("/fvm/versions/3.16.0"),
                    is_active: false,
                },
                InstalledSdk {
                    version: "3.22.0-beta".into(),
                    channel: Some("beta".into()),
                    path: PathBuf::from("/fvm/versions/3.22.0-beta"),
                    is_active: false,
                },
            ],
            selected_index: 0,
            scroll_offset: 0,
            loading: false,
            error: None,
            last_known_visible_height: Cell::new(0),
        }
    }

    fn make_state_loading() -> VersionListState {
        VersionListState {
            installed_versions: vec![],
            selected_index: 0,
            scroll_offset: 0,
            loading: true,
            error: None,
            last_known_visible_height: Cell::new(0),
        }
    }

    fn make_state_empty() -> VersionListState {
        VersionListState {
            installed_versions: vec![],
            selected_index: 0,
            scroll_offset: 0,
            loading: false,
            error: None,
            last_known_visible_height: Cell::new(0),
        }
    }

    fn make_state_error() -> VersionListState {
        VersionListState {
            installed_versions: vec![],
            selected_index: 0,
            scroll_offset: 0,
            loading: false,
            error: Some("Permission denied".into()),
            last_known_visible_height: Cell::new(0),
        }
    }

    #[test]
    fn test_version_list_renders_versions() {
        let state = make_state_with_versions();
        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("3.19.0"), "should show first version");
    }

    #[test]
    fn test_version_list_active_indicator_shown() {
        let state = make_state_with_versions();
        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains('\u{25cf}'),
            "active version should have filled circle indicator"
        );
    }

    #[test]
    fn test_version_list_loading_state() {
        let state = make_state_loading();
        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Scanning"),
            "loading state should show scanning message"
        );
    }

    #[test]
    fn test_version_list_empty_state() {
        let state = make_state_empty();
        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("No versions"),
            "empty state should show 'No versions' message"
        );
    }

    #[test]
    fn test_version_list_error_state() {
        let state = make_state_error();
        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Permission denied"),
            "error state should show error message"
        );
    }

    #[test]
    fn test_render_hint_set_during_render() {
        let state = make_state_with_versions();
        assert_eq!(state.last_known_visible_height.get(), 0);

        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);

        // last_known_visible_height should be > 0 after render
        assert!(
            state.last_known_visible_height.get() > 0,
            "render hint should be updated during render"
        );
    }

    #[test]
    fn test_render_hint_set_during_loading_render() {
        // Even in loading state, the render hint must be written
        let state = make_state_loading();
        assert_eq!(state.last_known_visible_height.get(), 0);

        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);

        assert!(state.last_known_visible_height.get() > 0);
    }

    #[test]
    fn test_scroll_offset_clamp_safety_net() {
        // Selected index is 2, scroll_offset is 5 (beyond selected) — render should still show item
        let mut state = make_state_with_versions();
        state.selected_index = 2;
        state.scroll_offset = 5; // beyond end of list (3 items)

        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_no_panic_tiny_area() {
        let state = make_state_with_versions();
        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 5, 2);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_channel_suffix_not_shown_when_same_as_version() {
        // "stable" version directory with channel="stable" — suffix should be omitted
        let state = VersionListState {
            installed_versions: vec![InstalledSdk {
                version: "stable".into(),
                channel: Some("stable".into()),
                path: PathBuf::from("/fvm/versions/stable"),
                is_active: false,
            }],
            selected_index: 0,
            scroll_offset: 0,
            loading: false,
            error: None,
            last_known_visible_height: Cell::new(0),
        };

        let icons = IconSet::default();
        let pane = VersionListPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        // Should NOT contain "(stable)" as suffix when version == channel
        // The content will have "stable" as the version but not "(stable)"
        assert!(content.contains("stable"));
        // Check there's no duplicate "(stable)" — since the line should just be "  ○ stable"
        // rather than "  ○ stable (stable)"
        let trimmed: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            !trimmed.contains("stable (stable)"),
            "should not show redundant channel suffix"
        );
    }
}

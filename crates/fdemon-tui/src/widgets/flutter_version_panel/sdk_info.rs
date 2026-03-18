//! # SDK Info Pane
//!
//! Left pane of the Flutter Version Panel.
//! Displays read-only details about the currently resolved Flutter SDK:
//! version, channel, source, SDK path, and bundled Dart version.
//! Shows "No Flutter SDK found" when no SDK is resolved.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use fdemon_app::flutter_version::SdkInfoState;
use fdemon_app::FlutterSdk;

use crate::theme::{icons::IconSet, palette};

/// Height of a label+value pair (2 lines: label on top, value below).
///
/// Derived from: 1 label line + 1 value line = 2 rows per field.
const FIELD_HEIGHT: u16 = 2;

/// Spacer height between field pairs.
///
/// Derived from: 1 blank row between each field group = 1 row.
const FIELD_SPACER: u16 = 1;

/// Maximum characters for SDK path display before truncation.
///
/// Derived from: typical pane width of ~30 chars minus 2 padding chars = 28.
const MAX_PATH_WIDTH: usize = 28;

/// Left pane — read-only SDK details.
pub struct SdkInfoPane<'a> {
    state: &'a SdkInfoState,
    focused: bool,
    /// Icon set (currently unused; reserved for future icon decorations).
    #[allow(dead_code)]
    icons: &'a IconSet,
}

impl<'a> SdkInfoPane<'a> {
    /// Create a new SDK info pane.
    ///
    /// # Arguments
    /// * `state`   – SDK info state snapshot
    /// * `focused` – Whether this pane has keyboard focus (for border highlight)
    /// * `icons`   – Runtime icon resolver
    pub fn new(state: &'a SdkInfoState, focused: bool, icons: &'a IconSet) -> Self {
        Self {
            state,
            focused,
            icons,
        }
    }

    /// Render a label/value pair at the top of `area`.
    ///
    /// Label text is dimmed; value text is primary or bright.
    fn render_field(label: &str, value: &str, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }
        let label_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(label, Style::default().fg(palette::TEXT_MUTED)),
        ]);
        Paragraph::new(label_line).render(Rect::new(area.x, area.y, area.width, 1), buf);

        if area.height >= 2 {
            let value_line = Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    value.to_string(),
                    Style::default()
                        .fg(palette::TEXT_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            Paragraph::new(value_line).render(Rect::new(area.x, area.y + 1, area.width, 1), buf);
        }
    }

    /// Render the SDK details grid.
    fn render_sdk_details(&self, sdk: &FlutterSdk, area: Rect, buf: &mut Buffer) {
        // Layout: 5 fields + 4 spacers + absorber
        // Each field = FIELD_HEIGHT(2) rows; each spacer = FIELD_SPACER(1) row.
        let chunks = Layout::vertical([
            Constraint::Length(FIELD_HEIGHT), // VERSION + CHANNEL row
            Constraint::Length(FIELD_SPACER), // spacer
            Constraint::Length(FIELD_HEIGHT), // SOURCE + PATH row
            Constraint::Length(FIELD_SPACER), // spacer
            Constraint::Length(FIELD_HEIGHT), // DART SDK row
            Constraint::Min(0),               // absorb remaining space
        ])
        .split(area);

        // Row 0: VERSION | CHANNEL (split horizontally)
        let row0 = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        Self::render_field("VERSION", &sdk.version, row0[0], buf);
        let channel_str = sdk.channel.as_deref().unwrap_or("unknown");
        Self::render_field("CHANNEL", channel_str, row0[1], buf);

        // Row 2: SOURCE | SDK PATH (split horizontally)
        let row2 = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[2]);

        let source_str = sdk.source.to_string();
        Self::render_field("SOURCE", &source_str, row2[0], buf);

        let path_str = format_path(&sdk.root, MAX_PATH_WIDTH);
        Self::render_field("SDK PATH", &path_str, row2[1], buf);

        // Row 4: DART SDK
        let dart_str = self.state.dart_version.as_deref().unwrap_or("\u{2014}"); // em-dash
        Self::render_field("DART SDK", dart_str, chunks[4], buf);
    }

    /// Render the "no SDK found" placeholder.
    fn render_no_sdk(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::vertical([
            Constraint::Min(0),    // push content to vertical center
            Constraint::Length(1), // main message
            Constraint::Length(1), // spacer
            Constraint::Length(1), // hint
            Constraint::Min(0),    // fill remaining
        ])
        .split(area);

        let msg = Line::from(Span::styled(
            "No Flutter SDK found",
            Style::default()
                .fg(palette::STATUS_YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
        let center_x = area.x + area.width.saturating_sub(20) / 2;
        Paragraph::new(msg).render(
            Rect::new(
                center_x,
                chunks[1].y,
                area.width.saturating_sub(center_x - area.x),
                1,
            ),
            buf,
        );

        let hint = Line::from(Span::styled(
            "Install Flutter or configure SDK path",
            Style::default().fg(palette::TEXT_MUTED),
        ));
        Paragraph::new(hint).render(
            Rect::new(area.x + 2, chunks[3].y, area.width.saturating_sub(2), 1),
            buf,
        );
    }
}

impl Widget for SdkInfoPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Apply focused border highlight to inner padding by rendering a subtle
        // top label in accent color when focused.
        if self.focused {
            if area.height >= 1 {
                let label = Line::from(Span::styled(
                    " SDK Info ",
                    Style::default()
                        .fg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ));
                Paragraph::new(label).render(Rect::new(area.x, area.y, area.width, 1), buf);
            }
            let content_area = if area.height > 1 {
                Rect::new(area.x, area.y + 1, area.width, area.height - 1)
            } else {
                return;
            };
            match &self.state.resolved_sdk {
                Some(sdk) => self.render_sdk_details(sdk, content_area, buf),
                None => self.render_no_sdk(content_area, buf),
            }
        } else {
            match &self.state.resolved_sdk {
                Some(sdk) => self.render_sdk_details(sdk, area, buf),
                None => self.render_no_sdk(area, buf),
            }
        }
    }
}

/// Format a path for display, replacing the home directory with `~`.
///
/// If the resulting string exceeds `max_width`, truncates from the left with
/// `…` prefix so the rightmost portion (filename/directory) remains visible.
fn format_path(path: &std::path::Path, max_width: usize) -> String {
    // Attempt home-dir substitution
    let path_str = if let Some(home) = dirs::home_dir() {
        let display = path.display().to_string();
        let home_display = home.display().to_string();
        if display.starts_with(&home_display) {
            format!("~{}", &display[home_display.len()..])
        } else {
            display
        }
    } else {
        path.display().to_string()
    };

    let char_count = path_str.chars().count();
    if char_count <= max_width {
        path_str
    } else if max_width <= 1 {
        "\u{2026}".to_string() // "…"
    } else {
        // Keep the rightmost (max_width - 1) chars, prefix with "…"
        let keep: String = path_str
            .chars()
            .skip(char_count - (max_width - 1))
            .collect();
        format!("\u{2026}{}", keep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ---- format_path tests ----

    #[test]
    fn test_format_path_short_path_unchanged() {
        let path = PathBuf::from("/usr/local/flutter");
        let result = format_path(&path, 30);
        assert_eq!(result, "/usr/local/flutter");
    }

    #[test]
    fn test_format_path_truncates_from_left() {
        let path = PathBuf::from("/a/very/long/path/that/exceeds/the/maximum/width/here");
        let result = format_path(&path, 20);
        assert_eq!(result.chars().count(), 20);
        assert!(result.starts_with('\u{2026}'), "should start with ellipsis");
    }

    #[test]
    fn test_format_path_exact_fit() {
        let s = "/short/path";
        let path = PathBuf::from(s);
        let result = format_path(&path, s.chars().count());
        assert_eq!(result, s);
    }

    #[test]
    fn test_format_path_max_width_one() {
        let path = PathBuf::from("/usr/local/flutter");
        let result = format_path(&path, 1);
        assert_eq!(result, "\u{2026}");
    }

    // ---- SdkInfoPane rendering smoke tests ----

    fn make_state_with_sdk() -> SdkInfoState {
        use fdemon_daemon::{FlutterExecutable, FlutterSdk, SdkSource};
        SdkInfoState {
            resolved_sdk: Some(FlutterSdk {
                root: PathBuf::from("/usr/local/flutter"),
                executable: FlutterExecutable::Direct(PathBuf::from(
                    "/usr/local/flutter/bin/flutter",
                )),
                source: SdkSource::SystemPath,
                version: "3.19.0".to_string(),
                channel: Some("stable".to_string()),
            }),
            dart_version: Some("3.3.0".to_string()),
        }
    }

    fn make_state_no_sdk() -> SdkInfoState {
        SdkInfoState {
            resolved_sdk: None,
            dart_version: None,
        }
    }

    #[test]
    fn test_sdk_info_pane_renders_with_sdk() {
        let state = make_state_with_sdk();
        let icons = IconSet::default();
        let pane = SdkInfoPane::new(&state, false, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 15);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        // Should contain version string somewhere
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("3.19.0"), "should show version");
        assert!(content.contains("stable"), "should show channel");
    }

    #[test]
    fn test_sdk_info_pane_renders_no_sdk() {
        let state = make_state_no_sdk();
        let icons = IconSet::default();
        let pane = SdkInfoPane::new(&state, false, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 15);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("No Flutter SDK"),
            "should show not-found message"
        );
    }

    #[test]
    fn test_sdk_info_pane_no_panic_tiny_area() {
        let state = make_state_with_sdk();
        let icons = IconSet::default();
        let pane = SdkInfoPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 5, 2);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_sdk_info_pane_focused_shows_label() {
        let state = make_state_with_sdk();
        let icons = IconSet::default();
        let pane = SdkInfoPane::new(&state, true, &icons);
        let area = ratatui::layout::Rect::new(0, 0, 40, 15);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("SDK Info"),
            "focused pane should show label"
        );
    }
}

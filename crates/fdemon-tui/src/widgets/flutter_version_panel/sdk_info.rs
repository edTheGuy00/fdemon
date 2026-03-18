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

use crate::theme::palette;

/// Height of the section header ("SDK Info") + underline separator.
///
/// Derived from: 1 title row + 1 separator row = 2 rows.
const HEADER_HEIGHT: u16 = 2;

/// Height of a label+value pair (2 lines: label on top, value below).
///
/// Derived from: 1 label line + 1 value line = 2 rows per field.
const FIELD_HEIGHT: u16 = 2;

/// Spacer height between field pairs.
///
/// Derived from: 1 blank row between each field group = 1 row.
const FIELD_SPACER: u16 = 1;

/// Minimum content-area height for expanded (2-row-per-field) layout.
///
/// Derived from: 4 field groups × 2 rows + 3 spacers = 11 rows.
const MIN_EXPANDED_CONTENT_HEIGHT: u16 = 11;

/// Safety margin subtracted from a column's pixel width when computing the
/// maximum path display width dynamically.
///
/// Derived from: 2 label-prefix spaces + 2 safety chars = 4.
const PATH_WIDTH_MARGIN: u16 = 4;

/// Left pane — read-only SDK details.
pub struct SdkInfoPane<'a> {
    state: &'a SdkInfoState,
    focused: bool,
}

impl<'a> SdkInfoPane<'a> {
    /// Create a new SDK info pane.
    ///
    /// # Arguments
    /// * `state`   – SDK info state snapshot
    /// * `focused` – Whether this pane has keyboard focus (for border highlight)
    pub fn new(state: &'a SdkInfoState, focused: bool) -> Self {
        Self { state, focused }
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
    ///
    /// Delegates to compact or expanded layout based on available area height.
    fn render_sdk_details(&self, sdk: &FlutterSdk, area: Rect, buf: &mut Buffer) {
        if area.height < MIN_EXPANDED_CONTENT_HEIGHT {
            self.render_sdk_details_compact(sdk, area, buf);
        } else {
            self.render_sdk_details_expanded(sdk, area, buf);
        }
    }

    /// Choose the display string for a probe-dependent field.
    ///
    /// - `Some(value)`: show the value normally
    /// - `None` + `probe_completed == false`: show `"..."` (loading indicator)
    /// - `None` + `probe_completed == true`: show `"—"` (em-dash, unavailable)
    fn probe_field_str(value: &Option<String>, probe_completed: bool) -> &str {
        match value {
            Some(s) => s.as_str(),
            None if probe_completed => "\u{2014}", // "—" em-dash
            None => "...",
        }
    }

    /// Render a probe-dependent label/value pair.
    ///
    /// The value is styled with `TEXT_MUTED` when showing the "..." loading
    /// indicator; otherwise uses the normal `TEXT_PRIMARY + BOLD` style.
    fn render_probe_field(
        label: &str,
        value: &Option<String>,
        probe_completed: bool,
        area: Rect,
        buf: &mut Buffer,
    ) {
        if area.height < 1 {
            return;
        }
        let label_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(label, Style::default().fg(palette::TEXT_MUTED)),
        ]);
        Paragraph::new(label_line).render(Rect::new(area.x, area.y, area.width, 1), buf);

        if area.height >= 2 {
            let display = Self::probe_field_str(value, probe_completed);
            let value_style = if value.is_none() && !probe_completed {
                // Loading indicator — use muted style so it's visually distinct
                Style::default().fg(palette::TEXT_MUTED)
            } else {
                Style::default()
                    .fg(palette::TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD)
            };
            let value_line = Line::from(vec![Span::raw("  "), Span::styled(display, value_style)]);
            Paragraph::new(value_line).render(Rect::new(area.x, area.y + 1, area.width, 1), buf);
        }
    }

    /// Render SDK fields in expanded layout: 2-row label/value pairs with spacers.
    ///
    /// Layout: 4 field groups × 2 rows + 3 spacers = 11 rows minimum.
    /// ```text
    ///   VERSION         CHANNEL
    ///   3.38.6          stable
    ///
    ///   SOURCE          SDK PATH
    ///   system PATH     ~/Dev/flutter
    ///
    ///   DART SDK        DEVTOOLS
    ///   3.10.7          2.51.1
    ///
    ///   FRAMEWORK       ENGINE
    ///   8b87286849      6f3039bf7c
    /// ```
    fn render_sdk_details_expanded(&self, sdk: &FlutterSdk, area: Rect, buf: &mut Buffer) {
        // Layout: 4 groups + 3 spacers + absorber
        // Each field = FIELD_HEIGHT(2) rows; each spacer = FIELD_SPACER(1) row.
        let chunks = Layout::vertical([
            Constraint::Length(FIELD_HEIGHT), // group 1: VERSION | CHANNEL
            Constraint::Length(FIELD_SPACER), // spacer
            Constraint::Length(FIELD_HEIGHT), // group 2: SOURCE | SDK PATH
            Constraint::Length(FIELD_SPACER), // spacer
            Constraint::Length(FIELD_HEIGHT), // group 3: DART SDK | DEVTOOLS
            Constraint::Length(FIELD_SPACER), // spacer
            Constraint::Length(FIELD_HEIGHT), // group 4: FRAMEWORK | ENGINE
            Constraint::Min(0),               // absorb remaining space
        ])
        .split(area);

        // Group 1: VERSION | CHANNEL (split horizontally 50/50)
        let row0 = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        Self::render_field("VERSION", &sdk.version, row0[0], buf);
        let channel_str = sdk.channel.as_deref().unwrap_or("\u{2014}");
        Self::render_field("CHANNEL", channel_str, row0[1], buf);

        // Group 2: SOURCE | SDK PATH (split horizontally 40/60)
        let row2 = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[2]);

        let source_str = sdk.source.to_string();
        Self::render_field("SOURCE", &source_str, row2[0], buf);

        // Dynamic path width: use the actual column pixel width minus safety margin
        let max_path_width = row2[1].width.saturating_sub(PATH_WIDTH_MARGIN) as usize;
        let max_path_width = max_path_width.max(1); // always allow at least 1 char
        let path_str = format_path(&sdk.root, max_path_width);
        Self::render_field("SDK PATH", &path_str, row2[1], buf);

        // Group 3: DART SDK | DEVTOOLS (split horizontally 50/50)
        let row4 = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[4]);

        let dart_str = self.state.dart_version.as_deref().unwrap_or("\u{2014}"); // em-dash
        Self::render_field("DART SDK", dart_str, row4[0], buf);

        // DEVTOOLS — probe-dependent field
        Self::render_probe_field(
            "DEVTOOLS",
            &self.state.devtools_version,
            self.state.probe_completed,
            row4[1],
            buf,
        );

        // Group 4: FRAMEWORK | ENGINE (split horizontally 50/50)
        let row6 = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[6]);

        // FRAMEWORK — probe-dependent field
        Self::render_probe_field(
            "FRAMEWORK",
            &self.state.framework_revision,
            self.state.probe_completed,
            row6[0],
            buf,
        );

        // ENGINE — probe-dependent field
        Self::render_probe_field(
            "ENGINE",
            &self.state.engine_revision,
            self.state.probe_completed,
            row6[1],
            buf,
        );
    }

    /// Render SDK fields in compact layout: single line per field, no spacers.
    ///
    /// Used when content area height < MIN_EXPANDED_CONTENT_HEIGHT.
    /// Format:
    /// ```text
    ///   3.38.6 stable (system PATH)
    ///   ~/Dev/flutter
    ///   Dart 3.10.7  DevTools 2.51.1
    ///   rev 8b87286849  engine 6f3039bf7c
    /// ```
    fn render_sdk_details_compact(&self, sdk: &FlutterSdk, area: Rect, buf: &mut Buffer) {
        let channel_str = sdk.channel.as_deref().unwrap_or("\u{2014}");
        let source_str = sdk.source.to_string();
        // Dynamic path width: use area width minus prefix chars and safety margin
        let max_path_width = area.width.saturating_sub(PATH_WIDTH_MARGIN) as usize;
        let max_path_width = max_path_width.max(1);
        let path_str = format_path(&sdk.root, max_path_width);
        let dart_str = self.state.dart_version.as_deref().unwrap_or("\u{2014}"); // em-dash
        let probe_completed = self.state.probe_completed;
        // Probe-dependent fields: show "..." while loading, "—" when done with no value
        let devtools_str = Self::probe_field_str(&self.state.devtools_version, probe_completed);
        let framework_str = Self::probe_field_str(&self.state.framework_revision, probe_completed);
        let engine_str = Self::probe_field_str(&self.state.engine_revision, probe_completed);

        let rows = [
            format!("  {} {} ({})", sdk.version, channel_str, source_str),
            format!("  {}", path_str),
            format!("  Dart {}  DevTools {}", dart_str, devtools_str),
            format!("  rev {}  engine {}", framework_str, engine_str),
        ];

        let chunks = Layout::vertical([
            Constraint::Length(1), // version / channel / source
            Constraint::Length(1), // SDK PATH
            Constraint::Length(1), // DART / DEVTOOLS
            Constraint::Length(1), // FRAMEWORK / ENGINE
            Constraint::Min(0),    // absorb remaining space
        ])
        .split(area);

        for (i, text) in rows.iter().enumerate() {
            let line = Line::from(Span::styled(
                text.as_str(),
                Style::default().fg(palette::TEXT_PRIMARY),
            ));
            Paragraph::new(line).render(chunks[i], buf);
        }
    }

    /// Render the "SDK Info" section header and underline separator.
    ///
    /// When focused the label is styled `ACCENT + BOLD`; when unfocused it uses
    /// `TEXT_SECONDARY`.  The separator line is always rendered in `BORDER_DIM`.
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
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

        let label = Line::from(vec![Span::raw("  "), Span::styled("SDK Info", title_style)]);
        Paragraph::new(label).render(Rect::new(area.x, area.y, area.width, 1), buf);

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
        // Always render header (label + underline) regardless of focus state.
        self.render_header(area, buf);

        // Content area starts below header; bail out if not enough space.
        let content_area = if area.height > HEADER_HEIGHT {
            Rect::new(
                area.x,
                area.y + HEADER_HEIGHT,
                area.width,
                area.height - HEADER_HEIGHT,
            )
        } else {
            return;
        };

        match &self.state.resolved_sdk {
            Some(sdk) => self.render_sdk_details(sdk, content_area, buf),
            None => self.render_no_sdk(content_area, buf),
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
            framework_revision: None,
            engine_revision: None,
            devtools_version: None,
            probe_completed: true, // Most tests assume probe done; loading tests override this.
        }
    }

    fn make_state_no_sdk() -> SdkInfoState {
        SdkInfoState {
            resolved_sdk: None,
            dart_version: None,
            framework_revision: None,
            engine_revision: None,
            devtools_version: None,
            probe_completed: true,
        }
    }

    #[test]
    fn test_sdk_info_pane_renders_with_sdk() {
        let state = make_state_with_sdk();
        let pane = SdkInfoPane::new(&state, false);
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
        let pane = SdkInfoPane::new(&state, false);
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
        let pane = SdkInfoPane::new(&state, true);
        let area = ratatui::layout::Rect::new(0, 0, 5, 2);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_sdk_info_pane_focused_shows_label() {
        let state = make_state_with_sdk();
        let pane = SdkInfoPane::new(&state, true);
        let area = ratatui::layout::Rect::new(0, 0, 40, 15);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("SDK Info"),
            "focused pane should show label"
        );
    }

    #[test]
    fn test_sdk_info_pane_unfocused_shows_label() {
        let state = make_state_with_sdk();
        let pane = SdkInfoPane::new(&state, false); // unfocused
        let area = ratatui::layout::Rect::new(0, 0, 40, 15);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("SDK Info"),
            "unfocused pane should still show label"
        );
    }

    #[test]
    fn test_sdk_info_pane_label_has_underline_separator() {
        let state = make_state_with_sdk();
        let pane = SdkInfoPane::new(&state, true);
        let area = ratatui::layout::Rect::new(0, 0, 40, 15);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        // Check row 1 contains separator character
        let row1: String = (0..area.width)
            .map(|x| {
                buf.cell((x, 1))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();
        assert!(
            row1.contains('\u{2500}'),
            "should have underline separator below label"
        );
    }

    #[test]
    fn test_sdk_info_compact_mode_all_fields_visible() {
        let state = make_state_with_sdk();
        let pane = SdkInfoPane::new(&state, true);
        // Very tight area: 8 rows total; 2 for header → 6 content rows (compact mode)
        let area = ratatui::layout::Rect::new(0, 0, 60, 8);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("3.19.0"), "compact should show version");
        assert!(content.contains("stable"), "compact should show channel");
        assert!(
            content.contains("3.3.0"),
            "compact should show dart version"
        );
    }

    #[test]
    fn test_sdk_info_expanded_mode_with_spacers() {
        let state = make_state_with_sdk();
        let pane = SdkInfoPane::new(&state, true);
        // Comfortable area: 15 rows total; 2 for header → 13 content rows (expanded mode)
        let area = ratatui::layout::Rect::new(0, 0, 40, 15);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("3.19.0"), "expanded should show version");
        assert!(
            content.contains("3.3.0"),
            "expanded should show dart version"
        );
    }

    #[test]
    fn test_sdk_info_extended_fields_render() {
        let mut state = make_state_with_sdk();
        state.framework_revision = Some("8b87286849".into());
        state.engine_revision = Some("6f3039bf7c".into());
        state.devtools_version = Some("2.51.1".into());
        let pane = SdkInfoPane::new(&state, true);
        let area = ratatui::layout::Rect::new(0, 0, 50, 20);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("8b87286849"),
            "should show framework revision"
        );
        assert!(
            content.contains("6f3039bf7c"),
            "should show engine revision"
        );
        assert!(content.contains("2.51.1"), "should show devtools version");
    }

    #[test]
    fn test_sdk_info_missing_extended_fields_show_dash_when_probe_completed() {
        let mut state = make_state_with_sdk();
        // probe_completed = true, fields still None → em-dash
        state.probe_completed = true;
        let pane = SdkInfoPane::new(&state, true);
        let area = ratatui::layout::Rect::new(0, 0, 50, 20);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains('\u{2014}'),
            "missing fields after probe completion should show em-dash"
        );
        assert!(
            !content.contains("..."),
            "should not show loading indicator after probe completion"
        );
    }

    #[test]
    fn test_sdk_info_missing_extended_fields_show_loading_when_probe_pending() {
        let mut state = make_state_with_sdk();
        // probe_completed = false (default), fields None → "..."
        state.probe_completed = false;
        let pane = SdkInfoPane::new(&state, true);
        let area = ratatui::layout::Rect::new(0, 0, 50, 20);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("..."),
            "missing probe fields while in-flight should show '...'"
        );
    }

    #[test]
    fn test_sdk_path_dynamic_width_wide_terminal() {
        let state = make_state_with_sdk();
        let pane = SdkInfoPane::new(&state, true);
        // Wide area — path should not be truncated (/usr/local/flutter is 18 chars)
        let area = ratatui::layout::Rect::new(0, 0, 80, 20);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        // Full path should be visible without ellipsis
        assert!(
            !content.contains('\u{2026}'),
            "wide terminal should not truncate path"
        );
    }
}

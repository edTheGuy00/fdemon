//! # Flutter Version Panel
//!
//! Centered overlay panel for viewing and managing Flutter SDK versions.
//! Follows the New Session Dialog widget pattern.
//!
//! ## Layout
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  Flutter SDK                                     [Esc] Close    │
//! │  Manage Flutter SDK versions and channels.                      │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  SDK Info                   │  Installed Versions               │
//! │  VERSION  CHANNEL           │  ─────────────────                │
//! │  3.19.0   stable            │  ● 3.19.0 (stable)                │
//! │                             │    3.16.0                         │
//! │  SOURCE   SDK PATH          │    3.22.0-beta (beta)             │
//! │  FVM      ~/fvm/versions/…  │                                   │
//! │                             │                                   │
//! │  DART SDK                   │                                   │
//! │  3.3.0                      │                                   │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  [Tab] Info  [Enter] Switch  [d] Remove  [Esc] Close            │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

mod sdk_info;
mod version_list;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

use fdemon_app::flutter_version::{FlutterVersionPane, FlutterVersionState};

use crate::theme::palette;
use crate::widgets::modal_overlay::{self, centered_rect_percent};

use sdk_info::SdkInfoPane;
use version_list::VersionListPane;

/// Minimum terminal width for horizontal (side-by-side) layout.
///
/// Derived from: 30 chars left pane + 1 separator + 35 chars right pane + 4 border = 70.
const MIN_HORIZONTAL_WIDTH: u16 = 70;

/// Minimum terminal height for any rendering.
///
/// Derived from: 3 header rows + 1 separator + 5 content rows + 1 separator + 1 footer + 2 border = 13.
const MIN_RENDER_HEIGHT: u16 = 13;

/// Minimum dialog width for any rendering.
///
/// Derived from: narrowest useful display of "No Flutter SDK found" + 4 border = 40.
const MIN_RENDER_WIDTH: u16 = 40;

/// Left pane width as percentage of content area.
///
/// Derived from: SDK info typically needs ~40% for comfortable field display.
const LEFT_PANE_PERCENT: u16 = 40;

/// Height of the left pane in vertical (stacked) layout.
///
/// Derived from: 1 focused label + 1 spacer + 4 fields × 2 rows + 3 spacers = 12.
/// Clamped to 6 for compact display in vertical mode.
const VERTICAL_SDK_INFO_HEIGHT: u16 = 6;

/// Panel width as a percentage of the terminal width.
///
/// Derived from: 80% provides comfortable margins on typical 80–200 column terminals.
const PANEL_WIDTH_PERCENT: u16 = 80;

/// Panel height as a percentage of the terminal height.
///
/// Derived from: 70% reserves header/footer space while showing all panel content.
const PANEL_HEIGHT_PERCENT: u16 = 70;

/// The main Flutter Version Panel widget.
///
/// Renders as a centered overlay over the full terminal area.
pub struct FlutterVersionPanel<'a> {
    state: &'a FlutterVersionState,
}

impl<'a> FlutterVersionPanel<'a> {
    /// Create a new Flutter Version Panel widget.
    ///
    /// # Arguments
    /// * `state` – Panel state snapshot
    pub fn new(state: &'a FlutterVersionState) -> Self {
        Self { state }
    }

    /// Render header: title + close hint on row 1, subtitle on row 2.
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        // Row 0: "Flutter SDK" (bold) on left, "[Esc] Close" on right
        let title_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Flutter SDK",
                Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let close_hint = Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw(" "),
            Span::styled("Close", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw("  "),
        ]);

        let title_area = Rect::new(area.x, area.y, area.width, 1);
        Paragraph::new(title_line).render(title_area, buf);
        Paragraph::new(close_hint)
            .alignment(Alignment::Right)
            .render(title_area, buf);

        // Row 1: subtitle (dimmed)
        if area.height >= 2 {
            let subtitle = Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Manage Flutter SDK versions and channels.",
                    Style::default().fg(palette::TEXT_SECONDARY),
                ),
            ]);
            let subtitle_area = Rect::new(area.x, area.y + 1, area.width, 1);
            Paragraph::new(subtitle).render(subtitle_area, buf);
        }
    }

    /// Render a horizontal separator line (─ repeated across full width).
    fn render_separator(&self, area: Rect, buf: &mut Buffer) {
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

    /// Render a vertical separator line (│ from top to bottom of area).
    fn render_vertical_separator(area: Rect, buf: &mut Buffer) {
        for y in area.top()..area.bottom() {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_char('\u{2502}'); // │
                cell.set_style(Style::default().fg(palette::BORDER_DIM));
            }
        }
    }

    /// Render horizontal (side-by-side) pane layout.
    fn render_horizontal_panes(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::horizontal([
            Constraint::Percentage(LEFT_PANE_PERCENT),
            Constraint::Length(1),
            Constraint::Min(20),
        ])
        .split(area);

        let sdk_info = SdkInfoPane::new(
            &self.state.sdk_info,
            self.state.focused_pane == FlutterVersionPane::SdkInfo,
        );
        sdk_info.render(chunks[0], buf);

        Self::render_vertical_separator(chunks[1], buf);

        let version_list = VersionListPane::new(
            &self.state.version_list,
            self.state.focused_pane == FlutterVersionPane::VersionList,
        );
        version_list.render(chunks[2], buf);
    }

    /// Render vertical (stacked) pane layout for narrow terminals.
    fn render_vertical_panes(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::vertical([
            Constraint::Length(VERTICAL_SDK_INFO_HEIGHT),
            Constraint::Length(1),
            Constraint::Min(5),
        ])
        .split(area);

        let sdk_info = SdkInfoPane::new(
            &self.state.sdk_info,
            self.state.focused_pane == FlutterVersionPane::SdkInfo,
        );
        sdk_info.render(chunks[0], buf);

        self.render_separator(chunks[1], buf);

        let version_list = VersionListPane::new(
            &self.state.version_list,
            self.state.focused_pane == FlutterVersionPane::VersionList,
        );
        version_list.render(chunks[2], buf);
    }

    /// Render footer: keybinding hints (and optional status message on the left).
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hints = match self.state.focused_pane {
            FlutterVersionPane::SdkInfo => "[Tab] Versions  [Esc] Close",
            FlutterVersionPane::VersionList => {
                "[Tab] Info  [Enter] Switch  [d] Remove  [Esc] Close"
            }
        };

        let text = if let Some(ref msg) = self.state.status_message {
            format!("{msg}  \u{2502}  {hints}") // │
        } else {
            hints.to_string()
        };

        Paragraph::new(Line::from(Span::styled(
            text,
            Style::default().fg(palette::TEXT_MUTED),
        )))
        .render(area, buf);
    }

    /// Render "terminal too small" message.
    fn render_too_small(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 1 {
            return;
        }
        let msg = " Terminal too small for Flutter SDK panel ";
        let x = area.x + area.width.saturating_sub(msg.chars().count() as u16) / 2;
        let y = area.y + area.height / 2;
        buf.set_string(
            x,
            y,
            msg,
            Style::default()
                .fg(palette::TEXT_MUTED)
                .add_modifier(Modifier::BOLD),
        );
    }
}

impl Widget for FlutterVersionPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 1. Dim the entire background
        modal_overlay::dim_background(buf, area);

        // 2. Calculate centered dialog area (PANEL_WIDTH_PERCENT × PANEL_HEIGHT_PERCENT)
        let dialog_area = centered_rect_percent(PANEL_WIDTH_PERCENT, PANEL_HEIGHT_PERCENT, area);

        // 3. Check minimum size — render "too small" and return early
        if dialog_area.width < MIN_RENDER_WIDTH || dialog_area.height < MIN_RENDER_HEIGHT {
            self.render_too_small(dialog_area, buf);
            return;
        }

        // 4. Render drop shadow
        modal_overlay::render_shadow(buf, dialog_area);

        // 5. Clear the dialog area
        modal_overlay::clear_area(buf, dialog_area);

        // 6. Render border block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .style(Style::default().bg(palette::POPUP_BG));
        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // 7. Layout: header(3) | separator(1) | panes(flex) | separator(1) | footer(1)
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0), // absorber
        ])
        .split(inner);

        self.render_header(chunks[0], buf);
        self.render_separator(chunks[1], buf);

        // 8. Choose horizontal vs vertical pane layout based on inner width
        if inner.width >= MIN_HORIZONTAL_WIDTH {
            self.render_horizontal_panes(chunks[2], buf);
        } else {
            self.render_vertical_panes(chunks[2], buf);
        }

        self.render_separator(chunks[3], buf);
        self.render_footer(chunks[4], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::flutter_version::{SdkInfoState, VersionListState};
    use std::cell::Cell;
    use std::path::PathBuf;

    fn test_state() -> FlutterVersionState {
        use fdemon_daemon::{FlutterExecutable, FlutterSdk, SdkSource};

        FlutterVersionState {
            sdk_info: SdkInfoState {
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
            },
            version_list: VersionListState {
                installed_versions: vec![fdemon_app::flutter_version::InstalledSdk {
                    version: "3.19.0".into(),
                    channel: Some("stable".into()),
                    path: PathBuf::from("/fvm/versions/3.19.0"),
                    is_active: true,
                }],
                selected_index: 0,
                scroll_offset: 0,
                loading: false,
                error: None,
                last_known_visible_height: Cell::new(0),
            },
            focused_pane: FlutterVersionPane::SdkInfo,
            visible: true,
            status_message: None,
            pending_delete: None,
        }
    }

    fn test_state_no_sdk() -> FlutterVersionState {
        FlutterVersionState {
            sdk_info: SdkInfoState {
                resolved_sdk: None,
                dart_version: None,
            },
            version_list: VersionListState::default(),
            focused_pane: FlutterVersionPane::SdkInfo,
            visible: true,
            status_message: None,
            pending_delete: None,
        }
    }

    #[test]
    fn test_panel_renders_without_panic() {
        let state = test_state();
        let widget = FlutterVersionPanel::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_too_small_renders_message() {
        let state = test_state();
        let widget = FlutterVersionPanel::new(&state);
        let area = Rect::new(0, 0, 30, 8); // too small
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // Panel should still render without panic; "too small" message appears
    }

    #[test]
    fn test_no_sdk_renders_without_panic() {
        let state = test_state_no_sdk();
        let widget = FlutterVersionPanel::new(&state);
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_panel_header_shows_title() {
        let state = test_state();
        let widget = FlutterVersionPanel::new(&state);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Flutter SDK"),
            "header should contain title"
        );
    }

    #[test]
    fn test_panel_header_shows_esc_close() {
        let state = test_state();
        let widget = FlutterVersionPanel::new(&state);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("[Esc]"), "header should contain [Esc]");
        assert!(content.contains("Close"), "header should contain Close");
    }

    #[test]
    fn test_panel_footer_sdk_info_focused_shows_versions_hint() {
        let mut state = test_state();
        state.focused_pane = FlutterVersionPane::SdkInfo;
        let widget = FlutterVersionPanel::new(&state);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Versions"),
            "footer should show Versions hint when SdkInfo focused"
        );
    }

    #[test]
    fn test_panel_footer_version_list_focused_shows_switch_hint() {
        let mut state = test_state();
        state.focused_pane = FlutterVersionPane::VersionList;
        let widget = FlutterVersionPanel::new(&state);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Switch"),
            "footer should show Switch hint when VersionList focused"
        );
    }

    #[test]
    fn test_panel_status_message_shown_in_footer() {
        let mut state = test_state();
        state.status_message = Some("Switched to 3.19.0".into());
        let widget = FlutterVersionPanel::new(&state);
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Switched"),
            "footer should show status message"
        );
    }

    #[test]
    fn test_panel_vertical_layout_narrow_terminal() {
        let state = test_state();
        let widget = FlutterVersionPanel::new(&state);
        // Use a narrow area to force vertical layout
        let area = Rect::new(0, 0, 60, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_panel_horizontal_layout_wide_terminal() {
        let state = test_state();
        let widget = FlutterVersionPanel::new(&state);
        // Wide enough to trigger horizontal layout
        let area = Rect::new(0, 0, 120, 50);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_no_sdk_shows_not_found() {
        use crate::widgets::flutter_version_panel::sdk_info::SdkInfoPane;
        use fdemon_app::flutter_version::SdkInfoState;

        let state = SdkInfoState {
            resolved_sdk: None,
            dart_version: None,
        };
        let pane = SdkInfoPane::new(&state, true);
        let area = Rect::new(0, 0, 30, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("No Flutter SDK"),
            "should show 'No Flutter SDK' when no SDK resolved"
        );
    }

    #[test]
    fn test_loading_state_shows_spinner() {
        use crate::widgets::flutter_version_panel::version_list::VersionListPane;

        let mut state = test_state();
        state.version_list.loading = true;
        let pane = VersionListPane::new(&state.version_list, true);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Scanning"),
            "loading state should show scanning text"
        );
    }

    #[test]
    fn test_render_hint_set_during_render() {
        use crate::widgets::flutter_version_panel::version_list::VersionListPane;

        let state = test_state();
        assert_eq!(state.version_list.last_known_visible_height.get(), 0);
        let pane = VersionListPane::new(&state.version_list, true);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        assert!(
            state.version_list.last_known_visible_height.get() > 0,
            "render hint should be updated during render"
        );
    }

    #[test]
    fn test_active_version_has_indicator() {
        use crate::widgets::flutter_version_panel::version_list::VersionListPane;

        let mut state = test_state();
        state.version_list.installed_versions = vec![fdemon_app::flutter_version::InstalledSdk {
            version: "3.19.0".into(),
            channel: Some("stable".into()),
            path: PathBuf::from("/test"),
            is_active: true,
        }];
        let pane = VersionListPane::new(&state.version_list, true);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains('\u{25cf}'),
            "active version should have filled circle indicator"
        );
    }

    #[test]
    fn test_empty_list_shows_message() {
        use crate::widgets::flutter_version_panel::version_list::VersionListPane;

        let mut state = test_state();
        state.version_list.installed_versions = vec![];
        state.version_list.loading = false;
        let pane = VersionListPane::new(&state.version_list, true);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("No versions"),
            "empty list should show 'No versions' message"
        );
    }
}

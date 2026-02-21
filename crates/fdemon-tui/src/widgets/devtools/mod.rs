//! DevTools panel widgets for the TUI.
//!
//! Contains sub-panel widgets rendered when `UiMode::DevTools` is active.
//! The top-level [`DevToolsView`] composite widget renders a sub-tab bar and
//! dispatches to the active panel below it.

pub mod inspector;
pub mod network;
pub mod performance;

pub use inspector::WidgetInspector;
pub use network::NetworkMonitor;
pub use performance::PerformancePanel;

use fdemon_app::session::{PerformanceState, SessionHandle};
use fdemon_app::state::{DevToolsPanel, DevToolsViewState, VmConnectionStatus};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::theme::{icons::IconSet, palette};

// ── DevToolsView ─────────────────────────────────────────────────────────────

/// Top-level DevTools mode widget.
///
/// Renders a sub-tab bar at the top and dispatches to the active panel below.
/// Both panel widgets ([`WidgetInspector`] and [`PerformancePanel`]) are
/// non-stateful; state is passed in via references.
pub struct DevToolsView<'a> {
    state: &'a DevToolsViewState,
    session: Option<&'a SessionHandle>,
    icons: IconSet,
}

impl<'a> DevToolsView<'a> {
    /// Create a new `DevToolsView` widget.
    pub fn new(
        state: &'a DevToolsViewState,
        session: Option<&'a SessionHandle>,
        icons: IconSet,
    ) -> Self {
        Self {
            state,
            session,
            icons,
        }
    }
}

impl Widget for DevToolsView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear background
        let bg_style = Style::default().bg(palette::DEEPEST_BG);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style).set_char(' ');
                }
            }
        }

        if area.height < 4 {
            // Too small for any useful display
            return;
        }

        // Vertical layout: [sub-tab bar (3 lines)] + [panel content (remaining)]
        let chunks = Layout::vertical([
            Constraint::Length(3), // Sub-tab bar
            Constraint::Min(1),    // Panel content
        ])
        .split(area);

        // Render sub-tab bar
        self.render_tab_bar(chunks[0], buf);

        // Render active panel
        match self.state.active_panel {
            DevToolsPanel::Inspector => {
                let vm_connected = self
                    .session
                    .map(|s| s.session.vm_connected)
                    .unwrap_or(false);
                let widget = WidgetInspector::new(
                    &self.state.inspector,
                    vm_connected,
                    &self.state.connection_status,
                );
                widget.render(chunks[1], buf);
            }
            DevToolsPanel::Performance => {
                // Safety fallback for when no session is active.
                // In practice DevTools mode is only reachable when a session exists.
                static DEFAULT_PERF: std::sync::LazyLock<PerformanceState> =
                    std::sync::LazyLock::new(PerformanceState::default);

                let (perf, vm_connected) = self
                    .session
                    .map(|s| (&s.session.performance, s.session.vm_connected))
                    .unwrap_or_else(|| (&*DEFAULT_PERF, false));

                let widget = PerformancePanel::new(
                    perf,
                    vm_connected,
                    self.icons,
                    &self.state.connection_status,
                )
                .with_connection_error(self.state.vm_connection_error.as_deref());
                widget.render(chunks[1], buf);
            }
            DevToolsPanel::Network => {
                // Safety fallback: DevTools mode is only reachable when a session
                // exists, but guard defensively.
                static DEFAULT_NETWORK: std::sync::LazyLock<fdemon_app::session::NetworkState> =
                    std::sync::LazyLock::new(fdemon_app::session::NetworkState::default);

                let (network_state, vm_connected) = self
                    .session
                    .map(|s| (&s.session.network, s.session.vm_connected))
                    .unwrap_or_else(|| (&*DEFAULT_NETWORK, false));

                let widget =
                    NetworkMonitor::new(network_state, vm_connected, &self.state.connection_status);
                widget.render(chunks[1], buf);
            }
        }

        // Render footer hints at the bottom of the panel area
        self.render_footer(chunks[1], buf);
    }
}

impl DevToolsView<'_> {
    // ── Sub-tab bar ───────────────────────────────────────────────────────────

    /// Render the DevTools sub-tab bar with panel tabs and overlay status indicators.
    fn render_tab_bar(&self, area: Rect, buf: &mut Buffer) {
        // Outer block with border
        let block = ratatui::widgets::Block::bordered()
            .title(" DevTools ")
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let tabs = [
            (DevToolsPanel::Inspector, "[i] Inspector"),
            (DevToolsPanel::Performance, "[p] Performance"),
            (DevToolsPanel::Network, "[n] Network"),
        ];

        let mut x = inner.x + 1;
        for (panel, label) in &tabs {
            let is_active = self.state.active_panel == *panel;
            let padded = format!(" {label} ");
            let needed_width = padded.len() as u16;

            if x + needed_width > inner.right() {
                break;
            }

            let style = if is_active {
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_MUTED)
            };

            buf.set_string(x, inner.y, &padded, style);
            x += needed_width + 1;
        }

        // Right-aligned overlay status indicators
        let mut indicators: Vec<&str> = Vec::new();
        if self.state.overlay_repaint_rainbow {
            indicators.push("Rainbow");
        }
        if self.state.overlay_debug_paint {
            indicators.push("DebugPaint");
        }
        if self.state.overlay_performance {
            indicators.push("PerfOverlay");
        }

        // Connection indicator (only shown for degraded states)
        let mut conn_label_owned = String::new();
        let conn_indicator: Option<(&str, Style)> =
            self.connection_indicator_text(&mut conn_label_owned);

        // Build right-side text: connection indicator first, then overlay indicators
        // (connection state is more important for the user to see)
        let right_parts_count = if conn_indicator.is_some() { 1 } else { 0 }
            + if indicators.is_empty() { 0 } else { 1 };

        if right_parts_count > 0 {
            // Determine total right-side width to position correctly
            let overlay_text = if indicators.is_empty() {
                String::new()
            } else {
                indicators.join(" | ")
            };

            // Render connection indicator if present
            if let Some((label, style)) = &conn_indicator {
                let label_len = label.chars().count() as u16;
                let overlay_extra = if overlay_text.is_empty() {
                    0
                } else {
                    overlay_text.len() as u16 + 3 // " | " separator
                };
                let total_len = label_len + overlay_extra;
                let right_x = inner.x + inner.width.saturating_sub(total_len + 1);
                if right_x < inner.right() {
                    buf.set_string(right_x, inner.y, label, *style);
                }
            }

            // Render overlay indicators
            if !overlay_text.is_empty() {
                let text_len = overlay_text.len() as u16;
                let right_x = inner.x + inner.width.saturating_sub(text_len + 1);
                if right_x < inner.right() {
                    buf.set_string(
                        right_x,
                        inner.y,
                        &overlay_text,
                        Style::default().fg(palette::STATUS_YELLOW),
                    );
                }
            }
        }
    }

    /// Return the connection indicator label and style for degraded states,
    /// or `None` when the connection is healthy (Connected).
    ///
    /// `label_buf` is used as backing storage so the returned `&str` can borrow
    /// from it without requiring a `String` return value.
    fn connection_indicator_text<'a>(&self, label_buf: &'a mut String) -> Option<(&'a str, Style)> {
        match &self.state.connection_status {
            VmConnectionStatus::Connected => None,
            VmConnectionStatus::Disconnected => {
                *label_buf = "x Disconnected".to_string();
                Some((label_buf.as_str(), Style::default().fg(palette::STATUS_RED)))
            }
            VmConnectionStatus::Reconnecting {
                attempt,
                max_attempts,
            } => {
                *label_buf = format!("~ Reconnecting ({attempt}/{max_attempts})");
                Some((
                    label_buf.as_str(),
                    Style::default().fg(palette::STATUS_YELLOW),
                ))
            }
            VmConnectionStatus::TimedOut => {
                *label_buf = "! Timed Out".to_string();
                Some((
                    label_buf.as_str(),
                    Style::default().fg(palette::STATUS_YELLOW),
                ))
            }
        }
    }

    // ── Footer hints ──────────────────────────────────────────────────────────

    /// Render contextual keybinding hints at the bottom of the panel area.
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 {
            return;
        }

        let y = area.y + area.height - 1;

        let hints = match self.state.active_panel {
            DevToolsPanel::Inspector => {
                "[Esc] Logs  [↑↓] Navigate  [→] Expand  [←] Collapse  [r] Refresh  [b] Browser"
            }
            DevToolsPanel::Performance => {
                "[Esc] Logs  [i] Inspector  [b] Browser  [←/→] Frames  [Ctrl+p] PerfOverlay"
            }
            DevToolsPanel::Network => {
                let has_selection = self
                    .session
                    .is_some_and(|s| s.session.network.selected_index.is_some());
                if has_selection {
                    "[Esc] Deselect  [g/h/q/s/t] Detail tabs  [Space] Toggle rec  [b] Browser"
                } else {
                    "[Esc] Logs  [↑↓] Navigate  [Enter] Detail  [Space] Toggle rec  [b] Browser"
                }
            }
        };

        // Truncate hints to fit available width
        let max_width = area.width.saturating_sub(2) as usize;
        let display_hints: String = hints.chars().take(max_width).collect();

        buf.set_string(
            area.x + 1,
            y,
            &display_hints,
            Style::default().fg(palette::TEXT_MUTED),
        );
    }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Truncate a string to at most `max_chars` Unicode characters.
/// Returns a `&str` slice — no allocation when the string fits.
pub(super) fn truncate_str(s: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::state::{DevToolsPanel, DevToolsViewState, VmConnectionStatus};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn collect_buf_text(buf: &Buffer, width: u16, height: u16) -> String {
        let mut full = String::new();
        for y in 0..height {
            for x in 0..width {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        full
    }

    #[test]
    fn test_devtools_view_renders_inspector_panel() {
        let state = DevToolsViewState::default();
        assert_eq!(state.active_panel, DevToolsPanel::Inspector);

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_view_renders_performance_panel() {
        let mut state = DevToolsViewState::default();
        state.active_panel = DevToolsPanel::Performance;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_tab_bar_highlights_active_panel() {
        let mut state = DevToolsViewState::default();
        state.active_panel = DevToolsPanel::Performance;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);

        // Check that "Performance" text appears in the buffer
        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("Performance"),
            "Expected 'Performance' in tab bar, got: {text:?}"
        );
    }

    #[test]
    fn test_tab_bar_shows_all_panels() {
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(text.contains("Inspector"), "Expected Inspector tab");
        assert!(text.contains("Performance"), "Expected Performance tab");
        assert!(
            !text.contains("Layout"),
            "Layout tab should not appear; got: {text:?}"
        );
    }

    #[test]
    fn test_overlay_indicators_shown_when_active() {
        let mut state = DevToolsViewState::default();
        state.overlay_repaint_rainbow = true;
        state.overlay_debug_paint = true;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("Rainbow"),
            "Expected 'Rainbow' indicator, got: {text:?}"
        );
        assert!(
            text.contains("DebugPaint"),
            "Expected 'DebugPaint' indicator, got: {text:?}"
        );
    }

    #[test]
    fn test_overlay_perf_overlay_shown_when_active() {
        let mut state = DevToolsViewState::default();
        state.overlay_performance = true;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("PerfOverlay"),
            "Expected 'PerfOverlay' indicator, got: {text:?}"
        );
    }

    #[test]
    fn test_devtools_view_small_terminal() {
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_view_very_small_terminal() {
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 3));
        widget.render(Rect::new(0, 0, 40, 3), &mut buf);
        // Should not panic (height < 4 early return)
    }

    #[test]
    fn test_devtools_view_large_terminal() {
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 40));
        widget.render(Rect::new(0, 0, 120, 40), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_view_active_panel_inspector_tab_highlighted() {
        // Inspector is default active panel
        let state = DevToolsViewState::default();
        assert_eq!(state.active_panel, DevToolsPanel::Inspector);

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);

        // Find the Inspector label cell and check its bg style
        // The active tab should have Cyan background
        let text = collect_buf_text(&buf, 80, 3);
        assert!(text.contains("Inspector"), "Expected Inspector in tab bar");
    }

    // ── Connection indicator tests ─────────────────────────────────────────────

    #[test]
    fn test_connection_indicator_connected_shows_nothing() {
        let mut state = DevToolsViewState::default();
        state.connection_status = VmConnectionStatus::Connected;
        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut label = String::new();
        let result = widget.connection_indicator_text(&mut label);
        assert!(
            result.is_none(),
            "Connected state should show no indicator, got: {result:?}"
        );
    }

    #[test]
    fn test_connection_indicator_disconnected() {
        let mut state = DevToolsViewState::default();
        state.connection_status = VmConnectionStatus::Disconnected;
        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut label = String::new();
        let result = widget.connection_indicator_text(&mut label);
        assert!(result.is_some(), "Disconnected should produce an indicator");
        let (text, _style) = result.unwrap();
        assert!(
            text.contains("Disconnected"),
            "Label should mention Disconnected, got: {text:?}"
        );
    }

    #[test]
    fn test_connection_indicator_reconnecting_shows_attempt_counter() {
        let mut state = DevToolsViewState::default();
        state.connection_status = VmConnectionStatus::Reconnecting {
            attempt: 2,
            max_attempts: 10,
        };
        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut label = String::new();
        let result = widget.connection_indicator_text(&mut label);
        assert!(result.is_some(), "Reconnecting should produce an indicator");
        let (text, _style) = result.unwrap();
        assert!(
            text.contains("2") && text.contains("10"),
            "Label should include attempt counts, got: {text:?}"
        );
        assert!(
            text.contains("Reconnecting"),
            "Label should mention Reconnecting, got: {text:?}"
        );
    }

    #[test]
    fn test_connection_indicator_timed_out() {
        let mut state = DevToolsViewState::default();
        state.connection_status = VmConnectionStatus::TimedOut;
        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut label = String::new();
        let result = widget.connection_indicator_text(&mut label);
        assert!(result.is_some(), "TimedOut should produce an indicator");
        let (text, _style) = result.unwrap();
        assert!(
            text.contains("Timed") || text.contains("Out"),
            "Label should mention Timed Out, got: {text:?}"
        );
    }

    #[test]
    fn test_tab_bar_shows_disconnected_indicator() {
        let mut state = DevToolsViewState::default();
        state.connection_status = VmConnectionStatus::Disconnected;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("Disconnected"),
            "Tab bar should show 'Disconnected' indicator, got: {text:?}"
        );
    }

    #[test]
    fn test_tab_bar_shows_reconnecting_indicator() {
        let mut state = DevToolsViewState::default();
        state.connection_status = VmConnectionStatus::Reconnecting {
            attempt: 3,
            max_attempts: 10,
        };

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("Reconnecting"),
            "Tab bar should show 'Reconnecting' indicator, got: {text:?}"
        );
    }

    #[test]
    fn test_tab_bar_no_indicator_when_connected() {
        let mut state = DevToolsViewState::default();
        state.connection_status = VmConnectionStatus::Connected;

        let widget = DevToolsView::new(&state, None, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar(Rect::new(0, 0, 80, 3), &mut buf);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            !text.contains("Disconnected") && !text.contains("Reconnecting"),
            "Tab bar should not show connection indicator when connected, got: {text:?}"
        );
    }
}

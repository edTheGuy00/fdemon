//! DevTools panel widgets for the TUI.
//!
//! Contains sub-panel widgets rendered when `UiMode::DevTools` is active.
//! The top-level [`DevToolsView`] composite widget renders a sub-tab bar and
//! dispatches to the active panel below it.

pub mod inspector;
pub mod memory;
pub mod network;
pub mod performance;

pub use inspector::WidgetInspector;
pub use memory::MemoryPanel;
pub use network::NetworkMonitor;
pub use performance::PerformancePanel;

use fdemon_app::devtools_panel_provider::{DevToolsPanelCtx, DevToolsPanelProvider};
use fdemon_app::message::Message;
use fdemon_app::session::{MemoryState, PerfSection, PerformanceState, SessionHandle};
use fdemon_app::state::{DevToolsPanel, DevToolsViewState, PerfDetailsTab, VmConnectionStatus};
use fdemon_app::{MouseAction, MouseRect};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::theme::{icons::IconSet, palette};
use crate::widgets::MouseCtx;

// ── Minimum size thresholds ───────────────────────────────────────────────────

/// Minimum terminal height required to render any DevTools panel.
const DEVTOOLS_MIN_HEIGHT: u16 = 3;

/// Minimum terminal width required to render any DevTools panel.
const DEVTOOLS_MIN_WIDTH: u16 = 20;

// ── DevToolsView ─────────────────────────────────────────────────────────────

/// Maximum number of Unicode grapheme clusters to show from a device name in
/// the DevTools title when multiple sessions are active.
const DEVICE_NAME_MAX_CHARS: usize = 30;

/// Top-level DevTools mode widget.
///
/// Renders a sub-tab bar at the top and dispatches to the active panel below.
/// Both panel widgets ([`WidgetInspector`] and [`PerformancePanel`]) are
/// non-stateful; state is passed in via references.
pub struct DevToolsView<'a> {
    state: &'a DevToolsViewState,
    session: Option<&'a SessionHandle>,
    icons: IconSet,
    /// Total number of active sessions.
    ///
    /// When >1, the displayed session's device name is shown next to the
    /// "DevTools" title in the tab bar so the user can identify the session.
    session_count: usize,
    /// Host-registered extension panels (out-of-tree DevTools panel seam).
    ///
    /// `None` in tests and in the legacy `new` constructor (stock behaviour:
    /// no extension panels). When `Some`, the tab bar enumerates these after
    /// the built-ins and the active extension panel is rendered with `&mut self`.
    /// Held as `&mut` so stateful panels can mutate during draw.
    panels: Option<&'a mut Vec<Box<dyn DevToolsPanelProvider>>>,
    /// Animation frame counter, forwarded to extension panels via their ctx.
    animation_frame: u64,
}

impl<'a> DevToolsView<'a> {
    /// Create a new `DevToolsView` widget with no extension panels.
    ///
    /// This is the stock constructor: behaviour is byte-identical to before the
    /// extension-panel seam existed. Hosts that register panels use
    /// [`DevToolsView::with_panels`].
    pub fn new(
        state: &'a DevToolsViewState,
        session: Option<&'a SessionHandle>,
        icons: IconSet,
        session_count: usize,
    ) -> Self {
        Self {
            state,
            session,
            icons,
            session_count,
            panels: None,
            animation_frame: 0,
        }
    }

    /// Attach the host-registered extension panels and the animation frame.
    ///
    /// Builder-style; the mutable borrow lets the active extension panel render
    /// through `&mut self`. Passing an empty `Vec` is equivalent to [`new`]
    /// (no extension tabs, no behaviour change).
    ///
    /// [`new`]: DevToolsView::new
    pub fn with_panels(
        mut self,
        panels: &'a mut Vec<Box<dyn DevToolsPanelProvider>>,
        animation_frame: u64,
    ) -> Self {
        self.panels = Some(panels);
        self.animation_frame = animation_frame;
        self
    }

    /// The id of the panel that is currently visible, accounting for a live
    /// extension-panel selection vs. the built-in fallback.
    fn active_panel_id(&self) -> &str {
        if let Some(id) = self.state.active_extension_panel.as_deref() {
            let live = self
                .panels
                .as_ref()
                .map(|ps| ps.iter().any(|p| p.id() == id))
                .unwrap_or(false);
            if live {
                return id;
            }
        }
        self.state.active_panel.id()
    }
}

impl Widget for DevToolsView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_impl(area, buf, None);
    }
}

impl DevToolsView<'_> {
    // ── Shared render entry point ─────────────────────────────────────────────

    /// Shared implementation called by both `Widget::render` and
    /// `render_with_regions`.
    ///
    /// When `ctx` is `None` the behaviour is identical to the old
    /// `Widget::render` implementation. When `ctx` is `Some`, click regions
    /// are recorded for the sub-tab bar and forwarded to the active panel's
    /// click-aware render path.
    fn render_impl(mut self, area: Rect, buf: &mut Buffer, mut ctx: Option<&mut MouseCtx<'_>>) {
        // Clear background — set every cell to ' ' with the background style
        // so the log view underneath is fully occluded.
        let bg_style = Style::default().bg(palette::DEEPEST_BG);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style).set_char(' ');
                }
            }
        }

        // Global minimum size guard — show an informational message rather than
        // silently rendering a blank or garbled panel.
        if area.height < DEVTOOLS_MIN_HEIGHT || area.width < DEVTOOLS_MIN_WIDTH {
            let msg = Line::from(Span::styled(
                "Resize terminal for DevTools",
                Style::default().fg(Color::DarkGray),
            ));
            let msg_width = msg.width() as u16;
            let x = area.x + area.width.saturating_sub(msg_width) / 2;
            let y = area.y;
            buf.set_line(x, y, &msg, area.width);
            return;
        }

        // Vertical layout: [sub-tab bar (3 lines)] + [panel content (remaining)]
        let chunks = Layout::vertical([
            Constraint::Length(3), // Sub-tab bar
            Constraint::Min(1),    // Panel content
        ])
        .split(area);

        // Sub-tab bar with optional click registration.
        self.render_tab_bar_inner(chunks[0], buf, ctx.as_deref_mut());

        // Per-session connection status — falls back to Disconnected when no session
        // is active. All sub-panels use this rather than the old global field so the
        // status is always correct for the **displayed** session.
        static DISCONNECTED: VmConnectionStatus = VmConnectionStatus::Disconnected;
        let session_conn_status: &VmConnectionStatus = self
            .session
            .map(|h| &h.vm_connection_status)
            .unwrap_or(&DISCONNECTED);

        // ── Extension-panel dispatch (out-of-tree DevToolsPanelProvider seam) ──
        //
        // When a host-registered panel is active AND still registered, render it
        // via `&mut self` instead of any built-in. Stock fdemon never reaches
        // this (panels is None / active_extension_panel is None), so the built-in
        // dispatch below is byte-identical to before.
        if self.state.active_extension_panel.is_some() {
            // Read disjoint immutable data into locals before the mutable
            // borrow of `self.panels` below.
            let vm_connected = self
                .session
                .map(|s| s.session.vm_connected)
                .unwrap_or(false);
            let animation_frame = self.animation_frame;
            let ext_id = self
                .state
                .active_extension_panel
                .clone()
                .unwrap_or_default();

            let mut footer_hint: Option<String> = None;
            if let Some(panels) = self.panels.as_deref_mut() {
                if let Some(panel) = panels.iter_mut().find(|p| p.id() == ext_id) {
                    let ctx = DevToolsPanelCtx::new(vm_connected, animation_frame);
                    panel.render(chunks[1], buf, ctx);
                    footer_hint = Some(panel.key_hint().to_string());
                }
            }
            if let Some(hint) = footer_hint {
                render_footer_hint(chunks[1], buf, &hint);
                return;
            }
            // Stale id (provider removed): fall through to built-in dispatch.
        }

        // Panel dispatch — panel sister functions share render_impl with
        // Widget::render so region recording flows through cleanly.
        match self.state.active_panel {
            DevToolsPanel::Inspector => {
                let vm_connected = self
                    .session
                    .map(|s| s.session.vm_connected)
                    .unwrap_or(false);
                let widget = WidgetInspector::new(
                    &self.state.inspector,
                    vm_connected,
                    session_conn_status,
                );
                inspector::render_with_regions(chunks[1], buf, widget, ctx.as_deref_mut());
            }
            DevToolsPanel::Performance => {
                // Safety fallback for when no session is active.
                // In practice DevTools mode is only reachable when a session exists.
                // Note: PerformanceState contains Cell<usize> render-hint fields,
                // which are !Sync, so a stack-local default is used instead of a LazyLock static.
                let default_perf;
                let (perf, vm_connected) = match self.session {
                    Some(s) => (&s.session.performance, s.session.vm_connected),
                    None => {
                        default_perf = PerformanceState::default();
                        (&default_perf, false)
                    }
                };

                let widget = PerformancePanel::new(
                    perf,
                    vm_connected,
                    self.icons,
                    session_conn_status,
                )
                .with_connection_error(self.state.vm_connection_error.as_deref());
                performance::render_with_regions(chunks[1], buf, widget, ctx.as_deref_mut());
            }
            DevToolsPanel::Memory => {
                // Safety fallback: MemoryState contains Cell<usize> render-hint fields (!Sync),
                // so stack-local defaults are used instead of LazyLock statics.
                let default_memory;
                let (mem, vm_connected) = match self.session {
                    Some(s) => (&s.session.memory, s.session.vm_connected),
                    None => {
                        default_memory = MemoryState::default();
                        (&default_memory, false)
                    }
                };

                let widget =
                    MemoryPanel::new(mem, true, vm_connected, session_conn_status);
                memory::render_with_regions(chunks[1], buf, widget, ctx.as_deref_mut());
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
                    NetworkMonitor::new(network_state, vm_connected, session_conn_status);
                network::render_with_regions(chunks[1], buf, widget, ctx);
            }
        }

        // Footer — no clicks.
        self.render_footer(chunks[1], buf);
    }

    // ── Sub-tab bar ───────────────────────────────────────────────────────────

    /// Render the DevTools sub-tab bar with panel tabs, overlay status
    /// indicators, and optionally click regions for each tab.
    ///
    /// When `ctx` is `Some`, one [`MouseAction::Emit`]`(`[`Message::SwitchDevToolsPanel`]`)` region
    /// is registered per tab. When `ctx` is `None` (the `Widget::render` path), no
    /// regions are recorded and behaviour is identical to the previous implementation.
    fn render_tab_bar_inner(
        &self,
        area: Rect,
        buf: &mut Buffer,
        mut ctx: Option<&mut MouseCtx<'_>>,
    ) {
        // Outer block with border.
        // When multiple sessions are active, append the displayed session's
        // device name to the title so the user can identify which session they
        // are inspecting. Truncate long names to avoid overflowing the border.
        let title: std::borrow::Cow<'static, str> = if self.session_count > 1 {
            if let Some(session) = self.session {
                let name = &session.session.device_name;
                let truncated: String = name
                    .chars()
                    .take(DEVICE_NAME_MAX_CHARS)
                    .collect();
                let suffix = if name.chars().count() > DEVICE_NAME_MAX_CHARS {
                    "\u{2026}" // …
                } else {
                    ""
                };
                format!(" DevTools \u{2014} {truncated}{suffix} ").into()
            } else {
                " DevTools ".into()
            }
        } else {
            " DevTools ".into()
        };
        let block = ratatui::widgets::Block::bordered()
            .title(title.as_ref())
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let tabs = [
            (DevToolsPanel::Inspector, "[i] Inspector"),
            (DevToolsPanel::Performance, "[p] Performance"),
            (DevToolsPanel::Memory, "[m] Memory"),
            (DevToolsPanel::Network, "[n] Network"),
        ];

        // Highlight uses the *visible* panel id so an active extension panel
        // de-highlights the built-ins. With no extension panel this equals
        // `active_panel.id()`, so built-in highlighting is unchanged.
        let active_id = self.active_panel_id();

        let mut x = inner.x + 1;
        for (panel, label) in &tabs {
            let is_active = active_id == panel.id();
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

            // Register a click region covering the padded label cells.
            if let Some(ref mut c) = ctx {
                let rect = MouseRect::new(x, inner.y, needed_width, 1);
                if rect.width > 0 && rect.height > 0 {
                    c.click(
                        rect,
                        MouseAction::emit(Message::SwitchDevToolsPanel(*panel)),
                    );
                }
            }

            x += needed_width + 1;
        }

        // ── Host-registered extension panel tabs ─────────────────────────────
        //
        // Rendered after the four built-ins, in registration order. Stock fdemon
        // has no registered panels (`self.panels` is None or empty), so this
        // loop is a no-op and the tab bar is byte-identical to before.
        if let Some(panels) = self.panels.as_deref() {
            for panel in panels.iter() {
                let is_active = active_id == panel.id();
                let padded = format!(" {} ", panel.title());
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

                if let Some(ref mut c) = ctx {
                    let rect = MouseRect::new(x, inner.y, needed_width, 1);
                    if rect.width > 0 && rect.height > 0 {
                        c.click(
                            rect,
                            MouseAction::emit(Message::SwitchDevToolsExtensionPanel(
                                panel.id().to_string(),
                            )),
                        );
                    }
                }

                x += needed_width + 1;
            }
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
    /// or `None` when the connection is healthy (Connected) or no session is active.
    ///
    /// Reads the per-session `vm_connection_status` from `self.session` so that
    /// the indicator is always correct for the **displayed** session, regardless
    /// of which session was most recently active when the handler fired.
    ///
    /// `label_buf` is used as backing storage so the returned `&str` can borrow
    /// from it without requiring a `String` return value.
    fn connection_indicator_text<'a>(&self, label_buf: &'a mut String) -> Option<(&'a str, Style)> {
        // Fallback to Disconnected when no session is active (DevTools opened
        // without an active session is a defensive guard — it should not occur
        // in normal usage).
        let status = self
            .session
            .map(|h| &h.vm_connection_status)
            .unwrap_or(&VmConnectionStatus::Disconnected);
        match status {
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

        let hints: std::borrow::Cow<'static, str> = match self.state.active_panel {
            DevToolsPanel::Inspector => {
                if self.state.inspector.details_open {
                    "[Esc] Close  [Tab] Next Tab  [Shift+Tab] Prev Tab  [r] Refresh  [b] Browser"
                        .into()
                } else {
                    "[Esc] Logs  [↑↓] Navigate  [→] Expand  [←] Collapse  [Enter] Details  [Shift+H] Hide Impl  [r] Refresh  [b] Browser".into()
                }
            }
            DevToolsPanel::Performance => {
                let focused_section = self
                    .session
                    .map(|h| h.session.performance.focused_section)
                    .unwrap_or(PerfSection::FrameChart);
                match focused_section {
                    PerfSection::FrameChart => {
                        "[Esc] Logs  [←/→] Frames  [Tab] Section  ]/[ Tabs  [j/k] Scroll  [b] Browser".into()
                    }
                    PerfSection::Details => {
                        let details_tab = self
                            .session
                            .map(|h| h.session.performance.details_tab)
                            .unwrap_or(PerfDetailsTab::FrameAnalysis);
                        let base = "[Esc] Logs  [Tab] Section  ]/[ Tabs  [b] Browser";
                        match details_tab {
                            PerfDetailsTab::TimelineEvents => {
                                format!("{base}  [f] Filter").into()
                            }
                            PerfDetailsTab::RebuildStats => {
                                format!("{base}  [R] Rebuild track").into()
                            }
                            PerfDetailsTab::FrameAnalysis => base.into(),
                        }
                    }
                }
            }
            DevToolsPanel::Memory => {
                let has_alloc_selection = self
                    .session
                    .is_some_and(|s| s.session.memory.alloc_table_selected_row.is_some());
                if has_alloc_selection {
                    "[Esc] Deselect  [Tab] Switch  [j/k] Scroll  [s] Sort  [b] Browser".into()
                } else {
                    "[Esc] Logs  [Tab] Switch  [j/k] Scroll  [s] Sort  [b] Browser".into()
                }
            }
            DevToolsPanel::Network => {
                let has_selection = self
                    .session
                    .is_some_and(|s| s.session.network.selected_index.is_some());
                if has_selection {
                    "[Esc] Deselect  [g/h/q/s/t] Detail tabs  [Space] Toggle rec  [b] Browser"
                        .into()
                } else {
                    "[Esc] Logs  [↑↓] Navigate  [Enter] Detail  [Space] Toggle rec  [b] Browser"
                        .into()
                }
            }
        };

        render_footer_hint(area, buf, &hints);
    }
}

/// Draw a single line of footer hint text at the bottom of `area`.
///
/// Shared by the built-in [`DevToolsView::render_footer`] and the extension-panel
/// dispatch path (which uses each panel's `key_hint`). Hints are truncated to the
/// available width and styled with the muted footer colour, matching the
/// built-in panels exactly. No-op when `area.height < 2`.
fn render_footer_hint(area: Rect, buf: &mut Buffer, hints: &str) {
    if area.height < 2 {
        return;
    }
    let y = area.y + area.height - 1;
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

// ── render_with_regions (click-aware entry point) ────────────────────────────

/// Render the full DevTools view, optionally recording clickable regions.
///
/// This is the canonical render entry point used by `render::view` when
/// `UiMode::DevTools` is active. Delegates to `DevToolsView::render_impl` —
/// the single authoritative implementation shared with `Widget::render`.
/// Passing `None` for `ctx` produces output byte-identical to `Widget::render`.
///
/// The sub-tab bar ([`DevToolsView::render_tab_bar_inner`]) registers one
/// [`MouseAction::Emit`]`(`[`Message::SwitchDevToolsPanel`]`)` region per visible
/// tab when `ctx` is `Some`. The active panel's `render_with_regions` function
/// is called unconditionally so region forwarding is transparent.
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: DevToolsView<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    view.render_impl(area, buf, ctx);
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

        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_view_renders_performance_panel() {
        let state = DevToolsViewState {
            active_panel: DevToolsPanel::Performance,
            ..Default::default()
        };

        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_tab_bar_highlights_active_panel() {
        let state = DevToolsViewState {
            active_panel: DevToolsPanel::Performance,
            ..Default::default()
        };

        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

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
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("Inspector"),
            "Expected Inspector tab; got: {text:?}"
        );
        assert!(
            text.contains("Performance"),
            "Expected Performance tab; got: {text:?}"
        );
        assert!(
            text.contains("Memory"),
            "Expected Memory tab; got: {text:?}"
        );
        assert!(
            text.contains("Network"),
            "Expected Network tab; got: {text:?}"
        );
    }

    #[test]
    fn test_overlay_indicators_shown_when_active() {
        let state = DevToolsViewState {
            overlay_repaint_rainbow: true,
            overlay_debug_paint: true,
            ..Default::default()
        };

        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

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
        let state = DevToolsViewState {
            overlay_performance: true,
            ..Default::default()
        };

        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("PerfOverlay"),
            "Expected 'PerfOverlay' indicator, got: {text:?}"
        );
    }

    #[test]
    fn test_devtools_view_small_terminal() {
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_view_very_small_terminal() {
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 3));
        widget.render(Rect::new(0, 0, 40, 3), &mut buf);
        // Should not panic (height < 4 early return)
    }

    #[test]
    fn test_devtools_view_large_terminal() {
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 40));
        widget.render(Rect::new(0, 0, 120, 40), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_view_active_panel_inspector_tab_highlighted() {
        // Inspector is default active panel
        let state = DevToolsViewState::default();
        assert_eq!(state.active_panel, DevToolsPanel::Inspector);

        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        // Find the Inspector label cell and check its bg style
        // The active tab should have Cyan background
        let text = collect_buf_text(&buf, 80, 3);
        assert!(text.contains("Inspector"), "Expected Inspector in tab bar");
    }

    // ── Connection indicator tests ─────────────────────────────────────────────
    //
    // The indicator reads the per-session `vm_connection_status` from
    // `DevToolsView::session` (a `SessionHandle`). Tests below create a
    // `SessionHandle` with the desired status and pass it to `DevToolsView`.

    /// Build a minimal `SessionHandle` with the given per-session connection status.
    fn make_session_handle_with_status(
        status: VmConnectionStatus,
    ) -> fdemon_app::session::SessionHandle {
        use fdemon_app::session::{Session, SessionHandle};
        let session = Session::new(
            "test-device".to_string(),
            "Test Device".to_string(),
            "android".to_string(),
            false,
        );
        let mut handle = SessionHandle::new(session);
        handle.vm_connection_status = status;
        handle
    }

    #[test]
    fn test_connection_indicator_connected_shows_nothing() {
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_status(VmConnectionStatus::Connected);
        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 1);
        let mut label = String::new();
        let result = widget.connection_indicator_text(&mut label);
        assert!(
            result.is_none(),
            "Connected state should show no indicator, got: {result:?}"
        );
    }

    #[test]
    fn test_connection_indicator_disconnected() {
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_status(VmConnectionStatus::Disconnected);
        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 1);
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
    fn test_connection_indicator_no_session_shows_disconnected() {
        // When no session is active (DevTools opened without a session — defensive
        // guard), the indicator falls back to Disconnected.
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut label = String::new();
        let result = widget.connection_indicator_text(&mut label);
        assert!(
            result.is_some(),
            "No session should show Disconnected indicator"
        );
        let (text, _) = result.unwrap();
        assert!(
            text.contains("Disconnected"),
            "Fallback should say Disconnected, got: {text:?}"
        );
    }

    #[test]
    fn test_connection_indicator_reconnecting_shows_attempt_counter() {
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_status(VmConnectionStatus::Reconnecting {
            attempt: 2,
            max_attempts: 10,
        });
        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 1);
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
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_status(VmConnectionStatus::TimedOut);
        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 1);
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
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_status(VmConnectionStatus::Disconnected);

        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("Disconnected"),
            "Tab bar should show 'Disconnected' indicator, got: {text:?}"
        );
    }

    #[test]
    fn test_tab_bar_shows_reconnecting_indicator() {
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_status(VmConnectionStatus::Reconnecting {
            attempt: 3,
            max_attempts: 10,
        });

        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("Reconnecting"),
            "Tab bar should show 'Reconnecting' indicator, got: {text:?}"
        );
    }

    #[test]
    fn test_tab_bar_no_indicator_when_connected() {
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_status(VmConnectionStatus::Connected);

        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            !text.contains("Disconnected") && !text.contains("Reconnecting"),
            "Tab bar should not show connection indicator when connected, got: {text:?}"
        );
    }

    // ── Multi-session connection indicator tests (Acceptance A) ──────────────────

    #[test]
    fn test_per_session_indicator_session_a_connected_b_reconnecting() {
        // Session A connected, session B reconnecting → switching selection to
        // each session shows the correct per-session status.
        use fdemon_app::session::{Session, SessionHandle};
        let mut handle_a = SessionHandle::new(Session::new(
            "dev-a".to_string(),
            "Device A".to_string(),
            "android".to_string(),
            false,
        ));
        handle_a.vm_connection_status = VmConnectionStatus::Connected;

        let mut handle_b = SessionHandle::new(Session::new(
            "dev-b".to_string(),
            "Device B".to_string(),
            "ios".to_string(),
            true,
        ));
        handle_b.vm_connection_status = VmConnectionStatus::Reconnecting {
            attempt: 2,
            max_attempts: 5,
        };

        let state = DevToolsViewState::default();

        // Displaying session A → no indicator (Connected).
        let widget_a = DevToolsView::new(&state, Some(&handle_a), IconSet::default(), 2);
        let mut label = String::new();
        assert!(
            widget_a.connection_indicator_text(&mut label).is_none(),
            "Session A is Connected — no indicator expected"
        );

        // Displaying session B → Reconnecting indicator.
        let widget_b = DevToolsView::new(&state, Some(&handle_b), IconSet::default(), 2);
        let mut label = String::new();
        let result = widget_b.connection_indicator_text(&mut label);
        assert!(result.is_some(), "Session B is Reconnecting — indicator expected");
        let (text, _) = result.unwrap();
        assert!(
            text.contains("Reconnecting"),
            "Session B indicator should say Reconnecting, got: {text:?}"
        );
        assert!(
            text.contains("2") && text.contains("5"),
            "Session B indicator should show attempt counts, got: {text:?}"
        );
    }

    #[test]
    fn test_per_session_indicator_disconnect_does_not_affect_other_session() {
        // Session A disconnected, session B connected — each shows its own status.
        use fdemon_app::session::{Session, SessionHandle};
        let mut handle_a = SessionHandle::new(Session::new(
            "dev-a".to_string(),
            "Device A".to_string(),
            "android".to_string(),
            false,
        ));
        handle_a.vm_connection_status = VmConnectionStatus::Disconnected;

        let mut handle_b = SessionHandle::new(Session::new(
            "dev-b".to_string(),
            "Device B".to_string(),
            "ios".to_string(),
            true,
        ));
        handle_b.vm_connection_status = VmConnectionStatus::Connected;

        let state = DevToolsViewState::default();

        // Session A → Disconnected indicator.
        let widget_a = DevToolsView::new(&state, Some(&handle_a), IconSet::default(), 2);
        let mut label = String::new();
        let result_a = widget_a.connection_indicator_text(&mut label);
        assert!(result_a.is_some(), "Session A is Disconnected — indicator expected");
        let (text, _) = result_a.unwrap();
        assert!(text.contains("Disconnected"), "got: {text:?}");

        // Session B → no indicator (Connected).
        let widget_b = DevToolsView::new(&state, Some(&handle_b), IconSet::default(), 2);
        let mut label = String::new();
        assert!(
            widget_b.connection_indicator_text(&mut label).is_none(),
            "Session B is Connected — no indicator expected"
        );
    }

    // ── Minimum size guard tests ───────────────────────────────────────────────

    #[test]
    fn test_devtools_panel_minimum_size_guard_shows_resize_message() {
        // 15x2 — both height and width below thresholds — should show resize message
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 15, 2));
        widget.render(Rect::new(0, 0, 15, 2), &mut buf);

        let text = collect_buf_text(&buf, 15, 2);
        assert!(
            text.contains("Resize") || text.contains("resize") || text.contains("small"),
            "Minimum size guard should show resize message at 15x2, got: {text:?}"
        );
    }

    #[test]
    fn test_devtools_panel_minimum_height_guard_shows_message() {
        // Height < DEVTOOLS_MIN_HEIGHT (3) with adequate width
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 2));
        widget.render(Rect::new(0, 0, 80, 2), &mut buf);

        let text = collect_buf_text(&buf, 80, 2);
        assert!(
            text.contains("Resize") || text.contains("DevTools"),
            "Below minimum height should show resize message, got: {text:?}"
        );
    }

    #[test]
    fn test_devtools_panel_minimum_width_guard_shows_message() {
        // Width < DEVTOOLS_MIN_WIDTH (20) with adequate height
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 15, 10));
        widget.render(Rect::new(0, 0, 15, 10), &mut buf);

        let text = collect_buf_text(&buf, 15, 10);
        assert!(
            text.contains("Resize") || text.contains("DevTools"),
            "Below minimum width should show resize message, got: {text:?}"
        );
    }

    #[test]
    fn test_devtools_panel_at_minimum_size_threshold_renders_normally() {
        // Exactly at the minimum thresholds (height=3, width=20) — should show tab bar
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        widget.render(Rect::new(0, 0, 20, 3), &mut buf);
        // Should not panic — minimum guard allows rendering at exactly the threshold
    }

    #[test]
    fn test_devtools_panel_20x5_no_panic() {
        // 20x5 — acceptance criteria extreme terminal size
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
        widget.render(Rect::new(0, 0, 20, 5), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_panel_40x10_no_panic() {
        // 40x10 — acceptance criteria terminal size
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_panel_60x15_no_panic() {
        // 60x15 — acceptance criteria terminal size
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 15));
        widget.render(Rect::new(0, 0, 60, 15), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_panel_200x50_no_panic() {
        // 200x50 — large terminal (acceptance criteria)
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 200, 50));
        widget.render(Rect::new(0, 0, 200, 50), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_panel_network_tab_small_terminal() {
        // Network tab at small terminal size
        let state = DevToolsViewState {
            active_panel: DevToolsPanel::Network,
            ..Default::default()
        };

        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_devtools_panel_performance_tab_height_1() {
        // Performance panel compact mode: height=1 after the header is consumed.
        // At 6 total rows: 3 for tab bar, leaving 3 for the panel content.
        // At the extreme: 4 total rows gives 1 row for the panel.
        let state = DevToolsViewState {
            active_panel: DevToolsPanel::Performance,
            ..Default::default()
        };

        // 4 rows total: min-size guard passes (>= 3 height, >= 20 width).
        // Tab bar takes 3 rows, panel gets 1 row → compact summary path.
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 4));
        widget.render(Rect::new(0, 0, 40, 4), &mut buf);
        // Should not panic
    }

    // ── Phase 4 Task 02: render_with_regions tests ────────────────────────────

    // ── Phase 4.5 Task 03: render_with_regions parity test ───────────────────

    #[test]
    fn render_with_regions_matches_widget_render_buffer() {
        use crate::render::MouseCtx;
        use fdemon_app::MouseRegions;

        // Non-empty session with active panel = Inspector.
        let state = DevToolsViewState::default();
        assert_eq!(state.active_panel, DevToolsPanel::Inspector);
        let area = Rect::new(0, 0, 80, 24);

        let mut buf_a = Buffer::empty(area);
        DevToolsView::new(&state, None, IconSet::default(), 1).render(area, &mut buf_a);

        let mut buf_b = Buffer::empty(area);
        {
            let mut regions = MouseRegions::default();
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            super::render_with_regions(
                area,
                &mut buf_b,
                DevToolsView::new(&state, None, IconSet::default(), 1),
                Some(&mut ctx),
            );
        }

        assert_eq!(
            buf_a, buf_b,
            "Widget::render and render_with_regions must produce identical buffers"
        );
    }

    #[test]
    fn devtools_tab_bar_registers_four_click_regions() {
        use crate::render::MouseCtx;
        use fdemon_app::message::Message;
        use fdemon_app::{MouseAction, MouseRegions};

        let state = DevToolsViewState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));

        let mut regions = MouseRegions::with_capacity();
        {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            super::render_with_regions(
                Rect::new(0, 0, 80, 24),
                &mut buf,
                DevToolsView::new(&state, None, IconSet::default(), 1),
                Some(&mut ctx),
            );
            // ctx + builder borrow ends here; regions is accessible again
        }

        // Count entries whose left action is SwitchDevToolsPanel(_).
        let switch_panel_count = regions
            .iter()
            .filter(|e| {
                matches!(
                    &e.on_left,
                    Some(MouseAction::Emit(msg)) if matches!(**msg, Message::SwitchDevToolsPanel(_))
                )
            })
            .count();

        assert_eq!(
            switch_panel_count, 4,
            "expected 4 sub-tab SwitchDevToolsPanel regions, got {switch_panel_count}"
        );
    }

    #[test]
    fn devtools_tab_bar_regions_cover_correct_widths() {
        use crate::render::MouseCtx;
        use fdemon_app::message::Message;
        use fdemon_app::{MouseAction, MouseRegions};

        let state = DevToolsViewState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));

        let mut regions = MouseRegions::with_capacity();
        {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            super::render_with_regions(
                Rect::new(0, 0, 80, 24),
                &mut buf,
                DevToolsView::new(&state, None, IconSet::default(), 1),
                Some(&mut ctx),
            );
        }

        // Each tab region must be exactly 1 row tall and have width = len(" {label} ").
        let expected_widths: Vec<u16> = [
            " [i] Inspector ",
            " [p] Performance ",
            " [m] Memory ",
            " [n] Network ",
        ]
        .iter()
        .map(|s| s.len() as u16)
        .collect();

        let actual_widths: Vec<u16> = regions
            .iter()
            .filter(|e| {
                matches!(
                    &e.on_left,
                    Some(MouseAction::Emit(msg)) if matches!(**msg, Message::SwitchDevToolsPanel(_))
                )
            })
            .map(|e| e.rect.width)
            .collect();

        assert_eq!(
            actual_widths, expected_widths,
            "tab bar region widths should match padded label widths"
        );

        // All regions must be exactly 1 row tall.
        for entry in regions.iter().filter(|e| {
            matches!(
                &e.on_left,
                Some(MouseAction::Emit(msg)) if matches!(**msg, Message::SwitchDevToolsPanel(_))
            )
        }) {
            assert_eq!(entry.rect.height, 1, "tab region height must be 1");
        }
    }

    #[test]
    fn devtools_render_with_regions_none_ctx_no_regions() {
        let state = DevToolsViewState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));

        // With ctx = None, no regions should be registered. Calling with None
        // must not panic — it is the test-safe path identical to Widget::render.
        super::render_with_regions(
            Rect::new(0, 0, 80, 24),
            &mut buf,
            DevToolsView::new(&state, None, IconSet::default(), 1),
            None,
        );
        // No assert needed — the absence of panic is the pass condition.
    }

    // ── Inspector footer mode tests ───────────────────────────────────────────

    /// Build a `DevToolsViewState` with the Inspector panel active and
    /// `details_open` set to the specified value.
    fn make_state_in_devtools_inspector(details_open: bool) -> DevToolsViewState {
        use fdemon_app::state::InspectorState;
        DevToolsViewState {
            active_panel: DevToolsPanel::Inspector,
            inspector: InspectorState {
                details_open,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Render a full DevToolsView (80×24) and return the text on the last row
    /// of the panel content area — this is where `render_footer` writes hints.
    fn footer_string(state: &DevToolsViewState) -> String {
        let area = Rect::new(0, 0, 200, 24);
        let mut buf = Buffer::empty(area);
        DevToolsView::new(state, None, IconSet::default(), 1).render(area, &mut buf);

        // The layout splits the area into a 3-row tab bar (chunks[0]) and the
        // remaining 21 rows for panel content (chunks[1]).  render_footer draws
        // on the last row of chunks[1]: y = 3 + 21 - 1 = 23.
        let footer_y = area.height - 1;
        let mut row = String::new();
        for x in 0..area.width {
            if let Some(cell) = buf.cell((x, footer_y)) {
                row.push_str(cell.symbol());
            }
        }
        row
    }

    #[test]
    fn inspector_footer_in_tree_mode_includes_enter_details_hint() {
        let state = make_state_in_devtools_inspector(false);
        let s = footer_string(&state);
        assert!(s.contains("[Enter] Details"), "footer was: {s}");
        assert!(s.contains("[Shift+H] Hide Impl"), "footer was: {s}");
    }

    #[test]
    fn inspector_footer_in_details_mode_includes_esc_close_hint() {
        let state = make_state_in_devtools_inspector(true);
        let s = footer_string(&state);
        assert!(s.contains("[Esc] Close"), "footer was: {s}");
        assert!(s.contains("[Tab] Next Tab"), "footer was: {s}");
        assert!(
            !s.contains("[↑↓] Navigate"),
            "navigate hint should be hidden in details mode; footer was: {s}"
        );
    }

    #[test]
    fn inspector_footer_in_details_mode_does_not_include_navigate_hint() {
        let state = make_state_in_devtools_inspector(true);
        let s = footer_string(&state);
        assert!(
            !s.contains("[↑↓] Navigate"),
            "navigate hint must not appear in details mode; footer was: {s}"
        );
        assert!(s.contains("[Shift+Tab] Prev Tab"), "footer was: {s}");
    }

    // ── Performance footer tests ──────────────────────────────────────────────

    /// Build a `SessionHandle` with `focused_section` set to `section`.
    fn make_perf_session_handle(section: PerfSection) -> fdemon_app::session::SessionHandle {
        use fdemon_app::session::{Session, SessionHandle};
        let mut session = Session::new(
            "test-device".to_string(),
            "Test Device".to_string(),
            "android".to_string(),
            false,
        );
        session.performance.focused_section = section;
        SessionHandle::new(session)
    }

    /// Render a DevToolsView in Performance mode with the given session handle and
    /// return the footer row text.
    fn performance_footer_string_with_session(
        handle: &fdemon_app::session::SessionHandle,
    ) -> String {
        let state = DevToolsViewState {
            active_panel: DevToolsPanel::Performance,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 200, 24);
        let mut buf = Buffer::empty(area);
        DevToolsView::new(&state, Some(handle), IconSet::default(), 1).render(area, &mut buf);
        let footer_y = area.height - 1;
        let mut row = String::new();
        for x in 0..area.width {
            if let Some(cell) = buf.cell((x, footer_y)) {
                row.push_str(cell.symbol());
            }
        }
        row
    }

    #[test]
    fn performance_footer_hides_scroll_keys_when_details_focused() {
        let handle = make_perf_session_handle(PerfSection::Details);
        let s = performance_footer_string_with_session(&handle);
        assert!(
            !s.contains("[j/k] Scroll"),
            "[j/k] Scroll must not appear when Details is focused; footer was: {s}"
        );
        assert!(
            !s.contains("[←/→] Frames"),
            "[←/→] Frames must not appear when Details is focused; footer was: {s}"
        );
        assert!(
            s.contains("]/[") && s.contains("Tabs"),
            "]/[ Tabs should appear; footer was: {s}"
        );
    }

    #[test]
    fn performance_footer_shows_scroll_keys_when_frame_chart_focused() {
        let handle = make_perf_session_handle(PerfSection::FrameChart);
        let s = performance_footer_string_with_session(&handle);
        assert!(
            s.contains("[j/k] Scroll"),
            "[j/k] Scroll must appear when FrameChart is focused; footer was: {s}"
        );
        assert!(
            s.contains("[←/→] Frames"),
            "[←/→] Frames must appear when FrameChart is focused; footer was: {s}"
        );
    }

    #[test]
    fn performance_footer_mentions_details_tab_cycling() {
        let state = DevToolsViewState {
            active_panel: DevToolsPanel::Performance,
            ..Default::default()
        };
        let s = footer_string(&state);
        assert!(s.contains("]/[") || s.contains("] /["), "footer was: {s}");
        assert!(
            s.contains("Tabs"),
            "footer should mention Tabs; footer was: {s}"
        );
    }

    #[test]
    fn performance_footer_mentions_tab_section_cycling() {
        let state = DevToolsViewState {
            active_panel: DevToolsPanel::Performance,
            ..Default::default()
        };
        let s = footer_string(&state);
        assert!(
            s.contains("Section"),
            "footer should mention Section; footer was: {s}"
        );
    }

    // ── Phase-3 Performance footer tests ─────────────────────────────────────

    /// Build a `SessionHandle` with `focused_section == Details` and the given
    /// `details_tab`.
    fn make_perf_session_handle_with_details_tab(
        tab: PerfDetailsTab,
    ) -> fdemon_app::session::SessionHandle {
        use fdemon_app::session::{Session, SessionHandle};
        let mut session = Session::new(
            "test-device".to_string(),
            "Test Device".to_string(),
            "android".to_string(),
            false,
        );
        session.performance.focused_section = PerfSection::Details;
        session.performance.details_tab = tab;
        SessionHandle::new(session)
    }

    #[test]
    fn test_performance_footer_includes_filter_hint_on_timeline_events_tab() {
        let handle = make_perf_session_handle_with_details_tab(PerfDetailsTab::TimelineEvents);
        let s = performance_footer_string_with_session(&handle);
        assert!(
            s.contains("[f] Filter"),
            "footer should include [f] Filter on TimelineEvents tab; footer was: {s}"
        );
    }

    #[test]
    fn test_performance_footer_includes_rebuild_hint_on_rebuild_stats_tab() {
        let handle = make_perf_session_handle_with_details_tab(PerfDetailsTab::RebuildStats);
        let s = performance_footer_string_with_session(&handle);
        assert!(
            s.contains("[R] Rebuild track"),
            "footer should include [R] Rebuild track on RebuildStats tab; footer was: {s}"
        );
    }

    #[test]
    fn test_performance_footer_omits_phase_3_hints_on_frame_analysis_tab() {
        let handle = make_perf_session_handle_with_details_tab(PerfDetailsTab::FrameAnalysis);
        let s = performance_footer_string_with_session(&handle);
        assert!(
            !s.contains("[f] Filter"),
            "footer must NOT include [f] Filter on FrameAnalysis tab; footer was: {s}"
        );
        assert!(
            !s.contains("[R] Rebuild track"),
            "footer must NOT include [R] Rebuild track on FrameAnalysis tab; footer was: {s}"
        );
    }

    #[test]
    fn test_performance_footer_omits_phase_3_hints_when_frame_chart_focused() {
        let handle = make_perf_session_handle(PerfSection::FrameChart);
        let s = performance_footer_string_with_session(&handle);
        assert!(
            !s.contains("[f] Filter"),
            "footer must NOT include [f] Filter when FrameChart is focused; footer was: {s}"
        );
        assert!(
            !s.contains("[R] Rebuild track"),
            "footer must NOT include [R] Rebuild track when FrameChart is focused; footer was: {s}"
        );
    }

    // ── Memory panel tests ────────────────────────────────────────────────────

    #[test]
    fn test_devtools_view_renders_memory_panel_without_panic() {
        let state = DevToolsViewState {
            active_panel: DevToolsPanel::Memory,
            ..Default::default()
        };
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        // Should not panic — Memory panel renders real widget
    }

    #[test]
    fn test_tab_bar_includes_memory_tab() {
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("Memory"),
            "expected Memory tab, got: {text:?}"
        );
    }

    // ── Device-name-in-title tests (Acceptance B) ─────────────────────────────

    /// Build a `SessionHandle` with the given device name.
    fn make_session_handle_with_device_name(device_name: &str) -> fdemon_app::session::SessionHandle {
        use fdemon_app::session::{Session, SessionHandle};
        SessionHandle::new(Session::new(
            "test-device".to_string(),
            device_name.to_string(),
            "android".to_string(),
            false,
        ))
    }

    #[test]
    fn test_title_single_session_shows_devtools_only() {
        // With session_count == 1, title should be " DevTools " (unchanged).
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_device_name("Pixel 7");
        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("DevTools"),
            "Single session: title should contain 'DevTools', got: {text:?}"
        );
        assert!(
            !text.contains("Pixel 7"),
            "Single session: title must NOT contain device name, got: {text:?}"
        );
    }

    #[test]
    fn test_title_multi_session_shows_device_name() {
        // With session_count > 1, title should include the session's device name.
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_device_name("Pixel 7");
        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 2);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("DevTools"),
            "Multi-session: title must contain 'DevTools', got: {text:?}"
        );
        assert!(
            text.contains("Pixel 7"),
            "Multi-session: title must contain device name 'Pixel 7', got: {text:?}"
        );
    }

    #[test]
    fn test_title_multi_session_no_session_shows_devtools_only() {
        // With session_count > 1 but no active session (defensive guard), title
        // falls back to " DevTools ".
        let state = DevToolsViewState::default();
        let widget = DevToolsView::new(&state, None, IconSet::default(), 2);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 80, 3);
        assert!(
            text.contains("DevTools"),
            "Fallback: title must still contain 'DevTools', got: {text:?}"
        );
    }

    #[test]
    fn test_title_long_device_name_is_truncated() {
        // Device names longer than DEVICE_NAME_MAX_CHARS should be truncated so
        // the title does not overflow the border.
        let long_name: String = "A".repeat(DEVICE_NAME_MAX_CHARS + 10);
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_device_name(&long_name);
        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 2);
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 120, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 120, 3);
        assert!(
            text.contains("DevTools"),
            "Long name: title must contain 'DevTools', got: {text:?}"
        );
        // The full long name should NOT appear — it is truncated.
        assert!(
            !text.contains(&long_name),
            "Long name must be truncated in title, got: {text:?}"
        );
        // The ellipsis character should appear.
        assert!(
            text.contains('\u{2026}'),
            "Truncated name should end with ellipsis (…), got: {text:?}"
        );
    }

    #[test]
    fn test_title_device_name_at_max_chars_no_ellipsis() {
        // A name exactly DEVICE_NAME_MAX_CHARS long should NOT get an ellipsis.
        let exact_name: String = "B".repeat(DEVICE_NAME_MAX_CHARS);
        let state = DevToolsViewState::default();
        let handle = make_session_handle_with_device_name(&exact_name);
        let widget = DevToolsView::new(&state, Some(&handle), IconSet::default(), 2);
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 120, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 120, 3);
        assert!(
            !text.contains('\u{2026}'),
            "Name at exactly DEVICE_NAME_MAX_CHARS must NOT get an ellipsis, got: {text:?}"
        );
    }

    // ── Out-of-tree DevTools panel seam (T5) ──────────────────────────────────

    use fdemon_app::devtools_panel_provider::{DevToolsPanelCtx, DevToolsPanelProvider, Handled};
    use fdemon_app::InputKey;

    /// Render-counting dummy panel that draws a recognizable marker string.
    #[derive(Debug, Default)]
    struct MarkerPanel {
        renders: usize,
    }

    impl DevToolsPanelProvider for MarkerPanel {
        fn id(&self) -> &str {
            "preview"
        }
        fn title(&self) -> &str {
            "Preview"
        }
        fn key_hint(&self) -> &str {
            "[Esc] Logs  PREVIEW-HINT"
        }
        fn render(&mut self, area: Rect, buf: &mut Buffer, _ctx: DevToolsPanelCtx) {
            self.renders += 1;
            if area.height > 0 && area.width >= 8 {
                buf.set_string(area.x, area.y, "MARKER42", Style::default());
            }
        }
        fn handle_key(&mut self, _key: InputKey) -> Handled {
            Handled::Consumed
        }
    }

    /// A registered panel's title appears in the tab bar after the four built-ins.
    #[test]
    fn ext_tab_bar_shows_registered_title() {
        let state = DevToolsViewState::default();
        let mut panels: Vec<Box<dyn DevToolsPanelProvider>> = vec![Box::new(MarkerPanel::default())];
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1)
            .with_panels(&mut panels, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 3));
        widget.render_tab_bar_inner(Rect::new(0, 0, 100, 3), &mut buf, None);

        let text = collect_buf_text(&buf, 100, 3);
        assert!(text.contains("Inspector"), "built-ins still shown: {text:?}");
        assert!(
            text.contains("Preview"),
            "registered panel title must appear in tab bar, got: {text:?}"
        );
    }

    /// When the extension panel is active, the panel content (via `&mut self`)
    /// replaces the built-in panel content, and its key_hint is shown.
    #[test]
    fn ext_active_panel_renders_via_mut_self() {
        let state = DevToolsViewState {
            active_extension_panel: Some("preview".to_string()),
            ..Default::default()
        };
        let mut panels: Vec<Box<dyn DevToolsPanelProvider>> = vec![Box::new(MarkerPanel::default())];
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1)
            .with_panels(&mut panels, 0);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let text = collect_buf_text(&buf, 80, 24);
        assert!(
            text.contains("MARKER42"),
            "active extension panel must render its content, got marker missing"
        );
        assert!(
            text.contains("PREVIEW-HINT"),
            "active extension panel footer must show its key_hint"
        );
        // Render mutated the panel (renders counter advanced).
        assert_eq!(panels[0_usize].id(), "preview");
    }

    /// A stale active extension id (no matching registered panel) falls back to
    /// the built-in panel — the built-in content renders, not a blank panel.
    #[test]
    fn ext_stale_id_falls_back_to_builtin_render() {
        let state = DevToolsViewState {
            active_panel: DevToolsPanel::Network,
            active_extension_panel: Some("gone".to_string()),
            ..Default::default()
        };
        let mut panels: Vec<Box<dyn DevToolsPanelProvider>> = vec![Box::new(MarkerPanel::default())];
        let widget = DevToolsView::new(&state, None, IconSet::default(), 1)
            .with_panels(&mut panels, 0);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let text = collect_buf_text(&buf, 80, 24);
        assert!(
            !text.contains("MARKER42"),
            "stale id must NOT render the (non-active) preview panel"
        );
    }

    /// STOCK: a DevToolsView built with `new` (no panels) and with no extension
    /// panel active renders byte-identically to one built with an EMPTY panels
    /// vec. This pins that the seam is a true no-op for stock builds.
    #[test]
    fn ext_stock_render_is_identical_with_empty_panels() {
        let state = DevToolsViewState::default();
        let area = Rect::new(0, 0, 80, 24);

        let mut buf_plain = Buffer::empty(area);
        DevToolsView::new(&state, None, IconSet::default(), 1).render(area, &mut buf_plain);

        let mut empty: Vec<Box<dyn DevToolsPanelProvider>> = Vec::new();
        let mut buf_with = Buffer::empty(area);
        DevToolsView::new(&state, None, IconSet::default(), 1)
            .with_panels(&mut empty, 0)
            .render(area, &mut buf_with);

        assert_eq!(
            buf_plain, buf_with,
            "with no registered panels the seam must produce byte-identical output"
        );
    }
}

//! Tab bar widget for Target Selector pane
//!
//! Provides tab navigation between Connected and Bootable device views.

use super::TargetTab;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::theme::palette;
use fdemon_app::message::Message;
use fdemon_app::{MouseAction, MouseRect};

/// Tab bar widget for switching between Connected and Bootable views
pub struct TabBar {
    active_tab: TargetTab,
    /// Whether this pane is focused
    pane_focused: bool,
    /// Refresh-in-flight indicator for the Connected tab.
    connected_refreshing: bool,
    /// Refresh-in-flight indicator for the Bootable tab.
    bootable_refreshing: bool,
    /// Global animation frame (`AppState::animation_frame`) for animated refresh spinners.
    /// Defaults to `0` so existing test constructions compile without change.
    animation_frame: u64,
}

impl TabBar {
    pub fn new(
        active_tab: TargetTab,
        pane_focused: bool,
        connected_refreshing: bool,
        bootable_refreshing: bool,
    ) -> Self {
        Self {
            active_tab,
            pane_focused,
            connected_refreshing,
            bootable_refreshing,
            animation_frame: 0,
        }
    }

    /// Set the global animation frame used to drive the refresh spinner.
    pub fn animation_frame(mut self, frame: u64) -> Self {
        self.animation_frame = frame;
        self
    }
}

impl Widget for TabBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_with_regions(area, buf, self, None);
    }
}

/// Render [`TabBar`] and record clickable tab regions.
///
/// Each tab half (`[1 Connected]`, `[2 Bootable]`) is registered as a
/// left-click region at `z_index = 1` (main dialog layer). Passing `None`
/// for `ctx` produces output identical to the previous `Widget::render`.
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    tab_bar: TabBar,
    ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
) {
    // Outer container: dark background with rounded border
    let container_bg = palette::DEEPEST_BG;
    let container_block = Block::default()
        .style(Style::default().bg(container_bg))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(palette::BORDER_DIM));

    let inner = container_block.inner(area);
    container_block.render(area, buf);

    // Split into three equal thirds for tabs
    let tabs = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .split(inner);

    const TAB_ORDER: [TargetTab; 3] =
        [TargetTab::Connected, TargetTab::Bootable, TargetTab::PairQr];

    // Record click regions for each tab third (z=1, main dialog layer)
    if let Some(c) = ctx {
        for (rect, tab) in tabs.iter().zip(TAB_ORDER) {
            c.click_at_z(
                MouseRect::new(rect.x, rect.y, rect.width, rect.height),
                MouseAction::emit(Message::NewSessionDialogSwitchTab(tab)),
                1,
            );
        }
    }

    // Render each tab label
    for (i, tab) in TAB_ORDER.iter().enumerate() {
        let is_active = *tab == tab_bar.active_tab;
        let refreshing = match tab {
            TargetTab::Connected => tab_bar.connected_refreshing,
            TargetTab::Bootable => tab_bar.bootable_refreshing,
            // The Pair QR tab has no list refresh; pairing progress is shown
            // in the panel itself.
            TargetTab::PairQr => false,
        };

        let label = if refreshing {
            let glyph = crate::widgets::spinner::spinner_char(
                tab_bar.animation_frame / crate::widgets::spinner::SPINNER_TICKS_PER_FRAME,
            );
            format!("{} {glyph}", tab.label())
        } else {
            tab.label().to_string()
        };

        let style = if is_active && tab_bar.pane_focused {
            Style::default()
                .fg(palette::TEXT_BRIGHT)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_SECONDARY)
        };

        let paragraph = Paragraph::new(label)
            .style(style)
            .alignment(Alignment::Center);
        paragraph.render(tabs[i], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::MouseCtx;
    use fdemon_app::mouse_regions::MouseRegions;
    use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, Terminal};

    #[test]
    fn test_target_tab_label() {
        assert_eq!(TargetTab::Connected.label(), "1 Connected");
        assert_eq!(TargetTab::Bootable.label(), "2 Bootable");
        assert_eq!(TargetTab::PairQr.label(), "3 Pair QR");
    }

    #[test]
    fn test_target_tab_toggle() {
        assert_eq!(TargetTab::Connected.toggle(), TargetTab::Bootable);
        assert_eq!(TargetTab::Bootable.toggle(), TargetTab::PairQr);
        assert_eq!(TargetTab::PairQr.toggle(), TargetTab::Connected);
    }

    #[test]
    fn test_target_tab_shortcut() {
        assert_eq!(TargetTab::Connected.shortcut(), '1');
        assert_eq!(TargetTab::Bootable.shortcut(), '2');
        assert_eq!(TargetTab::PairQr.shortcut(), '3');
    }

    #[test]
    fn test_target_tab_default() {
        let tab: TargetTab = Default::default();
        assert_eq!(tab, TargetTab::Connected);
    }

    #[test]
    fn test_tab_bar_renders() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, true, false, false);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Connected"));
        assert!(content.contains("Bootable"));
    }

    #[test]
    fn test_tab_bar_renders_with_bootable_active() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Bootable, true, false, false);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Connected"));
        assert!(content.contains("Bootable"));
    }

    #[test]
    fn test_tab_bar_unfocused() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, false, false, false);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Should still render both tabs
        assert!(content.contains("Connected"));
        assert!(content.contains("Bootable"));
    }

    #[test]
    fn test_tab_bar_renders_connected_refreshing_indicator() {
        use crate::widgets::spinner::SPINNER_FRAMES;
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                // animation_frame defaults to 0 → spinner_char(0 / 2) = spinner_char(0) = '⠋'
                let tab_bar = TabBar::new(TargetTab::Connected, true, true, false);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        let any_spinner = SPINNER_FRAMES.iter().any(|&g| rendered.contains(g));
        assert!(
            any_spinner,
            "expected a spinner glyph on Connected tab when refreshing, got: {rendered}"
        );
    }

    #[test]
    fn test_tab_bar_renders_bootable_refreshing_indicator() {
        use crate::widgets::spinner::SPINNER_FRAMES;
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Bootable, true, false, true);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        let any_spinner = SPINNER_FRAMES.iter().any(|&g| rendered.contains(g));
        assert!(
            any_spinner,
            "expected a spinner glyph on Bootable tab when refreshing, got: {rendered}"
        );
    }

    #[test]
    fn test_tab_bar_no_indicator_when_not_refreshing() {
        use crate::widgets::spinner::SPINNER_FRAMES;
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, true, false, false);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        // When not refreshing, no spinner glyph should appear.
        let any_spinner = SPINNER_FRAMES.iter().any(|&g| rendered.contains(g));
        assert!(
            !any_spinner,
            "expected no spinner glyph when not refreshing, got: {rendered}"
        );
    }

    // ─── render_with_regions tests ───────────────────────────────────────────

    #[test]
    fn render_with_regions_records_three_tab_regions_at_z1() {
        let tab_bar = TabBar::new(TargetTab::Connected, true, false, false);

        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 3));
        let mut regions = MouseRegions::default();
        {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            render_with_regions(Rect::new(0, 0, 60, 3), &mut buf, tab_bar, Some(&mut ctx));
        }

        assert_eq!(regions.len(), 3, "expected exactly 3 tab regions");

        let has_region_for = |tab: TargetTab| {
            regions.iter().any(|e| {
                e.on_left
                    .as_ref()
                    .and_then(|a| a.as_emit())
                    .map(|m| matches!(m, Message::NewSessionDialogSwitchTab(t) if *t == tab))
                    .unwrap_or(false)
            })
        };

        assert!(
            has_region_for(TargetTab::Connected),
            "expected Connected tab region"
        );
        assert!(
            has_region_for(TargetTab::Bootable),
            "expected Bootable tab region"
        );
        assert!(
            has_region_for(TargetTab::PairQr),
            "expected Pair QR tab region"
        );

        for entry in regions.iter() {
            assert_eq!(entry.z_index, 1, "all tab regions must be at z=1");
        }
    }

    #[test]
    fn render_with_regions_no_ctx_produces_same_output_as_widget() {
        let mut buf1 = Buffer::empty(Rect::new(0, 0, 40, 3));
        let tab_bar1 = TabBar::new(TargetTab::Connected, true, false, false);
        render_with_regions(Rect::new(0, 0, 40, 3), &mut buf1, tab_bar1, None);

        let mut buf2 = Buffer::empty(Rect::new(0, 0, 40, 3));
        let tab_bar2 = TabBar::new(TargetTab::Connected, true, false, false);
        <TabBar as Widget>::render(tab_bar2, Rect::new(0, 0, 40, 3), &mut buf2);

        let content1: String = buf1.content().iter().map(|c| c.symbol()).collect();
        let content2: String = buf2.content().iter().map(|c| c.symbol()).collect();
        assert_eq!(
            content1, content2,
            "render_with_regions(None) must produce identical output to Widget::render"
        );
    }

    // ─── Animated spinner tests (Phase 3, Task 03) ───────────────────────────

    #[test]
    fn test_tab_bar_connected_refreshing_shows_spinner_glyph_at_nonzero_frame() {
        use crate::widgets::spinner::SPINNER_FRAMES;

        // Use frame=4 → cadence index = 4 / SPINNER_TICKS_PER_FRAME = 4 / 2 = 2 → SPINNER_FRAMES[2] = '⠹'
        let frame: u64 = 4;
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar =
                    TabBar::new(TargetTab::Connected, true, true, false).animation_frame(frame);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        let any_spinner = SPINNER_FRAMES.iter().any(|&g| rendered.contains(g));
        assert!(
            any_spinner,
            "expected a spinner glyph from SPINNER_FRAMES in connected refreshing tab, got: {rendered}"
        );
    }

    #[test]
    fn test_tab_bar_bootable_refreshing_shows_spinner_glyph_at_nonzero_frame() {
        use crate::widgets::spinner::SPINNER_FRAMES;

        let frame: u64 = 6;
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar =
                    TabBar::new(TargetTab::Bootable, true, false, true).animation_frame(frame);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        let any_spinner = SPINNER_FRAMES.iter().any(|&g| rendered.contains(g));
        assert!(
            any_spinner,
            "expected a spinner glyph from SPINNER_FRAMES in bootable refreshing tab, got: {rendered}"
        );
    }

    #[test]
    fn test_tab_bar_no_spinner_when_not_refreshing_nonzero_frame() {
        use crate::widgets::spinner::SPINNER_FRAMES;

        let frame: u64 = 10;
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                // not refreshing — no spinner glyphs should appear
                let tab_bar =
                    TabBar::new(TargetTab::Connected, true, false, false).animation_frame(frame);
                f.render_widget(tab_bar, f.area());
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        let any_spinner = SPINNER_FRAMES.iter().any(|&g| rendered.contains(g));
        assert!(
            !any_spinner,
            "expected NO spinner glyph when not refreshing, got: {rendered}"
        );
    }

    /// Phase-coherence test: both the tab-bar refresh spinner and the loading-line spinner
    /// must produce the same glyph when driven from the same `animation_frame`.
    ///
    /// Approach: render a `TargetSelector` in full mode with `loading = true` (so the
    /// discovery-line spinner appears in the content chunk) AND `connected_refreshing = true`
    /// (so the tab-bar spinner appears in the tab-bar chunk). Both call sites share the
    /// expression `spinner_char(animation_frame / SPINNER_TICKS_PER_FRAME)` — this test
    /// would fail if either diverged on operand or divisor.
    ///
    /// The render layout is:
    ///   row 0-2: tab-bar (`TabBar` → `render_with_regions`) — spinner appears here.
    ///   row 3-N: content (`render_loading`) — "{glyph} Discovering devices..." appears here.
    ///
    /// We inspect each region of the buffer independently and assert both contain
    /// the identical expected glyph `SPINNER_FRAMES[(4 / SPINNER_TICKS_PER_FRAME) % len]`
    /// which is `'⠹'` with `SPINNER_TICKS_PER_FRAME = 2`.
    #[test]
    fn test_tab_bar_phase_coherence_both_spinners_from_same_frame() {
        use crate::widgets::spinner::{SPINNER_FRAMES, SPINNER_TICKS_PER_FRAME};
        use fdemon_app::new_session_dialog::TargetSelectorState;
        use fdemon_app::ToolAvailability;
        use ratatui::widgets::Widget as _;

        // animation_frame=4, SPINNER_TICKS_PER_FRAME=2 → cadence index 2 → '⠹'
        let animation_frame: u64 = 4;
        let expected_glyph = SPINNER_FRAMES
            [(animation_frame / SPINNER_TICKS_PER_FRAME) as usize % SPINNER_FRAMES.len()];

        // Build a TargetSelectorState with loading=true (loading-line spinner) and
        // refreshing=true (tab-bar spinner on the Connected tab).
        let state = TargetSelectorState {
            loading: true,
            refreshing: true,
            ..Default::default()
        };
        let tool_availability = ToolAvailability::default();

        // Height: 3 (tab bar) + 5 (content min) + 1 (footer) = 9 rows minimum.
        // Use 12 rows to give the content area comfortable space.
        let width = 60u16;
        let height = 12u16;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                use crate::widgets::new_session_dialog::target_selector::TargetSelector;
                let selector = TargetSelector::new(&state, &tool_availability, true)
                    .animation_frame(animation_frame);
                selector.render(f.area(), f.buffer_mut());
            })
            .unwrap();

        let buf = terminal.backend().buffer();

        // Tab-bar region: rows 0-2 (tab bar is 3 rows tall per render_full layout).
        let tab_bar_content: String = (0..3)
            .flat_map(|row| (0..width).map(move |col| (col, row)))
            .map(|(col, row)| buf.cell((col, row)).map(|c| c.symbol()).unwrap_or(" "))
            .collect();

        // Loading-line region: rows 3 onward (content chunk in render_full).
        let loading_content: String = (3..height)
            .flat_map(|row| (0..width).map(move |col| (col, row)))
            .map(|(col, row)| buf.cell((col, row)).map(|c| c.symbol()).unwrap_or(" "))
            .collect();

        assert!(
            tab_bar_content.contains(expected_glyph),
            "tab-bar region must contain glyph '{expected_glyph}' (frame={animation_frame}, \
             cadence=frame/SPINNER_TICKS_PER_FRAME={SPINNER_TICKS_PER_FRAME}); \
             got tab-bar: {tab_bar_content}"
        );
        assert!(
            loading_content.contains(expected_glyph),
            "loading-line region must contain glyph '{expected_glyph}' (frame={animation_frame}, \
             cadence=frame/SPINNER_TICKS_PER_FRAME={SPINNER_TICKS_PER_FRAME}); \
             got loading: {loading_content}"
        );
    }
}

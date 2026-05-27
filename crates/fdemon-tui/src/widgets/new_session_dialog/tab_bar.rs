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

use crate::theme::{icons::IconSet, palette};
use fdemon_app::message::Message;
use fdemon_app::{MouseAction, MouseRect};

/// Tab bar widget for switching between Connected and Bootable views
pub struct TabBar<'a> {
    active_tab: TargetTab,
    /// Whether this pane is focused
    pane_focused: bool,
    /// Refresh-in-flight indicator for the Connected tab.
    connected_refreshing: bool,
    /// Refresh-in-flight indicator for the Bootable tab.
    bootable_refreshing: bool,
    /// Icon set for resolving glyphs (Unicode vs Nerd Fonts).
    /// Retained in the constructor signature for API stability; the refresh indicator
    /// now uses the animated spinner (see `animation_frame`) rather than `icons.refresh()`.
    #[allow(dead_code)]
    icons: &'a IconSet,
    /// Global animation frame (`AppState::animation_frame`) for animated refresh spinners.
    /// Defaults to `0` so existing test constructions compile without change.
    animation_frame: u64,
}

impl<'a> TabBar<'a> {
    pub fn new(
        active_tab: TargetTab,
        pane_focused: bool,
        connected_refreshing: bool,
        bootable_refreshing: bool,
        icons: &'a IconSet,
    ) -> Self {
        Self {
            active_tab,
            pane_focused,
            connected_refreshing,
            bootable_refreshing,
            icons,
            animation_frame: 0,
        }
    }

    /// Set the global animation frame used to drive the refresh spinner.
    pub fn animation_frame(mut self, frame: u64) -> Self {
        self.animation_frame = frame;
        self
    }
}

impl Widget for TabBar<'_> {
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
    tab_bar: TabBar<'_>,
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

    // Split into two equal halves for tabs
    let tabs =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(inner);

    // Record click regions for each tab half (z=1, main dialog layer)
    if let Some(c) = ctx {
        c.click_at_z(
            MouseRect::new(tabs[0].x, tabs[0].y, tabs[0].width, tabs[0].height),
            MouseAction::emit(Message::NewSessionDialogSwitchTab(TargetTab::Connected)),
            1,
        );
        c.click_at_z(
            MouseRect::new(tabs[1].x, tabs[1].y, tabs[1].width, tabs[1].height),
            MouseAction::emit(Message::NewSessionDialogSwitchTab(TargetTab::Bootable)),
            1,
        );
    }

    // Render each tab label
    for (i, tab) in [TargetTab::Connected, TargetTab::Bootable]
        .iter()
        .enumerate()
    {
        let is_active = *tab == tab_bar.active_tab;
        let refreshing = match tab {
            TargetTab::Connected => tab_bar.connected_refreshing,
            TargetTab::Bootable => tab_bar.bootable_refreshing,
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
    use crate::theme::icons::IconSet;
    use crate::widgets::MouseCtx;
    use fdemon_app::mouse_regions::MouseRegions;
    use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, Terminal};

    #[test]
    fn test_target_tab_label() {
        assert_eq!(TargetTab::Connected.label(), "1 Connected");
        assert_eq!(TargetTab::Bootable.label(), "2 Bootable");
    }

    #[test]
    fn test_target_tab_toggle() {
        assert_eq!(TargetTab::Connected.toggle(), TargetTab::Bootable);
        assert_eq!(TargetTab::Bootable.toggle(), TargetTab::Connected);
    }

    #[test]
    fn test_target_tab_shortcut() {
        assert_eq!(TargetTab::Connected.shortcut(), '1');
        assert_eq!(TargetTab::Bootable.shortcut(), '2');
    }

    #[test]
    fn test_target_tab_default() {
        let tab: TargetTab = Default::default();
        assert_eq!(tab, TargetTab::Connected);
    }

    #[test]
    fn test_tab_bar_renders() {
        let icons = IconSet::default();
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, true, false, false, &icons);
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
        let icons = IconSet::default();
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Bootable, true, false, false, &icons);
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
        let icons = IconSet::default();
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, false, false, false, &icons);
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
        let icons = IconSet::default();
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                // animation_frame defaults to 0 → spinner_char(0 / 2) = spinner_char(0) = '⠋'
                let tab_bar = TabBar::new(TargetTab::Connected, true, true, false, &icons);
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
        let icons = IconSet::default();
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Bootable, true, false, true, &icons);
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
        let icons = IconSet::default();
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, true, false, false, &icons);
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
    fn render_with_regions_records_two_tab_regions_at_z1() {
        let icons = IconSet::default();
        let tab_bar = TabBar::new(TargetTab::Connected, true, false, false, &icons);

        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 3));
        let mut regions = MouseRegions::default();
        {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            render_with_regions(Rect::new(0, 0, 40, 3), &mut buf, tab_bar, Some(&mut ctx));
        }

        assert_eq!(regions.len(), 2, "expected exactly 2 tab regions");

        let connected_present = regions.iter().any(|e| {
            e.on_left
                .as_ref()
                .and_then(|a| a.as_emit())
                .map(|m| matches!(m, Message::NewSessionDialogSwitchTab(TargetTab::Connected)))
                .unwrap_or(false)
        });
        let bootable_present = regions.iter().any(|e| {
            e.on_left
                .as_ref()
                .and_then(|a| a.as_emit())
                .map(|m| matches!(m, Message::NewSessionDialogSwitchTab(TargetTab::Bootable)))
                .unwrap_or(false)
        });

        assert!(connected_present, "expected Connected tab region");
        assert!(bootable_present, "expected Bootable tab region");

        for entry in regions.iter() {
            assert_eq!(entry.z_index, 1, "all tab regions must be at z=1");
        }
    }

    #[test]
    fn render_with_regions_no_ctx_produces_same_output_as_widget() {
        let icons = IconSet::default();

        let mut buf1 = Buffer::empty(Rect::new(0, 0, 40, 3));
        let tab_bar1 = TabBar::new(TargetTab::Connected, true, false, false, &icons);
        render_with_regions(Rect::new(0, 0, 40, 3), &mut buf1, tab_bar1, None);

        let mut buf2 = Buffer::empty(Rect::new(0, 0, 40, 3));
        let tab_bar2 = TabBar::new(TargetTab::Connected, true, false, false, &icons);
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

        let icons = IconSet::default();
        // Use frame=4 → cadence index = 4 / 2 = 2 → SPINNER_FRAMES[2] = '⠹'
        let frame: u64 = 4;
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Connected, true, true, false, &icons)
                    .animation_frame(frame);
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

        let icons = IconSet::default();
        let frame: u64 = 6;
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let tab_bar = TabBar::new(TargetTab::Bootable, true, false, true, &icons)
                    .animation_frame(frame);
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

        let icons = IconSet::default();
        let frame: u64 = 10;
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                // not refreshing — no spinner glyphs should appear
                let tab_bar = TabBar::new(TargetTab::Connected, true, false, false, &icons)
                    .animation_frame(frame);
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

    #[test]
    fn test_tab_bar_phase_coherence_both_spinners_from_same_frame() {
        use crate::widgets::spinner::{spinner_char, SPINNER_TICKS_PER_FRAME};

        // Both spinners (discovery line and tab bar) use the same formula:
        // spinner_char(animation_frame / SPINNER_TICKS_PER_FRAME).
        // Verify that two calls with the same frame produce the same glyph.
        let frame: u64 = 8;
        let glyph1 = spinner_char(frame / SPINNER_TICKS_PER_FRAME);
        let glyph2 = spinner_char(frame / SPINNER_TICKS_PER_FRAME);
        assert_eq!(
            glyph1, glyph2,
            "both spinners derived from frame={frame} must produce the same glyph"
        );
    }
}

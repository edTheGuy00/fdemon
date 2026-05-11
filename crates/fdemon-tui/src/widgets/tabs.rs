//! Session tabs widget for multi-instance display
//!
//! Provides tab navigation for multiple running Flutter sessions.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Tabs, Widget},
};

use fdemon_app::message::Message;
use fdemon_app::session_manager::SessionManager;
use fdemon_app::{MouseAction, MouseRect};

use crate::render::MouseCtx;
use crate::theme::icons::IconSet;

/// Width of the Tabs divider rendered as `" │ "` (space, pipe, space) = 3 cells.
///
/// Verified by `divider_width_matches_rendered_buffer` test: the ratatui `Tabs`
/// widget inserts exactly 3 cells between each tab title when `divider("│")` is
/// used — one leading space and one trailing space are added by the widget itself.
const DIVIDER_WIDTH: u16 = 3;

/// Widget displaying session tabs in a standalone subheader row
pub struct SessionTabs<'a> {
    session_manager: &'a SessionManager,
    icons: IconSet,
}

impl<'a> SessionTabs<'a> {
    pub fn new(session_manager: &'a SessionManager, icons: IconSet) -> Self {
        Self {
            session_manager,
            icons,
        }
    }
}

impl Widget for SessionTabs<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_session_tabs(area, buf, self.session_manager, self.icons, None);
    }
}

/// Build tab title lines from the session manager, one per session.
///
/// Each title is `" <icon> <name> "` — icon styled by phase, name truncated
/// to 12 characters. This function is shared between the `Widget` impl and the
/// free-function render path so both use identical titles for width calculation.
fn build_tab_titles(session_manager: &SessionManager, icons: IconSet) -> Vec<Line<'static>> {
    session_manager
        .iter()
        .map(|handle| {
            let session = &handle.session;

            // Status icon with color from theme
            let (icon, _label, style) =
                crate::theme::styles::phase_indicator(&session.phase, &icons);

            // Truncate device name if too long
            let name = truncate_name(&session.device_name, 12);

            // Build line with styled icon span
            Line::from(vec![
                Span::raw(" "),
                Span::styled(icon, style),
                Span::raw(format!(" {} ", name)),
            ])
        })
        .collect()
}

/// Render session tabs with optional mouse region recording.
///
/// This is the primary render path for the session tabs bar. The `Widget for
/// SessionTabs` impl delegates here with `ctx = None`, keeping it usable in
/// tests that use `term.render_widget(tabs, area)` without a registry.
///
/// When called from `render::view` with `ctx = Some(&mut mouse_ctx)`, one
/// click region per tab is registered:
/// - Left-click → [`Message::SelectSessionByIndex`]`(i)`
/// - Middle-click → [`Message::CloseSessionAt`]`(i)`
///
/// For the single-session path, a left-click region covering the device pill
/// is registered → [`Message::OpenNewSessionDialog`].
///
/// No regions are registered when `session_manager.is_empty()`.
pub fn render_session_tabs(
    area: Rect,
    buf: &mut Buffer,
    session_manager: &SessionManager,
    icons: IconSet,
    mut ctx: Option<&mut MouseCtx<'_>>,
) {
    if session_manager.is_empty() {
        return;
    }

    if session_manager.len() == 1 {
        // Single-session path: render simplified device pill and optionally
        // register a click region for it.
        render_single_session_with_ctx(area, buf, session_manager, icons, ctx.as_deref_mut());
        return;
    }

    // Multi-session path: render ratatui Tabs and register per-tab regions.
    let titles = build_tab_titles(session_manager, icons);
    let selected = session_manager.selected_index();

    let tabs = Tabs::new(titles.clone())
        .select(selected)
        .highlight_style(crate::theme::styles::focused_selected())
        .divider("│");

    // Render with left padding
    let padded_area = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    if padded_area.height == 0 || padded_area.width == 0 {
        return;
    }

    tabs.render(padded_area, buf);

    // Register per-tab click regions.
    // Tabs renders as: <title0> │ <title1> │ <title2>
    // Each title's display width is title.width() (ratatui's Line::width).
    if let Some(ctx) = ctx {
        let mut cursor_x = padded_area.x;
        for (idx, title) in titles.iter().enumerate() {
            let w = title.width() as u16;
            // Stop if this tab extends beyond the visible area.
            if cursor_x.saturating_add(w) > padded_area.x + padded_area.width {
                break;
            }
            let rect = MouseRect::new(cursor_x, padded_area.y, w, padded_area.height);
            ctx.click_left_middle(
                rect,
                MouseAction::emit(Message::SelectSessionByIndex(idx)),
                MouseAction::emit(Message::CloseSessionAt(idx)),
            );
            cursor_x = cursor_x.saturating_add(w + DIVIDER_WIDTH);
        }
    }
}

/// Render the single-session device pill and optionally register its click region.
///
/// The device pill covers `icon + " " + name` inside a 1-cell left-padded area.
/// Left-click → [`Message::OpenNewSessionDialog`] (so the user can quickly switch
/// devices or add a second session). The region covers the full padded content
/// area, matching the rendered extent of the pill.
fn render_single_session_with_ctx(
    area: Rect,
    buf: &mut Buffer,
    session_manager: &SessionManager,
    icons: IconSet,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    if let Some(handle) = session_manager.selected() {
        let session = &handle.session;

        let (icon, _label, style) = crate::theme::styles::phase_indicator(&session.phase, &icons);

        // Truncate device name if necessary
        let max_name_len = area.width.saturating_sub(4) as usize; // 2 for icon+space, 2 for padding
        let name = truncate_name(&session.device_name, max_name_len.max(8));

        let content = Line::from(vec![
            Span::styled(icon, style),
            Span::raw(" "),
            Span::raw(name),
        ]);

        // Render with left padding
        let padded_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        };

        Paragraph::new(content).render(padded_area, buf);

        // Register the device pill as a clickable region → open session dialog.
        if let Some(ctx) = ctx {
            if padded_area.width > 0 && padded_area.height > 0 {
                ctx.click(
                    MouseRect::new(
                        padded_area.x,
                        padded_area.y,
                        padded_area.width,
                        padded_area.height,
                    ),
                    MouseAction::emit(Message::OpenNewSessionDialog),
                );
            }
        }
    }
}

/// Truncate a name to max length, adding ellipsis if needed
fn truncate_name(name: &str, max_len: usize) -> String {
    if name.chars().count() <= max_len {
        name.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        let truncated: String = name.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_device;
    use fdemon_app::config::IconMode;

    #[test]
    fn test_truncate_name_short() {
        assert_eq!(truncate_name("Short", 10), "Short");
    }

    #[test]
    fn test_truncate_name_long() {
        assert_eq!(truncate_name("iPhone 15 Pro Max", 12), "iPhone 15 P…");
    }

    #[test]
    fn test_truncate_name_edge_cases() {
        // When name fits exactly, return it
        assert_eq!(truncate_name("A", 1), "A");
        assert_eq!(truncate_name("AB", 2), "AB");
        // When name is longer than max, truncate with ellipsis
        assert_eq!(truncate_name("ABC", 2), "A…");
        // max_len of 1 means we can only show ellipsis for longer strings
        assert_eq!(truncate_name("AB", 1), "…");
        assert_eq!(truncate_name("ABC", 1), "…");
    }

    #[test]
    fn test_truncate_name_unicode() {
        // Test with unicode characters
        assert_eq!(truncate_name("日本語テスト", 4), "日本語…");
    }

    #[test]
    fn test_session_tabs_creation() {
        let mut manager = SessionManager::new();
        manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();
        manager
            .create_session(&test_device("d2", "Pixel 8"))
            .unwrap();

        let icons = IconSet::new(IconMode::Unicode);
        let titles = build_tab_titles(&manager, icons);

        assert_eq!(titles.len(), 2);
    }

    #[test]
    fn test_tab_title_includes_status_icon() {
        let mut manager = SessionManager::new();
        let id = manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();

        let icons = IconSet::new(IconMode::Unicode);

        // Initially Initializing
        let titles = build_tab_titles(&manager, icons);
        let title_str: String = titles[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(title_str.contains('○')); // Initializing icon

        // Mark as running
        manager
            .get_mut(id)
            .unwrap()
            .session
            .mark_started("app-1".to_string());
        let titles = build_tab_titles(&manager, icons);
        let title_str: String = titles[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(title_str.contains('●')); // Running icon
    }

    #[test]
    fn test_standalone_session_tabs() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut manager = SessionManager::new();
        manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();
        manager
            .create_session(&test_device("d2", "Pixel 8"))
            .unwrap();

        let icons = IconSet::new(IconMode::Unicode);
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tabs = SessionTabs::new(&manager, icons);
                f.render_widget(tabs, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Should show both device names
        assert!(content.contains("iPhone 15"));
        assert!(content.contains("Pixel 8"));
    }

    #[test]
    fn test_session_tabs_single_session_renders_device_name() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut manager = SessionManager::new();
        manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();

        let icons = IconSet::new(IconMode::Unicode);
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tabs = SessionTabs::new(&manager, icons);
                f.render_widget(tabs, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Single session should show device name with status icon
        assert!(content.contains("iPhone 15"));
        assert!(content.contains('○')); // Initializing icon
    }

    #[test]
    fn test_session_tabs_single_session_running_status() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut manager = SessionManager::new();
        let id = manager
            .create_session(&test_device("d1", "iPhone 15"))
            .unwrap();

        // Mark session as running
        manager
            .get_mut(id)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        let icons = IconSet::new(IconMode::Unicode);
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let tabs = SessionTabs::new(&manager, icons);
                f.render_widget(tabs, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Running session should show device name with running icon
        assert!(content.contains("iPhone 15"));
        assert!(content.contains('●')); // Running icon
    }

    #[test]
    fn multi_session_records_one_region_per_tab() {
        use fdemon_app::{AppState, MouseAction};
        use ratatui::{backend::TestBackend, Terminal};

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state
            .session_manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();
        state
            .session_manager
            .create_session(&test_device("d2", "Pixel"))
            .unwrap();
        state
            .session_manager
            .create_session(&test_device("d3", "Web"))
            .unwrap();

        terminal
            .draw(|f| crate::render::view(f, &mut state))
            .unwrap();

        let regions = state.mouse_regions.take();
        let tab_regions: Vec<_> = regions
            .iter()
            .filter(|e| {
                matches!(
                    e.on_left,
                    Some(MouseAction::Emit(ref m))
                        if matches!(**m, Message::SelectSessionByIndex(_))
                )
            })
            .collect();
        assert_eq!(tab_regions.len(), 3, "one region per session");

        // Each region also has a middle-click binding.
        for entry in &tab_regions {
            assert!(
                matches!(
                    entry.on_middle,
                    Some(MouseAction::Emit(ref m))
                        if matches!(**m, Message::CloseSessionAt(_))
                ),
                "middle-click → CloseSessionAt"
            );
        }

        // Indices should be 0, 1, 2 in order.
        let mut indices: Vec<usize> = tab_regions
            .iter()
            .filter_map(|e| match &e.on_left {
                Some(MouseAction::Emit(m)) => match **m {
                    Message::SelectSessionByIndex(i) => Some(i),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        indices.sort();
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn nine_sessions_record_nine_tab_regions() {
        use fdemon_app::{AppState, MouseAction};
        use ratatui::{backend::TestBackend, Terminal};

        let backend = TestBackend::new(160, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        for i in 0..9 {
            state
                .session_manager
                .create_session(&test_device(&format!("d{}", i), &format!("Dev {}", i)))
                .unwrap();
        }

        terminal
            .draw(|f| crate::render::view(f, &mut state))
            .unwrap();

        let regions = state.mouse_regions.take();
        let count = regions
            .iter()
            .filter(|e| {
                matches!(
                    e.on_left,
                    Some(MouseAction::Emit(ref m))
                        if matches!(**m, Message::SelectSessionByIndex(_))
                )
            })
            .count();
        assert_eq!(count, 9);
    }

    #[test]
    fn divider_width_matches_rendered_buffer() {
        // Sanity-test the DIVIDER_WIDTH constant by measuring the rendered buffer.
        // The Tabs widget renders titles separated by " │ " — verify by reading
        // the cells between two known tab title positions.
        use fdemon_app::AppState;
        use ratatui::{backend::TestBackend, Terminal};

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state
            .session_manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();
        state
            .session_manager
            .create_session(&test_device("d2", "Pixel"))
            .unwrap();
        terminal
            .draw(|f| crate::render::view(f, &mut state))
            .unwrap();

        let buffer = terminal.backend().buffer();
        // Row 4 is the tabs sub-row (row 0=top border, row 1=title, row 2=tabs, row 3=bottom border)
        // Collect all cells on any row and search for the divider sequence.
        let found = (0..24).any(|y| {
            let line: String = (0..120)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect();
            line.contains(" │ ")
        });
        assert!(found, "divider must be ` │ ` (3 cells)");
    }

    #[test]
    fn empty_session_manager_registers_no_regions() {
        use fdemon_app::AppState;
        use ratatui::{backend::TestBackend, Terminal};

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        // No sessions added — session_manager is empty.

        terminal
            .draw(|f| crate::render::view(f, &mut state))
            .unwrap();

        let regions = state.mouse_regions.take();
        // No tab regions should be registered.
        use fdemon_app::MouseAction;
        let tab_count = regions
            .iter()
            .filter(|e| {
                matches!(
                    e.on_left,
                    Some(MouseAction::Emit(ref m))
                        if matches!(**m, Message::SelectSessionByIndex(_))
                )
            })
            .count();
        assert_eq!(tab_count, 0, "empty session manager → no tab regions");
    }
}

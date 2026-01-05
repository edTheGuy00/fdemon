//! Main render/view function (View in TEA pattern)

use std::collections::VecDeque;

use super::{layout, widgets};
use crate::app::state::{AppState, UiMode};
use crate::core::LogEntry;
use crate::tui::widgets::LogViewState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

/// Render the complete UI (View function in TEA)
///
/// This is a pure rendering function - it should not modify state
/// except for widget state that tracks rendering info (scroll position).
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();
    let session_count = state.session_manager.len();
    let areas = layout::create_with_sessions(area, session_count);

    // Main header with project name and session tabs inside
    let header = widgets::MainHeader::new(state.project_name.as_deref())
        .with_sessions(&state.session_manager);
    frame.render_widget(header, areas.header);

    // Log view - use selected session's logs or show empty state
    if let Some(handle) = state.session_manager.selected_mut() {
        let mut log_view =
            widgets::LogView::new(&handle.session.logs).filter_state(&handle.session.filter_state);

        // Add search state if there's an active search
        if !handle.session.search_state.query.is_empty() {
            log_view = log_view.search_state(&handle.session.search_state);
        }

        // Add link highlight state if link mode is active (Phase 3.1)
        if handle.session.link_highlight_state.is_active() {
            log_view = log_view.link_highlight_state(&handle.session.link_highlight_state);
        }

        frame.render_stateful_widget(log_view, areas.logs, &mut handle.session.log_view_state);
    } else {
        // No session selected - show empty log view
        let empty_logs: VecDeque<LogEntry> = VecDeque::new();
        let log_view = widgets::LogView::new(&empty_logs);
        let mut empty_state = LogViewState::new();
        frame.render_stateful_widget(log_view, areas.logs, &mut empty_state);
    }

    // Status bar - use session data if available, otherwise use global state
    if layout::use_compact_status(area) {
        frame.render_widget(widgets::StatusBarCompact::new(state), areas.status);
    } else {
        frame.render_widget(widgets::StatusBar::new(state), areas.status);
    }

    // Render modal overlays based on UI mode
    match state.ui_mode {
        UiMode::DeviceSelector | UiMode::Loading => {
            // Render device selector modal with session awareness
            let has_sessions = state.session_manager.has_running_sessions();
            let selector =
                widgets::DeviceSelector::with_session_state(&state.device_selector, has_sessions);
            frame.render_widget(selector, area);
        }
        UiMode::ConfirmDialog => {
            // Render confirmation dialog
            if let Some(ref dialog_state) = state.confirm_dialog_state {
                let dialog = widgets::ConfirmDialog::new(dialog_state);
                frame.render_widget(dialog, area);
            }
        }
        UiMode::EmulatorSelector => {
            // Render emulator selector with session awareness
            let has_sessions = state.session_manager.has_running_sessions();
            let selector =
                widgets::DeviceSelector::with_session_state(&state.device_selector, has_sessions);
            frame.render_widget(selector, area);
        }
        UiMode::SearchInput => {
            // Render search input at bottom of log area
            if let Some(handle) = state.session_manager.selected() {
                // Calculate position for inline search (bottom of log area, inside border)
                let search_area = Rect::new(
                    areas.logs.x + 1,
                    areas.logs.y + areas.logs.height.saturating_sub(2),
                    areas.logs.width.saturating_sub(2),
                    1,
                );

                // Clear the line and render search input
                frame.render_widget(Clear, search_area);
                frame.render_widget(
                    widgets::SearchInput::new(&handle.session.search_state).inline(),
                    search_area,
                );
            }
        }
        UiMode::Normal => {
            // No overlay - but show search status if search has results
            if let Some(handle) = state.session_manager.selected() {
                if !handle.session.search_state.query.is_empty() {
                    // Show mini search status at bottom of log area
                    let search_area = Rect::new(
                        areas.logs.x + 1,
                        areas.logs.y + areas.logs.height.saturating_sub(2),
                        areas.logs.width.saturating_sub(2),
                        1,
                    );

                    frame.render_widget(Clear, search_area);
                    frame.render_widget(
                        widgets::SearchInput::new(&handle.session.search_state).inline(),
                        search_area,
                    );
                }
            }
        }
        UiMode::LinkHighlight => {
            // Link mode is active - the log view handles badge rendering
            // via link_highlight_state passed above (Phase 3.1 Task 07)
            // Instruction bar shows available shortcuts (Phase 3.1 Task 08)
            if let Some(handle) = state.session_manager.selected() {
                let link_count = handle.session.link_highlight_state.link_count();

                // Calculate position for instruction bar (bottom of log area, inside border)
                let bar_area = Rect::new(
                    areas.logs.x + 1,
                    areas.logs.y + areas.logs.height.saturating_sub(2),
                    areas.logs.width.saturating_sub(2),
                    1,
                );

                // Clear the line
                frame.render_widget(Clear, bar_area);

                // Build instruction text based on link count
                let instruction = if link_count == 0 {
                    // Empty state (shouldn't normally happen)
                    Line::from(vec![
                        Span::styled(
                            " No links found in viewport ",
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                        Span::styled("Esc", Style::default().fg(Color::Yellow)),
                        Span::styled(" to exit", Style::default().fg(Color::DarkGray)),
                    ])
                } else {
                    // Determine shortcut range text
                    let shortcut_range = match link_count {
                        1 => "1".to_string(),
                        2..=9 => format!("1-{}", link_count),
                        10..=35 => {
                            let last_letter = (b'a' + (link_count - 10) as u8) as char;
                            format!("1-9,a-{}", last_letter)
                        }
                        _ => "1-9,a-z".to_string(),
                    };

                    Line::from(vec![
                        Span::styled(" Links: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            link_count.to_string(),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" │ Press ", Style::default().fg(Color::DarkGray)),
                        Span::styled(shortcut_range, Style::default().fg(Color::Yellow)),
                        Span::styled(" to open │ ", Style::default().fg(Color::DarkGray)),
                        Span::styled("Esc", Style::default().fg(Color::Yellow)),
                        Span::styled(" cancel │ ", Style::default().fg(Color::DarkGray)),
                        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
                        Span::styled(" scroll", Style::default().fg(Color::DarkGray)),
                    ])
                };

                let bar =
                    Paragraph::new(instruction).style(Style::default().bg(Color::Rgb(30, 30, 30)));

                frame.render_widget(bar, bar_area);
            }
        }
    }
}

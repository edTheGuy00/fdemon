//! Main render/view function (View in TEA pattern)

#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use super::{layout, widgets};
use crate::widgets::LogViewState;
use fdemon_app::state::{AppState, LoadingState, UiMode};
use fdemon_core::LogEntry;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::theme::{icons::IconSet, palette};

/// Render search overlay at the bottom of the log area
///
/// # Arguments
/// * `frame` - Frame to render to
/// * `areas` - Screen layout areas
/// * `state` - Application state (to access session manager)
/// * `force` - If true, always render even if query is empty (for SearchInput mode)
fn render_search_overlay(
    frame: &mut Frame,
    areas: &layout::ScreenAreas,
    state: &AppState,
    force: bool,
) {
    if let Some(handle) = state.session_manager.selected() {
        if force || !handle.session.search_state.query.is_empty() {
            let search_area = Rect::new(
                areas.logs.x + 1,
                areas.logs.y + areas.logs.height.saturating_sub(3),
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

/// Render the complete UI (View function in TEA)
///
/// This is a pure rendering function - it should not modify state
/// except for widget state that tracks rendering info (scroll position).
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();

    // Fill entire terminal with deepest background color
    let bg_block = Block::default().style(Style::default().bg(palette::DEEPEST_BG));
    frame.render_widget(bg_block, area);

    let session_count = state.session_manager.len();
    let areas = layout::create_with_sessions(area, session_count);

    // Construct IconSet from settings
    let icons = IconSet::new(state.settings.ui.icons);

    // Main header with project name and session tabs inside
    let header = widgets::MainHeader::new(state.project_name.as_deref(), icons)
        .with_sessions(&state.session_manager);
    frame.render_widget(header, areas.header);

    // Log view - use selected session's logs or show empty state
    if let Some(handle) = state.session_manager.selected_mut() {
        let mut log_view = widgets::LogView::new(&handle.session.logs, icons)
            .filter_state(&handle.session.filter_state)
            .wrap_mode(handle.session.log_view_state.wrap_mode);

        // Add search state if there's an active search
        if !handle.session.search_state.query.is_empty() {
            log_view = log_view.search_state(&handle.session.search_state);
        }

        // Add link highlight state if link mode is active (Phase 3.1)
        if handle.session.link_highlight_state.is_active() {
            log_view = log_view.link_highlight_state(&handle.session.link_highlight_state);
        }

        // Build status info for bottom metadata bar (Phase 2 Task 4)
        let duration = handle.session.session_duration().and_then(|d| {
            let secs = d.num_seconds();
            if secs >= 0 {
                Some(std::time::Duration::from_secs(secs as u64))
            } else {
                None
            }
        });
        let status_info = widgets::StatusInfo {
            phase: &handle.session.phase,
            is_busy: handle.session.is_busy(),
            mode: handle.session.launch_config.as_ref().map(|cfg| &cfg.mode),
            flavor: handle
                .session
                .launch_config
                .as_ref()
                .and_then(|cfg| cfg.flavor.as_deref()),
            duration,
            error_count: handle.session.error_count(),
            vm_connected: handle.session.vm_connected,
        };
        log_view = log_view.with_status(status_info);

        frame.render_stateful_widget(log_view, areas.logs, &mut handle.session.log_view_state);
    } else {
        // No session selected - show empty log view
        let empty_logs: VecDeque<LogEntry> = VecDeque::new();
        let log_view = widgets::LogView::new(&empty_logs, icons);
        let mut empty_state = LogViewState::new();
        frame.render_stateful_widget(log_view, areas.logs, &mut empty_state);
    }

    // Status bar removed - status info is now integrated into the log view's bottom metadata bar
    // (see StatusInfo building above, passed to LogView::with_status())

    // Render modal overlays based on UI mode
    match state.ui_mode {
        UiMode::Startup | UiMode::NewSessionDialog => {
            // Render NewSessionDialog for both startup (no sessions) and add session cases
            let dialog = widgets::NewSessionDialog::new(
                &state.new_session_dialog_state,
                &state.tool_availability,
                &icons,
            );
            frame.render_widget(dialog, area);
        }
        // Legacy DeviceSelector removed - use NewSessionDialog instead
        UiMode::EmulatorSelector => {
            // Legacy EmulatorSelector - not rendered
        }
        UiMode::Loading => {
            // Render loading screen (Task 08d)
            if let Some(ref loading) = state.loading_state {
                render_loading_screen(frame, state, loading, area);
            }
        }
        UiMode::ConfirmDialog => {
            // Render confirmation dialog
            if let Some(ref dialog_state) = state.confirm_dialog_state {
                let dialog = widgets::ConfirmDialog::new(dialog_state);
                frame.render_widget(dialog, area);
            }
        }
        UiMode::SearchInput => {
            // Render search input at bottom of log area, above bottom metadata bar
            render_search_overlay(frame, &areas, state, true);
        }
        UiMode::Normal => {
            // No overlay - but show search status if search has results
            render_search_overlay(frame, &areas, state, false);
        }
        UiMode::LinkHighlight => {
            // Link mode is active - the log view handles badge rendering
            // via link_highlight_state passed above (Phase 3.1 Task 07)
            // Instruction bar shows available shortcuts (Phase 3.1 Task 08)
            if let Some(handle) = state.session_manager.selected() {
                let link_count = handle.session.link_highlight_state.link_count();

                // Calculate position for instruction bar above bottom metadata bar
                let bar_area = Rect::new(
                    areas.logs.x + 1,
                    areas.logs.y + areas.logs.height.saturating_sub(3),
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
                            Style::default().fg(palette::TEXT_MUTED),
                        ),
                        Span::styled("│ ", Style::default().fg(palette::TEXT_MUTED)),
                        Span::styled("Esc", Style::default().fg(palette::STATUS_YELLOW)),
                        Span::styled(" to exit", Style::default().fg(palette::TEXT_MUTED)),
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
                        Span::styled(" Links: ", Style::default().fg(palette::TEXT_MUTED)),
                        Span::styled(
                            link_count.to_string(),
                            Style::default()
                                .fg(palette::ACCENT)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" │ Press ", Style::default().fg(palette::TEXT_MUTED)),
                        Span::styled(shortcut_range, Style::default().fg(palette::STATUS_YELLOW)),
                        Span::styled(" to open │ ", Style::default().fg(palette::TEXT_MUTED)),
                        Span::styled("Esc", Style::default().fg(palette::STATUS_YELLOW)),
                        Span::styled(" cancel │ ", Style::default().fg(palette::TEXT_MUTED)),
                        Span::styled("↑↓", Style::default().fg(palette::STATUS_YELLOW)),
                        Span::styled(" scroll", Style::default().fg(palette::TEXT_MUTED)),
                    ])
                };

                let bar =
                    Paragraph::new(instruction).style(Style::default().bg(palette::LINK_BAR_BG));

                frame.render_widget(bar, bar_area);
            }
        }
        UiMode::Settings => {
            // Full-screen settings panel
            let settings_panel = widgets::SettingsPanel::new(&state.settings, &state.project_path);
            frame.render_stateful_widget(settings_panel, area, &mut state.settings_view_state);
        } // Legacy StartupDialog removed - use NewSessionDialog instead
        UiMode::DevTools => {
            // DevTools mode renders into the log area (below the header/tabs)
            // so the project name and session tabs remain visible.
            let devtools = widgets::devtools::DevToolsView::new(
                &state.devtools_view_state,
                state.session_manager.selected(),
                icons,
            );
            frame.render_widget(devtools, areas.logs);
        }
    }
}

/// Render loading screen during startup initialization (Task 08d)
///
/// Displays a centered loading screen with:
/// - App name/logo
/// - Animated spinner
/// - Current loading message
fn render_loading_screen(frame: &mut Frame, state: &AppState, loading: &LoadingState, area: Rect) {
    // Braille spinner characters for smooth animation
    const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    // Direct modulo - each tick is 100ms, each frame shows next spinner char
    let spinner_idx = (loading.animation_frame as usize) % SPINNER.len();
    let spinner_char = SPINNER[spinner_idx];

    // Create centered content box - smaller modal overlay
    let vertical_center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Length(7),
            Constraint::Percentage(35),
        ])
        .split(area);

    let horizontal_center = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(vertical_center[1]);

    let center_area = horizontal_center[1];

    // Only clear the modal area, not the entire screen
    frame.render_widget(Clear, center_area);

    // Build content lines
    let mut lines = vec![];

    // App name/logo
    let app_name = if let Some(ref name) = state.project_name {
        name.clone()
    } else {
        "Flutter Demon".to_string()
    };

    lines.push(Line::from(vec![Span::styled(
        app_name,
        Style::default()
            .fg(palette::ACCENT)
            .add_modifier(Modifier::BOLD),
    )]));

    lines.push(Line::from("")); // Spacing

    // Spinner and message
    lines.push(Line::from(vec![
        Span::styled(
            spinner_char,
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            &loading.message,
            Style::default().fg(palette::TEXT_SECONDARY),
        ),
    ]));

    // Create block with border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette::BORDER_DIM))
        .style(Style::default().bg(palette::DEEPEST_BG));

    // Create paragraph with centered content
    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, center_area);
}

//! Main render/view function (View in TEA pattern)

#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use super::{layout, widgets};
use crate::widgets::LogViewState;
use fdemon_app::state::{AppState, LoadingState, ToastLevel, UiMode};
use fdemon_app::{MouseAction, MouseRect, MouseRegionsBuilder};
use fdemon_core::LogEntry;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Borrowed bridge between [`view`] and widgets that record clickable regions
/// during render.
///
/// `MouseCtx` exists so widgets do not need to thread `&mut MouseRegionsBuilder`
/// directly (which collides ergonomically with the `Widget::render` trait that
/// only sees `area` and `buf`). Widgets that need region recording accept an
/// `Option<&mut MouseCtx<'_>>` constructor argument; passing `None` keeps the
/// widget usable in tests that render without a registry.
///
/// # Temporary placeholder note
///
/// Phase 3 Tasks 06 (header bracket regions) and 07 (tab/device-pill regions)
/// will pass `&mut mouse_ctx` into `MainHeader` and `SessionTabs` respectively.
/// Until those tasks land the ctx is constructed but not forwarded to any widget.
#[derive(Debug)]
pub struct MouseCtx<'a> {
    builder: MouseRegionsBuilder<'a>,
}

impl<'a> MouseCtx<'a> {
    /// Wrap a [`MouseRegionsBuilder`] in a [`MouseCtx`].
    pub fn new(builder: MouseRegionsBuilder<'a>) -> Self {
        Self { builder }
    }

    /// Register a left-click region at `z_index = 0`.
    pub fn click(&mut self, rect: MouseRect, action: MouseAction) {
        self.builder.click(rect, action);
    }

    /// Register a left-click region at a specific `z_index`. Phase 5
    /// dialogs/overlays use `z_index = 1`; Phase 3 widgets stay at `0`.
    pub fn click_at_z(&mut self, rect: MouseRect, action: MouseAction, z: u8) {
        self.builder.click_at_z(rect, action, z);
    }

    /// Register a region with separate left and middle bindings (used by
    /// session tabs: left = select, middle = close).
    pub fn click_left_middle(
        &mut self,
        rect: MouseRect,
        on_left: MouseAction,
        on_middle: MouseAction,
    ) {
        self.builder.click_left_middle(rect, on_left, on_middle);
    }
}

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

/// Returns `true` when the current UI mode is a modal overlay that should
/// suppress base-UI (header + log view) mouse-region recording.
///
/// When a modal is active, z=0 regions registered by `MainHeader` and
/// `LogView` would be reachable via `hit_test` for clicks that fall outside
/// the modal's z=1 rects.  By passing `None` instead of `Some(&mut mouse_ctx)`
/// to those widgets we prevent spurious z=0 hits while the modal is up.
///
/// `UiMode::LinkHighlight` is intentionally excluded: links are overlaid on
/// top of the log view and the user expects both the log view and the badge
/// regions to remain interactive (scrolling, clicking links).
///
/// `UiMode::Settings` renders a full-screen panel that *replaces* the log
/// view entirely — the header is not rendered in Settings mode.  Settings
/// regions are at z=0 and rely on the full-screen panel, so we suppress the
/// underlying header/log-view base-UI regions there too.
fn is_modal_ui_mode(mode: &UiMode) -> bool {
    matches!(
        mode,
        UiMode::Startup
            | UiMode::NewSessionDialog
            | UiMode::ConfirmDialog
            | UiMode::Settings
            | UiMode::FlutterVersion
            | UiMode::EmulatorSelector
    )
}

/// Render the complete UI (View function in TEA)
///
/// This is a pure rendering function - it should not modify state
/// except for widget state that tracks rendering info (scroll position).
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();

    // ── Mouse region registry: take, clear, render, put back ─────────────
    // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
    // RAII guard puts the registry back on Drop, even if rendering panics.
    let mut regions = state.mouse_regions.take_guard();
    regions.clear();

    // Build the borrowed mouse context. `mouse_ctx` lives for the entire
    // render body so widgets can register regions throughout the frame.
    // The borrow of `*regions` (via DerefMut) is released when `mouse_ctx`
    // is dropped, after which the guard's Drop puts the registry back.
    let mut mouse_ctx = MouseCtx::new(regions.builder());

    // Determine whether a modal overlay is active.  When `in_modal` is true
    // we suppress base-UI (header + log view) region recording by passing
    // `None` instead of `Some(&mut mouse_ctx)` to those widgets.  The
    // tag-filter overlay (`tag_filter_visible`) is also "modal" for click
    // purposes even though `ui_mode` remains `Normal`.
    let in_modal = is_modal_ui_mode(&state.ui_mode) || state.tag_filter_visible;

    // Fill entire terminal with deepest background color
    let bg_block = Block::default().style(Style::default().bg(palette::DEEPEST_BG));
    frame.render_widget(bg_block, area);

    let session_count = state.session_manager.len();
    let areas = layout::create_with_sessions(area, session_count);

    // Construct IconSet from settings
    let icons = IconSet::new(state.settings.ui.icons);

    // Main header with project name and session tabs inside.
    // Pass `Some(&mut mouse_ctx)` so header shortcut regions are registered
    // (Task 06), but only when no modal overlay is active.  When a modal is
    // up (`in_modal = true`) we pass `None` so that clicking header shortcuts
    // while a modal is displayed cannot fire base-UI actions.
    let header = widgets::MainHeader::new(state.project_name.as_deref(), icons)
        .with_sessions(&state.session_manager);
    let header_ctx: Option<&mut MouseCtx<'_>> = if in_modal { None } else { Some(&mut mouse_ctx) };
    widgets::header::render_main_header(areas.header, frame.buffer_mut(), &header, header_ctx);

    // Log view - use selected session's logs or show empty state.
    // Same modal-gate as the header: pass `None` when a modal overlay is active
    // so that clicks that miss the modal's z=1 rects cannot fall through to the
    // underlying log-view z=0 regions.
    if let Some(handle) = state.session_manager.selected_mut() {
        let unseen = handle.session.unseen_log_count;
        let mut log_view = widgets::LogView::new(&handle.session.logs, icons)
            .filter_state(&handle.session.filter_state)
            .wrap_mode(handle.session.log_view_state.wrap_mode)
            .unseen_log_count(unseen);

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
            dap_port: state.dap_status.port(),
            dap_config_ide: state.dap_config_status.as_ref().map(|s| s.ide_name.clone()),
            mouse_capture_active: state.mouse_capture_active,
            animation_frame: state.animation_frame,
            progress: handle.session.current_progress.as_deref(),
        };
        log_view = log_view.with_status(status_info);

        let log_ctx: Option<&mut MouseCtx<'_>> = if in_modal { None } else { Some(&mut mouse_ctx) };
        widgets::log_view::render_with_regions(
            areas.logs,
            frame.buffer_mut(),
            &mut handle.session.log_view_state,
            log_view,
            log_ctx,
        );
    } else {
        // No session selected - show empty log view
        let empty_logs: VecDeque<LogEntry> = VecDeque::new();
        let log_view = widgets::LogView::new(&empty_logs, icons);
        let mut empty_state = LogViewState::new();
        let log_ctx: Option<&mut MouseCtx<'_>> = if in_modal { None } else { Some(&mut mouse_ctx) };
        widgets::log_view::render_with_regions(
            areas.logs,
            frame.buffer_mut(),
            &mut empty_state,
            log_view,
            log_ctx,
        );
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
            )
            .startup_notice(state.startup_notice.as_ref())
            .enable_mouse(state.settings.ui.enable_mouse)
            .animation_frame(state.animation_frame);
            widgets::new_session_dialog::render_with_regions(
                area,
                frame.buffer_mut(),
                dialog,
                Some(&mut mouse_ctx),
            );
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
                widgets::confirm_dialog::render_with_regions(
                    area,
                    frame.buffer_mut(),
                    dialog,
                    Some(&mut mouse_ctx),
                );
            }
        }
        UiMode::SearchInput => {
            // Render search input at bottom of log area, above bottom metadata bar
            render_search_overlay(frame, &areas, state, true);
        }
        UiMode::Normal => {
            // No overlay - but show search status if search has results
            render_search_overlay(frame, &areas, state, false);

            // Tag filter overlay (Phase 2, Task 09) — drawn on top of normal log view.
            if state.tag_filter_visible {
                if let Some(handle) = state.session_manager.selected() {
                    widgets::render_tag_filter_with_regions(
                        frame,
                        areas.logs,
                        &handle.native_tag_state,
                        &state.tag_filter_ui,
                        Some(&mut mouse_ctx),
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
            widgets::settings_panel::render_with_regions(
                area,
                frame.buffer_mut(),
                settings_panel,
                &mut state.settings_view_state,
                Some(&mut mouse_ctx),
            );
        }
        UiMode::FlutterVersion => {
            // Render Flutter Version panel as an overlay on top of the normal log view.
            // The underlying header and log view are already rendered above; here we
            // render the dimmed overlay + centered panel dialog on top of them.
            let panel = widgets::FlutterVersionPanel::new(&state.flutter_version_state);
            frame.render_widget(panel, area);
        }
        // Legacy StartupDialog removed - use NewSessionDialog instead
        UiMode::DevTools => {
            // DevTools mode renders into the log area (below the header/tabs)
            // so the project name and session tabs remain visible.
            let devtools = widgets::devtools::DevToolsView::new(
                &state.devtools_view_state,
                state.session_manager.selected(),
                icons,
            );
            widgets::devtools::render_with_regions(
                areas.logs,
                frame.buffer_mut(),
                devtools,
                Some(&mut mouse_ctx),
            );
        }
    }

    // ── Toast notifications ────────────────────────────────────────────────
    // Rendered last so they appear on top of all other UI elements.
    // Toasts are transient one-line overlays that expire automatically.
    if !state.toasts.is_empty() {
        render_toasts(frame, area, &state.toasts);
    }

    // ── Registry put-back handled by guard's Drop ─────────────────────────
    // The `regions` guard (MouseRegionGuard) calls `state.mouse_regions.set`
    // automatically when it drops here. No explicit set() needed.
    // `mouse_ctx`'s borrow of `*regions` ends at its last use above (NLL);
    // the guard can then drop and put the registry back.
}

/// Render transient toast notifications as one-line overlays near the bottom
/// of the screen.
///
/// Toasts are drawn on top of all other UI elements. When multiple toasts are
/// queued they stack upward (most recent at the bottom). Each row is cleared
/// with [`Clear`] before the text is painted so underlying UI elements do not
/// bleed through.
///
/// The accent colour follows [`ToastLevel`]:
/// - [`ToastLevel::Warn`] — yellow (`STATUS_YELLOW`)
/// - [`ToastLevel::Info`] — blue (`STATUS_BLUE`)
fn render_toasts(frame: &mut Frame, area: Rect, toasts: &[fdemon_app::state::Toast]) {
    use crate::theme::palette;

    /// Left/right padding inside the toast pill.
    const HORIZONTAL_PADDING: u16 = 2;
    /// Vertical offset from the bottom edge of `area`.
    /// 2 rows up keeps the toast above the typical bottom metadata bar.
    const BOTTOM_OFFSET: u16 = 2;
    /// Display-width budget reserved for the leading icon.
    ///
    /// The icon string is `"⚠ "` or `"ℹ "` (2 codepoints each: glyph +
    /// space). The warning/info glyph is a non-ASCII codepoint that some
    /// terminals render at 2 cells (default-width / emoji presentation) and
    /// others at 1 cell (text presentation). Plus the trailing space gives
    /// 2–3 cells in practice. We budget 4 cells to leave a 1–2 cell safety
    /// margin so the pill never clips the icon — the cost is at most two
    /// blank cells on the right when text fits exactly.
    const ICON_DISPLAY_WIDTH: u16 = 4;

    // Render most recent toast at the bottom; older ones stack above it.
    for (i, toast) in toasts.iter().rev().enumerate() {
        let row_from_bottom = BOTTOM_OFFSET + i as u16;
        // Stop if we would overflow the top of the area.
        if row_from_bottom >= area.height {
            break;
        }
        let y = area.y + area.height.saturating_sub(row_from_bottom + 1);

        // Truncate the message to fit in the available width.
        let max_text_chars =
            area.width
                .saturating_sub(HORIZONTAL_PADDING * 2 + ICON_DISPLAY_WIDTH) as usize;
        let label = if toast.text.chars().count() > max_text_chars {
            format!(
                "{}…",
                toast
                    .text
                    .chars()
                    .take(max_text_chars.saturating_sub(1))
                    .collect::<String>()
            )
        } else {
            toast.text.clone()
        };

        let (accent, icon) = match toast.level {
            ToastLevel::Warn => (palette::STATUS_YELLOW, "⚠ "),
            ToastLevel::Info => (palette::STATUS_BLUE, "ℹ "),
        };

        // Use ICON_DISPLAY_WIDTH (not icon.chars().count()) so the toast
        // rect matches the budget used in `max_text_chars` above. Using
        // `chars().count() == 2` would undersize the rect on terminals
        // that render the glyph at 2 cells, clipping the icon.
        let text_width = (label.chars().count()
            + ICON_DISPLAY_WIDTH as usize
            + HORIZONTAL_PADDING as usize * 2) as u16;
        let toast_width = text_width.min(area.width);
        // Right-align the toast pill.
        let x = area
            .x
            .saturating_add(area.width.saturating_sub(toast_width));

        let toast_area = Rect::new(x, y, toast_width, 1);

        // Clear the row before painting the pill.
        frame.render_widget(Clear, toast_area);

        let line = Line::from(vec![
            Span::raw(" ".repeat(HORIZONTAL_PADDING as usize)),
            Span::styled(icon, Style::default().fg(accent)),
            Span::styled(label, Style::default().fg(palette::TEXT_PRIMARY)),
            Span::raw(" ".repeat(HORIZONTAL_PADDING as usize)),
        ]);

        let paragraph = Paragraph::new(line).style(Style::default().bg(palette::POPUP_BG));
        frame.render_widget(paragraph, toast_area);
    }
}

/// Render loading screen during startup initialization (Task 08d)
///
/// Displays a centered loading screen with:
/// - App name/logo
/// - Animated spinner
/// - Current loading message
fn render_loading_screen(frame: &mut Frame, state: &AppState, loading: &LoadingState, area: Rect) {
    // Use shared spinner helper — zero visual change; SPINNER_FRAMES[0] == '⠋'
    let glyph = crate::widgets::spinner::spinner_char(loading.animation_frame);

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
            glyph.to_string(),
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

//! Header bar widgets
//!
//! Provides the main header with project name and keybindings.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use fdemon_app::session_manager::SessionManager;
use fdemon_app::{Message, MouseAction, MouseRect};

use crate::theme::{icons::IconSet, palette, styles};
use crate::widgets::MouseCtx;

use super::tabs::render_session_tabs;

/// App version from Cargo.toml, surfaced in the title bar
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Padding around header sections for layout calculations
const HEADER_SECTION_PADDING: u16 = 4;

/// Peak blend fraction toward `STATUS_GREEN` at full flash intensity.
///
/// Kept well below 1.0 so the header tints (not floods) green — readability
/// of the title and tab labels is preserved. Tuned for a subtle but visible
/// pulse at the 50 ms tick cadence.
const RELOAD_FLASH_BLEND_CAP: f32 = 0.35;

/// Main header showing app title, project name, and keybindings
/// with optional session tabs rendered inside the bordered area
pub struct MainHeader<'a> {
    project_name: Option<&'a str>,
    session_manager: Option<&'a SessionManager>,
    icons: IconSet,
    /// Reload-success flash intensity in `[0.0, 1.0]`.
    /// `0.0` = no tint (steady state); `1.0` = peak blend at completion.
    /// Set via `.reload_flash(..)` builder.
    reload_flash: f32,
}

impl<'a> MainHeader<'a> {
    pub fn new(project_name: Option<&'a str>, icons: IconSet) -> Self {
        Self {
            project_name,
            session_manager: None,
            icons,
            reload_flash: 0.0,
        }
    }

    /// Add session manager to render tabs inside the header
    pub fn with_sessions(mut self, session_manager: &'a SessionManager) -> Self {
        self.session_manager = Some(session_manager);
        self
    }

    /// Tint the header background toward the success green by this `0.0..=1.0`
    /// reload-flash intensity (see `Session::reload_flash_alpha`).
    ///
    /// When `alpha` is `0.0` the header renders with the standard `CARD_BG`
    /// background. At `1.0` the background is blended by `RELOAD_FLASH_BLEND_CAP`
    /// toward `STATUS_GREEN`. The value is clamped to `[0.0, 1.0]` on store, so
    /// `self.reload_flash` is always a valid intensity even if a future caller
    /// passes an out-of-range value. (The sole current caller,
    /// `Session::reload_flash_alpha`, already returns a value in range.)
    pub fn reload_flash(mut self, alpha: f32) -> Self {
        self.reload_flash = alpha.clamp(0.0, 1.0);
        self
    }
}

impl Widget for MainHeader<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_main_header(area, buf, &self, None);
    }
}

/// Render the main header, optionally recording clickable regions into `ctx`.
///
/// This is the canonical render entry point used by `render::view`.  The
/// `Widget::render` impl delegates here with `ctx = None` so that existing tests
/// which call `frame.render_widget(header, area)` continue to work without any
/// mouse-region machinery.
///
/// # Arguments
/// * `area` - The full area (including border) allocated to the header.
/// * `buf` - Ratatui cell buffer to paint into.
/// * `header` - Borrowed `MainHeader` descriptor.
/// * `ctx` - Optional mutable reference to the per-frame mouse region context.
///   When `None`, no regions are registered (test-safe default).
pub fn render_main_header(
    area: Rect,
    buf: &mut Buffer,
    header: &MainHeader<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    // Blend header background from CARD_BG toward STATUS_GREEN based on the
    // reload-flash alpha. At alpha == 0.0, lerp_color returns CARD_BG unchanged.
    let bg = crate::widgets::shimmer::lerp_color(
        palette::CARD_BG,
        palette::STATUS_GREEN,
        header.reload_flash * RELOAD_FLASH_BLEND_CAP,
    );
    // Render glass container with rounded borders
    let block = styles::glass_block(false).style(Style::default().bg(bg));

    // Get inner content area (inside borders) before rendering
    let inner = block.inner(area);

    // Now render the block
    block.render(area, buf);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // Check if we have multiple sessions (need to show tabs)
    let has_multiple_sessions = header
        .session_manager
        .map(|sm| sm.len() > 1)
        .unwrap_or(false);

    if has_multiple_sessions {
        // Multi-session mode: split into title row and tabs row
        if inner.height >= 2 {
            // Title row — no shortcuts in multi-session mode (show_device = false)
            let title_area = Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            };
            header.render_title_row(title_area, buf, false, None);

            // Tabs row
            let tabs_area = Rect {
                x: inner.x,
                y: inner.y + 1,
                width: inner.width,
                height: inner.height.saturating_sub(1),
            };
            if let Some(session_manager) = header.session_manager {
                render_session_tabs(tabs_area, buf, session_manager, header.icons, ctx);
            }
        } else {
            // Not enough space for both rows, just render title
            header.render_title_row(inner, buf, false, None);
        }
    } else {
        // Single session or no session: render title + shortcuts + device pill in one row
        header.render_title_row(inner, buf, true, ctx);
    }
}

/// Mapping of bracketed shortcut characters to their `Message` actions.
///
/// Order matches left-to-right rendering order: r, R, x, d, D, q.
/// Each entry is `(rendered_label, action_message)` where `rendered_label`
/// is the text shown after the closing bracket (used to compute span widths).
#[allow(clippy::type_complexity)]
const SHORTCUTS_DEF: &[(&str, fn() -> Message)] = &[
    ("Run  ", || Message::HotReload),
    ("Restart  ", || Message::HotRestart),
    ("Stop  ", || Message::CloseCurrentSession),
    ("Debug  ", || Message::EnterDevToolsMode),
    ("DAP  ", || Message::ToggleDap),
    ("Quit", || Message::RequestQuit),
];

/// Width in terminal cells of the non-clickable prefix of each shortcut segment:
/// `'[' (1) + key_char (1) + ']' (1) + ' ' (1)`. The full segment is this prefix plus the
/// trailing label text (e.g., `"Run  "`). Used in `register_shortcut_clicks` to advance the
/// cursor between adjacent shortcuts.
const SHORTCUT_SEGMENT_PREFIX: u16 = 4;

/// Width in terminal cells of the clickable `[X` portion of each shortcut.
/// Only the bracket and letter are clickable, not the closing bracket or label.
const SHORTCUT_CLICK_WIDTH: u16 = 2;

/// Register left-click regions for the six bracketed shortcuts rendered on the
/// title row.  Called only when the full shortcut line fits (`total_content_width
/// <= area.width` branch).
///
/// `shortcuts_x` is the x-coordinate where the first shortcut `[` was painted.
/// `row_y` is the y-coordinate of the title row.
fn register_shortcut_clicks(ctx: &mut MouseCtx<'_>, shortcuts_x: u16, row_y: u16, area: Rect) {
    let mut cursor_x = shortcuts_x;
    for (label, make_msg) in SHORTCUTS_DEF {
        let click_x = cursor_x;

        // Full segment width: `[` (1) + letter (1) + `] ` (2) + label_text
        // where label already includes trailing spaces (e.g., "Run  " = 5 chars).
        let segment_width: u16 = u16::try_from(SHORTCUT_SEGMENT_PREFIX as usize + label.len())
            .expect("shortcut label fits in u16 segment width");
        cursor_x = cursor_x.saturating_add(segment_width);

        // Skip if the clickable cells fall outside the visible area.
        if click_x.saturating_add(SHORTCUT_CLICK_WIDTH) > area.x.saturating_add(area.width) {
            continue;
        }

        let rect = MouseRect::new(click_x, row_y, SHORTCUT_CLICK_WIDTH, 1);
        ctx.click(rect, MouseAction::emit(make_msg()));
    }
}

impl MainHeader<'_> {
    /// Render the title row with status dot, project name, shortcuts, and optional device pill.
    ///
    /// `show_device` — when `false` (multi-session mode) the device pill and shortcut hints are
    /// suppressed; only the left section (title, project name) is rendered.
    ///
    /// `ctx` — optional mouse context; when `Some`, shortcut click regions are registered.
    fn render_title_row(
        &self,
        area: Rect,
        buf: &mut Buffer,
        show_device: bool,
        ctx: Option<&mut MouseCtx<'_>>,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let project_name = self.project_name.unwrap_or("flutter");

        // Get status dot and device info from selected session
        let (status_icon, status_style, device_name, device_platform) =
            if let Some(session_manager) = self.session_manager {
                if let Some(handle) = session_manager.selected() {
                    let session = &handle.session;
                    let (icon, _label, style) =
                        styles::phase_indicator(&session.phase, &self.icons);
                    (
                        icon,
                        style,
                        Some(session.device_name.as_str()),
                        Some(session.platform.as_str()),
                    )
                } else {
                    (
                        self.icons.circle(),
                        Style::default().fg(palette::TEXT_MUTED),
                        None,
                        None,
                    )
                }
            } else {
                (
                    self.icons.circle(),
                    Style::default().fg(palette::TEXT_MUTED),
                    None,
                    None,
                )
            };

        // Build left section: status dot + "Flutter Demon" + version + "/" + project name
        let left_spans = vec![
            Span::raw(" "),
            Span::styled(status_icon, status_style),
            Span::raw(" "),
            Span::styled(
                "Flutter Demon",
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("v{}", APP_VERSION),
                Style::default().fg(palette::TEXT_MUTED),
            ),
            Span::raw(" "),
            Span::styled("/", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw(" "),
            Span::styled(project_name, Style::default().fg(palette::TEXT_SECONDARY)),
        ];

        let left_line = Line::from(left_spans);
        let left_width = left_line.width() as u16;

        // Build shortcut hints (center section)
        let shortcuts = vec![
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("r", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Run  ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("R", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Restart  ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("x", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Stop  ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("d", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Debug  ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("D", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] DAP  ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled("q", Style::default().fg(palette::STATUS_YELLOW)),
            Span::styled("] Quit", Style::default().fg(palette::TEXT_MUTED)),
        ];
        let shortcuts_line = Line::from(shortcuts);
        let shortcuts_width = shortcuts_line.width() as u16;

        // Build device pill (right section) if single session
        let device_content = if show_device && device_name.is_some() {
            let device_icon = device_icon_for_platform(device_platform, &self.icons);
            let device_spans = vec![
                Span::raw(" "),
                Span::raw(device_icon),
                Span::raw(" "),
                Span::styled(
                    device_name.unwrap_or(""),
                    Style::default().fg(palette::ACCENT),
                ),
                Span::raw(" "),
            ];
            Some(Line::from(device_spans))
        } else {
            None
        };
        let device_width = device_content
            .as_ref()
            .map(|l| l.width() as u16)
            .unwrap_or(0);

        // Calculate available space and positioning
        let total_content_width =
            left_width + shortcuts_width + device_width + HEADER_SECTION_PADDING;

        if total_content_width <= area.width {
            // Everything fits: left | center | right layout
            buf.set_line(area.x, area.y, &left_line, area.width);

            // Center the shortcuts
            let shortcuts_x = area.x + left_width + 2;
            if shortcuts_x + shortcuts_width <= area.x + area.width {
                buf.set_line(shortcuts_x, area.y, &shortcuts_line, shortcuts_width);

                // Register clickable regions for each bracketed shortcut, but only
                // when the shortcut line was actually rendered into the buffer.
                if let Some(ctx) = ctx {
                    register_shortcut_clicks(ctx, shortcuts_x, area.y, area);
                }
            }

            // Right-align device pill
            if let Some(device_line) = device_content {
                let device_x = area.x + area.width - device_width;
                if device_x >= area.x + left_width + shortcuts_width + HEADER_SECTION_PADDING {
                    buf.set_line(device_x, area.y, &device_line, device_width);
                }
            }
        } else if left_width + device_width + 2 <= area.width {
            // Shortcuts don't fit, but left + device does — no region registration.
            buf.set_line(area.x, area.y, &left_line, area.width);

            if let Some(device_line) = device_content {
                let device_x = area.x + area.width - device_width;
                if device_x >= area.x + left_width + 2 {
                    buf.set_line(device_x, area.y, &device_line, device_width);
                }
            }
        } else {
            // Only left section fits — no region registration.
            buf.set_line(area.x, area.y, &left_line, area.width);
        }
    }
}

/// Map platform string to device icon
fn device_icon_for_platform(platform: Option<&str>, icons: &IconSet) -> &'static str {
    match platform {
        Some(p) if p.contains("ios") || p.contains("simulator") => icons.smartphone(),
        Some(p) if p.contains("web") || p.contains("chrome") => icons.globe(),
        Some(p) if p.contains("macos") || p.contains("linux") || p.contains("windows") => {
            icons.monitor()
        }
        _ => icons.cpu(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{test_device_with_platform, TestTerminal};
    use fdemon_app::config::IconMode;

    #[test]
    fn test_header_renders_title() {
        let mut term = TestTerminal::new();
        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(None, icons);

        term.render_widget(header, term.area());

        // Should contain app name
        assert!(
            term.buffer_contains("Flutter Demon"),
            "Header should contain app title"
        );
    }

    #[test]
    fn test_header_renders_project_name() {
        let mut term = TestTerminal::new();
        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(Some("my_flutter_app"), icons);

        term.render_widget(header, term.area());

        assert!(
            term.buffer_contains("my_flutter_app"),
            "Header should contain project name"
        );
    }

    #[test]
    fn test_header_without_project_name() {
        let mut term = TestTerminal::new();
        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(None, icons);

        term.render_widget(header, term.area());

        // Should still render without crashing
        let content = term.content();
        assert!(!content.is_empty(), "Header should render something");
        // Default fallback is "flutter"
        assert!(
            term.buffer_contains("flutter"),
            "Header should use default project name"
        );
    }

    #[test]
    fn test_header_with_sessions() {
        let mut term = TestTerminal::new();
        let mut session_manager = SessionManager::new();

        // Add mock sessions
        session_manager
            .create_session(&test_device_with_platform("device1", "iPhone 15", "ios"))
            .unwrap();
        session_manager
            .create_session(&test_device_with_platform("device2", "Pixel 7", "android"))
            .unwrap();

        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(Some("test_app"), icons).with_sessions(&session_manager);

        term.render_widget(header, term.area());

        // Verify session tabs appear (tabs show device names with status icons)
        assert!(
            term.buffer_contains("iPhone 15"),
            "Header should show first session device name"
        );
        assert!(
            term.buffer_contains("Pixel 7"),
            "Header should show second session device name"
        );
        // Check for status icon (○ for initializing sessions)
        assert!(
            term.buffer_contains("○"),
            "Header should show status icons for sessions"
        );
    }

    #[test]
    fn test_header_truncates_long_project_name() {
        let mut term = TestTerminal::with_size(40, 5); // Narrow terminal
        let long_name = "this_is_a_very_long_flutter_project_name_that_should_truncate";
        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(Some(long_name), icons);

        term.render_widget(header, term.area());

        // Should not overflow - verify no panic and content fits
        let content = term.content();
        assert!(!content.is_empty(), "Should render without panic");
        // The header renders the full name but it gets truncated by the terminal width
        // Verify basic rendering worked without panic
        assert!(
            term.buffer_contains("Flutter Demon"),
            "Should still show app title"
        );
    }

    #[test]
    fn test_header_compact_mode() {
        let mut term = TestTerminal::compact();
        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(Some("app"), icons);

        term.render_widget(header, term.area());

        // Should adapt to compact size
        let content = term.content();
        assert!(!content.is_empty(), "Should render in compact mode");
        assert!(
            term.buffer_contains("Flutter Demon"),
            "Should contain title in compact mode"
        );
    }

    #[test]
    fn test_header_with_keybindings() {
        // Use wider terminal (120 cols) to ensure shortcuts fit
        let mut term = TestTerminal::with_size(120, 24);
        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(Some("test_project"), icons);

        term.render_widget(header, term.area());

        // Verify keybindings are present with new format (includes labels)
        assert!(term.buffer_contains("[r] Run"), "Should show reload key");
        assert!(
            term.buffer_contains("[R] Restart"),
            "Should show restart key"
        );
        assert!(term.buffer_contains("[x] Stop"), "Should show stop key");
        assert!(
            term.buffer_contains("[d] Debug"),
            "Should show debug/device selector key"
        );
        assert!(term.buffer_contains("[D] DAP"), "Should show DAP key");
        assert!(term.buffer_contains("[q] Quit"), "Should show quit key");
    }

    #[test]
    fn test_header_without_sessions() {
        let mut term = TestTerminal::new();
        let session_manager = SessionManager::new(); // Empty session manager

        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(Some("test_app"), icons).with_sessions(&session_manager);

        term.render_widget(header, term.area());

        // Should render without tabs when no sessions
        let content = term.content();
        assert!(!content.is_empty(), "Should render without sessions");
        assert!(term.buffer_contains("test_app"), "Should show project name");
    }

    #[test]
    fn test_header_with_nerd_fonts() {
        let mut term = TestTerminal::new();
        let icons = IconSet::new(IconMode::NerdFonts);
        let header = MainHeader::new(Some("test_project"), icons);

        term.render_widget(header, term.area());

        // Verify header renders without errors with NerdFonts mode
        let content = term.content();
        assert!(!content.is_empty(), "Should render with NerdFonts mode");
        assert!(
            term.buffer_contains("test_project"),
            "Should show project name"
        );
    }

    #[test]
    fn test_device_icon_for_platform_ios() {
        let icons = IconSet::new(IconMode::Unicode);
        assert_eq!(
            device_icon_for_platform(Some("ios"), &icons),
            icons.smartphone()
        );
    }

    #[test]
    fn test_device_icon_for_platform_web() {
        let icons = IconSet::new(IconMode::Unicode);
        assert_eq!(
            device_icon_for_platform(Some("web-chrome"), &icons),
            icons.globe()
        );
    }

    #[test]
    fn test_device_icon_for_platform_desktop() {
        let icons = IconSet::new(IconMode::Unicode);
        assert_eq!(
            device_icon_for_platform(Some("macos"), &icons),
            icons.monitor()
        );
    }

    #[test]
    fn test_device_icon_for_platform_unknown() {
        let icons = IconSet::new(IconMode::Unicode);
        assert_eq!(device_icon_for_platform(None, &icons), icons.cpu());
    }

    #[test]
    fn test_device_icon_for_platform_nerd_fonts() {
        let icons = IconSet::new(IconMode::NerdFonts);
        // Just verify the function works with NerdFonts - the actual icons differ
        assert_eq!(
            device_icon_for_platform(Some("ios"), &icons),
            icons.smartphone()
        );
        assert_eq!(device_icon_for_platform(Some("web"), &icons), icons.globe());
        assert_eq!(
            device_icon_for_platform(Some("macos"), &icons),
            icons.monitor()
        );
    }

    #[test]
    fn test_header_renders_version() {
        let mut term = TestTerminal::new();
        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(None, icons);
        term.render_widget(header, term.area());

        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        assert!(
            term.buffer_contains(&version),
            "Header should contain version string"
        );
    }

    #[test]
    fn test_header_version_visible_in_narrow_terminal() {
        // Version is part of the left section which is always rendered
        let mut term = TestTerminal::with_size(50, 5);
        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(Some("app"), icons);
        term.render_widget(header, term.area());

        assert!(term.buffer_contains("Flutter Demon"), "Title should show");
        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        assert!(term.buffer_contains(&version), "Version should show");
    }

    // ── Mouse region tests (Task 06) ─────────────────────────────────────

    #[test]
    fn header_records_six_bracketed_shortcut_regions_at_120x24() {
        use fdemon_app::{AppState, MouseAction};
        use ratatui::{backend::TestBackend, Terminal};

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new();

        terminal
            .draw(|f| crate::render::view(f, &mut state))
            .unwrap();

        let regions = state.mouse_regions.take();
        let actions: Vec<_> = regions
            .iter()
            .filter_map(|e| e.on_left.as_ref().map(|a| (e.rect, a.clone())))
            .collect();

        // Expect at least the 6 bracketed-shortcut regions on the header row.
        assert!(
            actions.len() >= 6,
            "expected >= 6 shortcut regions, got {}",
            actions.len()
        );

        // Order is r, R, x, d, D, q — left-to-right.
        let messages: Vec<_> = actions
            .iter()
            .filter_map(|(_, a)| match a {
                MouseAction::Emit(m) => Some(format!("{:?}", m)),
                _ => None,
            })
            .collect();
        assert!(
            messages.iter().any(|m| m.contains("HotReload")),
            "no HotReload region"
        );
        assert!(
            messages.iter().any(|m| m.contains("HotRestart")),
            "no HotRestart region"
        );
        assert!(
            messages.iter().any(|m| m.contains("CloseCurrentSession")),
            "no Close region"
        );
        assert!(
            messages.iter().any(|m| m.contains("EnterDevToolsMode")),
            "no DevTools region"
        );
        assert!(
            messages.iter().any(|m| m.contains("ToggleDap")),
            "no DAP region"
        );
        assert!(
            messages.iter().any(|m| m.contains("RequestQuit")),
            "no Quit region"
        );
    }

    #[test]
    fn header_skips_region_recording_when_shortcuts_clipped() {
        // At 40 cols, the existing header logic falls into the "Only left section
        // fits" branch and does not render shortcuts. No regions should be
        // registered for shortcuts.
        use fdemon_app::AppState;
        use ratatui::{backend::TestBackend, Terminal};

        let backend = TestBackend::new(40, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new();

        terminal
            .draw(|f| crate::render::view(f, &mut state))
            .unwrap();

        let regions = state.mouse_regions.take();
        // No bracketed-shortcut regions at this width.
        let shortcut_count = regions
            .iter()
            .filter(|e| e.rect.width == 2 && e.rect.height == 1)
            .count();
        assert_eq!(
            shortcut_count, 0,
            "shortcuts not visible at 40 cols => no clickable regions"
        );
    }

    #[test]
    fn header_shortcut_rect_is_two_cells_wide() {
        use fdemon_app::{AppState, Message, MouseAction};
        use ratatui::{backend::TestBackend, Terminal};

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new();

        terminal
            .draw(|f| crate::render::view(f, &mut state))
            .unwrap();

        let regions = state.mouse_regions.take();
        // Find the HotReload region; it should be exactly 2 cells wide.
        let entry = regions
            .iter()
            .find(|e| {
                matches!(
                    &e.on_left,
                    Some(MouseAction::Emit(m)) if matches!(**m, Message::HotReload)
                )
            })
            .expect("HotReload region must be registered");
        assert_eq!(entry.rect.width, 2);
        assert_eq!(entry.rect.height, 1);
    }

    // ── Reload-flash background tint tests (Phase 6, Task 02) ────────────────

    /// Helper: render a `MainHeader` with the given `reload_flash` alpha into an
    /// 80×5 buffer and return the background color of the first inner cell (row 1,
    /// col 1 — inside the rounded-border block).
    fn render_header_bg(alpha: f32) -> ratatui::style::Color {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let area = Rect::new(0, 0, 80, 5);
        let mut buf = Buffer::empty(area);
        let icons = IconSet::new(fdemon_app::config::IconMode::Unicode);
        let header = MainHeader::new(Some("test"), icons).reload_flash(alpha);
        render_main_header(area, &mut buf, &header, None);
        // Row 0, col 0 is the top-left border cell. Row 1, col 1 is the first
        // inner cell — inside the rounded glass_block border — which carries the
        // block background style.
        buf[(1, 1)]
            .style()
            .bg
            .unwrap_or(ratatui::style::Color::Reset)
    }

    #[test]
    fn header_bg_unchanged_without_flash() {
        // With alpha == 0.0, the header background must be exactly CARD_BG.
        let bg = render_header_bg(0.0);
        assert_eq!(
            bg,
            palette::CARD_BG,
            "expected CARD_BG with reload_flash=0.0, got {bg:?}"
        );
    }

    #[test]
    fn header_bg_tints_toward_green_with_flash() {
        // With alpha == 1.0, the background must be blended — neither CARD_BG nor
        // STATUS_GREEN (the blend cap is 0.35, so it sits between them).
        let bg = render_header_bg(1.0);
        assert_ne!(
            bg,
            palette::CARD_BG,
            "expected tinted bg with reload_flash=1.0, got CARD_BG unchanged"
        );
        assert_ne!(
            bg,
            palette::STATUS_GREEN,
            "expected partial blend (not full STATUS_GREEN) with reload_flash=1.0"
        );
        // Verify the tint is an RGB interpolation between the two palette colors.
        // Derive the bounds from the constants themselves so this stays correct
        // if the palette changes. At t = 1.0 * 0.35 the green channel lands
        // strictly between CARD_BG.g and STATUS_GREEN.g.
        let ratatui::style::Color::Rgb(_, card_g, _) = palette::CARD_BG else {
            panic!("CARD_BG must be an Rgb color");
        };
        let ratatui::style::Color::Rgb(_, green_g, _) = palette::STATUS_GREEN else {
            panic!("STATUS_GREEN must be an Rgb color");
        };
        if let ratatui::style::Color::Rgb(_r, g, _b) = bg {
            assert!(
                g > card_g && g < green_g,
                "green channel {g} should be between CARD_BG.g ({card_g}) and STATUS_GREEN.g ({green_g})"
            );
        } else {
            panic!("expected Rgb color, got {bg:?}");
        }
    }
}

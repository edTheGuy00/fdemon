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
use fdemon_app::{Message, MouseAction, MouseRect, StatusBadge, StatusBadgeKind};

use crate::theme::{branding, icons::IconSet, palette, styles};
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
    /// Optional embedder status badge rendered at the right edge of the title
    /// row (e.g. `"MCP 2 clients"`). Set via `.status_badge(..)` builder;
    /// `None` (the default) renders nothing and changes no layout.
    status_badge: Option<&'a StatusBadge>,
}

impl<'a> MainHeader<'a> {
    pub fn new(project_name: Option<&'a str>, icons: IconSet) -> Self {
        Self {
            project_name,
            session_manager: None,
            icons,
            reload_flash: 0.0,
            status_badge: None,
        }
    }

    /// Add session manager to render tabs inside the header
    pub fn with_sessions(mut self, session_manager: &'a SessionManager) -> Self {
        self.session_manager = Some(session_manager);
        self
    }

    /// Show a generic embedder status badge near the right edge of the title
    /// row. `None` leaves the header exactly as it renders without a badge.
    pub fn status_badge(mut self, badge: Option<&'a StatusBadge>) -> Self {
        self.status_badge = badge;
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
        // Multi-session mode: title → dim separator rule → tabs
        let title_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };

        if inner.height >= 3 {
            // Normal case (inner.height == 3 given header_height == 5):
            // title row, then dim separator rule, then tabs row.
            header.render_title_row(title_area, buf, false, None);

            let sep_area = Rect {
                x: inner.x,
                y: inner.y + 1,
                width: inner.width,
                height: 1,
            };
            render_separator_row(sep_area, buf, bg);

            let tabs_area = Rect {
                x: inner.x,
                y: inner.y + 2,
                width: inner.width,
                height: inner.height.saturating_sub(2),
            };
            if let Some(session_manager) = header.session_manager {
                render_session_tabs(tabs_area, buf, session_manager, header.icons, ctx);
            }
        } else if inner.height == 2 {
            // Squeezed terminal: title + tabs adjacent, no separator (prior behaviour)
            header.render_title_row(title_area, buf, false, None);
            let tabs_area = Rect {
                x: inner.x,
                y: inner.y + 1,
                width: inner.width,
                height: 1,
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
                branding::APP_TITLE,
                Style::default()
                    .fg(branding::TITLE_COLOR)
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

        // Build embedder status badge (rightmost section), if set.
        let badge_content = self.status_badge.map(|badge| {
            let color = match badge.kind {
                StatusBadgeKind::Info => palette::ACCENT,
                StatusBadgeKind::Active => palette::STATUS_GREEN,
                StatusBadgeKind::Warn => palette::STATUS_YELLOW,
            };
            Line::from(vec![
                Span::styled(self.icons.circle(), Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(badge.text.clone(), Style::default().fg(color)),
                Span::raw(" "),
            ])
        });
        let badge_width = badge_content
            .as_ref()
            .map(|l| l.width() as u16)
            .unwrap_or(0);
        // The badge claims the rightmost cells of the row; sections that were
        // previously right-aligned (the device pill) anchor against `badge_x`
        // instead of the area edge. With no badge, `badge_x` equals the right
        // edge, so the layout is byte-for-byte identical to before.
        let badge_x = (area.x + area.width).saturating_sub(badge_width);

        // Calculate available space and positioning
        let total_content_width =
            left_width + shortcuts_width + device_width + badge_width + HEADER_SECTION_PADDING;

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

            // Right-align device pill (left of the badge when one is set)
            if let Some(device_line) = device_content {
                let device_x = badge_x.saturating_sub(device_width);
                if device_x >= area.x + left_width + shortcuts_width + HEADER_SECTION_PADDING {
                    buf.set_line(device_x, area.y, &device_line, device_width);
                }
            }
        } else if left_width + device_width + badge_width + 2 <= area.width {
            // Shortcuts don't fit, but left + device does — no region registration.
            buf.set_line(area.x, area.y, &left_line, area.width);

            if let Some(device_line) = device_content {
                let device_x = badge_x.saturating_sub(device_width);
                if device_x >= area.x + left_width + 2 {
                    buf.set_line(device_x, area.y, &device_line, device_width);
                }
            }
        } else {
            // Only left section fits — no region registration.
            buf.set_line(area.x, area.y, &left_line, area.width);
        }

        // Paint the badge last so it can never be overdrawn by the sections
        // above; skipped when it would collide with the left section.
        if let Some(badge_line) = badge_content {
            if badge_x > area.x + left_width {
                buf.set_line(badge_x, area.y, &badge_line, badge_width);
            }
        }
    }
}

/// Render a dim horizontal rule separating the title row from the device tabs
/// in the multi-session header. Inset by one cell on each side to align with the
/// tabs' left/right padding; painted on `bg` so it tints with the reload flash.
fn render_separator_row(area: Rect, buf: &mut Buffer, bg: ratatui::style::Color) {
    if area.width <= 2 || area.height == 0 {
        return;
    }
    let rule_width = area.width.saturating_sub(2) as usize;
    let line = Line::from(Span::styled(
        "─".repeat(rule_width),
        Style::default().fg(palette::BORDER_DIM).bg(bg),
    ));
    let rule_area = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    line.render(rule_area, buf);
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
            term.buffer_contains(crate::theme::branding::APP_TITLE),
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
            term.buffer_contains(crate::theme::branding::APP_TITLE),
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
            term.buffer_contains(crate::theme::branding::APP_TITLE),
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

        assert!(
            term.buffer_contains(crate::theme::branding::APP_TITLE),
            "Title should show"
        );
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

    // ── Embedder status badge tests ──────────────────────────────────────────

    #[test]
    fn test_header_renders_status_badge_text() {
        let mut term = TestTerminal::with_size(120, 5);
        let icons = IconSet::new(IconMode::Unicode);
        let badge = StatusBadge::new("MCP 2 clients", StatusBadgeKind::Active);
        let header = MainHeader::new(Some("test_app"), icons).status_badge(Some(&badge));

        term.render_widget(header, term.area());

        assert!(
            term.buffer_contains("MCP 2 clients"),
            "Header should render the embedder status badge text"
        );
        // Existing sections must still render alongside the badge.
        assert!(term.buffer_contains("test_app"), "project name still shown");
        assert!(term.buffer_contains("[r] Run"), "shortcuts still shown");
    }

    #[test]
    fn test_header_without_badge_renders_nothing_extra() {
        let mut term = TestTerminal::with_size(120, 5);
        let icons = IconSet::new(IconMode::Unicode);
        let header = MainHeader::new(Some("test_app"), icons).status_badge(None);

        term.render_widget(header, term.area());

        assert!(
            !term.buffer_contains("MCP"),
            "No badge text should appear when the badge slot is None"
        );
        assert!(term.buffer_contains("test_app"));
        assert!(term.buffer_contains("[r] Run"));
    }

    #[test]
    fn test_header_badge_layout_identical_when_unset() {
        // Rendering with `.status_badge(None)` must produce exactly the same
        // buffer as not calling the builder at all.
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let area = Rect::new(0, 0, 120, 5);
        let icons = IconSet::new(IconMode::Unicode);

        let mut buf_plain = Buffer::empty(area);
        let header_plain = MainHeader::new(Some("app"), icons);
        render_main_header(area, &mut buf_plain, &header_plain, None);

        let mut buf_none = Buffer::empty(area);
        let header_none = MainHeader::new(Some("app"), icons).status_badge(None);
        render_main_header(area, &mut buf_none, &header_none, None);

        assert_eq!(
            buf_plain, buf_none,
            "status_badge(None) must not disturb the existing layout"
        );
    }

    #[test]
    fn test_header_badge_renders_in_multi_session_mode() {
        let mut term = TestTerminal::with_size(120, 5);
        let mut session_manager = SessionManager::new();
        session_manager
            .create_session(&test_device_with_platform("d1", "iPhone 15", "ios"))
            .unwrap();
        session_manager
            .create_session(&test_device_with_platform("d2", "Pixel 7", "android"))
            .unwrap();

        let icons = IconSet::new(IconMode::Unicode);
        let badge = StatusBadge::new("MCP 2 clients", StatusBadgeKind::Info);
        let header = MainHeader::new(Some("test_app"), icons)
            .with_sessions(&session_manager)
            .status_badge(Some(&badge));

        term.render_widget(header, term.area());

        assert!(
            term.buffer_contains("MCP 2 clients"),
            "badge should render on the title row in multi-session mode"
        );
        assert!(term.buffer_contains("iPhone 15"), "tabs still render");
    }

    #[test]
    fn test_header_badge_kind_controls_color() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        // Render a badge and find the foreground color of its first text cell.
        fn badge_text_fg(kind: StatusBadgeKind) -> ratatui::style::Color {
            let area = Rect::new(0, 0, 120, 5);
            let mut buf = Buffer::empty(area);
            let icons = IconSet::new(IconMode::Unicode);
            let badge = StatusBadge::new("BADGE", kind);
            let header = MainHeader::new(Some("app"), icons).status_badge(Some(&badge));
            render_main_header(area, &mut buf, &header, None);

            // Title row is y=1 (inside the border). Find the 'B' of "BADGE"
            // near the right edge.
            let row = 1u16;
            for x in (0..area.width).rev() {
                if buf[(x, row)].symbol() == "B" {
                    return buf[(x, row)]
                        .style()
                        .fg
                        .unwrap_or(ratatui::style::Color::Reset);
                }
            }
            panic!("badge text not found in buffer");
        }

        assert_eq!(badge_text_fg(StatusBadgeKind::Info), palette::ACCENT);
        assert_eq!(
            badge_text_fg(StatusBadgeKind::Active),
            palette::STATUS_GREEN
        );
        assert_eq!(badge_text_fg(StatusBadgeKind::Warn), palette::STATUS_YELLOW);
    }

    #[test]
    fn test_header_badge_skipped_when_no_room() {
        // At a very narrow width, the badge would collide with the left
        // section and must be skipped without panicking.
        let mut term = TestTerminal::with_size(30, 5);
        let icons = IconSet::new(IconMode::Unicode);
        let badge = StatusBadge::new("MCP 2 clients", StatusBadgeKind::Warn);
        let header = MainHeader::new(Some("a_long_project_name"), icons).status_badge(Some(&badge));

        term.render_widget(header, term.area());

        // Should render without panic; the title is still present.
        assert!(
            term.buffer_contains(crate::theme::branding::APP_TITLE)
                || term.buffer_contains("Flutter"),
            "left section should still render at narrow widths"
        );
    }

    // ── Multi-session separator tests (Phase 7, Task 01) ─────────────────────

    /// Build a SessionManager with two sessions and render into the given area,
    /// returning the buffer for direct cell inspection.
    fn render_multi_session_header(area: ratatui::layout::Rect) -> ratatui::buffer::Buffer {
        use ratatui::buffer::Buffer;

        let mut session_manager = SessionManager::new();
        session_manager
            .create_session(&test_device_with_platform("d1", "iPhone 15", "ios"))
            .unwrap();
        session_manager
            .create_session(&test_device_with_platform("d2", "Pixel 7", "android"))
            .unwrap();

        let icons = IconSet::new(fdemon_app::config::IconMode::Unicode);
        let header = MainHeader::new(Some("test_app"), icons).with_sessions(&session_manager);
        let mut buf = Buffer::empty(area);
        render_main_header(area, &mut buf, &header, None);
        buf
    }

    #[test]
    fn multi_session_header_renders_separator_between_title_and_tabs() {
        // height=5 → border(1) + title(1) + sep(1) + tabs(1) + border(1)
        // inner.y = 1 (after top border), so separator is at y=2, tabs at y=3.
        let area = ratatui::layout::Rect::new(0, 0, 80, 5);
        let buf = render_multi_session_header(area);

        // The separator rule is at inner.y + 1 = 2; inset 1 cell → x starts at 2
        // (x=1 is first inner cell, inset adds 1 more).
        // Check that at least one cell on row y=2, x >= 2 holds the '─' glyph.
        let sep_row = 2u16;
        let found_rule = (2u16..area.width - 1).any(|x| buf[(x, sep_row)].symbol() == "─");
        assert!(
            found_rule,
            "expected '─' rule on row {sep_row} (separator between title and tabs)"
        );

        // Tabs content (device names) must land on row y=3, not y=2.
        let tabs_row = 3u16;
        let title_row = 1u16;
        // The title row must not contain the '─' glyph.
        let rule_on_title = (0u16..area.width).any(|x| buf[(x, title_row)].symbol() == "─");
        assert!(
            !rule_on_title,
            "title row {title_row} must not contain '─' rule"
        );
        // The tabs row must contain text content (device name or status icon).
        let tabs_row_content: String = (0u16..area.width)
            .map(|x| buf[(x, tabs_row)].symbol().to_string())
            .collect();
        assert!(
            !tabs_row_content.trim().is_empty(),
            "tabs row {tabs_row} should contain tab content, got only whitespace"
        );
    }

    #[test]
    fn multi_session_header_has_no_trailing_empty_inner_row() {
        // With inner.height == 3 (header height=5), the three inner rows are:
        //   row 1 (inner.y+0): title
        //   row 2 (inner.y+1): separator
        //   row 3 (inner.y+2): tabs
        // Previously row 3 was blank; now it must carry tab content.
        let area = ratatui::layout::Rect::new(0, 0, 80, 5);
        let buf = render_multi_session_header(area);

        // inner.y = 1, so inner rows are y=1,2,3 and bottom border is y=4.
        // Row y=3 (tabs row, inner.y+2) must not be all-blank inside the border.
        let tabs_row = 3u16;
        // Inspect cells from x=1 to x=width-2 (inside the border).
        let inner_content: String = (1u16..area.width - 1)
            .map(|x| buf[(x, tabs_row)].symbol().to_string())
            .collect();
        assert!(
            !inner_content.trim().is_empty(),
            "inner bottom row (y={tabs_row}) must not be blank — tabs should occupy it"
        );
    }

    #[test]
    fn multi_session_header_squeezed_omits_separator() {
        // height=4 → border(1) + inner(2) + border(1); inner.height == 2.
        // Expected: title at y=1, tabs at y=2, no '─' anywhere in the inner area.
        let area = ratatui::layout::Rect::new(0, 0, 80, 4);
        let buf = render_multi_session_header(area);

        // No '─' glyph should appear in rows y=1 or y=2 (the two inner rows).
        for row in 1u16..3u16 {
            let found_rule = (0u16..area.width).any(|x| buf[(x, row)].symbol() == "─");
            assert!(
                !found_rule,
                "squeezed header (height=4) must not render '─' separator on row {row}"
            );
        }

        // Title row (y=1) should contain the app name content.
        let title_row_content: String = (0u16..area.width)
            .map(|x| buf[(x, 1u16)].symbol().to_string())
            .collect();
        assert!(
            title_row_content.contains("Flutter"),
            "title row should contain 'Flutter' in squeezed mode"
        );
    }
}

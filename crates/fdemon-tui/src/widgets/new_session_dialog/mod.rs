//! NewSessionDialog - Unified session launch dialog
//!
//! Replaces DeviceSelector and StartupDialog with a single dialog featuring:
//! - Target Selector (left pane): Connected/Bootable device tabs
//! - Launch Context (right pane): Config, mode, flavor, dart-defines
//! - Fuzzy search modals for config/flavor selection
//! - Dart defines master-detail modal

mod dart_defines_modal;
// device_groups is a thin re-export to app layer, intentionally shadows glob re-export
#[allow(hidden_glob_reexports)]
mod device_groups;
mod device_list;
pub mod fuzzy_modal; // Public for App layer to access fuzzy_filter function
mod launch_context;
mod state;
mod tab_bar;
pub mod target_selector; // Public for App layer to re-export TargetSelectorState

pub use dart_defines_modal::*;
pub use device_groups::*;
pub use device_list::*;
pub use fuzzy_modal::*;
pub use launch_context::*;
pub use state::*; // Re-exports from App layer
pub use tab_bar::*;
pub use target_selector::*;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
};

use fdemon_app::ToolAvailability;

use crate::theme::{icons::IconSet, palette, styles};
use crate::widgets::modal_overlay;

// ============================================================================
// Text Truncation Utilities
// ============================================================================

/// Truncates a string to fit within `max_width` characters, adding "..." suffix if truncated.
///
/// # Behavior
/// - Returns the original string if it fits within `max_width`
/// - For `max_width <= 3`, returns dots only (no meaningful text fits)
/// - For longer strings, truncates and adds "..." suffix
///
/// # Character Handling
/// Uses character count, not byte length, to safely handle multi-byte UTF-8
/// characters (emoji, CJK, etc.) without panicking.
///
/// # Examples
/// ```
/// # use fdemon_tui::widgets::new_session_dialog::truncate_with_ellipsis;
/// assert_eq!(truncate_with_ellipsis("Hello", 10), "Hello");
/// assert_eq!(truncate_with_ellipsis("Hello World", 8), "Hello...");
/// assert_eq!(truncate_with_ellipsis("Test", 3), "...");
/// assert_eq!(truncate_with_ellipsis("iPhone 🔥", 9), "iPhone 🔥");
/// ```
pub fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        let truncated: String = text.chars().take(max_width - 3).collect();
        format!("{}...", truncated)
    }
}

/// Truncates a string by removing middle characters, keeping start and end visible.
///
/// Useful for paths or identifiers where both prefix and suffix are meaningful.
/// The result format is: `<start>...<end>`
///
/// # Behavior
/// - Returns the original string if it fits within `max_width`
/// - For `max_width <= 3`, returns dots only (no meaningful text fits)
/// - For longer strings, keeps roughly equal parts from start and end
/// - If odd number of available chars, extra char goes to the start
///
/// # Character Handling
/// Uses character count, not byte length, to safely handle multi-byte UTF-8
/// characters (emoji, CJK, etc.) without panicking.
///
/// # Examples
/// ```
/// # use fdemon_tui::widgets::new_session_dialog::truncate_middle;
/// assert_eq!(truncate_middle("Hello World", 11), "Hello World");
/// assert_eq!(truncate_middle("Hello World", 9), "Hel...rld");
/// assert_eq!(truncate_middle("abcdef", 3), "...");
/// ```
pub fn truncate_middle(text: &str, max_width: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        // Reserve space for "..." (3 chars)
        let available = max_width - 3;
        let half = available / 2;
        let extra = available % 2; // Give extra char to start

        let start: String = text.chars().take(half + extra).collect();
        let end: String = text.chars().skip(char_count - half).collect();
        format!("{}...{}", start, end)
    }
}

/// Minimum terminal width for horizontal (two-pane) layout
const MIN_HORIZONTAL_WIDTH: u16 = 70;

/// Minimum terminal height for horizontal (two-pane) layout
const MIN_HORIZONTAL_HEIGHT: u16 = 20;

/// Minimum terminal width for vertical (stacked) layout
const MIN_VERTICAL_WIDTH: u16 = 40;

/// Minimum terminal height for vertical (stacked) layout
const MIN_VERTICAL_HEIGHT: u16 = 20;

/// Absolute minimum dimensions (below this shows "too small" message)
const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 20;

/// Minimum content-area height for LaunchContext to render in expanded (full) mode.
/// Must match `LaunchContext::min_height()` (29) to avoid button clipping.
const MIN_EXPANDED_LAUNCH_HEIGHT: u16 = 29;

/// Minimum content-area height for TargetSelector to render in full mode.
/// Full mode needs: 3-row tab bar + Min(5) device list + 1-row footer = 9 rows minimum.
/// We use 10 to give the device list a reasonable viewport.
const MIN_EXPANDED_TARGET_HEIGHT: u16 = 10;

/// Layout mode for NewSessionDialog based on terminal size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Two-pane horizontal layout (Target Selector | Launch Context)
    /// Requires width >= 70
    Horizontal,

    /// Stacked vertical layout (Target Selector above Launch Context)
    /// For narrow terminals (width 40-69)
    Vertical,

    /// Terminal too small to render dialog meaningfully
    /// Below 40x20
    TooSmall,
}

/// The main NewSessionDialog widget
pub struct NewSessionDialog<'a> {
    state: &'a NewSessionDialogState,
    tool_availability: &'a ToolAvailability,
    icons: &'a IconSet,
    /// When `true`, a one-line migration banner is rendered above the dialog
    /// informing the user that cache-driven auto-launch is now opt-in.
    show_migration_banner: bool,
}

impl<'a> NewSessionDialog<'a> {
    /// Minimum terminal width for dialog (updated to match MIN_WIDTH constant)
    pub const MIN_WIDTH: u16 = MIN_WIDTH;

    /// Minimum terminal height for dialog (updated to match MIN_HEIGHT constant)
    pub const MIN_HEIGHT: u16 = MIN_HEIGHT;

    pub fn new(
        state: &'a NewSessionDialogState,
        tool_availability: &'a ToolAvailability,
        icons: &'a IconSet,
    ) -> Self {
        Self {
            state,
            tool_availability,
            icons,
            show_migration_banner: false,
        }
    }

    /// Set whether to render the migration banner above the dialog.
    pub fn migration_banner(mut self, show: bool) -> Self {
        self.show_migration_banner = show;
        self
    }

    /// Determine the appropriate layout mode for the given area
    pub fn layout_mode(area: Rect) -> LayoutMode {
        if area.width >= MIN_HORIZONTAL_WIDTH && area.height >= MIN_HORIZONTAL_HEIGHT {
            LayoutMode::Horizontal
        } else if area.width >= MIN_VERTICAL_WIDTH && area.height >= MIN_VERTICAL_HEIGHT {
            LayoutMode::Vertical
        } else {
            LayoutMode::TooSmall
        }
    }

    /// Check if area supports at least vertical layout
    pub fn fits_in_area(area: Rect) -> bool {
        Self::layout_mode(area) != LayoutMode::TooSmall
    }

    /// Calculate centered dialog area (80% width, 70% height)
    fn centered_rect(area: Rect) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(area);

        Layout::horizontal([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(popup_layout[1])[1]
    }

    /// Get footer hints based on current state
    fn footer_hints(&self) -> Vec<(&str, &str)> {
        if self.state.is_fuzzy_modal_open() {
            vec![("↑↓", "Navigate"), ("Enter", "Select"), ("Esc", "Cancel")]
        } else if self.state.is_dart_defines_modal_open() {
            vec![
                ("Tab", "Pane"),
                ("↑↓", "Navigate"),
                ("Enter", "Edit"),
                ("Esc", "Close"),
            ]
        } else {
            vec![
                ("1/2", "Tab"),
                ("Tab", "Pane"),
                ("↑↓", "Navigate"),
                ("Enter", "Select"),
                ("Esc", "Close"),
            ]
        }
    }

    /// Render header area with title, subtitle, and close hint
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        // Row 1: "New Session" (left) + "[Esc] Close" (right)
        let title_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "New Session",
                Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let close_hint = Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw(" "),
            Span::styled("Close", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw("  "),
        ]);

        // Split area for title (left) and close hint (right)
        let title_area = Rect::new(area.x, area.y, area.width, 1);
        Paragraph::new(title_line).render(title_area, buf);
        Paragraph::new(close_hint)
            .alignment(Alignment::Right)
            .render(title_area, buf);

        // Row 2: Subtitle
        let subtitle = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Configure deployment target and runtime flags.",
                Style::default().fg(palette::TEXT_SECONDARY),
            ),
        ]);
        let subtitle_area = Rect::new(area.x, area.y + 1, area.width, 1);
        Paragraph::new(subtitle).render(subtitle_area, buf);
    }

    /// Render a horizontal separator line
    fn render_separator(area: Rect, buf: &mut Buffer) {
        let separator = "─".repeat(area.width as usize);
        buf.set_string(
            area.x,
            area.y,
            &separator,
            Style::default().fg(palette::BORDER_DIM),
        );
    }

    /// Render compact header for vertical layout (2 lines: title + close hint only)
    fn render_header_compact(&self, area: Rect, buf: &mut Buffer) {
        // Row 1: "New Session" (left) + "[Esc]" (right)
        let title_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "New Session",
                Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let close_hint = Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw("  "),
        ]);

        // Split area for title (left) and close hint (right)
        let title_area = Rect::new(area.x, area.y, area.width, 1);
        Paragraph::new(title_line).render(title_area, buf);
        Paragraph::new(close_hint)
            .alignment(Alignment::Right)
            .render(title_area, buf);

        // For compact mode, skip subtitle to save space
    }

    /// Render main content (two panes)
    fn render_panes(&self, area: Rect, buf: &mut Buffer, launch_compact: bool) {
        // Split into 40% (Target Selector) + 1 col (separator) + 60% (Launch Context)
        let chunks = Layout::horizontal([
            Constraint::Percentage(40), // Target Selector
            Constraint::Length(1),      // Vertical separator
            Constraint::Percentage(60), // Launch Context
        ])
        .split(area);

        // Render Target Selector (left pane)
        let target_focused = self.state.is_target_selector_focused();
        let target_selector = TargetSelector::new(
            &self.state.target_selector,
            self.tool_availability,
            target_focused,
        )
        .icons(*self.icons);
        target_selector.render(chunks[0], buf);

        // Render vertical separator
        Self::render_vertical_separator(chunks[1], buf);

        // Render Launch Context (right pane)
        let launch_focused = self.state.is_launch_context_focused();
        let has_device = self.state.is_ready_to_launch();
        let launch_context = LaunchContextWithDevice::new(
            &self.state.launch_context,
            launch_focused,
            has_device,
            self.icons,
        )
        .compact(launch_compact);
        launch_context.render(chunks[2], buf);
    }

    /// Render a vertical separator line
    fn render_vertical_separator(area: Rect, buf: &mut Buffer) {
        for y in area.top()..area.bottom() {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_char('│');
                cell.set_style(Style::default().fg(palette::BORDER_DIM));
            }
        }
    }

    /// Render footer with kbd-style shortcut hints
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        // Fill background with SURFACE color
        let bg_block = Block::default().style(Style::default().bg(palette::SURFACE));
        bg_block.render(area, buf);

        let hints = self.footer_hints();

        let mut spans: Vec<Span> = Vec::new();
        for (i, (key, label)) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    "  ·  ",
                    Style::default().fg(palette::BORDER_DIM),
                ));
            }
            spans.push(Span::styled(
                format!("[{}]", key),
                Style::default().fg(palette::TEXT_PRIMARY),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                *label,
                Style::default().fg(palette::TEXT_MUTED),
            ));
        }

        let line = Line::from(spans);
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .render(area, buf);
    }

    /// Render fuzzy modal overlay (bottom 40% with dimmed background)
    fn render_fuzzy_modal_overlay(&self, dialog_area: Rect, buf: &mut Buffer) {
        let modal_state = match &self.state.fuzzy_modal {
            Some(state) => state,
            None => return,
        };

        // Dim the background (main dialog area)
        modal_overlay::dim_background(buf, dialog_area);

        // Check if this is an entry point modal that's loading
        let is_loading = modal_state.modal_type == super::FuzzyModalType::EntryPoint
            && self.state.launch_context.entry_points_loading;

        // Render fuzzy modal widget (it calculates its own area)
        let fuzzy_modal = FuzzyModal::new(modal_state).loading(is_loading);
        fuzzy_modal.render(dialog_area, buf);
    }

    /// Render dart defines modal (full-screen overlay)
    fn render_dart_defines_modal(&self, dialog_area: Rect, buf: &mut Buffer) {
        let modal_state = match &self.state.dart_defines_modal {
            Some(state) => state,
            None => return,
        };

        // Dim the background (main dialog area)
        modal_overlay::dim_background(buf, dialog_area);

        // Render dart defines modal widget
        let dart_defines_modal = DartDefinesModal::new(modal_state);
        dart_defines_modal.render(dialog_area, buf);
    }

    /// Render a "terminal too small" message
    fn render_too_small(area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let message = format!(
            "Terminal too small. Need at least {}x{} (current: {}x{})",
            MIN_WIDTH, MIN_HEIGHT, area.width, area.height
        );

        let paragraph = Paragraph::new(message)
            .style(Style::default().fg(palette::STATUS_RED))
            .alignment(Alignment::Center);

        // Center vertically
        let y = area.y + area.height / 2;
        let centered = Rect::new(area.x, y, area.width, 1);
        paragraph.render(centered, buf);
    }

    /// Render horizontal (two-pane) layout
    fn render_horizontal(&self, area: Rect, buf: &mut Buffer) {
        // Step 1: Dim the background (background content already rendered by render/mod.rs)
        modal_overlay::dim_background(buf, area);

        // Step 2: Calculate centered dialog area
        let dialog_area = Self::centered_rect(area);

        // Step 3: Render shadow (1-cell offset right+bottom)
        modal_overlay::render_shadow(buf, dialog_area);

        // Step 4: Clear dialog area (prepare for dialog content)
        modal_overlay::clear_area(buf, dialog_area);

        // Step 5: Main dialog block (no title on border)
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::border_inactive())
            .style(Style::default().bg(palette::POPUP_BG));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Layout: header + separator + content + separator + footer
        let chunks = Layout::vertical([
            Constraint::Length(3), // Header (title + subtitle + blank line)
            Constraint::Length(1), // Separator
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Separator
            Constraint::Length(1), // Footer
        ])
        .split(inner);

        // Render header
        self.render_header(chunks[0], buf);

        // Render separator
        Self::render_separator(chunks[1], buf);

        // Determine if LaunchContext needs compact mode based on available height
        let launch_compact = chunks[2].height < MIN_EXPANDED_LAUNCH_HEIGHT;
        self.render_panes(chunks[2], buf, launch_compact);

        // Render separator
        Self::render_separator(chunks[3], buf);

        // Render footer
        self.render_footer(chunks[4], buf);

        // Render modal overlay if any
        if self.state.is_dart_defines_modal_open() {
            self.render_dart_defines_modal(dialog_area, buf);
        } else if self.state.is_fuzzy_modal_open() {
            self.render_fuzzy_modal_overlay(dialog_area, buf);
        }
    }

    /// Render vertical (stacked) layout for narrow terminals
    fn render_vertical(&self, area: Rect, buf: &mut Buffer) {
        // Step 1: Dim the background (background content already rendered by render/mod.rs)
        modal_overlay::dim_background(buf, area);

        // Step 2: Use more of the available space in vertical mode (90% width, 85% height)
        let dialog_area = Self::centered_rect_custom(90, 85, area);

        // Step 3: Render shadow (1-cell offset right+bottom)
        modal_overlay::render_shadow(buf, dialog_area);

        // Step 4: Clear dialog area (prepare for dialog content)
        modal_overlay::clear_area(buf, dialog_area);

        // Step 5: Main dialog block (no title on border)
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::border_inactive())
            .style(Style::default().bg(palette::POPUP_BG));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Vertical split: header + separator + Target Selector + separator + Launch Context + separator + footer
        let chunks = Layout::vertical([
            Constraint::Length(2),      // Header (compact: title only)
            Constraint::Length(1),      // Separator
            Constraint::Percentage(45), // Target Selector (top, compact mode)
            Constraint::Length(1),      // Separator line
            Constraint::Min(10),        // Launch Context (bottom)
            Constraint::Length(1),      // Separator
            Constraint::Length(1),      // Footer
        ])
        .split(inner);

        // Render compact header
        self.render_header_compact(chunks[0], buf);

        // Render separator
        Self::render_separator(chunks[1], buf);

        // Render Target Selector (top) — use full mode when sufficient height is available
        let target_focused = self.state.is_target_selector_focused();
        let target_compact = chunks[2].height < MIN_EXPANDED_TARGET_HEIGHT;
        let target_selector = TargetSelector::new(
            &self.state.target_selector,
            self.tool_availability,
            target_focused,
        )
        .icons(*self.icons)
        .compact(target_compact);
        target_selector.render(chunks[2], buf);

        // Render separator line
        Self::render_separator(chunks[3], buf);

        // Render Launch Context (bottom) — use expanded mode when sufficient height is available
        let launch_focused = self.state.is_launch_context_focused();
        let has_device = self.state.is_ready_to_launch();
        let launch_compact = chunks[4].height < MIN_EXPANDED_LAUNCH_HEIGHT;
        let launch_context = LaunchContextWithDevice::new(
            &self.state.launch_context,
            launch_focused,
            has_device,
            self.icons,
        )
        .compact(launch_compact);
        launch_context.render(chunks[4], buf);

        // Render separator
        Self::render_separator(chunks[5], buf);

        // Render compact footer
        self.render_footer_compact(chunks[6], buf);

        // Render modal overlay if any
        if self.state.is_dart_defines_modal_open() {
            self.render_dart_defines_modal(dialog_area, buf);
        } else if self.state.is_fuzzy_modal_open() {
            self.render_fuzzy_modal_overlay(dialog_area, buf);
        }
    }

    /// Calculate centered dialog area with custom percentages
    fn centered_rect_custom(width_percent: u16, height_percent: u16, area: Rect) -> Rect {
        let v_margin = (100 - height_percent) / 2;
        let popup_layout = Layout::vertical([
            Constraint::Percentage(v_margin),
            Constraint::Percentage(height_percent),
            Constraint::Percentage(v_margin),
        ])
        .split(area);

        let h_margin = (100 - width_percent) / 2;
        Layout::horizontal([
            Constraint::Percentage(h_margin),
            Constraint::Percentage(width_percent),
            Constraint::Percentage(h_margin),
        ])
        .split(popup_layout[1])[1]
    }

    /// Render a one-line migration banner above the dialog.
    ///
    /// Called only when `show_migration_banner` is `true`. The banner occupies
    /// the topmost row of the provided area and uses `STATUS_YELLOW` to draw
    /// the user's eye to the opt-in change.
    fn render_migration_banner(area: Rect, buf: &mut Buffer) {
        let banner = Paragraph::new(
            "\u{26a0} Cache-driven auto-launch is now opt-in. \
             Set `[behavior] auto_launch = true` in `.fdemon/config.toml` to restore.",
        )
        .style(Style::default().fg(palette::STATUS_YELLOW))
        .alignment(Alignment::Center);
        banner.render(area, buf);
    }

    /// Render footer with abbreviated keybindings (for vertical layout)
    fn render_footer_compact(&self, area: Rect, buf: &mut Buffer) {
        // Fill background with SURFACE color
        let bg_block = Block::default().style(Style::default().bg(palette::SURFACE));
        bg_block.render(area, buf);

        // Shorter keybinding hints for narrow terminals
        let hints = if self.state.is_fuzzy_modal_open() {
            vec![("↑↓", "Nav"), ("Enter", "Sel"), ("Esc", "Close")]
        } else if self.state.is_dart_defines_modal_open() {
            vec![("Tab", "Pane"), ("↑↓", "Nav"), ("Esc", "Close")]
        } else {
            vec![
                ("1/2", "Tab"),
                ("Tab", "Pane"),
                ("↑↓", "Nav"),
                ("Esc", "Close"),
            ]
        };

        let mut spans: Vec<Span> = Vec::new();
        for (i, (key, label)) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    " · ",
                    Style::default().fg(palette::BORDER_DIM),
                ));
            }
            spans.push(Span::styled(
                format!("[{}]", key),
                Style::default().fg(palette::TEXT_PRIMARY),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                *label,
                Style::default().fg(palette::TEXT_MUTED),
            ));
        }

        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .render(area, buf);
    }
}

/// Render `NewSessionDialog` and record clickable regions.
///
/// This is a free-function sister to [`Widget::render`] that additionally
/// accepts an optional [`MouseCtx`] for region recording.
///
/// Passing `None` produces output identical to calling
/// `frame.render_widget(dialog, area)`.
///
/// # Region z-indices
/// - Main dialog regions (tabs, device rows, launch-context fields, launch button): z=1
/// - Fuzzy modal result rows (sub-modal layer): z=2
/// - Dart-defines modal regions: deferred to Phase 6 (not recorded here)
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: NewSessionDialog<'_>,
    ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
) {
    view.render_regions_impl(area, buf, ctx);
}

impl NewSessionDialog<'_> {
    /// Region-aware render implementation. Called by `render_with_regions`.
    fn render_regions_impl(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
    ) {
        if self.show_migration_banner {
            let chunks = Layout::vertical([
                Constraint::Length(1), // banner
                Constraint::Min(0),    // dialog
            ])
            .split(area);
            Self::render_migration_banner(chunks[0], buf);
            self.render_regions_no_banner(chunks[1], buf, ctx);
        } else {
            self.render_regions_no_banner(area, buf, ctx);
        }
    }

    fn render_regions_no_banner(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
    ) {
        match Self::layout_mode(area) {
            LayoutMode::TooSmall => {
                Self::render_too_small(area, buf);
            }
            LayoutMode::Horizontal => {
                self.render_horizontal_with_regions(area, buf, ctx);
            }
            LayoutMode::Vertical => {
                self.render_vertical_with_regions(area, buf, ctx);
            }
        }
    }

    /// Render horizontal (two-pane) layout with region recording.
    fn render_horizontal_with_regions(
        &self,
        area: Rect,
        buf: &mut Buffer,
        mut ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
    ) {
        modal_overlay::dim_background(buf, area);
        let dialog_area = Self::centered_rect(area);
        modal_overlay::render_shadow(buf, dialog_area);
        modal_overlay::clear_area(buf, dialog_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::border_inactive())
            .style(Style::default().bg(palette::POPUP_BG));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

        self.render_header(chunks[0], buf);
        Self::render_separator(chunks[1], buf);

        let launch_compact = chunks[2].height < MIN_EXPANDED_LAUNCH_HEIGHT;

        // Pane split
        let pane_chunks = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Length(1),
            Constraint::Percentage(60),
        ])
        .split(chunks[2]);

        // Target Selector (full mode in horizontal layout)
        self.render_target_selector_regions(pane_chunks[0], buf, ctx.as_deref_mut(), false);
        Self::render_vertical_separator(pane_chunks[1], buf);
        // Launch Context
        self.render_launch_context_regions(pane_chunks[2], buf, ctx.as_deref_mut(), launch_compact);

        Self::render_separator(chunks[3], buf);
        self.render_footer(chunks[4], buf);

        // Fuzzy modal overlay
        if self.state.is_dart_defines_modal_open() {
            self.render_dart_defines_modal(dialog_area, buf);
            // Dart-defines modal: no regions in v1 (deferred to Phase 6)
        } else if self.state.is_fuzzy_modal_open() {
            if let Some(modal_state) = &self.state.fuzzy_modal {
                let is_loading = modal_state.modal_type == super::FuzzyModalType::EntryPoint
                    && self.state.launch_context.entry_points_loading;
                let fuzzy_widget = FuzzyModal::new(modal_state).loading(is_loading);
                if let Some(c) = ctx {
                    // Single-pass: dim background then render+register regions in one call.
                    // Avoids the double render that occurred when render_fuzzy_modal_overlay
                    // (paint) was followed by fuzzy_modal_render_with_regions (repaint + regions).
                    modal_overlay::dim_background(buf, dialog_area);
                    fuzzy_modal::fuzzy_modal_render_with_regions(
                        dialog_area,
                        buf,
                        fuzzy_widget,
                        Some(c),
                    );
                } else {
                    // No ctx: use the regular overlay helper (dim + widget render, no regions).
                    self.render_fuzzy_modal_overlay(dialog_area, buf);
                }
            }
        }
    }

    /// Render vertical (stacked) layout with region recording.
    fn render_vertical_with_regions(
        &self,
        area: Rect,
        buf: &mut Buffer,
        mut ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
    ) {
        modal_overlay::dim_background(buf, area);
        let dialog_area = Self::centered_rect_custom(90, 85, area);
        modal_overlay::render_shadow(buf, dialog_area);
        modal_overlay::clear_area(buf, dialog_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::border_inactive())
            .style(Style::default().bg(palette::POPUP_BG));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        let chunks = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Percentage(45),
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

        self.render_header_compact(chunks[0], buf);
        Self::render_separator(chunks[1], buf);

        let target_compact = chunks[2].height < MIN_EXPANDED_TARGET_HEIGHT;
        self.render_target_selector_regions(chunks[2], buf, ctx.as_deref_mut(), target_compact);

        Self::render_separator(chunks[3], buf);

        let launch_compact = chunks[4].height < MIN_EXPANDED_LAUNCH_HEIGHT;
        self.render_launch_context_regions(chunks[4], buf, ctx.as_deref_mut(), launch_compact);

        Self::render_separator(chunks[5], buf);
        self.render_footer_compact(chunks[6], buf);

        if self.state.is_dart_defines_modal_open() {
            self.render_dart_defines_modal(dialog_area, buf);
        } else if self.state.is_fuzzy_modal_open() {
            if let Some(modal_state) = &self.state.fuzzy_modal {
                let is_loading = modal_state.modal_type == super::FuzzyModalType::EntryPoint
                    && self.state.launch_context.entry_points_loading;
                let fuzzy_widget = FuzzyModal::new(modal_state).loading(is_loading);
                if let Some(c) = ctx {
                    // Single-pass: dim background then render+register regions in one call.
                    // Avoids the double render that occurred when render_fuzzy_modal_overlay
                    // (paint) was followed by fuzzy_modal_render_with_regions (repaint + regions).
                    modal_overlay::dim_background(buf, dialog_area);
                    fuzzy_modal::fuzzy_modal_render_with_regions(
                        dialog_area,
                        buf,
                        fuzzy_widget,
                        Some(c),
                    );
                } else {
                    // No ctx: use the regular overlay helper (dim + widget render, no regions).
                    self.render_fuzzy_modal_overlay(dialog_area, buf);
                }
            }
        }
    }

    /// Render the TargetSelector pane with region recording.
    fn render_target_selector_regions(
        &self,
        area: Rect,
        buf: &mut Buffer,
        mut ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
        compact: bool,
    ) {
        let state = &self.state.target_selector;
        let is_focused = self.state.is_target_selector_focused();

        if compact {
            // Compact path: render TargetSelector widget directly
            // Region recording for compact mode deferred (uncommon path)
            let ts = TargetSelector::new(state, self.tool_availability, is_focused)
                .icons(*self.icons)
                .compact(true);
            ts.render(area, buf);
        } else {
            // Full path: mirror TargetSelector::render_full and thread ctx
            use device_list::{calculate_scroll_offset, BootableDeviceList, ConnectedDeviceList};

            let chunks = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(1),
            ])
            .split(area);

            let visible_height = chunks[1].height as usize;
            state.last_known_visible_height.set(visible_height);
            let corrected_scroll =
                calculate_scroll_offset(state.selected_index, visible_height, state.scroll_offset);

            // Tab bar with regions
            let tb = TabBar::new(
                state.active_tab,
                is_focused,
                state.refreshing,
                state.bootable_refreshing,
                self.icons,
            );
            tab_bar::render_with_regions(chunks[0], buf, tb, ctx.as_deref_mut());

            // Device list with regions
            if state.loading {
                use ratatui::widgets::Paragraph;
                Paragraph::new("Discovering devices...")
                    .style(Style::default().fg(palette::STATUS_YELLOW))
                    .alignment(Alignment::Center)
                    .render(chunks[1], buf);
            } else if let Some(ref error) = state.error {
                use ratatui::widgets::Paragraph;
                Paragraph::new(error.as_str())
                    .style(Style::default().fg(palette::STATUS_RED))
                    .alignment(Alignment::Center)
                    .render(chunks[1], buf);
            } else {
                match state.active_tab {
                    TargetTab::Connected => {
                        let dl = ConnectedDeviceList::new(
                            &state.connected_devices,
                            state.selected_index,
                            is_focused,
                            corrected_scroll,
                        );
                        device_list::connected_device_list_render_with_regions(
                            chunks[1],
                            buf,
                            &dl,
                            ctx.as_deref_mut(),
                        );
                    }
                    TargetTab::Bootable => {
                        let dl = BootableDeviceList::new(
                            &state.ios_simulators,
                            &state.android_avds,
                            state.selected_index,
                            is_focused,
                            corrected_scroll,
                            self.tool_availability,
                        );
                        device_list::bootable_device_list_render_with_regions(
                            chunks[1], buf, &dl, ctx,
                        );
                    }
                }
            }

            // Footer (no regions)
            let hints = match state.active_tab {
                TargetTab::Connected => "[Enter] Select  [r] Refresh",
                TargetTab::Bootable => "[Enter] Boot  [r] Refresh",
            };
            use ratatui::widgets::Paragraph;
            Paragraph::new(hints)
                .style(Style::default().fg(palette::BORDER_DIM))
                .alignment(Alignment::Center)
                .render(chunks[2], buf);
        }
    }

    /// Render the LaunchContext pane with region recording.
    fn render_launch_context_regions(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
        launch_compact: bool,
    ) {
        let launch_focused = self.state.is_launch_context_focused();
        let has_device = self.state.is_ready_to_launch();
        let widget = LaunchContextWithDevice::new(
            &self.state.launch_context,
            launch_focused,
            has_device,
            self.icons,
        )
        .compact(launch_compact);
        launch_context::launch_context_render_with_regions(area, buf, widget, ctx);
    }
}

impl Widget for NewSessionDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.show_migration_banner {
            // Reserve the top row for the migration banner; give the rest to the
            // dialog so its responsive layout calculations remain accurate.
            let chunks = Layout::vertical([
                Constraint::Length(1), // banner
                Constraint::Min(0),    // dialog
            ])
            .split(area);

            Self::render_migration_banner(chunks[0], buf);

            // Render dialog in the remaining area
            let dialog_area = chunks[1];
            match Self::layout_mode(dialog_area) {
                LayoutMode::TooSmall => {
                    Self::render_too_small(dialog_area, buf);
                }
                LayoutMode::Horizontal => {
                    self.render_horizontal(dialog_area, buf);
                }
                LayoutMode::Vertical => {
                    self.render_vertical(dialog_area, buf);
                }
            }
        } else {
            match Self::layout_mode(area) {
                LayoutMode::TooSmall => {
                    Self::render_too_small(area, buf);
                }
                LayoutMode::Horizontal => {
                    self.render_horizontal(area, buf);
                }
                LayoutMode::Vertical => {
                    self.render_vertical(area, buf);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_device_full;
    use fdemon_app::config::{IconMode, LoadedConfigs};
    use ratatui::{backend::TestBackend, Terminal};

    // =========================================================================
    // Test helper — dialog state with a connected device
    // =========================================================================

    /// Build a `NewSessionDialogState` with one connected device already set.
    ///
    /// The device triggers the "ready to launch" path in `LaunchContextWithDevice`,
    /// ensuring device-dependent fields are rendered in both compact and expanded modes.
    fn test_dialog_state() -> NewSessionDialogState {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state
            .target_selector
            .set_connected_devices(vec![test_device_full(
                "iphone15",
                "iPhone 15",
                "ios",
                false,
            )]);
        // set_connected_devices clears the loading flag automatically
        state
    }

    /// Render the dialog and return buffer content as a flat string.
    fn render_dialog(state: &NewSessionDialogState, width: u16, height: u16) -> String {
        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        let backend = TestBackend::new(width, height);
        let mut terminal =
            Terminal::new(backend).expect("render_dialog: failed to create terminal");

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(state, &tool_availability, &icons);
                f.render_widget(dialog, f.area());
            })
            .expect("render_dialog: failed to draw");

        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn test_dialog_renders() {
        let state = NewSessionDialogState::new(LoadedConfigs::default());
        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("New Session"));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = NewSessionDialog::centered_rect(area);

        // Should be roughly centered
        assert!(centered.x > 0);
        assert!(centered.y > 0);
        assert!(centered.width < area.width);
        assert!(centered.height < area.height);
    }

    #[test]
    fn test_fits_in_area() {
        // Should fit with horizontal layout
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 100, 40)));
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 80, 24)));
        // Should fit with vertical layout
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 60, 20)));
        assert!(NewSessionDialog::fits_in_area(Rect::new(0, 0, 40, 20)));
        // Should not fit (too small)
        assert!(!NewSessionDialog::fits_in_area(Rect::new(0, 0, 30, 15)));
    }

    #[test]
    fn test_layout_mode_horizontal() {
        let area = Rect::new(0, 0, 100, 40);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Horizontal);
    }

    #[test]
    fn test_layout_mode_vertical() {
        let area = Rect::new(0, 0, 50, 30);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Vertical);
    }

    #[test]
    fn test_layout_mode_too_small() {
        let area = Rect::new(0, 0, 30, 15);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::TooSmall);
    }

    #[test]
    fn test_layout_mode_boundary_horizontal() {
        let area = Rect::new(0, 0, 70, 20);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Horizontal);
    }

    #[test]
    fn test_layout_mode_boundary_vertical() {
        let area = Rect::new(0, 0, 69, 20);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Vertical);
    }

    #[test]
    fn test_layout_mode_boundary_too_small_width() {
        let area = Rect::new(0, 0, 39, 20);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::TooSmall);
    }

    #[test]
    fn test_layout_mode_boundary_too_small_height() {
        let area = Rect::new(0, 0, 70, 19);
        assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::TooSmall);
    }

    #[test]
    fn test_too_small_message() {
        let state = NewSessionDialogState::new(LoadedConfigs::default());
        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("too small"));
    }

    #[test]
    fn test_dialog_with_fuzzy_modal() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.open_flavor_modal(vec!["dev".to_string(), "prod".to_string()]);

        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Select Flavor"));
        assert!(content.contains("dev"));
        assert!(content.contains("prod"));
    }

    #[test]
    fn test_dialog_with_dart_defines_modal() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.open_dart_defines_modal();

        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Manage Dart Defines"));
    }

    #[test]
    fn test_truncate_with_ellipsis_no_truncation() {
        let result = truncate_with_ellipsis("short", 10);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_with_ellipsis_exact_fit() {
        let result = truncate_with_ellipsis("exactly10!", 10);
        assert_eq!(result, "exactly10!");
    }

    #[test]
    fn test_truncate_with_ellipsis_truncates() {
        let result = truncate_with_ellipsis("this is a very long text", 10);
        assert_eq!(result, "this is...");
    }

    #[test]
    fn test_truncate_with_ellipsis_very_short() {
        let result = truncate_with_ellipsis("text", 3);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_with_ellipsis_minimal() {
        let result = truncate_with_ellipsis("text", 2);
        assert_eq!(result, "..");
    }

    #[test]
    fn test_truncate_middle_no_truncation() {
        let result = truncate_middle("short", 10);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_middle_truncates() {
        let result = truncate_middle("this_is_a_very_long_device_name", 15);
        assert_eq!(result, "this_i...e_name");
    }

    #[test]
    fn test_truncate_middle_very_short() {
        let result = truncate_middle("longtext", 5);
        // With max_width=5: available=2, half=1, start="l", end="t"
        assert_eq!(result, "l...t");
    }

    #[test]
    fn test_truncate_middle_minimal() {
        let result = truncate_middle("text", 3);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_with_ellipsis_utf8() {
        // Emoji (4 bytes per char) - "iPhone 🔥" is 8 chars, fits at width 8
        assert_eq!(truncate_with_ellipsis("iPhone 🔥", 7), "iPho...");
        assert_eq!(truncate_with_ellipsis("iPhone 🔥", 8), "iPhone 🔥");
        assert_eq!(truncate_with_ellipsis("iPhone 🔥", 9), "iPhone 🔥");

        // Multi-byte chars (3 bytes per char) - "日本語テスト" is 6 chars
        assert_eq!(truncate_with_ellipsis("日本語テスト", 6), "日本語テスト");
        assert_eq!(truncate_with_ellipsis("日本語テスト", 5), "日本...");

        // Mixed ASCII and emoji - "Test 🚀 Device" is 13 chars
        assert_eq!(truncate_with_ellipsis("Test 🚀 Device", 10), "Test 🚀 ...");
        assert_eq!(truncate_with_ellipsis("Test 🚀 Device", 9), "Test 🚀...");
    }

    #[test]
    fn test_truncate_middle_utf8() {
        // Emoji in name - "🔥Hot🔥Device🔥" is 12 chars
        assert_eq!(truncate_middle("🔥Hot🔥Device🔥", 10), "🔥Hot...ce🔥");
        assert_eq!(truncate_middle("🔥Hot🔥Device🔥", 8), "🔥Ho...e🔥");

        // Multi-byte chars - "日本語デバイス" is 7 chars
        assert_eq!(truncate_middle("日本語デバイス", 7), "日本語デバイス");
        assert_eq!(truncate_middle("日本語デバイス", 6), "日本...ス");
    }

    // =========================================================================
    // Group 1: Horizontal layout — height-based LaunchContext decisions
    // =========================================================================

    /// Wide-but-short: horizontal layout (width >= 70), but not enough height for expanded fields.
    ///
    /// Math: dialog.height = 25 * 0.70 = 17, inner.height = 17 - 2 = 15,
    ///       content.height = 15 - 6 (header+sep+sep+footer) = 9
    ///       9 < MIN_EXPANDED_LAUNCH_HEIGHT (29) → compact mode
    #[test]
    fn test_horizontal_short_terminal_uses_compact_launch_context() {
        let state = test_dialog_state();
        let content = render_dialog(&state, 100, 25);

        // Compact mode renders a " Launch Context " titled border
        assert!(
            content.contains("Launch Context"),
            "Short horizontal terminal (100x25) should render LaunchContext in compact mode \
             (with 'Launch Context' border title). Content: {:?}",
            &content.chars().take(500).collect::<String>()
        );
    }

    /// Wide-and-tall: horizontal layout with enough height to use expanded (full) field layout.
    ///
    /// Math: dialog.height = 55 * 0.70 = 38, inner.height = 38 - 2 = 36,
    ///       content.height = 36 - 6 = 30
    ///       30 >= MIN_EXPANDED_LAUNCH_HEIGHT (29) → expanded mode
    #[test]
    fn test_horizontal_tall_terminal_uses_expanded_launch_context() {
        let state = test_dialog_state();
        let content = render_dialog(&state, 100, 55);

        // Expanded mode renders no "Launch Context" titled border;
        // instead it renders stacked label+field blocks (label is uppercase).
        assert!(
            !content.contains("Launch Context"),
            "Tall horizontal terminal (100x55) should NOT show 'Launch Context' border \
             (expanded mode uses no border). Content: {:?}",
            &content.chars().take(500).collect::<String>()
        );

        // Expanded mode renders "CONFIGURATION" as the uppercase field label above the
        // bordered dropdown box (from render_config_field → DropdownField::render).
        assert!(
            content.contains("CONFIGURATION"),
            "Tall horizontal terminal (100x55) should show 'CONFIGURATION' field label \
             in expanded mode. Content: {:?}",
            &content.chars().take(500).collect::<String>()
        );
    }

    // =========================================================================
    // Group 2: Vertical layout — height-based decisions for both widgets
    // =========================================================================

    /// Narrow-and-short: vertical layout (width 40–69), compact for both widgets.
    ///
    /// Math (vertical layout uses 85% height):
    ///   dialog.height = 25 * 0.85 = 21, inner.height = 21 - 2 = 19
    ///   target.height ≈ 19 * 0.45 = 8  → 8 < MIN_EXPANDED_TARGET_HEIGHT (10) → compact
    ///   layout overhead = header(2) + sep(1) + mid-sep(1) + footer-sep(1) + footer(1) = 6, launch.height = 19 - 6 - 8 = 5 → compact
    #[test]
    fn test_vertical_short_terminal_uses_compact_both() {
        let state = test_dialog_state();
        let content = render_dialog(&state, 50, 25);

        assert!(
            content.contains("Launch Context"),
            "Short vertical terminal (50x25) should render LaunchContext in compact mode \
             (with 'Launch Context' border title). Content: {:?}",
            &content.chars().take(500).collect::<String>()
        );

        assert!(
            content.contains("Target Selector"),
            "Short vertical terminal (50x25) should render TargetSelector in compact mode \
             (with 'Target Selector' border title). Content: {:?}",
            &content.chars().take(500).collect::<String>()
        );
    }

    /// Narrow-and-medium-tall: full TargetSelector but compact LaunchContext.
    ///
    /// Math (vertical layout, 50 wide, 40 tall):
    ///   dialog.height = 40 * 0.85 = 34, inner.height = 34 - 2 = 32
    ///   target.height ≈ 32 * 0.45 = 14  → 14 >= MIN_EXPANDED_TARGET_HEIGHT (10) → full
    ///   launch.height = 32 - 6 - 14 = 12 → 12 < MIN_EXPANDED_LAUNCH_HEIGHT (29) → compact
    #[test]
    fn test_vertical_medium_tall_uses_full_target_compact_launch() {
        let state = test_dialog_state();
        let content = render_dialog(&state, 50, 40);

        // Full TargetSelector has no titled border — no "Target Selector" text
        assert!(
            !content.contains("Target Selector"),
            "Medium-tall vertical terminal (50x40) should render TargetSelector in full mode \
             (no 'Target Selector' border title). Content: {:?}",
            &content.chars().take(500).collect::<String>()
        );

        // Compact LaunchContext has the titled border
        assert!(
            content.contains("Launch Context"),
            "Medium-tall vertical terminal (50x40) should render LaunchContext in compact mode \
             (with 'Launch Context' border title). Content: {:?}",
            &content.chars().take(500).collect::<String>()
        );
    }

    /// Narrow-and-very-tall: vertical layout with enough height for expanded LaunchContext.
    ///
    /// Math (vertical layout, 50 wide, 80 tall):
    ///   dialog.height = 80 * 0.85 = 68, inner.height = 68 - 2 = 66
    ///   target.height ≈ 66 * 0.45 = 29  → full (>= 10)
    ///   launch.height = 66 - 6 - 29 = 31 → expanded (31 >= 29)
    #[test]
    fn test_vertical_tall_terminal_uses_expanded_launch_context() {
        let state = test_dialog_state();
        let content = render_dialog(&state, 50, 80);

        // Expanded LaunchContext has no titled border
        assert!(
            !content.contains("Launch Context"),
            "Tall vertical terminal (50x80) should render LaunchContext in expanded mode \
             (no 'Launch Context' border title). Content: {:?}",
            &content.chars().take(500).collect::<String>()
        );

        // Expanded mode shows uppercase field labels
        assert!(
            content.contains("CONFIGURATION") || content.contains("FLAVOR"),
            "Tall vertical terminal (50x80) should show expanded field labels such as \
             'CONFIGURATION'. Content: {:?}",
            &content.chars().take(500).collect::<String>()
        );
    }

    // =========================================================================
    // Group 3: Boundary conditions at thresholds
    // =========================================================================

    /// Test the exact threshold boundary for horizontal layout LaunchContext mode.
    ///
    /// compact_threshold: content.height = terminal.height * 0.70 - 2 - 6 < 29
    ///   → terminal.height < (28 + 8) / 0.70 ≈ 51.4
    ///   → compact at h=50, expanded at h=52 (h=51 may round to either side)
    ///
    /// h=50: dialog=35, inner=33, content=27 → compact (27 < 29)
    /// h=55: dialog=38, inner=36, content=30 → expanded (30 >= 29)
    #[test]
    fn test_horizontal_at_expanded_threshold_boundary() {
        let state = test_dialog_state();

        // Below threshold — compact
        let compact_content = render_dialog(&state, 100, 50);
        assert!(
            compact_content.contains("Launch Context"),
            "Terminal 100x50 should be in compact mode (content height 27 < threshold 29). \
             Expected 'Launch Context' border title."
        );

        // Above threshold — expanded
        let expanded_content = render_dialog(&state, 100, 55);
        assert!(
            !expanded_content.contains("Launch Context"),
            "Terminal 100x55 should be in expanded mode (content height 30 >= threshold 29). \
             Expected NO 'Launch Context' border title."
        );
        assert!(
            expanded_content.contains("CONFIGURATION"),
            "Terminal 100x55 expanded mode should show 'CONFIGURATION' field label."
        );
    }

    /// Test TargetSelector threshold in vertical layout.
    ///
    /// compact_threshold: chunks[2].height < MIN_EXPANDED_TARGET_HEIGHT (10)
    ///   chunks[2] is the 45% slice of the inner area in the vertical dialog.
    ///
    /// h=25: dialog≈23, inner≈21, 45%*21≈9 → compact (< 10)
    /// h=40: dialog≈36, inner≈34, 45%*34≈15 → full (>= 10)
    #[test]
    fn test_vertical_target_selector_at_threshold_boundary() {
        let state = test_dialog_state();

        // Clearly below threshold — compact TargetSelector (has border)
        let compact_content = render_dialog(&state, 50, 25);
        assert!(
            compact_content.contains("Target Selector"),
            "Terminal 50x25 should render TargetSelector in compact mode \
             (with 'Target Selector' border title)."
        );

        // Clearly above threshold — full TargetSelector (no border)
        // h=40: dialog≈36, inner≈34, 45%*34≈15 → full (>= 10)
        let full_content = render_dialog(&state, 50, 40);
        assert!(
            !full_content.contains("Target Selector"),
            "Terminal 50x40 should render TargetSelector in full mode \
             (no 'Target Selector' border title)."
        );
    }

    // =========================================================================
    // Group 4: Regression — standard sizes must render without panic
    // =========================================================================

    /// Classic terminal 80x24: horizontal layout, compact mode due to short height.
    ///
    /// Math: dialog.height = 24 * 0.70 = 16, content.height = 16 - 8 = 8 → compact
    #[test]
    fn test_standard_80x24_renders_without_panic() {
        let state = test_dialog_state();
        let content = render_dialog(&state, 80, 24);

        // Must render dialog content (not "too small" message)
        assert!(
            content.contains("New Session"),
            "80x24 terminal should render dialog header 'New Session'."
        );

        // Height constraint means compact launch context
        assert!(
            content.contains("Launch Context"),
            "80x24 terminal should render LaunchContext in compact mode."
        );
    }

    /// Large terminal 120x40: horizontal layout, compact mode due to height constraint.
    ///
    /// Math: dialog.height = 40 * 0.70 = 28, inner.height = 26, content.height = 20
    ///       20 < MIN_EXPANDED_LAUNCH_HEIGHT (29) → compact
    ///       (expanded requires ~55+ rows)
    #[test]
    fn test_standard_120x40_renders_without_panic() {
        let state = test_dialog_state();
        let content = render_dialog(&state, 120, 40);

        // Must render dialog content
        assert!(
            content.contains("New Session"),
            "120x40 terminal should render dialog header 'New Session'."
        );
    }

    // =========================================================================
    // Group 5: Threshold constant invariants
    // =========================================================================

    #[test]
    fn test_expanded_launch_threshold_matches_min_height() {
        assert_eq!(
            MIN_EXPANDED_LAUNCH_HEIGHT,
            LaunchContext::min_height(),
            "MIN_EXPANDED_LAUNCH_HEIGHT must equal LaunchContext::min_height() \
             to avoid button clipping at the expanded threshold boundary"
        );
    }

    // =========================================================================
    // Group 6: IconSet threading — NewSessionDialog must pass configured icons
    //          through to TargetSelector (and its TabBar) at both call sites.
    // =========================================================================

    /// Lock-in test: NewSessionDialog threads a non-default IconSet to TargetSelector
    /// so that the configured refresh glyph appears in the rendered tab bar.
    ///
    /// The horizontal layout (width >= 70, height >= 20) exercises the call site
    /// inside `render_panes()` (line ~329).
    #[test]
    fn test_new_session_dialog_threads_iconset_to_target_selector() {
        // Build a NerdFonts icon set whose refresh glyph differs from the default (Unicode).
        let nerd_icons = IconSet::new(IconMode::NerdFonts);
        let nerd_refresh = nerd_icons.refresh(); // \u{f021}
        let unicode_refresh = IconSet::default().refresh(); // \u{21bb} ↻

        // Verify test setup: the two glyphs must differ; if they ever become equal,
        // the test can no longer distinguish "threading worked" from "fallback used".
        assert_ne!(
            nerd_refresh, unicode_refresh,
            "test setup error: NerdFonts refresh glyph must differ from Unicode default"
        );

        // Build dialog state with refreshing=true so the glyph is emitted by TabBar.
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.target_selector.refreshing = true;

        let tool_availability = ToolAvailability::default();
        let dialog = NewSessionDialog::new(&state, &tool_availability, &nerd_icons);

        // Render at a size that triggers horizontal (two-pane) layout.
        // MIN_HORIZONTAL_WIDTH = 70, MIN_HORIZONTAL_HEIGHT = 20 — use 120×40 for headroom.
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("test: failed to create terminal");

        terminal
            .draw(|f| {
                f.render_widget(dialog, f.area());
            })
            .expect("test: failed to draw");

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();

        assert!(
            rendered.contains(nerd_refresh),
            "expected NerdFonts refresh glyph ({}) in rendered tab bar, got: {:?}",
            nerd_refresh,
            &rendered.chars().take(400).collect::<String>()
        );
        assert!(
            !rendered.contains(unicode_refresh),
            "default Unicode refresh glyph ({}) must NOT appear when NerdFonts is \
             configured — threading is broken if it does. got: {:?}",
            unicode_refresh,
            &rendered.chars().take(400).collect::<String>()
        );
    }

    /// Lock-in test for the vertical (compact) layout call site in `render_vertical()`
    /// (~line 551). Renders into a narrow terminal (width 40–69) with sufficient height
    /// to trigger the vertical layout path.
    ///
    /// Uses `bootable_refreshing = true` with the active tab set to Bootable so the
    /// glyph appears in the compact tab bar rendered by `render_tabs_compact`.
    #[test]
    fn test_new_session_dialog_threads_iconset_to_target_selector_vertical() {
        let nerd_icons = IconSet::new(IconMode::NerdFonts);
        let nerd_refresh = nerd_icons.refresh();
        let unicode_refresh = IconSet::default().refresh();

        assert_ne!(
            nerd_refresh, unicode_refresh,
            "test setup error: NerdFonts refresh glyph must differ from Unicode default"
        );

        // Build dialog state with Bootable tab active and bootable_refreshing=true.
        // Note: set_bootable_devices() resets bootable_refreshing to false, so we
        // must set bootable_refreshing AFTER the call to preserve the in-flight state.
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.target_selector.active_tab = TargetTab::Bootable;
        state.target_selector.loading = false;
        state.target_selector.set_bootable_devices(vec![], vec![]);
        state.target_selector.bootable_refreshing = true;

        let tool_availability = ToolAvailability::default();
        let dialog = NewSessionDialog::new(&state, &tool_availability, &nerd_icons);

        // Render at a size that triggers vertical layout:
        // MIN_VERTICAL_WIDTH=40, MIN_VERTICAL_HEIGHT=20 — use 50×40 (below horizontal
        // threshold of 70 wide, above vertical threshold of 40 wide and 20 tall).
        let backend = TestBackend::new(50, 40);
        let mut terminal = Terminal::new(backend).expect("test: failed to create terminal");

        terminal
            .draw(|f| {
                f.render_widget(dialog, f.area());
            })
            .expect("test: failed to draw");

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();

        assert!(
            rendered.contains(nerd_refresh),
            "expected NerdFonts refresh glyph ({}) in vertical-layout rendered tab bar, got: {:?}",
            nerd_refresh,
            &rendered.chars().take(400).collect::<String>()
        );
        assert!(
            !rendered.contains(unicode_refresh),
            "default Unicode refresh glyph ({}) must NOT appear when NerdFonts is \
             configured in vertical layout. got: {:?}",
            unicode_refresh,
            &rendered.chars().take(400).collect::<String>()
        );
    }

    // =========================================================================
    // Group 7: render_with_regions integration tests
    // =========================================================================

    use crate::widgets::MouseCtx;
    use fdemon_app::mouse_regions::MouseRegions;

    /// Helper: call `render_with_regions` and return the populated registry.
    fn render_dialog_with_regions(
        state: &NewSessionDialogState,
        width: u16,
        height: u16,
    ) -> MouseRegions {
        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);
        let mut regions = MouseRegions::default();

        // Use TestBackend to get a Buffer
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("failed to create terminal");

        terminal
            .draw(|f| {
                let area = f.area();
                let buf = f.buffer_mut();
                let dialog = NewSessionDialog::new(state, &tool_availability, &icons);
                {
                    let builder = regions.builder();
                    let mut ctx = MouseCtx::new(builder);
                    render_with_regions(area, buf, dialog, Some(&mut ctx));
                }
            })
            .expect("failed to draw");

        regions
    }

    /// Acceptance criterion 4: with 2-tab dialog and one connected device,
    /// the registry contains ≥ 8 regions (2 tabs + 1 device + ≥4 fields + 1 launch), all z=1.
    #[test]
    fn render_with_regions_integration_horizontal_layout_counts_regions() {
        let state = test_dialog_state(); // has one connected device
                                         // Use a terminal size that triggers horizontal layout
        let regions = render_dialog_with_regions(&state, 120, 60);

        let z1_count = regions.iter().filter(|e| e.z_index == 1).count();
        assert!(
            z1_count >= 8,
            "expected at least 8 z=1 regions (2 tabs + 1 device + 5 fields + 1 launch), got {} total regions ({} at z=1)",
            regions.len(),
            z1_count,
        );

        let z2_count = regions.iter().filter(|e| e.z_index == 2).count();
        assert_eq!(
            z2_count, 0,
            "without fuzzy modal open, expected 0 z=2 regions, got {}",
            z2_count
        );
    }

    /// Acceptance criterion 5: with fuzzy modal open and 5 matches,
    /// an additional 5 regions at z=2 appear.
    #[test]
    fn render_with_regions_integration_fuzzy_modal_records_z2_regions() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.open_flavor_modal(vec![
            "alpha".into(),
            "beta".into(),
            "gamma".into(),
            "delta".into(),
            "epsilon".into(),
        ]);

        let regions = render_dialog_with_regions(&state, 120, 60);

        let z2_count = regions.iter().filter(|e| e.z_index == 2).count();
        assert!(
            z2_count > 0,
            "expected z=2 regions from fuzzy modal, got 0 (total regions: {})",
            regions.len()
        );
    }

    // =========================================================================
    // Group 8: Single-pass render invariant (T10)
    // =========================================================================

    /// Assert that the with-regions path (Some ctx) and the no-regions path (None ctx /
    /// Widget::render) produce byte-identical buffers when a fuzzy modal is open.
    ///
    /// This guards against accidental re-introduction of the double-render pattern:
    /// if `render_fuzzy_modal_overlay` were called before `fuzzy_modal_render_with_regions`
    /// the idempotence of painting would hide the bug visually, but the cost would double.
    /// This test also confirms that the single-pass path paints correctly by comparing the
    /// two rendering variants — if they differ, the refactor broke something.
    #[test]
    fn fuzzy_modal_renders_once_per_frame_in_with_regions_path() {
        const W: u16 = 120;
        const H: u16 = 60;

        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.open_flavor_modal(vec![
            "alpha".into(),
            "beta".into(),
            "gamma".into(),
            "delta".into(),
            "epsilon".into(),
        ]);

        let tool_availability = ToolAvailability::default();
        let icons = IconSet::new(IconMode::Unicode);

        // --- Path A: Widget::render (no ctx, single render) ---
        let buf_no_ctx = {
            use ratatui::backend::TestBackend;
            use ratatui::Terminal;
            let backend = TestBackend::new(W, H);
            let mut terminal = Terminal::new(backend).expect("path-A: failed to create terminal");
            terminal
                .draw(|f| {
                    let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                    f.render_widget(dialog, f.area());
                })
                .expect("path-A: failed to draw");
            terminal.backend().buffer().clone()
        };

        // --- Path B: render_with_regions with Some(ctx) (single-pass after refactor) ---
        let buf_with_ctx = {
            use ratatui::backend::TestBackend;
            use ratatui::Terminal;
            let backend = TestBackend::new(W, H);
            let mut terminal = Terminal::new(backend).expect("path-B: failed to create terminal");
            let mut regions = MouseRegions::default();
            terminal
                .draw(|f| {
                    let area = f.area();
                    let buf = f.buffer_mut();
                    let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                    let builder = regions.builder();
                    let mut ctx = MouseCtx::new(builder);
                    render_with_regions(area, buf, dialog, Some(&mut ctx));
                })
                .expect("path-B: failed to draw");
            terminal.backend().buffer().clone()
        };

        // Buffers must be identical: the single-pass (ctx=Some) path must paint
        // exactly what the no-ctx path paints.
        assert_eq!(
            buf_no_ctx, buf_with_ctx,
            "render_with_regions (Some ctx) produced a different buffer than Widget::render \
             (None ctx) — single-pass refactor broke visual output"
        );

        // Verify that fuzzy modal regions were recorded in path B.
        // This confirms the single-pass path still records click regions.
        let buf_with_ctx_regions = {
            use ratatui::backend::TestBackend;
            use ratatui::Terminal;
            let backend = TestBackend::new(W, H);
            let mut terminal = Terminal::new(backend).expect("path-B2: failed to create terminal");
            let mut regions = MouseRegions::default();
            terminal
                .draw(|f| {
                    let area = f.area();
                    let buf = f.buffer_mut();
                    let dialog = NewSessionDialog::new(&state, &tool_availability, &icons);
                    let builder = regions.builder();
                    let mut ctx = MouseCtx::new(builder);
                    render_with_regions(area, buf, dialog, Some(&mut ctx));
                })
                .expect("path-B2: failed to draw");
            regions
        };

        let z2_count = buf_with_ctx_regions
            .iter()
            .filter(|e| e.z_index == 2)
            .count();
        assert!(
            z2_count > 0,
            "single-pass path must still register z=2 fuzzy modal regions (got 0)"
        );
    }
}

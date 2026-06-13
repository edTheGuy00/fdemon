//! Settings panel widget - full-screen settings UI
//!
//! Displays a tabbed interface for managing:
//! - Project settings (config.toml)
//! - User preferences (settings.local.toml)
//! - Launch configurations (launch.toml)
//! - VSCode configurations (launch.json, read-only)

mod styles;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, StatefulWidget, Widget},
};

use crate::theme::{icons::IconSet, palette};
use crate::widgets::modal_overlay;
use crate::widgets::new_session_dialog::{DartDefinesModal, FuzzyModal};

use std::path::Path;

use fdemon_app::config::{
    SettingItem, Settings, SettingsTab, UserPreferences, BUILTIN_SETTINGS_TAB_COUNT,
};
use fdemon_app::new_session_dialog::{DartDefinesModalState, FuzzyModalState};
use fdemon_app::settings_items::{
    launch_config_items, project_settings_items, user_prefs_items, visual_row_of_item,
    vscode_config_items,
};
use fdemon_app::state::SettingsViewState;

// Use styles module
use styles::{
    add_new_style, config_header_style, description_style, editing_style, label_style,
    override_indicator_style, readonly_label_style, readonly_value_style, truncate_str,
    value_style, vscode_header_style, INDICATOR_WIDTH, LABEL_WIDTH, LABEL_WIDTH_SHORT,
    LABEL_WIDTH_VSCODE, VALUE_WIDTH, VALUE_WIDTH_VSCODE,
};

// ─────────────────────────────────────────────────────────────────────────────
// Layout constants — single source of truth shared by renderers and region
// recorder.  Changing any value here automatically keeps visual output and
// mouse-region rects in sync.
// ─────────────────────────────────────────────────────────────────────────────

/// Fixed pixel width (columns) of each settings tab pill.
const SETTINGS_TAB_WIDTH: u16 = 12;

/// Gap (columns) between adjacent tab pills.
const SETTINGS_TAB_GAP: u16 = 1;

/// Height (rows) of the info banner rendered above the User Preferences item list.
const SETTINGS_USER_PREFS_BANNER_HEIGHT: u16 = 4;

/// Height (rows) of the info banner rendered above the VSCode config item list.
const SETTINGS_VSCODE_BANNER_HEIGHT: u16 = 4;

/// Full-screen settings panel widget
pub struct SettingsPanel<'a> {
    /// Reference to application settings
    settings: &'a Settings,

    /// Project path for loading configurations
    project_path: &'a Path,

    /// Host-injected extra settings tabs, rendered after the four built-ins.
    extra_tabs: &'a [Box<dyn fdemon_app::settings_tab_provider::SettingsTabProvider>],

    /// Title to display in header
    title: &'a str,
}

impl<'a> SettingsPanel<'a> {
    pub fn new(
        settings: &'a Settings,
        project_path: &'a Path,
        extra_tabs: &'a [Box<dyn fdemon_app::settings_tab_provider::SettingsTabProvider>],
    ) -> Self {
        Self {
            settings,
            project_path,
            extra_tabs,
            title: "Settings",
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }
}

impl StatefulWidget for SettingsPanel<'_> {
    type State = SettingsViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Clear the background with a solid color
        let bg_style = Style::default().bg(palette::DEEPEST_BG);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buf[(x, y)].set_style(bg_style).set_char(' ');
            }
        }

        // Create main layout
        let chunks = Layout::vertical([
            Constraint::Length(5), // Header: title row + gap + tab row + gap + border
            Constraint::Min(5),    // Content area
            Constraint::Length(3), // Footer with shortcuts (3 lines for better visibility)
        ])
        .split(area);

        // Render header with tabs
        self.render_header(chunks[0], buf, state);

        // Render content based on active tab
        self.render_content(chunks[1], buf, state);

        // Render footer with keyboard shortcuts
        self.render_footer(chunks[2], buf, state);

        // Modal overlays (rendered last to appear on top).
        // Only one modal is open at a time (enforced by has_modal_open() check on open handlers).
        if let Some(dart_defines_modal) = &state.dart_defines_modal {
            self.render_dart_defines_modal_overlay(area, buf, dart_defines_modal);
        } else if let Some(extra_args_modal) = &state.extra_args_modal {
            self.render_extra_args_modal_overlay(area, buf, extra_args_modal);
        }
    }
}

impl SettingsPanel<'_> {
    // ─────────────────────────────────────────────────────────────────────────────
    // Header and Tab Rendering
    // ─────────────────────────────────────────────────────────────────────────────

    fn render_header(&self, area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
        // Background: SURFACE for the entire header area
        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::border_inactive())
            .style(Style::default().bg(palette::SURFACE));

        let inner = header_block.inner(area);
        header_block.render(area, buf);

        // Row 1: Icon + Title (left) ... [Esc] Close (right)
        let title_y = inner.top();

        // Left: settings icon + title
        let icons = IconSet::new(self.settings.ui.icons);
        let icon_span = Span::styled(
            format!("{} ", icons.settings()),
            Style::default().fg(palette::ACCENT),
        );
        let title_span = Span::styled(
            "System Settings",
            Style::default()
                .fg(palette::TEXT_BRIGHT)
                .add_modifier(Modifier::BOLD),
        );
        let title_line = Line::from(vec![icon_span, title_span]);
        buf.set_line(inner.left() + 1, title_y, &title_line, inner.width - 2);

        // Right: [Esc] Close
        let esc_badge = Span::styled(" Esc ", styles::kbd_badge_style());
        let close_label = Span::styled(" Close", styles::kbd_label_style());
        let close_line = Line::from(vec![esc_badge, close_label]);
        let close_width = 11; // " Esc  Close"
        buf.set_line(
            inner.right() - close_width - 1,
            title_y,
            &close_line,
            close_width,
        );

        // Row 3 (skip 1 line gap): Tab bar
        let tab_y = title_y + 2;
        let tab_area = Rect::new(inner.left() + 1, tab_y, inner.width - 2, 1);
        self.render_tab_bar(tab_area, buf, state);
    }

    fn render_tab_bar(&self, area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
        let mut tabs: Vec<(SettingsTab, String)> = vec![
            (SettingsTab::Project, "1. PROJECT".to_string()),
            (SettingsTab::UserPrefs, "2. USER".to_string()),
            (SettingsTab::LaunchConfig, "3. LAUNCH".to_string()),
            (SettingsTab::VSCodeConfig, "4. VSCODE".to_string()),
        ];
        // Append one pill per host-injected tab, numbered after the built-ins.
        for (i, provider) in self.extra_tabs.iter().enumerate() {
            tabs.push((
                SettingsTab::Extra(i),
                format!(
                    "{}. {}",
                    BUILTIN_SETTINGS_TAB_COUNT + 1 + i,
                    provider.title().to_uppercase()
                ),
            ));
        }

        let tab_width = SETTINGS_TAB_WIDTH;
        let gap = SETTINGS_TAB_GAP;

        let mut x = area.left();
        for (tab, label) in tabs {
            if x + tab_width > area.right() {
                break;
            }

            let is_active = state.active_tab == tab;
            let tab_rect = Rect::new(x, area.top(), tab_width, 1);

            if is_active {
                // Active: ACCENT bg, TEXT_BRIGHT fg, BOLD
                let style = Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .bg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD);
                let centered = format!("{:^width$}", label, width = tab_width as usize);
                buf.set_string(tab_rect.left(), tab_rect.top(), &centered, style);
            } else {
                // Inactive: no bg, TEXT_SECONDARY
                let style = Style::default().fg(palette::TEXT_SECONDARY);
                let centered = format!("{:^width$}", label, width = tab_width as usize);
                buf.set_string(tab_rect.left(), tab_rect.top(), &centered, style);
            }

            x += tab_width + gap;
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Content Rendering
    // ─────────────────────────────────────────────────────────────────────────────

    fn render_content(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        use fdemon_app::config::{launch::load_launch_configs, load_vscode_configs};

        let content_block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_set(symbols::border::ROUNDED);

        let inner = content_block.inner(area);
        content_block.render(area, buf);

        // Create IconSet once for all renderers
        let icons = IconSet::new(self.settings.ui.icons);

        // Load configs once per render pass — only for the active tab that needs them.
        // The loaded vec is threaded into the tab renderer so neither renderer nor the
        // companion `render_with_regions` region recorder needs a second disk read.
        match state.active_tab {
            SettingsTab::Project => self.render_project_tab(inner, buf, state, &icons),
            SettingsTab::UserPrefs => self.render_user_prefs_tab(inner, buf, state, &icons),
            SettingsTab::LaunchConfig => {
                let configs = load_launch_configs(self.project_path);
                self.render_launch_tab(inner, buf, state, &icons, &configs);
            }
            SettingsTab::VSCodeConfig => {
                let configs = load_vscode_configs(self.project_path);
                self.render_vscode_tab(inner, buf, state, &icons, &configs);
            }
            SettingsTab::Extra(i) => {
                if let Some(provider) = self.extra_tabs.get(i) {
                    let items = provider.items();
                    self.render_generic_tab(inner, buf, state, &icons, &items);
                }
            }
        }
    }

    /// Clamp `state.scroll_offset` so the selected row (at visual row
    /// `sel_vrow`) stays within the viewport of `viewport_rows` rows, then return
    /// the resolved scroll offset.
    ///
    /// The region recorder reads the SAME `state.scroll_offset` after this runs,
    /// so click targets and rendered rows stay in lockstep.
    // EXCEPTION (TEA render purity): resolve_scroll writes the render-derived scroll_offset clamp; StatefulWidget supplies &mut state so a Cell<usize> is unnecessary. See docs/REVIEW_FOCUS.md.
    fn resolve_scroll(
        state: &mut SettingsViewState,
        sel_vrow: usize,
        viewport_rows: usize,
    ) -> usize {
        if viewport_rows == 0 {
            return state.scroll_offset;
        }
        if sel_vrow < state.scroll_offset {
            state.scroll_offset = sel_vrow;
        } else if sel_vrow >= state.scroll_offset + viewport_rows {
            state.scroll_offset = sel_vrow + 1 - viewport_rows;
        }
        state.scroll_offset
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
        // Dark background block with rounded border
        let footer_block = Block::default()
            .borders(Borders::ALL ^ Borders::TOP)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .style(Style::default().bg(palette::DEEPEST_BG));

        let inner = footer_block.inner(area);
        footer_block.render(area, buf);

        // Create IconSet for rendering icons
        let icons = IconSet::new(self.settings.ui.icons);

        // Render hints based on state
        if state.editing {
            self.render_editing_footer_hints(inner, buf, &icons);
        } else {
            self.render_normal_footer_hints(inner, buf, &icons, state.dirty);
        }
    }

    /// Render footer hints in normal (non-editing) mode
    fn render_normal_footer_hints(
        &self,
        area: Rect,
        buf: &mut Buffer,
        icons: &IconSet,
        is_dirty: bool,
    ) {
        // Build 4 shortcut hints
        let hints = [
            self.build_hint(icons.keyboard(), "Tab:", "Switch tabs", false),
            self.build_hint(icons.chevron_right(), "j/k:", "Navigate", false),
            self.build_hint(icons.chevron_right(), "Enter:", "Edit", false),
            self.build_hint(
                icons.save(),
                "Ctrl+S:",
                if is_dirty {
                    "Save Changes*"
                } else {
                    "Save Changes"
                },
                true, // emphasized
            ),
        ];

        // Combine hints with spacing
        let mut spans: Vec<Span> = Vec::new();
        for (i, hint) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("    ")); // 4-space gap between hints
            }
            spans.extend(hint.spans.clone());
        }

        let centered_line = Line::from(spans).alignment(Alignment::Center);
        buf.set_line(area.left(), area.top(), &centered_line, area.width);
    }

    /// Render footer hints in editing mode
    fn render_editing_footer_hints(&self, area: Rect, buf: &mut Buffer, icons: &IconSet) {
        let hints = Line::from(vec![
            Span::styled(
                format!("{} ", icons.check()),
                Style::default().fg(palette::STATUS_GREEN),
            ),
            Span::styled("Enter:", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(" Confirm", styles::kbd_label_style()),
            Span::raw("    "),
            Span::styled(
                format!("{} ", icons.close()),
                Style::default().fg(palette::STATUS_RED),
            ),
            Span::styled("Esc:", Style::default().fg(palette::TEXT_SECONDARY)),
            Span::styled(" Cancel", styles::kbd_label_style()),
        ])
        .alignment(Alignment::Center);

        buf.set_line(area.left(), area.top(), &hints, area.width);
    }

    /// Build a single hint with icon, key, and label
    fn build_hint<'a>(
        &self,
        icon: &'a str,
        key: &'a str,
        label: &'a str,
        emphasized: bool,
    ) -> Line<'a> {
        let icon_style = if emphasized {
            Style::default().fg(palette::ACCENT)
        } else {
            Style::default().fg(palette::TEXT_MUTED)
        };

        let key_style = if emphasized {
            styles::kbd_accent_style() // ACCENT fg
        } else {
            Style::default().fg(palette::TEXT_SECONDARY)
        };

        let label_style = styles::kbd_label_style(); // TEXT_MUTED

        Line::from(vec![
            Span::styled(format!("{} ", icon), icon_style),
            Span::styled(key, key_style),
            Span::styled(format!(" {}", label), label_style),
        ])
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Modal Overlay Rendering
    // ─────────────────────────────────────────────────────────────────────────────

    /// Render the dart defines modal overlay on top of the settings panel.
    ///
    /// Dims the background and then renders the `DartDefinesModal` widget over
    /// the full area.  The widget self-computes its position (full-screen minus
    /// margins) and calls `Clear` on its area internally, so settings content
    /// behind it is properly overwritten.
    fn render_dart_defines_modal_overlay(
        &self,
        area: Rect,
        buf: &mut Buffer,
        modal_state: &DartDefinesModalState,
    ) {
        modal_overlay::dim_background(buf, area);

        let modal = DartDefinesModal::new(modal_state);
        modal.render(area, buf);
    }

    /// Render the extra args fuzzy modal overlay on top of the settings panel.
    ///
    /// Dims the background and then renders the `FuzzyModal` widget over the
    /// full area.  The widget self-positions at the bottom ~50% of the given
    /// area and calls `Clear` on its area internally.
    fn render_extra_args_modal_overlay(
        &self,
        area: Rect,
        buf: &mut Buffer,
        modal_state: &FuzzyModalState,
    ) {
        modal_overlay::dim_background(buf, area);

        let modal = FuzzyModal::new(modal_state);
        modal.render(area, buf);
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Project Tab
    // ─────────────────────────────────────────────────────────────────────────────

    fn render_project_tab(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut SettingsViewState,
        icons: &IconSet,
    ) {
        let items = project_settings_items(self.settings);

        let list_top = area.y;
        let list_bottom = area.bottom();
        let viewport_rows = list_bottom.saturating_sub(list_top) as usize;

        // Scroll-follow: ensure the selected item is on-screen.
        let sel_vrow = visual_row_of_item(&items, state.selected_index);
        // EXCEPTION (TEA): render-time scroll clamp (see resolve_scroll).
        let scroll = Self::resolve_scroll(state, sel_vrow, viewport_rows);

        // Group items by section, walking visual rows so we can apply the scroll
        // offset and break once we run off the bottom of the viewport.
        let mut current_section = String::new();
        let mut vrow = 0usize;

        for (idx, item) in items.iter().enumerate() {
            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    vrow += 1; // Spacer between sections (no draw)
                }
                if vrow >= scroll {
                    let y = list_top + (vrow - scroll) as u16;
                    if y >= list_bottom {
                        break;
                    }
                    self.render_section_header(area.x, y, area.width, buf, &item.section, icons);
                }
                vrow += 1;
                current_section = item.section.clone();
            }

            // Setting row
            if vrow >= scroll {
                let y = list_top + (vrow - scroll) as u16;
                if y >= list_bottom {
                    break;
                }
                let is_selected = idx == state.selected_index;
                let is_editing = is_selected && state.editing;
                self.render_setting_row(
                    area.x,
                    y,
                    area.width,
                    buf,
                    item,
                    is_selected,
                    is_editing,
                    &state.edit_buffer,
                );
            }
            vrow += 1;
        }
    }

    /// Render a host-injected (Extra) tab's item list.
    ///
    /// This is the generic sibling of [`render_project_tab`](Self::render_project_tab):
    /// it performs the IDENTICAL scroll-aware, section-grouped visual-row walk
    /// over a caller-supplied `items` slice. Keeping the two loops byte-identical
    /// guarantees the mouse region recorder (which reuses
    /// [`register_setting_row_regions`]) stays in lockstep with what is drawn.
    fn render_generic_tab(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut SettingsViewState,
        icons: &IconSet,
        items: &[SettingItem],
    ) {
        let list_top = area.y;
        let list_bottom = area.bottom();
        let viewport_rows = list_bottom.saturating_sub(list_top) as usize;

        // Scroll-follow: ensure the selected item is on-screen.
        let sel_vrow = visual_row_of_item(items, state.selected_index);
        // EXCEPTION (TEA): render-time scroll clamp (see resolve_scroll).
        let scroll = Self::resolve_scroll(state, sel_vrow, viewport_rows);

        let mut current_section = String::new();
        let mut vrow = 0usize;

        for (idx, item) in items.iter().enumerate() {
            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    vrow += 1; // Spacer between sections (no draw)
                }
                if vrow >= scroll {
                    let y = list_top + (vrow - scroll) as u16;
                    if y >= list_bottom {
                        break;
                    }
                    self.render_section_header(area.x, y, area.width, buf, &item.section, icons);
                }
                vrow += 1;
                current_section = item.section.clone();
            }

            // Setting row
            if vrow >= scroll {
                let y = list_top + (vrow - scroll) as u16;
                if y >= list_bottom {
                    break;
                }
                let is_selected = idx == state.selected_index;
                let is_editing = is_selected && state.editing;
                self.render_setting_row(
                    area.x,
                    y,
                    area.width,
                    buf,
                    item,
                    is_selected,
                    is_editing,
                    &state.edit_buffer,
                );
            }
            vrow += 1;
        }
    }

    fn render_section_header(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        section: &str,
        icons: &IconSet,
    ) {
        // Map section name to icon
        let icon = match section.to_lowercase().as_str() {
            "behavior" => icons.zap(),
            "watcher" => icons.eye(),
            "ui" | "ui preferences" => icons.monitor(),
            "devtools" => icons.cpu(),
            "editor" | "editor override" => icons.code(),
            "session memory" => icons.user(),
            _ => icons.settings(),
        };

        // Normal uppercase letter spacing: "Behavior" → "BEHAVIOR".
        let label = section.to_uppercase();

        let icon_span = Span::styled(format!("  {} ", icon), styles::group_header_icon_style());
        let label_span = Span::styled(
            label,
            styles::section_header_style(), // Now returns ACCENT_DIM + BOLD
        );

        let line = Line::from(vec![icon_span, label_span]);
        buf.set_line(x, y, &line, width);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_setting_row(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        item: &SettingItem,
        is_selected: bool,
        is_editing: bool,
        edit_buffer: &str,
    ) {
        // Apply background for selected row
        if is_selected {
            let bg_style = styles::selected_row_bg();
            for col in x..x + width {
                if let Some(cell) = buf.cell_mut((col, y)) {
                    cell.set_style(bg_style);
                }
            }
        }

        let mut col = x;

        // Column 0: Left accent bar (1 char)
        if is_selected {
            if let Some(cell) = buf.cell_mut((col, y)) {
                cell.set_symbol("▎");
                cell.set_fg(palette::ACCENT);
            }
        }
        col += INDICATOR_WIDTH; // 3 chars total: bar + 2 spaces

        // Column 1: Label (LABEL_WIDTH chars)
        let label_text = truncate_str(&item.label, LABEL_WIDTH as usize);
        let label_style = label_style(is_selected);
        buf.set_string(
            col,
            y,
            format!("{:<width$}", label_text, width = LABEL_WIDTH as usize),
            label_style,
        );
        col += LABEL_WIDTH;

        // Column 2: Value (VALUE_WIDTH chars)
        if is_editing && is_selected {
            // Show edit buffer + cursor
            let display = format!("{}▌", edit_buffer);
            let truncated = truncate_str(&display, VALUE_WIDTH as usize);
            buf.set_string(
                col,
                y,
                format!("{:<width$}", truncated, width = VALUE_WIDTH as usize),
                editing_style(),
            );
        } else {
            let display = item.value.display();
            let modified_marker = if item.is_modified() { "*" } else { "" };
            let display_with_marker = format!("{}{}", display, modified_marker);
            let truncated = truncate_str(&display_with_marker, VALUE_WIDTH as usize);
            let val_style = value_style(&item.value, is_selected);
            buf.set_string(
                col,
                y,
                format!("{:<width$}", truncated, width = VALUE_WIDTH as usize),
                val_style,
            );
        }
        col += VALUE_WIDTH;

        // Column 3: Description (remaining width, italic)
        let remaining = width.saturating_sub(col - x);
        if remaining > 3 {
            let desc = truncate_str(&item.description, remaining as usize);
            buf.set_string(col, y, &desc, description_style());
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // User Preferences Tab
    // ─────────────────────────────────────────────────────────────────────────────

    fn render_user_prefs_tab(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut SettingsViewState,
        icons: &IconSet,
    ) {
        // Render info banner about local settings
        let info_area = Rect::new(
            area.x,
            area.y,
            area.width,
            SETTINGS_USER_PREFS_BANNER_HEIGHT,
        );
        self.render_user_prefs_info(info_area, buf);

        // Content area below info banner
        let content_area = Rect::new(
            area.x,
            area.y + SETTINGS_USER_PREFS_BANNER_HEIGHT,
            area.width,
            area.height
                .saturating_sub(SETTINGS_USER_PREFS_BANNER_HEIGHT),
        );

        let items = user_prefs_items(&state.user_prefs, self.settings);

        let list_top = content_area.y;
        let list_bottom = content_area.bottom();
        let viewport_rows = list_bottom.saturating_sub(list_top) as usize;

        let sel_vrow = visual_row_of_item(&items, state.selected_index);
        // EXCEPTION (TEA): render-time scroll clamp (see resolve_scroll).
        let scroll = Self::resolve_scroll(state, sel_vrow, viewport_rows);

        // Group items by section, walking visual rows (offset-aware).
        let mut current_section = String::new();
        let mut vrow = 0usize;

        for (idx, item) in items.iter().enumerate() {
            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    vrow += 1; // Spacer (no draw)
                }
                if vrow >= scroll {
                    let y = list_top + (vrow - scroll) as u16;
                    if y >= list_bottom {
                        break;
                    }
                    self.render_section_header(
                        content_area.x,
                        y,
                        content_area.width,
                        buf,
                        &item.section,
                        icons,
                    );
                }
                vrow += 1;
                current_section = item.section.clone();
            }

            // Setting row
            if vrow >= scroll {
                let y = list_top + (vrow - scroll) as u16;
                if y >= list_bottom {
                    break;
                }
                let is_selected = idx == state.selected_index;
                let is_editing = is_selected && state.editing;
                self.render_user_pref_row(
                    content_area.x,
                    y,
                    content_area.width,
                    buf,
                    item,
                    &state.user_prefs,
                    is_selected,
                    is_editing,
                    &state.edit_buffer,
                );
            }
            vrow += 1;
        }
    }

    fn render_user_prefs_info(&self, area: Rect, buf: &mut Buffer) {
        let icons = IconSet::new(self.settings.ui.icons);

        // Glass info banner: rounded border, accent-tinted bg
        let banner = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::info_banner_border_style()) // ACCENT_DIM
            .style(styles::info_banner_bg()); // SELECTED_ROW_BG bg

        let inner = banner.inner(area);
        banner.render(area, buf);

        if inner.height < 2 {
            return;
        }

        // Line 1: icon + title
        let icon_span = Span::styled(
            format!(" {} ", icons.info()),
            Style::default().fg(palette::ACCENT),
        );
        let title_span = Span::styled(
            "Local Settings Active",
            Style::default()
                .fg(palette::TEXT_BRIGHT)
                .add_modifier(Modifier::BOLD),
        );
        let title_line = Line::from(vec![icon_span, title_span]);
        buf.set_line(inner.left(), inner.top(), &title_line, inner.width);

        // Line 2: subtitle (indented to align with title text)
        if inner.height >= 2 {
            let subtitle = Span::styled(
                "    Stored in: .fdemon/settings.local.toml",
                Style::default().fg(palette::ACCENT_DIM),
            );
            buf.set_line(
                inner.left(),
                inner.top() + 1,
                &Line::from(subtitle),
                inner.width,
            );
        }
    }

    /// Check if user pref overrides project setting
    fn is_override_active(&self, prefs: &UserPreferences, item_id: &str) -> bool {
        match item_id {
            "editor.command" => prefs
                .editor
                .as_ref()
                .map(|e| !e.command.is_empty())
                .unwrap_or(false),
            "editor.open_pattern" => prefs
                .editor
                .as_ref()
                .map(|e| e.open_pattern != "$EDITOR $FILE:$LINE")
                .unwrap_or(false),
            "theme" => prefs.theme.is_some(),
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_user_pref_row(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        item: &SettingItem,
        prefs: &UserPreferences,
        is_selected: bool,
        is_editing: bool,
        edit_buffer: &str,
    ) {
        // Apply background for selected row
        if is_selected {
            let bg_style = styles::selected_row_bg();
            for col in x..x + width {
                if let Some(cell) = buf.cell_mut((col, y)) {
                    cell.set_style(bg_style);
                }
            }
        }

        let mut col = x;
        let is_override = self.is_override_active(prefs, &item.id);

        // Column 0: Left accent bar + override indicator
        if is_selected {
            if let Some(cell) = buf.cell_mut((col, y)) {
                cell.set_symbol("▎");
                cell.set_fg(palette::ACCENT);
            }
        }
        col += 1;

        // Override indicator (⚡ if override active)
        if is_override {
            buf.set_string(col, y, "⚡", override_indicator_style(true, is_selected));
        }
        col += 2; // Space for indicator + gap

        // Column 1: Label (LABEL_WIDTH_SHORT chars)
        let label = truncate_str(&item.label, LABEL_WIDTH_SHORT as usize);
        buf.set_string(
            col,
            y,
            format!("{:<width$}", label, width = LABEL_WIDTH_SHORT as usize),
            label_style(is_selected),
        );
        col += LABEL_WIDTH_SHORT;

        // Column 2: Value (VALUE_WIDTH chars)
        if is_editing && is_selected {
            let display = format!("{}▌", edit_buffer);
            let truncated = truncate_str(&display, VALUE_WIDTH as usize);
            buf.set_string(
                col,
                y,
                format!("{:<width$}", truncated, width = VALUE_WIDTH as usize),
                editing_style(),
            );
        } else {
            let modified_indicator = if item.is_modified() { "*" } else { "" };
            let display_val = item.value.display();
            let display_str = if display_val.is_empty() {
                "<empty>".to_string()
            } else {
                display_val
            };
            let display_with_marker = format!("{}{}", display_str, modified_indicator);
            let truncated = truncate_str(&display_with_marker, VALUE_WIDTH as usize);
            let val_style = value_style(&item.value, is_selected);
            buf.set_string(
                col,
                y,
                format!("{:<width$}", truncated, width = VALUE_WIDTH as usize),
                val_style,
            );
        }
        col += VALUE_WIDTH;

        // Column 3: Description (remaining width, italic)
        let remaining = width.saturating_sub(col - x);
        if remaining > 3 {
            let desc = truncate_str(&item.description, remaining as usize);
            buf.set_string(col, y, &desc, description_style());
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Launch Config Tab
    // ─────────────────────────────────────────────────────────────────────────────

    fn render_launch_tab(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut SettingsViewState,
        _icons: &IconSet,
        configs: &[fdemon_app::config::ResolvedLaunchConfig],
    ) {
        if configs.is_empty() {
            self.render_launch_empty_state(area, buf);
            return;
        }

        // Generate all items from configs
        let mut all_items: Vec<SettingItem> = Vec::new();
        for (idx, resolved) in configs.iter().enumerate() {
            all_items.extend(launch_config_items(&resolved.config, idx));
        }

        let list_top = area.y;
        let list_bottom = area.bottom();
        let viewport_rows = list_bottom.saturating_sub(list_top) as usize;

        // Scroll-follow: the add-new sentinel selection (selected_index ==
        // all_items.len()) maps to the trailing-sentinel visual row.
        let sel_vrow = visual_row_of_item(&all_items, state.selected_index);
        // EXCEPTION (TEA): render-time scroll clamp (see resolve_scroll).
        let scroll = Self::resolve_scroll(state, sel_vrow, viewport_rows);

        // Render items with sections (offset-aware visual-row walk).
        let mut current_section = String::new();
        let mut vrow = 0usize;
        let mut overflowed = false;

        for (idx, item) in all_items.iter().enumerate() {
            // Section header (configuration separator)
            if item.section != current_section {
                if !current_section.is_empty() {
                    vrow += 1; // Spacer between configurations (no draw)
                }
                if vrow >= scroll {
                    let y = list_top + (vrow - scroll) as u16;
                    if y >= list_bottom {
                        overflowed = true;
                        break;
                    }
                    self.render_config_header(area.x, y, area.width, buf, &item.section);
                }
                vrow += 1;
                current_section = item.section.clone();
            }

            // Setting row
            if vrow >= scroll {
                let y = list_top + (vrow - scroll) as u16;
                if y >= list_bottom {
                    overflowed = true;
                    break;
                }
                let is_selected = idx == state.selected_index;
                let is_editing = is_selected && state.editing;
                self.render_setting_row(
                    area.x,
                    y,
                    area.width,
                    buf,
                    item,
                    is_selected,
                    is_editing,
                    &state.edit_buffer,
                );
            }
            vrow += 1;
        }

        // Add "New Configuration" option at bottom (one spacer after last item).
        if !overflowed {
            let sentinel_vrow = vrow + 1; // matches visual_row_of_item(all_items.len())
            if sentinel_vrow >= scroll {
                let y = list_top + (sentinel_vrow - scroll) as u16;
                if y < list_bottom {
                    let is_selected = state.selected_index == all_items.len();
                    self.render_add_config_option(area.x, y, area.width, buf, is_selected);
                }
            }
        }
    }

    fn render_launch_empty_state(&self, area: Rect, buf: &mut Buffer) {
        let icons = IconSet::new(self.settings.ui.icons);

        // Center vertically: icon box (3 lines) + gap (1) + title (1) + gap (1) + subtitle (1) = 7 lines
        let total_height = 7u16;
        if area.height < total_height {
            // Not enough space, degrade gracefully - show just title
            if area.height >= 1 {
                let title = Line::from(Span::styled(
                    "No launch configurations found",
                    styles::empty_state_title_style(),
                ))
                .alignment(Alignment::Center);
                buf.set_line(
                    area.left(),
                    area.top() + area.height / 2,
                    &title,
                    area.width,
                );
            }
            return;
        }

        let start_y = area.top() + 1;

        // Icon container: centered 9-wide box
        let icon_width = 9u16;
        let icon_x = area.left() + area.width.saturating_sub(icon_width) / 2;

        if start_y + 3 <= area.bottom() {
            let icon_rect = Rect::new(icon_x, start_y, icon_width, 3);
            let icon_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::BORDER_DIM));
            let icon_inner = icon_block.inner(icon_rect);
            icon_block.render(icon_rect, buf);

            // Center the icon glyph
            let icon_str = icons.layers();
            let icon_span = Span::styled(icon_str, styles::empty_state_icon_style());
            let icon_line = Line::from(icon_span).alignment(Alignment::Center);
            buf.set_line(
                icon_inner.left(),
                icon_inner.top(),
                &icon_line,
                icon_inner.width,
            );
        }

        // Title
        let title_y = start_y + 4;
        if title_y < area.bottom() {
            let title = Line::from(Span::styled(
                "No launch configurations found",
                styles::empty_state_title_style(),
            ))
            .alignment(Alignment::Center);
            buf.set_line(area.left(), title_y, &title, area.width);
        }

        // Subtitle
        let subtitle_y = start_y + 6;
        if subtitle_y < area.bottom() {
            let subtitle = Line::from(vec![
                Span::styled(
                    "Create .fdemon/launch.toml or press '",
                    styles::empty_state_subtitle_style(),
                ),
                Span::styled("n", Style::default().fg(palette::ACCENT)),
                Span::styled("' to create one.", styles::empty_state_subtitle_style()),
            ])
            .alignment(Alignment::Center);
            buf.set_line(area.left(), subtitle_y, &subtitle, area.width);
        }
    }

    fn render_config_header(&self, x: u16, y: u16, width: u16, buf: &mut Buffer, section: &str) {
        // Configuration header with visual separator
        let header_line = format!("─── {} ", section);
        let padding_len = (width as usize).saturating_sub(header_line.len() + 2);
        let padding = "─".repeat(padding_len);
        let full_header = format!("{}{}", header_line, padding);

        buf.set_string(x + 1, y, &full_header, config_header_style());
    }

    fn render_add_config_option(
        &self,
        x: u16,
        y: u16,
        _width: u16,
        buf: &mut Buffer,
        is_selected: bool,
    ) {
        let indicator = if is_selected { "▶ " } else { "  " };
        let style = add_new_style(is_selected);

        buf.set_string(x, y, indicator, style);
        buf.set_string(x + 2, y, "+ Add New Configuration", style);
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // VSCode Config Tab
    // ─────────────────────────────────────────────────────────────────────────────

    fn render_vscode_tab(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut SettingsViewState,
        _icons: &IconSet,
        configs: &[fdemon_app::config::ResolvedLaunchConfig],
    ) {
        // Info banner about read-only nature
        let info_area = Rect::new(area.x, area.y, area.width, SETTINGS_VSCODE_BANNER_HEIGHT);
        self.render_vscode_info(info_area, buf);

        // Content area
        let content_area = Rect::new(
            area.x,
            area.y + SETTINGS_VSCODE_BANNER_HEIGHT,
            area.width,
            area.height.saturating_sub(SETTINGS_VSCODE_BANNER_HEIGHT),
        );

        if configs.is_empty() {
            // Check if the file exists at all
            let launch_json = self.project_path.join(".vscode").join("launch.json");
            if launch_json.exists() {
                self.render_vscode_empty(content_area, buf);
            } else {
                self.render_vscode_not_found(content_area, buf);
            }
            return;
        }

        // Generate all items from configs
        let mut all_items: Vec<SettingItem> = Vec::new();
        for (idx, resolved) in configs.iter().enumerate() {
            all_items.extend(vscode_config_items(&resolved.config, idx));
        }

        let list_top = content_area.y;
        let list_bottom = content_area.bottom();
        let viewport_rows = list_bottom.saturating_sub(list_top) as usize;

        let sel_vrow = visual_row_of_item(&all_items, state.selected_index);
        // EXCEPTION (TEA): render-time scroll clamp (see resolve_scroll).
        let scroll = Self::resolve_scroll(state, sel_vrow, viewport_rows);

        // Render items with sections (read-only styling, offset-aware).
        let mut current_section = String::new();
        let mut vrow = 0usize;

        for (idx, item) in all_items.iter().enumerate() {
            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    vrow += 1; // Spacer (no draw)
                }
                if vrow >= scroll {
                    let y = list_top + (vrow - scroll) as u16;
                    if y >= list_bottom {
                        break;
                    }
                    self.render_vscode_config_header(
                        content_area.x,
                        y,
                        content_area.width,
                        buf,
                        &item.section,
                    );
                }
                vrow += 1;
                current_section = item.section.clone();
            }

            // Setting row (read-only)
            if vrow >= scroll {
                let y = list_top + (vrow - scroll) as u16;
                if y >= list_bottom {
                    break;
                }
                let is_selected = idx == state.selected_index;
                self.render_readonly_row(
                    content_area.x,
                    y,
                    content_area.width,
                    buf,
                    item,
                    is_selected,
                );
            }
            vrow += 1;
        }
    }

    fn render_vscode_info(&self, area: Rect, buf: &mut Buffer) {
        let icons = IconSet::new(self.settings.ui.icons);

        // Glass info banner: rounded border, accent-tinted bg (same as User tab)
        let banner = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::info_banner_border_style()) // ACCENT_DIM
            .style(styles::info_banner_bg()); // SELECTED_ROW_BG bg

        let inner = banner.inner(area);
        banner.render(area, buf);

        if inner.height < 2 {
            return;
        }

        // Line 1: icon + title
        let icon_span = Span::styled(
            format!(" {} ", icons.info()),
            Style::default().fg(palette::ACCENT),
        );
        let title_span = Span::styled(
            "VSCode Launch Configurations (Read-Only)",
            Style::default()
                .fg(palette::TEXT_BRIGHT)
                .add_modifier(Modifier::BOLD),
        );
        let title_line = Line::from(vec![icon_span, title_span]);
        buf.set_line(inner.left(), inner.top(), &title_line, inner.width);

        // Line 2: subtitle (indented to align with title text)
        if inner.height >= 2 {
            let subtitle = Span::styled(
                "    Displaying Dart configurations from .vscode/launch.json",
                Style::default().fg(palette::ACCENT_DIM),
            );
            buf.set_line(
                inner.left(),
                inner.top() + 1,
                &Line::from(subtitle),
                inner.width,
            );
        }
    }

    fn render_vscode_not_found(&self, area: Rect, buf: &mut Buffer) {
        let icons = IconSet::new(self.settings.ui.icons);

        // Center vertically: icon box (3 lines) + gap (1) + title (1) + gap (1) + subtitle (2) = 8 lines
        let total_height = 8u16;
        if area.height < total_height {
            // Not enough space, degrade gracefully - show just title
            if area.height >= 1 {
                let title = Line::from(Span::styled(
                    "No .vscode/launch.json found",
                    styles::empty_state_title_style(),
                ))
                .alignment(Alignment::Center);
                buf.set_line(
                    area.left(),
                    area.top() + area.height / 2,
                    &title,
                    area.width,
                );
            }
            return;
        }

        let start_y = area.top() + 1;

        // Icon container: centered 9-wide box
        let icon_width = 9u16;
        let icon_x = area.left() + area.width.saturating_sub(icon_width) / 2;

        if start_y + 3 <= area.bottom() {
            let icon_rect = Rect::new(icon_x, start_y, icon_width, 3);
            let icon_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::BORDER_DIM));
            let icon_inner = icon_block.inner(icon_rect);
            icon_block.render(icon_rect, buf);

            // Center the icon glyph
            let icon_str = icons.code();
            let icon_span = Span::styled(icon_str, styles::empty_state_icon_style());
            let icon_line = Line::from(icon_span).alignment(Alignment::Center);
            buf.set_line(
                icon_inner.left(),
                icon_inner.top(),
                &icon_line,
                icon_inner.width,
            );
        }

        // Title
        let title_y = start_y + 4;
        if title_y < area.bottom() {
            let title = Line::from(Span::styled(
                "No .vscode/launch.json found",
                styles::empty_state_title_style(),
            ))
            .alignment(Alignment::Center);
            buf.set_line(area.left(), title_y, &title, area.width);
        }

        // Subtitle line 1
        let subtitle1_y = start_y + 6;
        if subtitle1_y < area.bottom() {
            let subtitle1 = Line::from(Span::styled(
                "Create launch configurations in VSCode:",
                styles::empty_state_subtitle_style(),
            ))
            .alignment(Alignment::Center);
            buf.set_line(area.left(), subtitle1_y, &subtitle1, area.width);
        }

        // Subtitle line 2 (command in accent)
        let subtitle2_y = start_y + 7;
        if subtitle2_y < area.bottom() {
            let subtitle2 = Line::from(Span::styled(
                "Run > Add Configuration > Dart & Flutter",
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::ITALIC),
            ))
            .alignment(Alignment::Center);
            buf.set_line(area.left(), subtitle2_y, &subtitle2, area.width);
        }
    }

    fn render_vscode_empty(&self, area: Rect, buf: &mut Buffer) {
        let icons = IconSet::new(self.settings.ui.icons);

        // Center vertically: icon box (3 lines) + gap (1) + title (1) + gap (1) + subtitle (2) = 8 lines
        let total_height = 8u16;
        if area.height < total_height {
            // Not enough space, degrade gracefully - show just title
            if area.height >= 1 {
                let title = Line::from(Span::styled(
                    "launch.json exists but has no Dart configurations",
                    styles::empty_state_title_style(),
                ))
                .alignment(Alignment::Center);
                buf.set_line(
                    area.left(),
                    area.top() + area.height / 2,
                    &title,
                    area.width,
                );
            }
            return;
        }

        let start_y = area.top() + 1;

        // Icon container: centered 9-wide box
        let icon_width = 9u16;
        let icon_x = area.left() + area.width.saturating_sub(icon_width) / 2;

        if start_y + 3 <= area.bottom() {
            let icon_rect = Rect::new(icon_x, start_y, icon_width, 3);
            let icon_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::BORDER_DIM));
            let icon_inner = icon_block.inner(icon_rect);
            icon_block.render(icon_rect, buf);

            // Center the icon glyph
            let icon_str = icons.code();
            let icon_span = Span::styled(icon_str, styles::empty_state_icon_style());
            let icon_line = Line::from(icon_span).alignment(Alignment::Center);
            buf.set_line(
                icon_inner.left(),
                icon_inner.top(),
                &icon_line,
                icon_inner.width,
            );
        }

        // Title
        let title_y = start_y + 4;
        if title_y < area.bottom() {
            let title = Line::from(Span::styled(
                "launch.json exists but has no Dart configurations",
                styles::empty_state_title_style(),
            ))
            .alignment(Alignment::Center);
            buf.set_line(area.left(), title_y, &title, area.width);
        }

        // Subtitle line 1
        let subtitle1_y = start_y + 6;
        if subtitle1_y < area.bottom() {
            let subtitle1 = Line::from(Span::styled(
                "Add a Dart configuration in VSCode:",
                styles::empty_state_subtitle_style(),
            ))
            .alignment(Alignment::Center);
            buf.set_line(area.left(), subtitle1_y, &subtitle1, area.width);
        }

        // Subtitle line 2 (command in accent)
        let subtitle2_y = start_y + 7;
        if subtitle2_y < area.bottom() {
            let subtitle2 = Line::from(Span::styled(
                "Run > Add Configuration > Dart: Flutter",
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::ITALIC),
            ))
            .alignment(Alignment::Center);
            buf.set_line(area.left(), subtitle2_y, &subtitle2, area.width);
        }
    }

    fn render_vscode_config_header(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        section: &str,
    ) {
        // Configuration header with visual separator (blue for VSCode)
        let header_line = format!("─── {} ", section);
        let padding_len = (width as usize).saturating_sub(header_line.len() + 2);
        let padding = "─".repeat(padding_len);
        let full_header = format!("{}{}", header_line, padding);

        buf.set_string(x + 1, y, &full_header, vscode_header_style());
    }

    fn render_readonly_row(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        item: &SettingItem,
        is_selected: bool,
    ) {
        // Apply background for selected row
        if is_selected {
            let bg_style = styles::selected_row_bg();
            for col in x..x + width {
                if let Some(cell) = buf.cell_mut((col, y)) {
                    cell.set_style(bg_style);
                }
            }
        }

        let mut col = x;

        // Column 0: Left accent bar (dimmed for read-only)
        if is_selected {
            let bar = Span::styled("▎", Style::default().fg(palette::TEXT_MUTED));
            buf.set_line(col, y, &Line::from(bar), 1);
        }
        col += INDICATOR_WIDTH; // 3 chars total: bar + 2 spaces

        // Column 1: Label (LABEL_WIDTH_VSCODE chars)
        let label = truncate_str(&item.label, LABEL_WIDTH_VSCODE as usize);
        buf.set_string(
            col,
            y,
            format!("{:<width$}", label, width = LABEL_WIDTH_VSCODE as usize),
            readonly_label_style(is_selected),
        );
        col += LABEL_WIDTH_VSCODE;

        // Column 2: Value (VALUE_WIDTH_VSCODE chars)
        let value_str = item.value.display();
        let value_display = truncate_str(&value_str, VALUE_WIDTH_VSCODE as usize);
        buf.set_string(
            col,
            y,
            format!(
                "{:<width$}",
                value_display,
                width = VALUE_WIDTH_VSCODE as usize
            ),
            readonly_value_style(),
        );
        col += VALUE_WIDTH_VSCODE;

        // Lock icon to indicate read-only
        if is_selected {
            let lock_x = col + 1;
            if lock_x < x + width - 2 {
                buf.set_string(lock_x, y, "🔒", Style::default());
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Editor Helper Methods (Task 10)
    // ─────────────────────────────────────────────────────────────────────────────

    /// Get the currently selected item for editing
    ///
    /// Delegated to the app layer's `get_selected_item` function (moved in Phase 1, Task 05).
    pub fn get_selected_item(&self, state: &SettingsViewState) -> Option<SettingItem> {
        fdemon_app::settings_items::get_selected_item(
            self.settings,
            self.project_path,
            state,
            self.extra_tabs,
        )
    }
}

/// Render `SettingsPanel` and record clickable row regions.
///
/// This is a free-function sister to [`StatefulWidget::render`] that
/// additionally accepts an optional [`crate::widgets::MouseCtx`] for region
/// recording.
///
/// When `ctx` is `Some`, this function registers:
/// - Four tab-header click regions (`z_index = 0`) emitting
///   [`fdemon_app::message::Message::SettingsGotoTab(i)`].
/// - One click region per visible setting row (`z_index = 0`) emitting
///   [`fdemon_app::message::Message::SettingsClickRow { index }`] where
///   `index` is the flat item index (section-header rows are skipped).
///
/// Passing `None` produces output identical to calling
/// `frame.render_stateful_widget(panel, area, state)`.
///
/// # Sub-modal note
///
/// When `state.dart_defines_modal` or `state.extra_args_modal` is open, the
/// underlying tab + row regions are still registered. The dispatcher's editing
/// gate (Task 05) handles click suppression for the modal case. Sub-modal
/// regions (dart-defines, extra-args) are deferred to Phase 6.
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: SettingsPanel<'_>,
    state: &mut SettingsViewState,
    ctx: Option<&mut crate::widgets::MouseCtx<'_>>,
) {
    // 1. Delegate rendering to StatefulWidget::render (visual output unchanged).
    // We keep a copy of relevant layout references for region recording below.
    let settings = view.settings;
    let project_path = view.project_path;
    // Capture extra-tab data BEFORE the `view` move so the region recorder can
    // mirror the dynamic tab bar and the generic-tab rows. `items()` is called
    // once here (and once inside render_content); both produce the same list.
    let extra_count = view.extra_tabs.len();
    let extra_items: Vec<Vec<SettingItem>> = view.extra_tabs.iter().map(|p| p.items()).collect();

    <SettingsPanel as StatefulWidget>::render(view, area, buf, state);

    // StatefulWidget::render above wrote the corrected scroll offset into
    // `state`. Capture it now so the region recorder uses the SAME value,
    // guaranteeing click targets match the rendered rows.
    let scroll = state.scroll_offset;

    // 2. If no context, we are done.
    let ctx = match ctx {
        Some(c) => c,
        None => return,
    };

    // ── Mirror the layout from StatefulWidget::render ────────────────────────

    // Main vertical layout (same constraints as in render()):
    //   chunks[0] = header (height 5)
    //   chunks[1] = content (min 5)
    //   chunks[2] = footer (height 3)
    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(5),
        ratatui::layout::Constraint::Min(5),
        ratatui::layout::Constraint::Length(3),
    ])
    .split(area);

    let header_area = chunks[0];
    let content_area = chunks[1];

    // ── Tab bar regions ───────────────────────────────────────────────────────

    // Mirror render_header / render_tab_bar layout:
    //   header_block has Borders::ALL (rounded) → inner shrinks by 1 on each side
    //   title_y = inner.top()
    //   tab_y   = inner.top() + 2
    //   tab_area = Rect::new(inner.left() + 1, tab_y, inner.width - 2, 1)
    let header_inner_x = header_area.x.saturating_add(1);
    let header_inner_y = header_area.y.saturating_add(1);
    let header_inner_width = header_area.width.saturating_sub(2);

    let tab_y = header_inner_y.saturating_add(2);
    let tab_area_x = header_inner_x.saturating_add(1);
    let tab_area_width = header_inner_width.saturating_sub(2);

    let tab_width: u16 = SETTINGS_TAB_WIDTH;
    let gap: u16 = SETTINGS_TAB_GAP;

    let mut x = tab_area_x;
    for i in 0..(BUILTIN_SETTINGS_TAB_COUNT + extra_count) {
        if x + tab_width > tab_area_x + tab_area_width {
            break;
        }
        let rect = fdemon_app::MouseRect::new(x, tab_y, tab_width, 1);
        if !rect.is_empty() {
            ctx.click(
                rect,
                fdemon_app::MouseAction::emit(fdemon_app::message::Message::SettingsGotoTab(i)),
            );
        }
        x = x.saturating_add(tab_width + gap);
    }

    // ── Setting row regions ───────────────────────────────────────────────────

    // Mirror content layout: content_block has Borders::LEFT | Borders::RIGHT
    // → inner shrinks by 1 on left and right only (no top/bottom offset).
    let inner_x = content_area.x.saturating_add(1);
    let inner_y = content_area.y;
    let inner_width = content_area.width.saturating_sub(2);
    let inner_bottom = content_area.bottom();

    // Build the item list and starting Y offset for each tab, mirroring the
    // per-tab render functions exactly (section-header skip logic must match).
    //
    // For LaunchConfig and VSCode tabs the configs are loaded ONCE here and
    // threaded to the region recorder — no second disk read in this path.
    // (StatefulWidget::render above already did one load inside render_content;
    // the region recorder must not perform an additional load.)
    match state.active_tab {
        SettingsTab::Project => {
            let items = project_settings_items(settings);
            register_setting_row_regions(
                ctx,
                &items,
                inner_x,
                inner_y,
                inner_width,
                inner_bottom,
                scroll,
            );
        }
        SettingsTab::UserPrefs => {
            let items = user_prefs_items(&state.user_prefs, settings);
            // User prefs tab renders a SETTINGS_USER_PREFS_BANNER_HEIGHT-row info banner above.
            let content_y = inner_y.saturating_add(SETTINGS_USER_PREFS_BANNER_HEIGHT);
            register_setting_row_regions(
                ctx,
                &items,
                inner_x,
                content_y,
                inner_width,
                inner_bottom,
                scroll,
            );
        }
        SettingsTab::LaunchConfig => {
            use fdemon_app::config::launch::load_launch_configs;

            // Single load for the region-recorder path (renderer loaded once above).
            let configs = load_launch_configs(project_path);
            if !configs.is_empty() {
                let mut all_items: Vec<fdemon_app::config::SettingItem> = Vec::new();
                for (idx, resolved) in configs.iter().enumerate() {
                    all_items.extend(launch_config_items(&resolved.config, idx));
                }
                let after_items_y = register_setting_row_regions(
                    ctx,
                    &all_items,
                    inner_x,
                    inner_y,
                    inner_width,
                    inner_bottom,
                    scroll,
                );

                // ── Sentinel: "Add New Configuration" row ──────────────────
                // Mirrors render_launch_tab: rendered at the trailing-sentinel
                // visual row (one spacer after the last item), only when no item
                // overflowed the bottom (after_items_y stays in-bounds).
                let sentinel_index = all_items.len();
                let sentinel_vrow = visual_row_of_item(&all_items, sentinel_index);
                if after_items_y < inner_bottom && sentinel_vrow >= scroll {
                    let sentinel_y = inner_y.saturating_add((sentinel_vrow - scroll) as u16);
                    if sentinel_y < inner_bottom {
                        let sentinel_rect =
                            fdemon_app::MouseRect::new(inner_x, sentinel_y, inner_width, 1);
                        if !sentinel_rect.is_empty() {
                            ctx.click(
                                sentinel_rect,
                                fdemon_app::MouseAction::emit(
                                    fdemon_app::message::Message::SettingsClickRow {
                                        index: sentinel_index,
                                    },
                                ),
                            );
                        }
                    }
                }
            }
        }
        SettingsTab::VSCodeConfig => {
            use fdemon_app::config::load_vscode_configs;

            // Single load for the region-recorder path (renderer loaded once above).
            let configs = load_vscode_configs(project_path);
            if !configs.is_empty() {
                // VSCode tab renders a SETTINGS_VSCODE_BANNER_HEIGHT-row info banner above.
                let content_y = inner_y.saturating_add(SETTINGS_VSCODE_BANNER_HEIGHT);

                let mut all_items: Vec<fdemon_app::config::SettingItem> = Vec::new();
                for (idx, resolved) in configs.iter().enumerate() {
                    all_items.extend(vscode_config_items(&resolved.config, idx));
                }
                register_setting_row_regions(
                    ctx,
                    &all_items,
                    inner_x,
                    content_y,
                    inner_width,
                    inner_bottom,
                    scroll,
                );
            }
        }
        SettingsTab::Extra(i) => {
            // Host-injected tab: register rows over the provider's items,
            // reusing the SAME offset-aware walk as render_generic_tab.
            if let Some(items) = extra_items.get(i) {
                register_setting_row_regions(
                    ctx,
                    items,
                    inner_x,
                    inner_y,
                    inner_width,
                    inner_bottom,
                    scroll,
                );
            }
        }
    }
}

/// Register one left-click region per visible setting row, mirroring the
/// section-header skip logic used by the per-tab renderers.
///
/// `items` is the flat `Vec<SettingItem>` for the active tab.  Section
/// headers consume one `y` row each (plus a one-row spacer between sections)
/// but are **not** registered as click regions.  The `index` stored in each
/// [`Message::SettingsClickRow`] is the flat item index into `items`.
///
/// Returns the `y` value immediately after the last registered row.  Callers
/// that need to append extra rows (e.g. the "Add New Configuration" sentinel
/// on the LaunchConfig tab) use this value to compute the correct y position.
fn register_setting_row_regions(
    ctx: &mut crate::widgets::MouseCtx<'_>,
    items: &[fdemon_app::config::SettingItem],
    x: u16,
    start_y: u16,
    width: u16,
    bottom: u16,
    scroll_offset: usize,
) -> u16 {
    let mut current_section = String::new();
    let mut vrow = 0usize;

    for (idx, item) in items.iter().enumerate() {
        // Section header — mirrors the renderer's offset-aware section walk.
        if item.section != current_section {
            if !current_section.is_empty() {
                vrow += 1; // spacer row between sections (not drawn)
            }
            if vrow >= scroll_offset {
                let y = start_y + (vrow - scroll_offset) as u16;
                if y >= bottom {
                    // Header row would be off-screen → the rest is too.
                    return y;
                }
                // Section header row — NOT clickable, just advance.
            }
            vrow += 1;
            current_section = item.section.clone();
        }

        // Setting row — register as clickable when on-screen.
        if vrow >= scroll_offset {
            let y = start_y + (vrow - scroll_offset) as u16;
            if y >= bottom {
                return y;
            }
            let rect = fdemon_app::MouseRect::new(x, y, width, 1);
            if !rect.is_empty() {
                ctx.click(
                    rect,
                    fdemon_app::MouseAction::emit(fdemon_app::message::Message::SettingsClickRow {
                        index: idx,
                    }),
                );
            }
        }
        vrow += 1;
    }

    // Return the visual-row count consumed (in absolute vrow space, mapped to a
    // y coordinate). Callers needing the trailing sentinel y recompute it from
    // `vrow` so the scroll math stays consistent.
    start_y.saturating_add((vrow.saturating_sub(scroll_offset)) as u16)
}

#[cfg(test)]
mod tests;

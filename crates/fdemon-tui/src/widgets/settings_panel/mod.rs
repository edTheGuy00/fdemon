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

use std::path::Path;

use fdemon_app::config::{SettingItem, Settings, SettingsTab, UserPreferences};
use fdemon_app::settings_items::{
    launch_config_items, project_settings_items, user_prefs_items, vscode_config_items,
};
use fdemon_app::state::SettingsViewState;

// Use styles module
use styles::{
    add_new_style, config_header_style, description_style, editing_style, label_style,
    override_indicator_style, readonly_label_style, readonly_value_style, truncate_str,
    value_style, vscode_header_style, INDICATOR_WIDTH, LABEL_WIDTH, LABEL_WIDTH_SHORT,
    LABEL_WIDTH_VSCODE, VALUE_WIDTH, VALUE_WIDTH_VSCODE,
};

/// Full-screen settings panel widget
pub struct SettingsPanel<'a> {
    /// Reference to application settings
    settings: &'a Settings,

    /// Project path for loading configurations
    project_path: &'a Path,

    /// Title to display in header
    title: &'a str,
}

impl<'a> SettingsPanel<'a> {
    pub fn new(settings: &'a Settings, project_path: &'a Path) -> Self {
        Self {
            settings,
            project_path,
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
        let tabs = [
            (SettingsTab::Project, "1. PROJECT"),
            (SettingsTab::UserPrefs, "2. USER"),
            (SettingsTab::LaunchConfig, "3. LAUNCH"),
            (SettingsTab::VSCodeConfig, "4. VSCODE"),
        ];

        let tab_width = 12u16; // Fixed width per tab
        let gap = 1u16; // Gap between tabs

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
        let content_block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_set(symbols::border::ROUNDED);

        let inner = content_block.inner(area);
        content_block.render(area, buf);

        // Create IconSet once for all renderers
        let icons = IconSet::new(self.settings.ui.icons);

        // Dispatch to tab-specific renderer
        match state.active_tab {
            SettingsTab::Project => self.render_project_tab(inner, buf, state, &icons),
            SettingsTab::UserPrefs => self.render_user_prefs_tab(inner, buf, state, &icons),
            SettingsTab::LaunchConfig => self.render_launch_tab(inner, buf, state, &icons),
            SettingsTab::VSCodeConfig => self.render_vscode_tab(inner, buf, state, &icons),
        }
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

        // Group items by section
        let mut current_section = String::new();
        let mut y = area.y;

        for (idx, item) in items.iter().enumerate() {
            if y >= area.bottom() {
                break; // Out of space
            }

            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    y += 1; // Spacer between sections
                }

                if y < area.bottom() {
                    self.render_section_header(area.x, y, area.width, buf, &item.section, icons);
                    y += 1;
                }
                current_section = item.section.clone();
            }

            // Setting row
            if y < area.bottom() {
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
                y += 1;
            }
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

        // Spaced uppercase: "Behavior" → "B E H A V I O R"
        let upper = section.to_uppercase();
        let chars: Vec<char> = upper.chars().collect();
        let mut spaced = String::new();
        for (i, ch) in chars.iter().enumerate() {
            if i > 0 {
                spaced.push(' ');
            }
            spaced.push(*ch);
        }

        let icon_span = Span::styled(format!("  {} ", icon), styles::group_header_icon_style());
        let label_span = Span::styled(
            spaced,
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
        let info_area = Rect::new(area.x, area.y, area.width, 4);
        self.render_user_prefs_info(info_area, buf);

        // Content area below info banner
        let content_area = Rect::new(
            area.x,
            area.y + 4,
            area.width,
            area.height.saturating_sub(4),
        );

        let items = user_prefs_items(&state.user_prefs, self.settings);

        // Group items by section
        let mut current_section = String::new();
        let mut y = content_area.y;

        for (idx, item) in items.iter().enumerate() {
            if y >= content_area.bottom() {
                break;
            }

            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    y += 1; // Spacer
                }

                if y < content_area.bottom() {
                    self.render_section_header(
                        content_area.x,
                        y,
                        content_area.width,
                        buf,
                        &item.section,
                        icons,
                    );
                    y += 1;
                }
                current_section = item.section.clone();
            }

            // Setting row
            if y < content_area.bottom() {
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
                y += 1;
            }
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
    ) {
        use fdemon_app::config::launch::load_launch_configs;

        // Load configurations from disk
        let configs = load_launch_configs(self.project_path);

        if configs.is_empty() {
            self.render_launch_empty_state(area, buf);
            return;
        }

        // Generate all items from configs
        let mut all_items: Vec<SettingItem> = Vec::new();
        for (idx, resolved) in configs.iter().enumerate() {
            all_items.extend(launch_config_items(&resolved.config, idx));
        }

        // Render items with sections
        let mut current_section = String::new();
        let mut y = area.y;

        for (idx, item) in all_items.iter().enumerate() {
            if y >= area.bottom() {
                break; // Out of space
            }

            // Section header (configuration separator)
            if item.section != current_section {
                if !current_section.is_empty() {
                    y += 1; // Spacer between configurations
                }

                if y < area.bottom() {
                    self.render_config_header(area.x, y, area.width, buf, &item.section);
                    y += 1;
                }
                current_section = item.section.clone();
            }

            // Setting row
            if y < area.bottom() {
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
                y += 1;
            }
        }

        // Add "New Configuration" option at bottom
        if y + 2 < area.bottom() {
            y += 1; // Spacer
            let is_selected = state.selected_index == all_items.len();
            self.render_add_config_option(area.x, y, area.width, buf, is_selected);
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
    ) {
        use fdemon_app::config::load_vscode_configs;

        // Info banner about read-only nature
        let info_area = Rect::new(area.x, area.y, area.width, 4);
        self.render_vscode_info(info_area, buf);

        // Content area
        let content_area = Rect::new(
            area.x,
            area.y + 4,
            area.width,
            area.height.saturating_sub(4),
        );

        // Load configs (Dart-only, filtered by vscode.rs)
        let configs = load_vscode_configs(self.project_path);

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

        // Render items with sections (read-only styling)
        let mut current_section = String::new();
        let mut y = content_area.y;

        for (idx, item) in all_items.iter().enumerate() {
            if y >= content_area.bottom() {
                break;
            }

            // Section header
            if item.section != current_section {
                if !current_section.is_empty() {
                    y += 1; // Spacer
                }

                if y < content_area.bottom() {
                    self.render_vscode_config_header(
                        content_area.x,
                        y,
                        content_area.width,
                        buf,
                        &item.section,
                    );
                    y += 1;
                }
                current_section = item.section.clone();
            }

            // Setting row (read-only)
            if y < content_area.bottom() {
                let is_selected = idx == state.selected_index;
                self.render_readonly_row(
                    content_area.x,
                    y,
                    content_area.width,
                    buf,
                    item,
                    is_selected,
                );
                y += 1;
            }
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
        fdemon_app::settings_items::get_selected_item(self.settings, self.project_path, state)
    }
}

#[cfg(test)]
mod tests;

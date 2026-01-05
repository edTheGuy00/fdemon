//! Settings panel widget - full-screen settings UI
//!
//! Displays a tabbed interface for managing:
//! - Project settings (config.toml)
//! - User preferences (settings.local.toml)
//! - Launch configurations (launch.toml)
//! - VSCode configurations (launch.json, read-only)

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};

use std::path::Path;

use crate::app::state::SettingsViewState;
use crate::config::{SettingItem, SettingValue, Settings, SettingsTab, UserPreferences};

/// Full-screen settings panel widget
pub struct SettingsPanel<'a> {
    /// Reference to application settings
    #[allow(dead_code)] // Used in future tasks for rendering tab content
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
        let bg_style = Style::default().bg(Color::Black);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buf[(x, y)].set_style(bg_style).set_char(' ');
            }
        }

        // Create main layout
        let chunks = Layout::vertical([
            Constraint::Length(3), // Header with tabs
            Constraint::Min(5),    // Content area
            Constraint::Length(2), // Footer with shortcuts
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
    fn render_header(&self, area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
        // Header block with title
        let block = Block::default()
            .title(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(self.title, Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(" ", Style::default()),
            ]))
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED);

        let inner = block.inner(area);
        block.render(area, buf);

        // Calculate tab positions
        let tab_width = inner.width / 4;
        let tabs_area = Rect::new(inner.x, inner.y, inner.width, 1);

        // Render each tab
        for (i, tab) in [
            SettingsTab::Project,
            SettingsTab::UserPrefs,
            SettingsTab::LaunchConfig,
            SettingsTab::VSCodeConfig,
        ]
        .iter()
        .enumerate()
        {
            let is_active = *tab == state.active_tab;
            let tab_area = Rect::new(
                tabs_area.x + (i as u16 * tab_width),
                tabs_area.y,
                tab_width.min(tabs_area.width.saturating_sub(i as u16 * tab_width)),
                1,
            );

            self.render_tab(tab_area, buf, tab, is_active);
        }

        // Render underline indicator for active tab
        self.render_tab_underline(
            Rect::new(inner.x, inner.y.saturating_add(1), inner.width, 1),
            buf,
            state,
            tab_width,
        );

        // Close hint
        let close_hint = "[Esc] Close ";
        let hint_x = area.right().saturating_sub(close_hint.len() as u16 + 2);
        if hint_x > area.x + 20 {
            buf.set_string(
                hint_x,
                area.y,
                close_hint,
                Style::default().fg(Color::DarkGray),
            );
        }
    }

    fn render_tab(&self, area: Rect, buf: &mut Buffer, tab: &SettingsTab, is_active: bool) {
        let num = format!("{}.", tab.index() + 1);
        let label = tab.label();

        // Calculate centering
        let total_len = num.len() + label.len();
        let padding = area.width.saturating_sub(total_len as u16) / 2;

        let (num_style, label_style, bg_style) = if is_active {
            (
                Style::default().fg(Color::DarkGray).bg(Color::Cyan),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                Style::default().bg(Color::Cyan),
            )
        } else {
            (
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::White),
                Style::default(),
            )
        };

        // Fill background for active tab
        if is_active {
            for x in area.x..area.right() {
                buf[(x, area.y)].set_style(bg_style);
            }
        }

        // Render number and label
        let x = area.x + padding;
        buf.set_string(x, area.y, &num, num_style);
        buf.set_string(x + num.len() as u16, area.y, label, label_style);
    }

    fn render_tab_underline(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &SettingsViewState,
        tab_width: u16,
    ) {
        let active_idx = state.active_tab.index();

        // Draw underline for active tab
        let underline_x = area.x + (active_idx as u16 * tab_width);
        let underline_width =
            tab_width.min(area.width.saturating_sub(active_idx as u16 * tab_width));

        for x in underline_x..underline_x + underline_width {
            if x < area.right() {
                buf[(x, area.y)]
                    .set_char('─')
                    .set_style(Style::default().fg(Color::Cyan));
            }
        }
    }

    fn render_content(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        let content_block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_set(symbols::border::ROUNDED);

        let inner = content_block.inner(area);
        content_block.render(area, buf);

        // Dispatch to tab-specific renderer
        match state.active_tab {
            SettingsTab::Project => self.render_project_tab(inner, buf, state),
            SettingsTab::UserPrefs => self.render_user_prefs_tab(inner, buf, state),
            SettingsTab::LaunchConfig => self.render_launch_tab(inner, buf, state),
            SettingsTab::VSCodeConfig => self.render_vscode_tab(inner, buf, state),
        }
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
        let footer_block = Block::default()
            .borders(Borders::ALL ^ Borders::TOP)
            .border_set(symbols::border::ROUNDED);

        let inner = footer_block.inner(area);
        footer_block.render(area, buf);

        // Build shortcut text
        let shortcuts = if state.editing {
            "Enter: Confirm  Esc: Cancel"
        } else if state.dirty {
            "Tab: Switch tabs  j/k: Navigate  Enter: Edit  Ctrl+S: Save (unsaved changes)"
        } else {
            "Tab: Switch tabs  j/k: Navigate  Enter: Edit  Ctrl+S: Save"
        };

        let footer = Paragraph::new(shortcuts)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));

        footer.render(inner, buf);
    }

    // Project tab renderer
    fn render_project_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
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
                    self.render_section_header(area.x, y, area.width, buf, &item.section);
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

    fn render_section_header(&self, x: u16, y: u16, _width: u16, buf: &mut Buffer, section: &str) {
        let header = format!("[{}]", section);
        buf.set_string(
            x + 1,
            y,
            &header,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
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
        // Layout: [indicator] [label............] [value.....] [description]
        let indicator_width: u16 = 3;
        let label_width: u16 = 25;
        let value_width: u16 = 15;
        let desc_start: u16 = indicator_width + label_width + value_width + 2;

        // Selection indicator
        let indicator = if is_selected { "▶ " } else { "  " };
        let indicator_style = if is_selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        buf.set_string(x, y, indicator, indicator_style);

        // Label
        let label_style = if is_selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let label = truncate_str(&item.label, label_width as usize - 1);
        buf.set_string(x + indicator_width, y, &label, label_style);

        // Value
        let value_x = x + indicator_width + label_width;
        let (value_str, value_style) = if is_editing {
            (
                format!("{}▌", edit_buffer), // Show cursor
                Style::default().fg(Color::Yellow).bg(Color::DarkGray),
            )
        } else {
            let modified_indicator = if item.is_modified() { "*" } else { "" };
            (
                format!("{}{}", item.value.display(), modified_indicator),
                self.value_style(&item.value, is_selected),
            )
        };
        let value_display = truncate_str(&value_str, value_width as usize);
        buf.set_string(value_x, y, &value_display, value_style);

        // Description (dimmed)
        if width > desc_start + 10 {
            let desc_width = (width - desc_start) as usize;
            let desc = truncate_str(&item.description, desc_width);
            buf.set_string(
                x + desc_start,
                y,
                &desc,
                Style::default().fg(Color::DarkGray),
            );
        }
    }

    fn value_style(&self, value: &SettingValue, is_selected: bool) -> Style {
        let base = if is_selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        match value {
            SettingValue::Bool(true) => base.fg(Color::Green),
            SettingValue::Bool(false) => base.fg(Color::Red),
            SettingValue::Number(_) | SettingValue::Float(_) => base.fg(Color::Cyan),
            SettingValue::String(s) if s.is_empty() => base.fg(Color::DarkGray),
            SettingValue::String(_) => base.fg(Color::White),
            SettingValue::Enum { .. } => base.fg(Color::Magenta),
            SettingValue::List(_) => base.fg(Color::Blue),
        }
    }

    fn render_user_prefs_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        // Render info banner about local settings
        let info_area = Rect::new(area.x, area.y, area.width, 3);
        self.render_user_prefs_info(info_area, buf);

        // Content area below info banner
        let content_area = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(3),
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
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Local Settings ");

        let inner = block.inner(area);
        block.render(area, buf);

        let info = Paragraph::new(vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "These settings are stored in .fdemon/settings.local.toml",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "They are gitignored and override project settings for you only.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ]);

        info.render(inner, buf);
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
        // Layout: [indicator] [label............] [value.....] [description]
        let indicator_width: u16 = 4; // Extra space for override marker
        let label_width: u16 = 24;
        let value_width: u16 = 15;
        let desc_start: u16 = indicator_width + label_width + value_width + 2;

        // Override indicator
        let is_override = self.is_override_active(prefs, &item.id);
        let indicator = if is_selected {
            if is_override {
                "▶⚡"
            } else {
                "▶ "
            }
        } else if is_override {
            " ⚡"
        } else {
            "  "
        };

        let indicator_style = if is_override {
            Style::default().fg(Color::Yellow)
        } else if is_selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        buf.set_string(x, y, indicator, indicator_style);

        // Label
        let label_style = if is_selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let label = truncate_str(&item.label, label_width as usize - 1);
        buf.set_string(x + indicator_width, y, &label, label_style);

        // Value
        let value_x = x + indicator_width + label_width;
        let (value_str, value_style) = if is_editing {
            (
                format!("{}▌", edit_buffer), // Show cursor
                Style::default().fg(Color::Yellow).bg(Color::DarkGray),
            )
        } else {
            let modified_indicator = if item.is_modified() { "*" } else { "" };
            let display_val = item.value.display();
            let display_str = if display_val.is_empty() {
                "<empty>".to_string()
            } else {
                display_val
            };
            (
                format!("{}{}", display_str, modified_indicator),
                self.value_style(&item.value, is_selected),
            )
        };
        let value_display = truncate_str(&value_str, value_width as usize);
        buf.set_string(value_x, y, &value_display, value_style);

        // Description (dimmed)
        if width > desc_start + 10 {
            let desc_width = (width - desc_start) as usize;
            let desc = truncate_str(&item.description, desc_width);
            buf.set_string(
                x + desc_start,
                y,
                &desc,
                Style::default().fg(Color::DarkGray),
            );
        }
    }

    fn render_launch_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        use crate::config::launch::load_launch_configs;

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
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "No launch configurations found",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Create .fdemon/launch.toml or press ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("n", Style::default().fg(Color::Cyan)),
                Span::styled(" to create one.", Style::default().fg(Color::DarkGray)),
            ]),
        ])
        .alignment(Alignment::Center);

        empty.render(area, buf);
    }

    fn render_config_header(&self, x: u16, y: u16, width: u16, buf: &mut Buffer, section: &str) {
        // Configuration header with visual separator
        let header_line = format!("─── {} ", section);
        let padding_len = (width as usize).saturating_sub(header_line.len() + 2);
        let padding = "─".repeat(padding_len);
        let full_header = format!("{}{}", header_line, padding);

        buf.set_string(x + 1, y, &full_header, Style::default().fg(Color::Cyan));
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
        let style = if is_selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };

        buf.set_string(x, y, indicator, style);
        buf.set_string(x + 2, y, "+ Add New Configuration", style);
    }

    fn render_vscode_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        use crate::config::load_vscode_configs;

        // Info banner about read-only nature
        let info_area = Rect::new(area.x, area.y, area.width, 3);
        self.render_vscode_info(info_area, buf);

        // Content area
        let content_area = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(3),
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
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" VSCode Launch Configurations ");

        let inner = block.inner(area);
        block.render(area, buf);

        let info = Paragraph::new(vec![
            Line::from(vec![
                Span::raw("  🔒 "),
                Span::styled(
                    "Read-only view of .vscode/launch.json (Dart configurations only)",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::raw("     "),
                Span::styled(
                    "Edit this file directly in VSCode for changes.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ]);

        info.render(inner, buf);
    }

    fn render_vscode_not_found(&self, area: Rect, buf: &mut Buffer) {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "No .vscode/launch.json found",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Create launch configurations in VSCode:",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                "Run > Add Configuration > Dart & Flutter",
                Style::default().fg(Color::Cyan),
            )]),
        ])
        .alignment(Alignment::Center);

        msg.render(area, buf);
    }

    fn render_vscode_empty(&self, area: Rect, buf: &mut Buffer) {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "launch.json exists but has no Dart configurations",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Add a Dart configuration in VSCode:",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                "Run > Add Configuration > Dart: Flutter",
                Style::default().fg(Color::Cyan),
            )]),
        ])
        .alignment(Alignment::Center);

        msg.render(area, buf);
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

        buf.set_string(x + 1, y, &full_header, Style::default().fg(Color::Blue));
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
        let indicator_width: u16 = 3;
        let label_width: u16 = 20;
        let value_width: u16 = 20;

        // Selection indicator (dimmed for readonly)
        let indicator = if is_selected { "› " } else { "  " };
        let indicator_style = Style::default().fg(Color::DarkGray);
        buf.set_string(x, y, indicator, indicator_style);

        // Label (dimmed)
        let label_style = if is_selected {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };
        let label = truncate_str(&item.label, label_width as usize - 1);
        buf.set_string(x + indicator_width, y, &label, label_style);

        // Value (read-only styling)
        let value_x = x + indicator_width + label_width;
        let value_str = item.value.display();
        let value_style = Style::default().fg(Color::DarkGray);
        let value_display = truncate_str(&value_str, value_width as usize);
        buf.set_string(value_x, y, &value_display, value_style);

        // Lock icon to indicate read-only
        if is_selected {
            let lock_x = value_x + value_display.len() as u16 + 1;
            if lock_x < x + width - 2 {
                buf.set_string(lock_x, y, "🔒", Style::default());
            }
        }
    }
}

/// Generate settings items for a single launch configuration
fn launch_config_items(config: &crate::config::LaunchConfig, idx: usize) -> Vec<SettingItem> {
    let prefix = format!("launch.{}", idx);

    vec![
        SettingItem::new(format!("{}.name", prefix), "Name")
            .description("Configuration display name")
            .value(SettingValue::String(config.name.clone()))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.device", prefix), "Device")
            .description("Target device ID or 'auto'")
            .value(SettingValue::String(config.device.clone()))
            .default(SettingValue::String("auto".to_string()))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.mode", prefix), "Mode")
            .description("Flutter build mode")
            .value(SettingValue::Enum {
                value: config.mode.to_string(),
                options: vec![
                    "debug".to_string(),
                    "profile".to_string(),
                    "release".to_string(),
                ],
            })
            .default(SettingValue::String("debug".to_string()))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.flavor", prefix), "Flavor")
            .description("Build flavor (optional)")
            .value(SettingValue::String(
                config.flavor.clone().unwrap_or_default(),
            ))
            .default(SettingValue::String(String::new()))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.auto_start", prefix), "Auto Start")
            .description("Start this config automatically")
            .value(SettingValue::Bool(config.auto_start))
            .default(SettingValue::Bool(false))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.dart_defines", prefix), "Dart Defines")
            .description("--dart-define values")
            .value(SettingValue::List(
                config
                    .dart_defines
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect(),
            ))
            .default(SettingValue::List(vec![]))
            .section(format!("Configuration {}", idx + 1)),
        SettingItem::new(format!("{}.extra_args", prefix), "Extra Args")
            .description("Additional flutter run arguments")
            .value(SettingValue::List(config.extra_args.clone()))
            .default(SettingValue::List(vec![]))
            .section(format!("Configuration {}", idx + 1)),
    ]
}

/// Generate read-only settings items for VSCode launch configuration
fn vscode_config_items(config: &crate::config::LaunchConfig, idx: usize) -> Vec<SettingItem> {
    let prefix = format!("vscode.{}", idx);

    vec![
        SettingItem::new(format!("{}.name", prefix), "Name")
            .description("Configuration name")
            .value(SettingValue::String(config.name.clone()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.device", prefix), "Device ID")
            .description("Target device")
            .value(SettingValue::String(config.device.clone()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.mode", prefix), "Flutter Mode")
            .description("Build mode")
            .value(SettingValue::String(config.mode.to_string()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.flavor", prefix), "Flavor")
            .description("Build flavor")
            .value(SettingValue::String(
                config.flavor.clone().unwrap_or_else(|| "-".to_string()),
            ))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.entry_point", prefix), "Entry Point")
            .description("Program entry point")
            .value(SettingValue::String(
                config
                    .entry_point
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "lib/main.dart".to_string()),
            ))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
        SettingItem::new(format!("{}.extra_args", prefix), "Arguments")
            .description("Additional arguments")
            .value(SettingValue::List(config.extra_args.clone()))
            .section(format!("Configuration {}", idx + 1))
            .readonly(),
    ]
}

/// Generate settings items for the Project tab from Settings struct
fn project_settings_items(settings: &Settings) -> Vec<SettingItem> {
    vec![
        // ─────────────────────────────────────────────────────────
        // Behavior Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("behavior.auto_start", "Auto Start")
            .description("Skip device selector and start immediately")
            .value(SettingValue::Bool(settings.behavior.auto_start))
            .default(SettingValue::Bool(false))
            .section("Behavior"),
        SettingItem::new("behavior.confirm_quit", "Confirm Quit")
            .description("Ask before quitting with running apps")
            .value(SettingValue::Bool(settings.behavior.confirm_quit))
            .default(SettingValue::Bool(true))
            .section("Behavior"),
        // ─────────────────────────────────────────────────────────
        // Watcher Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("watcher.paths", "Watch Paths")
            .description("Directories to watch for changes")
            .value(SettingValue::List(settings.watcher.paths.clone()))
            .default(SettingValue::List(vec!["lib".to_string()]))
            .section("Watcher"),
        SettingItem::new("watcher.debounce_ms", "Debounce (ms)")
            .description("Delay before triggering reload")
            .value(SettingValue::Number(settings.watcher.debounce_ms as i64))
            .default(SettingValue::Number(500))
            .section("Watcher"),
        SettingItem::new("watcher.auto_reload", "Auto Reload")
            .description("Hot reload on file changes")
            .value(SettingValue::Bool(settings.watcher.auto_reload))
            .default(SettingValue::Bool(true))
            .section("Watcher"),
        SettingItem::new("watcher.extensions", "Extensions")
            .description("File extensions to watch")
            .value(SettingValue::List(settings.watcher.extensions.clone()))
            .default(SettingValue::List(vec!["dart".to_string()]))
            .section("Watcher"),
        // ─────────────────────────────────────────────────────────
        // UI Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("ui.log_buffer_size", "Log Buffer Size")
            .description("Maximum log entries to keep")
            .value(SettingValue::Number(settings.ui.log_buffer_size as i64))
            .default(SettingValue::Number(10000))
            .section("UI"),
        SettingItem::new("ui.show_timestamps", "Show Timestamps")
            .description("Display timestamps in logs")
            .value(SettingValue::Bool(settings.ui.show_timestamps))
            .default(SettingValue::Bool(true))
            .section("UI"),
        SettingItem::new("ui.compact_logs", "Compact Logs")
            .description("Collapse similar consecutive logs")
            .value(SettingValue::Bool(settings.ui.compact_logs))
            .default(SettingValue::Bool(false))
            .section("UI"),
        SettingItem::new("ui.theme", "Theme")
            .description("Color theme")
            .value(SettingValue::Enum {
                value: settings.ui.theme.clone(),
                options: vec![
                    "default".to_string(),
                    "dark".to_string(),
                    "light".to_string(),
                ],
            })
            .default(SettingValue::Enum {
                value: "default".to_string(),
                options: vec![
                    "default".to_string(),
                    "dark".to_string(),
                    "light".to_string(),
                ],
            })
            .section("UI"),
        SettingItem::new("ui.stack_trace_collapsed", "Collapse Stack Traces")
            .description("Start stack traces collapsed")
            .value(SettingValue::Bool(settings.ui.stack_trace_collapsed))
            .default(SettingValue::Bool(true))
            .section("UI"),
        SettingItem::new("ui.stack_trace_max_frames", "Max Frames")
            .description("Frames shown when collapsed")
            .value(SettingValue::Number(
                settings.ui.stack_trace_max_frames as i64,
            ))
            .default(SettingValue::Number(3))
            .section("UI"),
        // ─────────────────────────────────────────────────────────
        // DevTools Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("devtools.auto_open", "Auto Open DevTools")
            .description("Open DevTools when app starts")
            .value(SettingValue::Bool(settings.devtools.auto_open))
            .default(SettingValue::Bool(false))
            .section("DevTools"),
        SettingItem::new("devtools.browser", "Browser")
            .description("Browser for DevTools (empty = default)")
            .value(SettingValue::String(settings.devtools.browser.clone()))
            .default(SettingValue::String(String::new()))
            .section("DevTools"),
        // ─────────────────────────────────────────────────────────
        // Editor Section
        // ─────────────────────────────────────────────────────────
        SettingItem::new("editor.command", "Editor Command")
            .description("Editor to open files (empty = auto-detect)")
            .value(SettingValue::String(settings.editor.command.clone()))
            .default(SettingValue::String(String::new()))
            .section("Editor"),
        SettingItem::new("editor.open_pattern", "Open Pattern")
            .description("Pattern for opening files ($FILE, $LINE, $COLUMN)")
            .value(SettingValue::String(settings.editor.open_pattern.clone()))
            .default(SettingValue::String("$EDITOR $FILE:$LINE".to_string()))
            .section("Editor"),
    ]
}

/// Generate settings items for the User Preferences tab
fn user_prefs_items(prefs: &UserPreferences, settings: &Settings) -> Vec<SettingItem> {
    vec![
        // ─────────────────────────────────────────────────────────
        // Editor Override
        // ─────────────────────────────────────────────────────────
        SettingItem::new("editor.command", "Editor Command")
            .description("Override project editor setting")
            .value(SettingValue::String(
                prefs
                    .editor
                    .as_ref()
                    .map(|e| e.command.clone())
                    .unwrap_or_default(),
            ))
            .default(SettingValue::String(settings.editor.command.clone()))
            .section("Editor Override"),
        SettingItem::new("editor.open_pattern", "Open Pattern")
            .description("Override project open pattern")
            .value(SettingValue::String(
                prefs
                    .editor
                    .as_ref()
                    .map(|e| e.open_pattern.clone())
                    .unwrap_or_default(),
            ))
            .default(SettingValue::String(settings.editor.open_pattern.clone()))
            .section("Editor Override"),
        // ─────────────────────────────────────────────────────────
        // UI Preferences
        // ─────────────────────────────────────────────────────────
        SettingItem::new("theme", "Theme Override")
            .description("Personal theme preference")
            .value(SettingValue::Enum {
                value: prefs.theme.clone().unwrap_or_default(),
                options: vec![
                    "".to_string(), // Use project default
                    "default".to_string(),
                    "dark".to_string(),
                    "light".to_string(),
                ],
            })
            .default(SettingValue::String(String::new()))
            .section("UI Preferences"),
        // ─────────────────────────────────────────────────────────
        // Session Memory
        // ─────────────────────────────────────────────────────────
        SettingItem::new("last_device", "Last Device")
            .description("Device from last session (auto-saved)")
            .value(SettingValue::String(
                prefs.last_device.clone().unwrap_or_default(),
            ))
            .default(SettingValue::String(String::new()))
            .section("Session Memory")
            .readonly(),
        SettingItem::new("last_config", "Last Config")
            .description("Launch config from last session")
            .value(SettingValue::String(
                prefs.last_config.clone().unwrap_or_default(),
            ))
            .default(SettingValue::String(String::new()))
            .section("Session Memory")
            .readonly(),
    ]
}

/// Truncate string with ellipsis if too long
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len <= 1 {
        s.chars().take(max_len).collect()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use tempfile::tempdir;

    #[test]
    fn test_settings_panel_renders() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        let temp = tempdir().unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings, temp.path());
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Settings"));
        assert!(content.contains("Project"));
        assert!(content.contains("User"));
        assert!(content.contains("Launch"));
        assert!(content.contains("VSCode"));
    }

    #[test]
    fn test_settings_panel_shows_active_tab() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.active_tab = SettingsTab::LaunchConfig;
        let temp = tempdir().unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings, temp.path());
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        // Verify Launch tab content is shown (empty state in this case)
        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("No launch configurations"));
    }

    #[test]
    fn test_settings_panel_dirty_indicator() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.dirty = true;
        let temp = tempdir().unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings, temp.path());
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("unsaved"));
    }

    #[test]
    fn test_tab_navigation_wraps() {
        let mut state = SettingsViewState::new();
        assert_eq!(state.active_tab, SettingsTab::Project);

        // Forward through all tabs
        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::UserPrefs);
        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::LaunchConfig);
        state.next_tab();
        assert_eq!(state.active_tab, SettingsTab::VSCodeConfig);
        state.next_tab(); // Wrap
        assert_eq!(state.active_tab, SettingsTab::Project);
    }

    #[test]
    fn test_tab_switch_resets_selection() {
        let mut state = SettingsViewState::new();
        state.selected_index = 5;

        state.next_tab();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_tab_switch_exits_edit_mode() {
        let mut state = SettingsViewState::new();
        state.editing = true;
        state.edit_buffer = "test".to_string();

        state.next_tab();
        assert!(!state.editing);
        assert!(state.edit_buffer.is_empty());
    }

    #[test]
    fn test_goto_tab() {
        let mut state = SettingsViewState::new();

        state.goto_tab(SettingsTab::VSCodeConfig);
        assert_eq!(state.active_tab, SettingsTab::VSCodeConfig);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_tab_readonly() {
        assert!(!SettingsTab::Project.is_readonly());
        assert!(!SettingsTab::UserPrefs.is_readonly());
        assert!(!SettingsTab::LaunchConfig.is_readonly());
        assert!(SettingsTab::VSCodeConfig.is_readonly());
    }

    #[test]
    fn test_render_shows_all_tabs() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        let temp = tempdir().unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings, temp.path());
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("1.Project"));
        assert!(content.contains("2.User"));
        assert!(content.contains("3.Launch"));
        assert!(content.contains("4.VSCode"));
    }

    #[test]
    fn test_tab_icons() {
        assert_eq!(SettingsTab::Project.icon(), "⚙");
        assert_eq!(SettingsTab::UserPrefs.icon(), "👤");
        assert_eq!(SettingsTab::LaunchConfig.icon(), "▶");
        assert_eq!(SettingsTab::VSCodeConfig.icon(), "📁");
    }

    #[test]
    fn test_project_settings_items_count() {
        let settings = Settings::default();
        let items = project_settings_items(&settings);

        // Should have 16 items across 5 sections
        assert_eq!(items.len(), 16);
    }

    #[test]
    fn test_project_settings_sections() {
        let settings = Settings::default();
        let items = project_settings_items(&settings);

        let sections: Vec<&str> = items.iter().map(|i| i.section.as_str()).collect();
        assert!(sections.contains(&"Behavior"));
        assert!(sections.contains(&"Watcher"));
        assert!(sections.contains(&"UI"));
        assert!(sections.contains(&"DevTools"));
        assert!(sections.contains(&"Editor"));
    }

    #[test]
    fn test_setting_is_modified() {
        let settings = Settings::default();
        let items = project_settings_items(&settings);

        // Default values should not be modified
        for item in &items {
            assert!(
                !item.is_modified(),
                "Item {} should not be modified",
                item.id
            );
        }
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is long", 8), "this is…");
        assert_eq!(truncate_str("ab", 2), "ab");
        assert_eq!(truncate_str("abc", 2), "a…");
    }

    #[test]
    fn test_render_project_tab() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.active_tab = SettingsTab::Project;
        let temp = tempdir().unwrap();

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings, temp.path());
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Check sections are rendered
        assert!(content.contains("[Behavior]"));
        assert!(content.contains("[Watcher]"));
        assert!(content.contains("[UI]"));

        // Check some settings are rendered
        assert!(content.contains("Auto Start"));
        assert!(content.contains("Debounce"));
        assert!(content.contains("Log Buffer"));
    }

    #[test]
    fn test_launch_config_items() {
        use crate::config::{FlutterMode, LaunchConfig};

        let config = LaunchConfig {
            name: "Development".to_string(),
            device: "iphone".to_string(),
            mode: FlutterMode::Debug,
            flavor: Some("dev".to_string()),
            auto_start: true,
            dart_defines: [("API_URL".to_string(), "https://dev.api.com".to_string())]
                .into_iter()
                .collect(),
            extra_args: vec!["--verbose".to_string()],
            entry_point: None,
        };

        let items = launch_config_items(&config, 0);

        assert_eq!(items.len(), 7);
        assert!(items.iter().any(|i| i.id == "launch.0.name"));
        assert!(items.iter().any(|i| i.id == "launch.0.mode"));
    }

    #[test]
    fn test_render_launch_tab_empty() {
        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.active_tab = SettingsTab::LaunchConfig;
        let temp = tempdir().unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings, temp.path());
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("No launch configurations"));
    }

    #[test]
    fn test_render_launch_tab_with_configs() {
        use crate::config::launch::init_launch_file;

        let settings = Settings::default();
        let mut state = SettingsViewState::new();
        state.active_tab = SettingsTab::LaunchConfig;
        let temp = tempdir().unwrap();

        // Create a launch.toml file
        init_launch_file(temp.path()).unwrap();

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let panel = SettingsPanel::new(&settings, temp.path());
                frame.render_stateful_widget(panel, frame.area(), &mut state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        // Should show configuration header
        assert!(content.contains("Configuration 1"));
        // Should show setting fields
        assert!(content.contains("Name"));
        assert!(content.contains("Device"));
        assert!(content.contains("Mode"));
        // Should show "+ Add New Configuration" option
        assert!(content.contains("Add New Configuration"));
    }
}

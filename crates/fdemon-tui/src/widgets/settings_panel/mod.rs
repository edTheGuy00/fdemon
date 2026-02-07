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
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};

use crate::theme::palette;

use std::path::Path;

use fdemon_app::config::{SettingItem, Settings, SettingsTab, UserPreferences};
use fdemon_app::settings_items::{
    launch_config_items, project_settings_items, user_prefs_items, vscode_config_items,
};
use fdemon_app::state::SettingsViewState;

// Use styles module
use styles::{
    add_new_style, config_header_style, description_style, editing_style, indicator_style,
    info_border_style, label_style, override_indicator_style, readonly_indicator_style,
    readonly_label_style, readonly_value_style, section_header_style, truncate_str, value_style,
    vscode_header_style, INDICATOR_WIDTH, INDICATOR_WIDTH_OVERRIDE, LABEL_WIDTH, LABEL_WIDTH_SHORT,
    LABEL_WIDTH_VSCODE, VALUE_WIDTH, VALUE_WIDTH_VSCODE,
};

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
        let bg_style = Style::default().bg(palette::DEEPEST_BG);
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
    // ─────────────────────────────────────────────────────────────────────────────
    // Header and Tab Rendering
    // ─────────────────────────────────────────────────────────────────────────────

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
                Style::default().fg(palette::TEXT_MUTED),
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
                Style::default().fg(palette::TEXT_MUTED).bg(palette::ACCENT),
                Style::default()
                    .fg(Color::Black)
                    .bg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
                Style::default().bg(palette::ACCENT),
            )
        } else {
            (
                Style::default().fg(palette::TEXT_MUTED),
                Style::default().fg(palette::TEXT_PRIMARY),
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
                    .set_style(Style::default().fg(palette::ACCENT));
            }
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
            .style(Style::default().fg(palette::TEXT_MUTED));

        footer.render(inner, buf);
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Project Tab
    // ─────────────────────────────────────────────────────────────────────────────

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
        buf.set_string(x + 1, y, &header, section_header_style());
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
        let desc_start: u16 = INDICATOR_WIDTH + LABEL_WIDTH + VALUE_WIDTH + 2;

        // Selection indicator
        let indicator = if is_selected { "▶ " } else { "  " };
        buf.set_string(x, y, indicator, indicator_style(is_selected));

        // Label
        let label = truncate_str(&item.label, LABEL_WIDTH as usize - 1);
        buf.set_string(x + INDICATOR_WIDTH, y, &label, label_style(is_selected));

        // Value
        let value_x = x + INDICATOR_WIDTH + LABEL_WIDTH;
        let (value_str, style) = if is_editing {
            (format!("{}▌", edit_buffer), editing_style())
        } else {
            let modified_indicator = if item.is_modified() { "*" } else { "" };
            (
                format!("{}{}", item.value.display(), modified_indicator),
                value_style(&item.value, is_selected),
            )
        };
        let value_display = truncate_str(&value_str, VALUE_WIDTH as usize);
        buf.set_string(value_x, y, &value_display, style);

        // Description (dimmed)
        if width > desc_start + 10 {
            let desc_width = (width - desc_start) as usize;
            let desc = truncate_str(&item.description, desc_width);
            buf.set_string(x + desc_start, y, &desc, description_style());
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // User Preferences Tab
    // ─────────────────────────────────────────────────────────────────────────────

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
            .border_style(info_border_style())
            .title(" Local Settings ");

        let inner = block.inner(area);
        block.render(area, buf);

        let info = Paragraph::new(vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "These settings are stored in .fdemon/settings.local.toml",
                    Style::default().fg(palette::TEXT_PRIMARY),
                ),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "They are gitignored and override project settings for you only.",
                    Style::default().fg(palette::TEXT_MUTED),
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
        let desc_start: u16 = INDICATOR_WIDTH_OVERRIDE + LABEL_WIDTH_SHORT + VALUE_WIDTH + 2;

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

        buf.set_string(
            x,
            y,
            indicator,
            override_indicator_style(is_override, is_selected),
        );

        // Label
        let label = truncate_str(&item.label, LABEL_WIDTH_SHORT as usize - 1);
        buf.set_string(
            x + INDICATOR_WIDTH_OVERRIDE,
            y,
            &label,
            label_style(is_selected),
        );

        // Value
        let value_x = x + INDICATOR_WIDTH_OVERRIDE + LABEL_WIDTH_SHORT;
        let (value_str, style) = if is_editing {
            (format!("{}▌", edit_buffer), editing_style())
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
                value_style(&item.value, is_selected),
            )
        };
        let value_display = truncate_str(&value_str, VALUE_WIDTH as usize);
        buf.set_string(value_x, y, &value_display, style);

        // Description (dimmed)
        if width > desc_start + 10 {
            let desc_width = (width - desc_start) as usize;
            let desc = truncate_str(&item.description, desc_width);
            buf.set_string(x + desc_start, y, &desc, description_style());
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Launch Config Tab
    // ─────────────────────────────────────────────────────────────────────────────

    fn render_launch_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
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
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "No launch configurations found",
                Style::default().fg(palette::STATUS_YELLOW),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Create .fdemon/launch.toml or press ",
                    Style::default().fg(palette::TEXT_MUTED),
                ),
                Span::styled("n", Style::default().fg(palette::ACCENT)),
                Span::styled(" to create one.", Style::default().fg(palette::TEXT_MUTED)),
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

    fn render_vscode_tab(&self, area: Rect, buf: &mut Buffer, state: &mut SettingsViewState) {
        use fdemon_app::config::load_vscode_configs;

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
            .border_style(info_border_style())
            .title(" VSCode Launch Configurations ");

        let inner = block.inner(area);
        block.render(area, buf);

        let info = Paragraph::new(vec![
            Line::from(vec![
                Span::raw("  🔒 "),
                Span::styled(
                    "Read-only view of .vscode/launch.json (Dart configurations only)",
                    Style::default().fg(palette::TEXT_PRIMARY),
                ),
            ]),
            Line::from(vec![
                Span::raw("     "),
                Span::styled(
                    "Edit this file directly in VSCode for changes.",
                    Style::default().fg(palette::TEXT_MUTED),
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
                Style::default().fg(palette::STATUS_YELLOW),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Create launch configurations in VSCode:",
                Style::default().fg(palette::TEXT_MUTED),
            )]),
            Line::from(vec![Span::styled(
                "Run > Add Configuration > Dart & Flutter",
                Style::default().fg(palette::ACCENT),
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
                Style::default().fg(palette::STATUS_YELLOW),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Add a Dart configuration in VSCode:",
                Style::default().fg(palette::TEXT_MUTED),
            )]),
            Line::from(vec![Span::styled(
                "Run > Add Configuration > Dart: Flutter",
                Style::default().fg(palette::ACCENT),
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
        // Selection indicator (dimmed for readonly)
        let indicator = if is_selected { "› " } else { "  " };
        buf.set_string(x, y, indicator, readonly_indicator_style());

        // Label (dimmed)
        let label = truncate_str(&item.label, LABEL_WIDTH_VSCODE as usize - 1);
        buf.set_string(
            x + INDICATOR_WIDTH,
            y,
            &label,
            readonly_label_style(is_selected),
        );

        // Value (read-only styling)
        let value_x = x + INDICATOR_WIDTH + LABEL_WIDTH_VSCODE;
        let value_str = item.value.display();
        let value_display = truncate_str(&value_str, VALUE_WIDTH_VSCODE as usize);
        buf.set_string(value_x, y, &value_display, readonly_value_style());

        // Lock icon to indicate read-only
        if is_selected {
            let lock_x = value_x + value_display.len() as u16 + 1;
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

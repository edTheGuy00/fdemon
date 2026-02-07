//! Interactive project selector for choosing between multiple Flutter projects
//!
//! This module provides a Ratatui-based selector that runs BEFORE
//! the main TUI initializes, allowing users to pick which project to run.

use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use fdemon_core::prelude::*;

/// Maximum number of projects to display (limited by single-digit selection)
const MAX_DISPLAY_PROJECTS: usize = 9;

/// Result of project selection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionResult {
    /// User selected a project
    Selected(PathBuf),
    /// User cancelled selection (pressed 'q' or Ctrl+C)
    Cancelled,
}

/// State for the selector UI
struct SelectorState {
    /// Index of currently selected project
    selected: usize,
    /// List widget state
    list_state: ListState,
}

impl SelectorState {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            selected: 0,
            list_state,
        }
    }

    fn select_next(&mut self, max: usize) {
        if self.selected < max.saturating_sub(1) {
            self.selected += 1;
            self.list_state.select(Some(self.selected));
        }
    }

    fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.list_state.select(Some(self.selected));
        }
    }

    fn select_index(&mut self, index: usize, max: usize) {
        if index < max {
            self.selected = index;
            self.list_state.select(Some(self.selected));
        }
    }
}

/// Display a project selector and wait for user input
///
/// # Arguments
/// * `projects` - List of discovered project paths to display
/// * `searched_from` - Base path that was searched (for display context)
///
/// # Returns
/// * `Result<SelectionResult>` - Selected project or cancellation
///
/// # Panics
/// * Panics if `projects` is empty (caller should handle this case)
pub fn select_project(projects: &[PathBuf], searched_from: &Path) -> Result<SelectionResult> {
    assert!(!projects.is_empty(), "projects list must not be empty");

    // Initialize Ratatui terminal
    let mut terminal = ratatui::init();
    let mut state = SelectorState::new();

    let display_count = projects.len().min(MAX_DISPLAY_PROJECTS);

    // Main event loop
    let result = loop {
        // Render the selector
        terminal
            .draw(|frame| render_selector(frame, projects, searched_from, &mut state))
            .map_err(|e| Error::terminal(e.to_string()))?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(100))
            .map_err(|e| Error::terminal(e.to_string()))?
        {
            if let Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }) = event::read().map_err(|e| Error::terminal(e.to_string()))?
            {
                // Check for cancellation
                if is_cancel_key(code, modifiers) {
                    break SelectionResult::Cancelled;
                }

                match code {
                    // Arrow key navigation
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.select_previous();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        state.select_next(display_count);
                    }
                    // Enter to confirm selection
                    KeyCode::Enter => {
                        break SelectionResult::Selected(projects[state.selected].clone());
                    }
                    // Number key selection
                    KeyCode::Char(c) => {
                        if let Some(index) = validate_selection(c, projects) {
                            state.select_index(index, display_count);
                            break SelectionResult::Selected(projects[index].clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    };

    // Restore terminal
    ratatui::restore();

    Ok(result)
}

/// Render the selector UI
fn render_selector(
    frame: &mut Frame,
    projects: &[PathBuf],
    searched_from: &Path,
    state: &mut SelectorState,
) {
    let area = frame.area();

    // Calculate modal dimensions
    let modal_width = (area.width * 70 / 100).clamp(40, 60);
    let display_count = projects.len().min(MAX_DISPLAY_PROJECTS);
    let content_height = display_count as u16 + 10; // projects + header/footer/padding
    let modal_height = content_height.min(area.height.saturating_sub(4));

    // Center the modal
    let modal_area = center_rect(modal_width, modal_height, area);

    // Clear the area behind the modal
    frame.render_widget(Clear, modal_area);

    // Outer block with title
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Flutter Demon ")
        .title_style(Style::default().fg(Color::Cyan).bold());

    let inner_area = outer_block.inner(modal_area);
    frame.render_widget(outer_block, modal_area);

    // Split inner area into sections
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header text
        Constraint::Min(3),    // Project list
        Constraint::Length(2), // Footer/help
    ])
    .split(inner_area);

    // Header - context info
    let header_text = vec![
        Line::from("Multiple Flutter projects found in:"),
        Line::from(Span::styled(
            truncate_path(searched_from, (modal_width - 4) as usize),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let header = Paragraph::new(header_text).alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    // Project list
    let items: Vec<ListItem> = projects
        .iter()
        .take(display_count)
        .enumerate()
        .map(|(i, p)| {
            let relative_path = format_relative_path(p, searched_from);
            let content = Line::from(vec![
                Span::styled(
                    format!("[{}] ", i + 1),
                    Style::default().fg(Color::Yellow).bold(),
                ),
                Span::raw(relative_path),
            ]);
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Select a project ")
                .title_style(Style::default().fg(Color::White)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[1], &mut state.list_state);

    // Footer - help text
    let footer_text = Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
        Span::raw(" Navigate  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" Select  "),
        Span::styled("1-9", Style::default().fg(Color::Yellow)),
        Span::raw(" Quick select  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(" Quit"),
    ]);
    let footer = Paragraph::new(footer_text).alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

/// Center a rectangle within another rectangle
fn center_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);

    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

/// Truncate a path to fit within a given width
fn truncate_path(path: &Path, max_width: usize) -> String {
    let s = path.display().to_string();
    if s.len() <= max_width {
        s
    } else {
        format!("...{}", &s[s.len() - max_width + 3..])
    }
}

/// Format a project path relative to the base path
pub fn format_relative_path(project: &Path, base: &Path) -> String {
    match project.strip_prefix(base) {
        Ok(relative) => {
            if relative.as_os_str().is_empty() {
                ".".to_string()
            } else {
                relative.display().to_string()
            }
        }
        Err(_) => project.display().to_string(),
    }
}

/// Validate and convert a key press to a project index
pub fn validate_selection(key: char, projects: &[PathBuf]) -> Option<usize> {
    // Only accept digits 1-9
    if !key.is_ascii_digit() || key == '0' {
        return None;
    }

    let index = (key as usize) - ('1' as usize);
    let max_index = projects.len().min(MAX_DISPLAY_PROJECTS);

    if index < max_index {
        Some(index)
    } else {
        None
    }
}

/// Check if a key press is a cancellation request
pub fn is_cancel_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => true,
        KeyCode::Esc => true,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_relative_path() {
        let base = PathBuf::from("/home/user/projects");
        let project = PathBuf::from("/home/user/projects/my_app");

        let relative = format_relative_path(&project, &base);
        assert_eq!(relative, "my_app");
    }

    #[test]
    fn test_format_relative_path_nested() {
        let base = PathBuf::from("/home/user/projects");
        let project = PathBuf::from("/home/user/projects/examples/counter");

        let relative = format_relative_path(&project, &base);
        assert_eq!(relative, "examples/counter");
    }

    #[test]
    fn test_format_relative_path_same() {
        let base = PathBuf::from("/home/user/projects");
        let project = PathBuf::from("/home/user/projects");

        let relative = format_relative_path(&project, &base);
        assert_eq!(relative, ".");
    }

    #[test]
    fn test_format_relative_path_outside() {
        let base = PathBuf::from("/home/user/projects");
        let project = PathBuf::from("/other/path/app");

        let relative = format_relative_path(&project, &base);
        assert_eq!(relative, "/other/path/app");
    }

    #[test]
    fn test_validate_selection_valid() {
        let projects = vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];

        assert_eq!(validate_selection('1', &projects), Some(0));
        assert_eq!(validate_selection('2', &projects), Some(1));
        assert_eq!(validate_selection('3', &projects), Some(2));
        assert_eq!(validate_selection('4', &projects), None); // Out of range
        assert_eq!(validate_selection('0', &projects), None); // Zero not valid
        assert_eq!(validate_selection('a', &projects), None); // Letter not valid
    }

    #[test]
    fn test_validate_selection_max_projects() {
        let projects: Vec<PathBuf> = (0..15).map(|i| PathBuf::from(format!("/{}", i))).collect();

        // Should only allow selection up to MAX_DISPLAY_PROJECTS (9)
        assert_eq!(validate_selection('9', &projects), Some(8));
        // Even though there are more projects, we can't select them with single digit
    }

    #[test]
    fn test_is_cancel_key() {
        assert!(is_cancel_key(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(is_cancel_key(KeyCode::Char('Q'), KeyModifiers::NONE));
        assert!(is_cancel_key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(is_cancel_key(KeyCode::Char('c'), KeyModifiers::CONTROL));

        assert!(!is_cancel_key(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(!is_cancel_key(KeyCode::Char('1'), KeyModifiers::NONE));
        assert!(!is_cancel_key(KeyCode::Enter, KeyModifiers::NONE));
    }

    #[test]
    fn test_selection_result_eq() {
        let path = PathBuf::from("/test/app");
        assert_eq!(
            SelectionResult::Selected(path.clone()),
            SelectionResult::Selected(path)
        );
        assert_eq!(SelectionResult::Cancelled, SelectionResult::Cancelled);
        assert_ne!(
            SelectionResult::Selected(PathBuf::from("/a")),
            SelectionResult::Cancelled
        );
    }

    #[test]
    fn test_truncate_path_short() {
        let path = Path::new("/short/path");
        assert_eq!(truncate_path(path, 50), "/short/path");
    }

    #[test]
    fn test_truncate_path_long() {
        let path = Path::new("/very/long/path/that/exceeds/max/width");
        let truncated = truncate_path(path, 20);
        assert!(truncated.starts_with("..."));
        assert!(truncated.len() <= 20);
    }

    #[test]
    fn test_selector_state_navigation() {
        let mut state = SelectorState::new();
        assert_eq!(state.selected, 0);

        state.select_next(5);
        assert_eq!(state.selected, 1);

        state.select_next(5);
        assert_eq!(state.selected, 2);

        state.select_previous();
        assert_eq!(state.selected, 1);

        state.select_previous();
        assert_eq!(state.selected, 0);

        // Can't go below 0
        state.select_previous();
        assert_eq!(state.selected, 0);

        // Can't go beyond max
        state.select_index(4, 5);
        assert_eq!(state.selected, 4);
        state.select_next(5);
        assert_eq!(state.selected, 4);
    }
}

//! Interactive project selector for choosing between multiple Flutter projects
//!
//! This module provides a simple terminal-based selector that runs BEFORE
//! the main TUI initializes, allowing users to pick which project to run.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{Color, Print, ResetColor, SetForegroundColor, Stylize},
    terminal::{self, ClearType},
    ExecutableCommand, QueueableCommand,
};

use crate::common::prelude::*;

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

/// RAII guard for raw terminal mode
/// Ensures terminal is restored even on panic
struct RawModeGuard {
    was_raw: bool,
}

impl RawModeGuard {
    fn new() -> Result<Self> {
        let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
        if !was_raw {
            terminal::enable_raw_mode().map_err(|e| Error::terminal(e.to_string()))?;
        }
        Ok(Self { was_raw })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if !self.was_raw {
            let _ = terminal::disable_raw_mode();
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

    // Enter raw mode with RAII guard
    let _guard = RawModeGuard::new()?;

    // Display the menu
    display_menu(projects, searched_from)?;

    // Wait for valid input
    loop {
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
                    clear_and_reset()?;
                    return Ok(SelectionResult::Cancelled);
                }

                // Check for valid number selection
                if let KeyCode::Char(c) = code {
                    if let Some(index) = validate_selection(c, projects) {
                        clear_and_reset()?;
                        return Ok(SelectionResult::Selected(projects[index].clone()));
                    }
                }
                // Invalid key - ignore and continue waiting
            }
        }
    }
}

/// Display the project selection menu
fn display_menu(projects: &[PathBuf], searched_from: &Path) -> Result<()> {
    let mut stdout = io::stdout();

    // Clear screen and move to top
    stdout
        .execute(terminal::Clear(ClearType::All))
        .map_err(|e| Error::terminal(e.to_string()))?;
    stdout
        .execute(cursor::MoveTo(0, 0))
        .map_err(|e| Error::terminal(e.to_string()))?;

    // Title
    stdout.queue(Print("\n  "))?;
    stdout.queue(SetForegroundColor(Color::Cyan))?;
    stdout.queue(Print("Flutter Demon".bold()))?;
    stdout.queue(ResetColor)?;
    stdout.queue(Print("\n\n"))?;

    // Context
    stdout.queue(Print("  Multiple Flutter projects found in:\n"))?;
    stdout.queue(Print("  "))?;
    stdout.queue(SetForegroundColor(Color::DarkGrey))?;
    stdout.queue(Print(searched_from.display()))?;
    stdout.queue(ResetColor)?;
    stdout.queue(Print("\n\n"))?;

    // Instruction
    stdout.queue(Print("  Select a project:\n\n"))?;

    // Project list
    let display_count = projects.len().min(MAX_DISPLAY_PROJECTS);
    for (i, project) in projects.iter().take(display_count).enumerate() {
        let relative_path = format_relative_path(project, searched_from);

        stdout.queue(Print("    "))?;
        stdout.queue(SetForegroundColor(Color::Yellow))?;
        stdout.queue(Print(format!("[{}]", i + 1).bold()))?;
        stdout.queue(ResetColor)?;
        stdout.queue(Print(format!(" {}\n", relative_path)))?;
    }

    // Show truncation message if needed
    if projects.len() > MAX_DISPLAY_PROJECTS {
        stdout.queue(Print("\n"))?;
        stdout.queue(SetForegroundColor(Color::DarkGrey))?;
        stdout.queue(Print(format!(
            "    ... and {} more (showing first {})\n",
            projects.len() - MAX_DISPLAY_PROJECTS,
            MAX_DISPLAY_PROJECTS
        )))?;
        stdout.queue(ResetColor)?;
    }

    // Prompt
    stdout.queue(Print("\n  "))?;
    stdout.queue(Print(
        format!("Enter number (1-{}) or 'q' to quit: ", display_count).bold(),
    ))?;

    stdout.flush().map_err(|e| Error::terminal(e.to_string()))?;

    Ok(())
}

/// Clear screen and reset cursor position
fn clear_and_reset() -> Result<()> {
    let mut stdout = io::stdout();
    stdout
        .execute(terminal::Clear(ClearType::All))
        .map_err(|e| Error::terminal(e.to_string()))?;
    stdout
        .execute(cursor::MoveTo(0, 0))
        .map_err(|e| Error::terminal(e.to_string()))?;
    Ok(())
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
}

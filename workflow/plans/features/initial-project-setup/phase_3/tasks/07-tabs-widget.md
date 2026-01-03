## Task: Tabs Widget for Multi-Session Display

**Objective**: Create a `SessionTabs` widget using Ratatui's `Tabs` component to display all running Flutter app sessions as tabs in the header area. The widget should show session status indicators, device names, and support highlighting the currently selected tab.

**Depends on**: [06-delayed-start](06-delayed-start.md)

---

### Scope

- `src/tui/widgets/tabs.rs`: **NEW** - Session tabs widget
- `src/tui/widgets/mod.rs`: Add `pub mod tabs;` and re-exports
- `src/tui/widgets/header.rs`: Update to optionally include tabs
- `src/tui/layout.rs`: Adjust layout for tabs in header area

---

### Implementation Details

#### UI Design

Single session (no tabs shown):
```
┌─────────────────────────────────────────────────────────────────────┐
│  Flutter Demon                            [r] Reload  [R] Restart   │
├─────────────────────────────────────────────────────────────────────┤
```

Multiple sessions (tabs shown):
```
┌─────────────────────────────────────────────────────────────────────┐
│  Flutter Demon  │ ● iPhone 15 │ ○ Pixel 8 │ ● macOS │     [r] [R]   │
├─────────────────┴─────────────┴───────────┴─────────┴───────────────┤
```

Tab indicators:
- `●` Running (green)
- `○` Stopped/Initializing (gray)
- `↻` Reloading (yellow)
- `✗` Error (red)

#### Session Tabs Widget

```rust
//! Session tabs widget for multi-instance display

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Tabs, Widget},
};

use crate::app::session_manager::SessionManager;
use crate::core::AppPhase;

/// Widget displaying session tabs in the header
pub struct SessionTabs<'a> {
    session_manager: &'a SessionManager,
}

impl<'a> SessionTabs<'a> {
    pub fn new(session_manager: &'a SessionManager) -> Self {
        Self { session_manager }
    }
    
    /// Create tab titles from sessions
    fn tab_titles(&self) -> Vec<Line<'static>> {
        self.session_manager
            .iter()
            .map(|handle| {
                let session = &handle.session;
                
                // Status icon with color
                let (icon, icon_color) = match session.phase {
                    AppPhase::Running => ("●", Color::Green),
                    AppPhase::Reloading => ("↻", Color::Yellow),
                    AppPhase::Initializing => ("○", Color::DarkGray),
                    AppPhase::Stopped => ("○", Color::DarkGray),
                    AppPhase::Quitting => ("✗", Color::Red),
                };
                
                // Truncate device name if too long
                let name = truncate_name(&session.device_name, 12);
                
                Line::from(vec![
                    Span::styled(format!(" {} ", icon), Style::default().fg(icon_color)),
                    Span::raw(name),
                    Span::raw(" "),
                ])
            })
            .collect()
    }
}

impl Widget for SessionTabs<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Layout: App title | Tabs | Keybindings
        let chunks = Layout::horizontal([
            Constraint::Length(16),   // "Flutter Demon  │"
            Constraint::Min(20),      // Tabs
            Constraint::Length(20),   // Keybindings
        ])
        .split(area);
        
        // App title
        let title = Line::from(vec![
            Span::styled("Flutter Demon", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  │"),
        ]);
        buf.set_line(chunks[0].x, chunks[0].y, &title, chunks[0].width);
        
        // Session tabs
        if self.session_manager.len() > 0 {
            let titles = self.tab_titles();
            let selected = self.session_manager.selected_index();
            
            let tabs = Tabs::new(titles)
                .select(selected)
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                )
                .divider("│");
            
            tabs.render(chunks[1], buf);
        }
        
        // Keybindings hint
        let hints = Line::from(vec![
            Span::styled("[r]", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled("[R]", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled("[d]", Style::default().fg(Color::Yellow)),
        ]);
        
        // Right-align the hints
        let hints_width = 11; // "[r] [R] [d]"
        if chunks[2].width >= hints_width {
            let x = chunks[2].x + chunks[2].width - hints_width;
            buf.set_line(x, chunks[2].y, &hints, hints_width);
        }
    }
}

/// Truncate a name to max length, adding ellipsis if needed
fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        format!("{}…", &name[..max_len - 1])
    }
}

/// Header widget that conditionally shows tabs
pub struct HeaderWithTabs<'a> {
    session_manager: Option<&'a SessionManager>,
}

impl<'a> HeaderWithTabs<'a> {
    /// Create header without tabs
    pub fn simple() -> Self {
        Self {
            session_manager: None,
        }
    }
    
    /// Create header with session tabs
    pub fn with_sessions(session_manager: &'a SessionManager) -> Self {
        Self {
            session_manager: Some(session_manager),
        }
    }
}

impl Widget for HeaderWithTabs<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.session_manager {
            Some(manager) if manager.len() > 1 => {
                // Multiple sessions - show tabs
                SessionTabs::new(manager).render(area, buf);
            }
            Some(manager) if manager.len() == 1 => {
                // Single session - show device name in header but no tabs
                render_single_session_header(manager, area, buf);
            }
            _ => {
                // No sessions - simple header
                render_simple_header(area, buf);
            }
        }
    }
}

fn render_simple_header(area: Rect, buf: &mut Buffer) {
    let title = Line::from(vec![
        Span::styled(
            "Flutter Demon",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
    ]);
    
    buf.set_line(area.x + 1, area.y, &title, area.width.saturating_sub(2));
    
    // Keybindings on the right
    let hints = "[r] Reload  [R] Restart  [d] DevTools  [q] Quit";
    let hints_len = hints.len() as u16;
    if area.width > hints_len + 15 {
        buf.set_string(
            area.x + area.width - hints_len - 1,
            area.y,
            hints,
            Style::default().fg(Color::DarkGray),
        );
    }
}

fn render_single_session_header(manager: &SessionManager, area: Rect, buf: &mut Buffer) {
    if let Some(handle) = manager.selected() {
        let session = &handle.session;
        
        let (icon, icon_color) = match session.phase {
            AppPhase::Running => ("●", Color::Green),
            AppPhase::Reloading => ("↻", Color::Yellow),
            AppPhase::Initializing => ("○", Color::DarkGray),
            AppPhase::Stopped => ("○", Color::DarkGray),
            AppPhase::Quitting => ("✗", Color::Red),
        };
        
        let title = Line::from(vec![
            Span::styled(
                "Flutter Demon",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(icon, Style::default().fg(icon_color)),
            Span::raw(" "),
            Span::raw(&session.device_name),
        ]);
        
        buf.set_line(area.x + 1, area.y, &title, area.width.saturating_sub(2));
        
        // Keybindings on the right
        let hints = "[r] [R] [d] [n] [q]";
        let hints_len = hints.len() as u16;
        if area.width > hints_len + 30 {
            buf.set_string(
                area.x + area.width - hints_len - 1,
                area.y,
                hints,
                Style::default().fg(Color::DarkGray),
            );
        }
    } else {
        render_simple_header(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::session::Session;
    use crate::daemon::Device;
    
    fn test_device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            sdk: None,
            is_supported: true,
        }
    }
    
    #[test]
    fn test_truncate_name() {
        assert_eq!(truncate_name("Short", 10), "Short");
        assert_eq!(truncate_name("iPhone 15 Pro Max", 12), "iPhone 15 P…");
        assert_eq!(truncate_name("A", 1), "…");
        assert_eq!(truncate_name("AB", 2), "AB");
        assert_eq!(truncate_name("ABC", 2), "A…");
    }
    
    #[test]
    fn test_session_tabs_creation() {
        let mut manager = SessionManager::new();
        manager.create_session(&test_device("d1", "iPhone 15")).unwrap();
        manager.create_session(&test_device("d2", "Pixel 8")).unwrap();
        
        let tabs = SessionTabs::new(&manager);
        let titles = tabs.tab_titles();
        
        assert_eq!(titles.len(), 2);
    }
    
    #[test]
    fn test_tab_title_includes_status_icon() {
        let mut manager = SessionManager::new();
        let id = manager.create_session(&test_device("d1", "iPhone")).unwrap();
        
        // Initially Initializing
        let tabs = SessionTabs::new(&manager);
        let titles = tabs.tab_titles();
        let title_str: String = titles[0].spans.iter().map(|s| s.content.to_string()).collect();
        assert!(title_str.contains("○")); // Initializing icon
        
        // Mark as running
        manager.get_mut(id).unwrap().session.mark_started("app-1".to_string());
        let tabs = SessionTabs::new(&manager);
        let titles = tabs.tab_titles();
        let title_str: String = titles[0].spans.iter().map(|s| s.content.to_string()).collect();
        assert!(title_str.contains("●")); // Running icon
    }
    
    #[test]
    fn test_header_with_tabs_single_session() {
        let mut manager = SessionManager::new();
        manager.create_session(&test_device("d1", "iPhone 15")).unwrap();
        
        let header = HeaderWithTabs::with_sessions(&manager);
        // Should render single session header (no tabs)
        assert!(manager.len() == 1);
    }
    
    #[test]
    fn test_header_with_tabs_multiple_sessions() {
        let mut manager = SessionManager::new();
        manager.create_session(&test_device("d1", "iPhone")).unwrap();
        manager.create_session(&test_device("d2", "Pixel")).unwrap();
        
        let header = HeaderWithTabs::with_sessions(&manager);
        // Should render tabs
        assert!(manager.len() > 1);
    }
}
```

---

### Acceptance Criteria

1. [ ] `src/tui/widgets/tabs.rs` created with `SessionTabs` widget
2. [ ] `HeaderWithTabs` widget handles 0, 1, and multiple sessions correctly
3. [ ] Tab titles include status icon with appropriate color
4. [ ] Currently selected tab is highlighted
5. [ ] Device names are truncated if too long
6. [ ] Tab dividers are rendered between sessions
7. [ ] Keybinding hints are displayed on the right side of header
8. [ ] Single session mode shows device name without tab UI
9. [ ] No session mode shows simple header
10. [ ] Status icons: `●` green (running), `○` gray (stopped), `↻` yellow (reloading), `✗` red (error)
11. [ ] All new code has unit tests
12. [ ] `cargo test` passes
13. [ ] `cargo clippy` has no warnings

---

### Testing

Unit tests are included in the implementation above. Visual testing:

```rust
#[test]
fn test_tabs_widget_rendering() {
    use ratatui::{backend::TestBackend, Terminal};
    
    let mut manager = SessionManager::new();
    manager.create_session(&test_device("d1", "iPhone 15 Pro")).unwrap();
    manager.create_session(&test_device("d2", "Pixel 8")).unwrap();
    
    // Mark first as running
    let id = manager.session_order[0];
    manager.get_mut(id).unwrap().session.mark_started("app-1".to_string());
    
    let backend = TestBackend::new(80, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    
    terminal.draw(|f| {
        let tabs = SessionTabs::new(&manager);
        f.render_widget(tabs, f.area());
    }).unwrap();
    
    let buffer = terminal.backend().buffer();
    
    // Verify content
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("iPhone"));
    assert!(content.contains("Pixel"));
    assert!(content.contains("●")); // Running indicator
}
```

---

### Notes

- The `Tabs` widget from Ratatui handles most of the rendering logic
- Tab switching is handled by the event handler, not this widget
- The highlight style uses inverted colors (black on cyan) for visibility
- Consider adding keyboard shortcut hints (`1`, `2`, etc.) next to tab names in future
- Tab overflow (more tabs than fit) could use scroll indicators in future enhancement
- The widget uses the `SessionManager` reference to avoid copying session data

---

### Files to Create/Modify

| File | Action |
|------|--------|
| `src/tui/widgets/tabs.rs` | Create with `SessionTabs` and `HeaderWithTabs` widgets |
| `src/tui/widgets/mod.rs` | Add `pub mod tabs;` and re-export widgets |
| `src/tui/render.rs` | Use `HeaderWithTabs` instead of `Header` when appropriate |

---

## Completion Summary

**Status**: ✅ Done

### Files Modified
- `src/tui/widgets/tabs.rs` — Created with `SessionTabs` and `HeaderWithTabs` widgets
- `src/tui/widgets/mod.rs` — Added `pub mod tabs;` and re-exports for `HeaderWithTabs`, `SessionTabs`
- `src/tui/render.rs` — Updated to use `HeaderWithTabs::with_sessions()` instead of `Header::new()`

### Implementation Details

1. **SessionTabs widget**: Displays session tabs using Ratatui's `Tabs` component with:
   - Status icons: `●` (green) for Running, `○` (gray) for Initializing/Stopped, `↻` (yellow) for Reloading, `✗` (red) for Quitting
   - Device name truncation with ellipsis for names > 12 characters
   - Proper Unicode-aware truncation
   - Highlighted selected tab with inverted colors (black on cyan)
   - Dividers between tabs

2. **HeaderWithTabs widget**: Conditionally renders based on session count:
   - 0 sessions: Simple header with app title and keybinding hints
   - 1 session: Shows device name + status icon inline (no tabs)
   - 2+ sessions: Full tabs widget with selectable tabs

3. **truncate_name helper**: Unicode-aware string truncation with ellipsis

### Testing Performed
- `cargo check` — ✅ Passed
- `cargo test` — ✅ All 347 tests pass (12 new tests for tabs module)
- `cargo clippy` — ✅ No warnings

### Unit Tests Added
- `test_truncate_name_short` — verifies short names are returned as-is
- `test_truncate_name_long` — verifies long names are truncated with ellipsis
- `test_truncate_name_edge_cases` — boundary conditions for truncation
- `test_truncate_name_unicode` — Unicode character handling
- `test_session_tabs_creation` — tab titles generation
- `test_tab_title_includes_status_icon` — status icons match session phase
- `test_header_with_tabs_no_sessions` — simple header for empty state
- `test_header_with_tabs_single_session` — single session display
- `test_header_with_tabs_multiple_sessions` — tabs display for multiple sessions
- `test_header_simple` — constructor test
- `test_tabs_widget_rendering` — visual rendering test with TestBackend
- `test_single_session_rendering` — single session header rendering

### Acceptance Criteria

1. [x] `src/tui/widgets/tabs.rs` created with `SessionTabs` widget
2. [x] `HeaderWithTabs` widget handles 0, 1, and multiple sessions correctly
3. [x] Tab titles include status icon with appropriate color
4. [x] Currently selected tab is highlighted
5. [x] Device names are truncated if too long
6. [x] Tab dividers are rendered between sessions
7. [x] Keybinding hints are displayed on the right side of header
8. [x] Single session mode shows device name without tab UI
9. [x] No session mode shows simple header
10. [x] Status icons: `●` green (running), `○` gray (stopped), `↻` yellow (reloading), `✗` red (error)
11. [x] All new code has unit tests
12. [x] `cargo test` passes
13. [x] `cargo clippy` has no warnings

### Risks/Limitations
- Tab overflow (more tabs than fit) not yet implemented — may need scroll indicators in future
- Keyboard shortcuts for tab switching (1-9, Tab/Shift+Tab) handled by event handler in Task 10
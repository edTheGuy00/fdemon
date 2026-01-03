## Task: Refined Layout for Cockpit UI

**Objective**: Polish the overall UI layout to accommodate multi-session tabs, improve visual hierarchy, and create a cohesive "cockpit" experience for Flutter development. This includes responsive layout adjustments, proper spacing, and visual refinements.

**Depends on**: [07-tabs-widget](07-tabs-widget.md)

---

### Scope

- `src/tui/layout.rs`: Update layout calculations for tabs and new UI elements
- `src/tui/render.rs`: Refine rendering logic for polished appearance
- `src/tui/widgets/header.rs`: Update header widget for refined layout
- `src/tui/widgets/status_bar.rs`: Update status bar to use session data
- `src/tui/widgets/log_view.rs`: Minor polish and improvements

---

### Implementation Details

#### Target UI Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Flutter Demon  │ ● iPhone 15 │ ○ Pixel 8 │ ● macOS │  [r] [R] [d] [q]  │  <- Header with tabs
├─────────────────┴─────────────┴───────────┴─────────┴───────────────────┤
│                                                                         │
│  [12:34:56] ● flutter: App started                                      │  <- Log area
│  [12:34:57] ○ flutter: Building widget tree...                          │
│  [12:35:01] ● Reloaded 1 of 423 libraries in 234ms                      │
│  [12:35:15] ○ flutter: Button pressed                                   │
│  [12:35:16] ✗ flutter: Error: Widget overflow by 42 pixels              │
│                                                                         │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│  ● Running on iPhone 15 Pro (simulator) │ Reloads: 3 │ 00:05:23         │  <- Status bar
└─────────────────────────────────────────────────────────────────────────┘
```

#### Responsive Breakpoints

| Terminal Width | Layout Adjustments |
|----------------|-------------------|
| < 60 cols | Compact mode: minimal headers, abbreviated status |
| 60-80 cols | Standard mode: full status bar, truncated tabs |
| 80-120 cols | Comfortable mode: full tabs, spaced elements |
| > 120 cols | Wide mode: additional info, wider log timestamps |

#### Updated Layout Module (`src/tui/layout.rs`)

```rust
//! Layout calculations for the TUI

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Layout areas for the main UI
#[derive(Debug, Clone, Copy)]
pub struct LayoutAreas {
    /// Header area (title + tabs)
    pub header: Rect,
    
    /// Main content area (log view)
    pub content: Rect,
    
    /// Status bar area
    pub status: Rect,
}

/// Layout mode based on terminal size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Very narrow terminal (< 60 cols)
    Compact,
    /// Standard terminal (60-80 cols)
    Standard,
    /// Comfortable width (80-120 cols)
    Comfortable,
    /// Wide terminal (> 120 cols)
    Wide,
}

impl LayoutMode {
    /// Determine layout mode from terminal width
    pub fn from_width(width: u16) -> Self {
        match width {
            0..=59 => LayoutMode::Compact,
            60..=79 => LayoutMode::Standard,
            80..=119 => LayoutMode::Comfortable,
            _ => LayoutMode::Wide,
        }
    }
}

/// Create the main layout areas
pub fn create(area: Rect) -> LayoutAreas {
    let mode = LayoutMode::from_width(area.width);
    
    // Header and status bar heights
    let header_height = header_height(mode);
    let status_height = status_height(mode);
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(3), // Content area
            Constraint::Length(status_height),
        ])
        .split(area);
    
    LayoutAreas {
        header: chunks[0],
        content: chunks[1],
        status: chunks[2],
    }
}

/// Get header height for layout mode
fn header_height(mode: LayoutMode) -> u16 {
    match mode {
        LayoutMode::Compact => 1,
        _ => 1,
    }
}

/// Get status bar height for layout mode
fn status_height(mode: LayoutMode) -> u16 {
    match mode {
        LayoutMode::Compact => 1,
        _ => 1,
    }
}

/// Check if compact status bar should be used
pub fn use_compact_status(area: Rect) -> bool {
    area.width < 60
}

/// Check if compact header should be used
pub fn use_compact_header(area: Rect) -> bool {
    area.width < 60
}

/// Get maximum tab count that fits in the header
pub fn max_visible_tabs(area: Rect) -> usize {
    let mode = LayoutMode::from_width(area.width);
    
    // Reserve space for "Flutter Demon" title and keybindings
    let reserved = 35; // ~15 for title, ~20 for keybindings
    let available = area.width.saturating_sub(reserved);
    
    // Each tab is approximately 15-20 chars
    let tab_width = match mode {
        LayoutMode::Compact => 10,
        LayoutMode::Standard => 14,
        LayoutMode::Comfortable => 16,
        LayoutMode::Wide => 20,
    };
    
    (available / tab_width).max(1) as usize
}

/// Get timestamp format for log entries based on width
pub fn timestamp_format(area: Rect) -> &'static str {
    let mode = LayoutMode::from_width(area.width);
    
    match mode {
        LayoutMode::Compact => "%H:%M",      // 12:34
        LayoutMode::Standard => "%H:%M:%S",   // 12:34:56
        LayoutMode::Comfortable => "%H:%M:%S", // 12:34:56
        LayoutMode::Wide => "%H:%M:%S%.3f",   // 12:34:56.789
    }
}

/// Get the width available for log messages
pub fn log_message_width(area: Rect, show_timestamps: bool) -> u16 {
    let timestamp_width = if show_timestamps {
        match LayoutMode::from_width(area.width) {
            LayoutMode::Compact => 7,    // "[12:34]"
            LayoutMode::Standard => 11,  // "[12:34:56] "
            _ => 15,                     // "[12:34:56.789] "
        }
    } else {
        0
    };
    
    let prefix_width = 3; // "● " or similar
    
    area.width
        .saturating_sub(timestamp_width)
        .saturating_sub(prefix_width)
        .saturating_sub(2) // margins
}

/// Layout for status bar content
#[derive(Debug, Clone, Copy)]
pub struct StatusBarLayout {
    /// Area for status indicator and device name
    pub device: Rect,
    /// Area for reload count
    pub reloads: Rect,
    /// Area for session timer
    pub timer: Rect,
}

/// Create status bar internal layout
pub fn create_status_bar_layout(area: Rect) -> StatusBarLayout {
    let mode = LayoutMode::from_width(area.width);
    
    match mode {
        LayoutMode::Compact => {
            // Just device name in compact mode
            StatusBarLayout {
                device: area,
                reloads: Rect::default(),
                timer: Rect::default(),
            }
        }
        LayoutMode::Standard => {
            let chunks = Layout::horizontal([
                Constraint::Min(20),      // Device
                Constraint::Length(15),   // Timer
            ])
            .split(area);
            
            StatusBarLayout {
                device: chunks[0],
                reloads: Rect::default(),
                timer: chunks[1],
            }
        }
        _ => {
            let chunks = Layout::horizontal([
                Constraint::Min(30),      // Device
                Constraint::Length(12),   // Reloads
                Constraint::Length(12),   // Timer
            ])
            .split(area);
            
            StatusBarLayout {
                device: chunks[0],
                reloads: chunks[1],
                timer: chunks[2],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_layout_mode_from_width() {
        assert_eq!(LayoutMode::from_width(40), LayoutMode::Compact);
        assert_eq!(LayoutMode::from_width(59), LayoutMode::Compact);
        assert_eq!(LayoutMode::from_width(60), LayoutMode::Standard);
        assert_eq!(LayoutMode::from_width(79), LayoutMode::Standard);
        assert_eq!(LayoutMode::from_width(80), LayoutMode::Comfortable);
        assert_eq!(LayoutMode::from_width(119), LayoutMode::Comfortable);
        assert_eq!(LayoutMode::from_width(120), LayoutMode::Wide);
        assert_eq!(LayoutMode::from_width(200), LayoutMode::Wide);
    }
    
    #[test]
    fn test_create_layout() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = create(area);
        
        assert_eq!(layout.header.height, 1);
        assert_eq!(layout.status.height, 1);
        assert!(layout.content.height >= 3);
        
        // Total height should match
        assert_eq!(
            layout.header.height + layout.content.height + layout.status.height,
            area.height
        );
    }
    
    #[test]
    fn test_max_visible_tabs() {
        assert!(max_visible_tabs(Rect::new(0, 0, 60, 24)) >= 1);
        assert!(max_visible_tabs(Rect::new(0, 0, 120, 24)) > max_visible_tabs(Rect::new(0, 0, 60, 24)));
    }
    
    #[test]
    fn test_timestamp_format() {
        assert_eq!(timestamp_format(Rect::new(0, 0, 50, 24)), "%H:%M");
        assert_eq!(timestamp_format(Rect::new(0, 0, 70, 24)), "%H:%M:%S");
        assert_eq!(timestamp_format(Rect::new(0, 0, 130, 24)), "%H:%M:%S%.3f");
    }
    
    #[test]
    fn test_log_message_width() {
        let area = Rect::new(0, 0, 80, 24);
        
        let with_timestamps = log_message_width(area, true);
        let without_timestamps = log_message_width(area, false);
        
        assert!(without_timestamps > with_timestamps);
        assert!(with_timestamps > 50);
    }
    
    #[test]
    fn test_status_bar_layout() {
        // Compact
        let compact = create_status_bar_layout(Rect::new(0, 0, 50, 1));
        assert!(compact.reloads.width == 0);
        assert!(compact.timer.width == 0);
        
        // Standard
        let standard = create_status_bar_layout(Rect::new(0, 0, 70, 1));
        assert!(standard.timer.width > 0);
        
        // Wide
        let wide = create_status_bar_layout(Rect::new(0, 0, 120, 1));
        assert!(wide.reloads.width > 0);
        assert!(wide.timer.width > 0);
    }
}
```

#### Updated Status Bar Widget

```rust
//! Status bar widget with session data

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::app::session::Session;
use crate::core::AppPhase;
use crate::tui::layout::{create_status_bar_layout, LayoutMode};

/// Status bar widget displaying current session info
pub struct StatusBar<'a> {
    session: Option<&'a Session>,
}

impl<'a> StatusBar<'a> {
    /// Create status bar from a session
    pub fn from_session(session: &'a Session) -> Self {
        Self {
            session: Some(session),
        }
    }
    
    /// Create empty status bar
    pub fn empty() -> Self {
        Self { session: None }
    }
    
    /// Get status indicator with color
    fn status_indicator(phase: &AppPhase) -> (char, Color) {
        match phase {
            AppPhase::Running => ('●', Color::Green),
            AppPhase::Reloading => ('↻', Color::Yellow),
            AppPhase::Initializing => ('○', Color::DarkGray),
            AppPhase::Stopped => ('○', Color::DarkGray),
            AppPhase::Quitting => ('✗', Color::Red),
        }
    }
    
    /// Get phase text
    fn phase_text(phase: &AppPhase) -> &'static str {
        match phase {
            AppPhase::Running => "Running",
            AppPhase::Reloading => "Reloading",
            AppPhase::Initializing => "Starting",
            AppPhase::Stopped => "Stopped",
            AppPhase::Quitting => "Quitting",
        }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = create_status_bar_layout(area);
        let mode = LayoutMode::from_width(area.width);
        
        match self.session {
            Some(session) => {
                let (indicator, color) = Self::status_indicator(&session.phase);
                
                // Device section
                let device_line = if mode == LayoutMode::Compact {
                    // Compact: just indicator and short name
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            indicator.to_string(),
                            Style::default().fg(color),
                        ),
                        Span::raw(" "),
                        Span::raw(truncate(&session.device_name, 20)),
                    ])
                } else {
                    // Standard+: full info
                    let emulator_type = if session.is_emulator {
                        match session.platform.as_str() {
                            p if p.starts_with("ios") => "(simulator)",
                            p if p.starts_with("android") => "(emulator)",
                            _ => "(virtual)",
                        }
                    } else {
                        "(physical)"
                    };
                    
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            indicator.to_string(),
                            Style::default().fg(color),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            Self::phase_text(&session.phase),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" on "),
                        Span::raw(&session.device_name),
                        Span::raw(" "),
                        Span::styled(
                            emulator_type,
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])
                };
                
                buf.set_line(layout.device.x, layout.device.y, &device_line, layout.device.width);
                
                // Reloads section (if space available)
                if layout.reloads.width > 0 && session.reload_count > 0 {
                    let reloads_line = Line::from(vec![
                        Span::raw("│ "),
                        Span::styled("Reloads:", Style::default().fg(Color::DarkGray)),
                        Span::raw(" "),
                        Span::raw(session.reload_count.to_string()),
                    ]);
                    buf.set_line(layout.reloads.x, layout.reloads.y, &reloads_line, layout.reloads.width);
                }
                
                // Timer section (if space available)
                if layout.timer.width > 0 {
                    if let Some(duration) = session.session_duration_display() {
                        let timer_line = Line::from(vec![
                            Span::raw("│ "),
                            Span::raw(duration),
                        ]);
                        buf.set_line(layout.timer.x, layout.timer.y, &timer_line, layout.timer.width);
                    }
                }
            }
            None => {
                // No session - show prompt
                let line = Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        "No active session",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(" - press "),
                    Span::styled("n", Style::default().fg(Color::Yellow)),
                    Span::raw(" to start"),
                ]);
                buf.set_line(area.x, area.y, &line, area.width);
            }
        }
    }
}

/// Compact status bar for narrow terminals
pub struct StatusBarCompact<'a> {
    session: Option<&'a Session>,
}

impl<'a> StatusBarCompact<'a> {
    pub fn from_session(session: &'a Session) -> Self {
        Self {
            session: Some(session),
        }
    }
    
    pub fn empty() -> Self {
        Self { session: None }
    }
}

impl Widget for StatusBarCompact<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if let Some(session) = self.session {
            let (indicator, color) = StatusBar::status_indicator(&session.phase);
            
            let line = Line::from(vec![
                Span::styled(
                    indicator.to_string(),
                    Style::default().fg(color),
                ),
                Span::raw(" "),
                Span::raw(truncate(&session.device_name, (area.width - 3) as usize)),
            ]);
            
            buf.set_line(area.x + 1, area.y, &line, area.width.saturating_sub(2));
        }
    }
}

/// Truncate string to max length with ellipsis
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_status_indicator() {
        let (icon, color) = StatusBar::status_indicator(&AppPhase::Running);
        assert_eq!(icon, '●');
        assert_eq!(color, Color::Green);
        
        let (icon, color) = StatusBar::status_indicator(&AppPhase::Reloading);
        assert_eq!(icon, '↻');
        assert_eq!(color, Color::Yellow);
    }
    
    #[test]
    fn test_truncate() {
        assert_eq!(truncate("Short", 10), "Short");
        assert_eq!(truncate("Very Long Name Here", 10), "Very Long…");
        assert_eq!(truncate("AB", 1), "…");
    }
}
```

---

### Visual Polish Details

1. **Consistent Spacing**
   - 2-space margin on left side of all content
   - 1-space padding between UI elements
   - Proper alignment of columns

2. **Color Scheme**
   - Cyan: App title, highlights
   - Green: Running/success states
   - Yellow: Warnings, reloading, keybindings
   - Red: Errors
   - DarkGray: Secondary text, separators

3. **Status Indicators**
   - `●` (filled circle): Running
   - `○` (empty circle): Stopped/Initializing
   - `↻` (reload): Reloading
   - `✗` (cross): Error/Quitting

4. **Borders**
   - Rounded corners for modals
   - Simple lines for main UI sections
   - Vertical bars `│` as separators

---

### Acceptance Criteria

1. [ ] `LayoutMode` enum correctly identifies terminal width ranges
2. [ ] Layout adapts to terminal resize events
3. [ ] Compact mode shows abbreviated content for narrow terminals
4. [ ] Wide mode uses extra space for additional information
5. [ ] Status bar shows device name, status, reload count, and timer
6. [ ] Timestamp format adapts to available width
7. [ ] Tab overflow is handled gracefully (truncation or scroll indicators)
8. [ ] Color scheme is consistent across all widgets
9. [ ] Status indicators use correct symbols and colors
10. [ ] Empty states show helpful prompts
11. [ ] All new code has unit tests
12. [ ] `cargo test` passes
13. [ ] `cargo clippy` has no warnings

---

### Testing

```rust
#[test]
fn test_layout_responsiveness() {
    use ratatui::{backend::TestBackend, Terminal};
    
    // Test different terminal widths
    for width in [40, 60, 80, 120, 160] {
        let backend = TestBackend::new(width, 24);
        let terminal = Terminal::new(backend).unwrap();
        
        let area = terminal.backend().size().unwrap();
        let layout = create(area);
        
        // Verify layout is valid
        assert!(layout.header.width == area.width);
        assert!(layout.content.width == area.width);
        assert!(layout.status.width == area.width);
        assert!(layout.content.height > 0);
    }
}

#[test]
fn test_status_bar_rendering() {
    use ratatui::{backend::TestBackend, Terminal};
    
    let mut session = Session::new(
        "device-123".to_string(),
        "iPhone 15 Pro".to_string(),
        "ios".to_string(),
        true,
    );
    session.mark_started("app-1".to_string());
    session.complete_reload();
    
    let backend = TestBackend::new(100, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    
    terminal.draw(|f| {
        let status = StatusBar::from_session(&session);
        f.render_widget(status, f.area());
    }).unwrap();
    
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    
    assert!(content.contains("Running"));
    assert!(content.contains("iPhone 15 Pro"));
    assert!(content.contains("simulator"));
    assert!(content.contains("Reloads:"));
}
```

---

### Notes

- Responsive design is critical for terminal applications as users have varying terminal sizes
- The layout should gracefully degrade rather than break in very small terminals
- Consider adding a minimum terminal size check with helpful error message
- Color support depends on terminal capabilities; consider fallback for basic terminals
- Unicode symbols (●, ○, ↻, ✗) should display correctly in most modern terminals

---

### Files to Create/Modify

| File | Action |
|------|--------|
| `src/tui/layout.rs` | Major update with layout modes and responsive calculations |
| `src/tui/widgets/status_bar.rs` | Update to use session data and responsive layout |
| `src/tui/widgets/header.rs` | Minor updates for consistency |
| `src/tui/render.rs` | Update to use new layout system |
## Task: LineGauge Progress Indicator for Device Selector

**Objective**: Replace the text spinner in the device selector loading state with an animated `LineGauge` widget, providing a more polished visual indication of device discovery progress.

**Depends on**: None (standalone UI improvement)

---

### Scope

- `src/tui/widgets/device_selector.rs`: Replace spinner with LineGauge

---

### Current State

```rust
// In src/tui/widgets/device_selector.rs

const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl DeviceSelectorState {
    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
    }

    pub fn spinner_char(&self) -> &'static str {
        SPINNER_FRAMES[self.animation_frame as usize % SPINNER_FRAMES.len()]
    }
}

// In Widget impl - render loading state
if self.state.loading {
    let spinner = self.state.spinner_char();
    let loading_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(spinner, Style::default().fg(Color::Cyan)),
            Span::styled(
                " Discovering devices...",
                Style::default().fg(Color::Yellow),
            ),
        ]),
    ];
    let loading = Paragraph::new(loading_text).alignment(Alignment::Center);
    loading.render(chunks[0], buf);
}
```

**Problem:** The spinner is a simple text animation. Task 09-refined-layout.md specifies using an animated LineGauge for a more polished look.

---

### Implementation Details

#### 1. Import LineGauge Widget

```rust
// In src/tui/widgets/device_selector.rs
use ratatui::{
    // ... existing imports ...
    widgets::{Block, Borders, Clear, LineGauge, List, ListItem, Paragraph, Widget},
};
```

#### 2. Calculate Indeterminate Progress

For an indeterminate progress indicator (we don't know how long discovery takes), we use a "bouncing" animation pattern:

```rust
impl DeviceSelectorState {
    /// Calculate indeterminate progress ratio (0.0 to 1.0)
    /// Creates a bouncing effect from left to right and back
    pub fn indeterminate_ratio(&self) -> f64 {
        // Complete cycle every 60 frames (about 1 second at 60fps)
        let cycle_length = 60;
        let position = self.animation_frame % cycle_length;
        
        // First half: 0.0 -> 1.0, Second half: 1.0 -> 0.0
        let half = cycle_length / 2;
        if position < half {
            position as f64 / half as f64
        } else {
            (cycle_length - position) as f64 / half as f64
        }
    }
    
    /// Calculate progress for a "moving window" effect
    /// Shows a small filled section that moves across the gauge
    pub fn sliding_window_ratio(&self) -> (f64, f64) {
        // Window width as fraction of total
        let window_width = 0.2;
        
        // Position cycles 0.0 -> 1.0 -> 0.0
        let base = self.indeterminate_ratio();
        
        // Start and end of the filled section
        let start = base * (1.0 - window_width);
        let end = start + window_width;
        
        (start, end)
    }
}
```

#### 3. Update Loading State Rendering

```rust
impl Widget for DeviceSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ... existing modal setup ...

        if self.state.loading {
            // Loading state with animated LineGauge
            let chunks = Layout::vertical([
                Constraint::Length(2), // Spacer
                Constraint::Length(1), // Text
                Constraint::Length(1), // Spacer  
                Constraint::Length(1), // Gauge
                Constraint::Min(0),    // Rest
            ])
            .split(inner);

            // "Discovering devices..." text
            let loading_text = Paragraph::new("Discovering devices...")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Yellow));
            loading_text.render(chunks[1], buf);

            // Animated LineGauge
            let ratio = self.state.indeterminate_ratio();
            
            // Create padded area for the gauge
            let gauge_area = Rect {
                x: chunks[3].x + 4,
                y: chunks[3].y,
                width: chunks[3].width.saturating_sub(8),
                height: 1,
            };

            let gauge = LineGauge::default()
                .ratio(ratio)
                .filled_style(Style::default().fg(Color::Cyan))
                .unfilled_style(Style::default().fg(Color::DarkGray))
                .line_set(symbols::line::THICK);

            gauge.render(gauge_area, buf);
        }
        
        // ... rest of rendering ...
    }
}
```

#### 4. Alternative: Custom Sliding Window LineGauge

For a more sophisticated "scanning" effect:

```rust
fn render_sliding_gauge(state: &DeviceSelectorState, area: Rect, buf: &mut Buffer) {
    // Create a custom sliding window effect
    let width = area.width as usize;
    let window_width = (width as f64 * 0.3) as usize; // 30% window
    
    // Calculate window position
    let cycle_length = 80;
    let frame = state.animation_frame as usize % cycle_length;
    let position = if frame < cycle_length / 2 {
        // Moving right
        (frame as f64 / (cycle_length / 2) as f64 * (width - window_width) as f64) as usize
    } else {
        // Moving left
        let reverse_frame = cycle_length - frame;
        (reverse_frame as f64 / (cycle_length / 2) as f64 * (width - window_width) as f64) as usize
    };
    
    // Build the gauge string manually
    let mut gauge_str = String::with_capacity(width);
    for i in 0..width {
        if i >= position && i < position + window_width {
            gauge_str.push('━'); // Filled
        } else {
            gauge_str.push('─'); // Unfilled
        }
    }
    
    let line = Line::from(vec![
        Span::styled(
            &gauge_str[..position.min(width)],
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            &gauge_str[position..position.saturating_add(window_width).min(width)],
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(
            &gauge_str[position.saturating_add(window_width).min(width)..],
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    
    buf.set_line(area.x, area.y, &line, area.width);
}
```

#### 5. Update Tick Handling

Ensure `tick()` is called regularly for smooth animation:

```rust
// In the main event loop (tui/event.rs or similar)
// The tick should happen at ~60fps for smooth animation

// In tui/mod.rs - run_loop
while !state.should_quit() {
    // ... existing message/event handling ...
    
    // Tick animation state
    if state.ui_mode == UiMode::DeviceSelector || state.ui_mode == UiMode::Loading {
        state.device_selector.tick();
    }
    
    // Render
    terminal.draw(|frame| render::view(frame, state))?;
}
```

---

### Visual Design

```
┌─────────────────────────────────────────────┐
│           Select Target Device              │
├─────────────────────────────────────────────┤
│                                             │
│          Discovering devices...             │
│                                             │
│      ────────━━━━━━━━━━━────────           │
│                                             │
│                                             │
├─────────────────────────────────────────────┤
│   ↑↓ Navigate  Enter Select  Esc Cancel    │
└─────────────────────────────────────────────┘

Animation: The cyan "━━━━" section slides left-right-left
```

### Animation Timing

| Parameter | Value | Notes |
|-----------|-------|-------|
| Cycle length | 60-80 frames | ~1-1.3 seconds per full cycle |
| Window width | 20-30% | Visible sliding section |
| Tick rate | ~60fps | Match terminal refresh rate |

---

### Acceptance Criteria

1. [ ] LineGauge widget used instead of text spinner
2. [ ] Animation shows smooth left-to-right-to-left motion
3. [ ] Cyan filled section on dark gray unfilled background
4. [ ] "Discovering devices..." text displayed above gauge
5. [ ] Animation frame advances with each tick
6. [ ] Gauge properly centered in modal
7. [ ] No performance issues from animation

---

### Testing

```rust
#[test]
fn test_indeterminate_ratio_bounds() {
    let mut state = DeviceSelectorState::new();
    
    // Test many frames
    for _ in 0..200 {
        state.tick();
        let ratio = state.indeterminate_ratio();
        
        // Ratio should always be 0.0 to 1.0
        assert!(ratio >= 0.0);
        assert!(ratio <= 1.0);
    }
}

#[test]
fn test_indeterminate_ratio_oscillates() {
    let mut state = DeviceSelectorState::new();
    
    let mut ratios = Vec::new();
    for _ in 0..60 {
        state.tick();
        ratios.push(state.indeterminate_ratio());
    }
    
    // Should have both increasing and decreasing sections
    let has_increase = ratios.windows(2).any(|w| w[1] > w[0]);
    let has_decrease = ratios.windows(2).any(|w| w[1] < w[0]);
    
    assert!(has_increase);
    assert!(has_decrease);
}

#[test]
fn test_loading_with_linegauge_renders() {
    use ratatui::{backend::TestBackend, Terminal};
    
    let mut state = DeviceSelectorState::new();
    state.show_loading();
    
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    
    terminal
        .draw(|f| {
            let selector = DeviceSelector::new(&state);
            f.render_widget(selector, f.area());
        })
        .unwrap();
    
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    
    // Should show the text
    assert!(content.contains("Discovering"));
    
    // Should have gauge characters (thick horizontal lines)
    assert!(content.contains('━') || content.contains('─'));
}

#[test]
fn test_gauge_area_calculation() {
    let inner = Rect::new(5, 5, 60, 10);
    
    // Gauge should be horizontally padded
    let gauge_area = Rect {
        x: inner.x + 4,
        y: inner.y + 3,
        width: inner.width.saturating_sub(8),
        height: 1,
    };
    
    assert_eq!(gauge_area.x, 9);
    assert_eq!(gauge_area.width, 52);
}

#[test]
fn test_animation_smooth_at_boundaries() {
    let mut state = DeviceSelectorState::new();
    
    // Test frame wraparound
    state.animation_frame = u8::MAX - 1;
    state.tick();
    assert_eq!(state.animation_frame, u8::MAX);
    
    state.tick();
    assert_eq!(state.animation_frame, 0); // Wrapped
    
    // Ratio should still be valid
    let ratio = state.indeterminate_ratio();
    assert!(ratio >= 0.0 && ratio <= 1.0);
}
```

---

### Implementation Notes

1. **LineGauge vs Custom**: LineGauge is simpler but less flexible. For a sliding window effect, custom rendering may be needed.

2. **Line Sets**: Use `symbols::line::THICK` for bold lines or `symbols::line::NORMAL` for thinner appearance.

3. **Performance**: Animation runs during loading only - no impact on normal operation.

4. **Fallback**: If terminal doesn't support unicode, LineGauge degrades gracefully.

5. **Color Choices**:
   - Cyan: Active/filled (matches header color scheme)
   - DarkGray: Unfilled (subtle background)

6. **Tick Frequency**: The tick happens in the main event loop. If using event polling with timeout, the animation rate depends on the poll timeout.
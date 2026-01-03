## Task: LineGauge Progress Indicator for Device Selector

**Objective**: Replace the text spinner in the device selector loading state with an animated `LineGauge` widget, fix footer visibility, and conditionally display the Esc keybinding based on whether sessions are running.

**Depends on**: None (standalone UI improvement)

---

### Scope

- `src/tui/widgets/device_selector.rs`: Replace spinner with LineGauge, fix footer, add conditional Esc

---

### Current Issues

#### Issue 1: Text Spinner Instead of LineGauge
```rust
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
```
**Problem:** The spinner is a simple text animation. Task 09-refined-layout.md specifies using an animated LineGauge for a more polished look.

#### Issue 2: Footer Not Visible
```rust
// Footer with keybindings
let footer = Paragraph::new("↑↓ Navigate  Enter Select  Esc Cancel  r Refresh")
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::DarkGray));  // <-- DarkGray on DarkGray background!
footer.render(chunks[1], buf);
```
**Problem:** The footer text uses `Color::DarkGray` but the modal background is also `Color::DarkGray`, making the footer invisible.

#### Issue 3: Esc Shows When It Does Nothing
```rust
// In handler.rs - Esc only works when sessions exist
Message::HideDeviceSelector => {
    if state.session_manager.has_running_sessions() {
        state.device_selector.hide();
        state.ui_mode = UiMode::Normal;
    }
    UpdateResult::none()
}
```
**Problem:** The footer always shows "Esc Cancel" even on startup when there are no sessions and Esc does nothing. This is confusing UX.

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
}
```

#### 3. Update DeviceSelector Widget to Accept Session State

```rust
/// Device selector modal widget
pub struct DeviceSelector<'a> {
    state: &'a DeviceSelectorState,
    /// Whether there are running sessions (affects Esc behavior)
    has_running_sessions: bool,
}

impl<'a> DeviceSelector<'a> {
    /// Create a new device selector widget
    pub fn new(state: &'a DeviceSelectorState) -> Self {
        Self { 
            state,
            has_running_sessions: false,  // Default for backward compatibility
        }
    }

    /// Create with session awareness for conditional Esc display
    pub fn with_session_state(state: &'a DeviceSelectorState, has_running_sessions: bool) -> Self {
        Self {
            state,
            has_running_sessions,
        }
    }
}
```

#### 4. Update render.rs to Pass Session State

```rust
// In src/tui/render.rs
UiMode::DeviceSelector | UiMode::Loading => {
    let has_sessions = state.session_manager.has_running_sessions();
    let selector = widgets::DeviceSelector::with_session_state(
        &state.device_selector,
        has_sessions,
    );
    frame.render_widget(selector, area);
}
```

#### 5. Fix Footer Visibility and Conditional Esc

```rust
impl Widget for DeviceSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ... existing modal setup ...

        // Build footer text conditionally
        let footer_text = if self.has_running_sessions {
            "↑↓ Navigate  Enter Select  Esc Cancel  r Refresh"
        } else {
            "↑↓ Navigate  Enter Select  r Refresh"
        };

        // Footer with keybindings - use visible color!
        let footer = Paragraph::new(footer_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));  // Gray on DarkGray = visible!
        footer.render(chunks[1], buf);
    }
}
```

#### 6. Update Loading State with LineGauge

```rust
impl Widget for DeviceSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ... existing modal setup ...

        if self.state.loading {
            // Loading state with animated LineGauge
            let loading_chunks = Layout::vertical([
                Constraint::Length(2), // Spacer
                Constraint::Length(1), // Text
                Constraint::Length(1), // Spacer  
                Constraint::Length(1), // Gauge
                Constraint::Min(0),    // Rest
            ])
            .split(chunks[0]);

            // "Discovering devices..." text
            let loading_text = Paragraph::new("Discovering devices...")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Yellow));
            loading_text.render(loading_chunks[1], buf);

            // Animated LineGauge
            let ratio = self.state.indeterminate_ratio();
            
            // Create padded area for the gauge
            let gauge_area = Rect {
                x: loading_chunks[3].x + 4,
                y: loading_chunks[3].y,
                width: loading_chunks[3].width.saturating_sub(8),
                height: 1,
            };

            let gauge = LineGauge::default()
                .ratio(ratio)
                .filled_style(Style::default().fg(Color::Cyan))
                .unfilled_style(Style::default().fg(Color::Black))
                .line_set(symbols::line::THICK);

            gauge.render(gauge_area, buf);
        }
        
        // ... rest of rendering (error, empty, device list) ...
    }
}
```

---

### Visual Design

#### Loading State (Startup - No Sessions)
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
│      ↑↓ Navigate  Enter Select  r Refresh  │
└─────────────────────────────────────────────┘
Note: No "Esc Cancel" shown - there's nothing to cancel to
```

#### Loading State (Has Running Sessions)
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
│  ↑↓ Navigate  Enter Select  Esc Cancel  r  │
└─────────────────────────────────────────────┘
Note: "Esc Cancel" shown - user can return to normal mode
```

Animation: The cyan "━━━━" section slides left-right-left

### Animation Timing

| Parameter | Value | Notes |
|-----------|-------|-------|
| Cycle length | 60 frames | ~1 second per full cycle |
| Tick rate | ~60fps | Match terminal refresh rate |

---

### Acceptance Criteria

1. [ ] LineGauge widget used instead of text spinner
2. [ ] Animation shows smooth left-to-right-to-left motion
3. [ ] Cyan filled section on dark/black unfilled background
4. [ ] "Discovering devices..." text displayed above gauge
5. [ ] Animation frame advances with each tick
6. [ ] Gauge properly centered in modal
7. [ ] **Footer text is visible** (not DarkGray on DarkGray)
8. [ ] **"Esc Cancel" only shown when sessions are running**
9. [ ] DeviceSelector widget accepts session state parameter
10. [ ] render.rs updated to pass session state

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
fn test_device_selector_with_session_state() {
    let state = DeviceSelectorState::new();
    
    // Without sessions
    let selector = DeviceSelector::with_session_state(&state, false);
    assert!(!selector.has_running_sessions);
    
    // With sessions
    let selector = DeviceSelector::with_session_state(&state, true);
    assert!(selector.has_running_sessions);
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
fn test_footer_shows_esc_only_with_sessions() {
    use ratatui::{backend::TestBackend, Terminal};
    
    let mut state = DeviceSelectorState::new();
    state.set_devices(vec![]); // Not loading, show footer
    
    // Without sessions - no Esc
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| {
        let selector = DeviceSelector::with_session_state(&state, false);
        f.render_widget(selector, f.area());
    }).unwrap();
    
    let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
    assert!(!content.contains("Esc Cancel"));
    assert!(content.contains("Navigate"));
    
    // With sessions - shows Esc
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| {
        let selector = DeviceSelector::with_session_state(&state, true);
        f.render_widget(selector, f.area());
    }).unwrap();
    
    let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("Esc Cancel"));
}
```

---

### Implementation Notes

1. **Footer Color**: Changed from `Color::DarkGray` to `Color::Gray` for visibility against DarkGray background.

2. **Session State**: The DeviceSelector now needs to know about session state. Using a builder pattern (`with_session_state`) maintains backward compatibility.

3. **LineGauge vs Custom**: LineGauge is simpler but less flexible. The bouncing animation is a common UX pattern for indeterminate progress.

4. **Line Sets**: Use `symbols::line::THICK` for bold lines.

5. **Color Choices**:
   - Cyan: Active/filled (matches header color scheme)
   - Black: Unfilled (subtle background, visible contrast)
   - Gray: Footer text (visible on DarkGray)

6. **Esc UX**: The handler already correctly ignores Esc when no sessions exist. This task just improves the footer to not mislead users.

---

## Completion Summary

**Status:** ✅ Done

### Files Modified

- `src/tui/widgets/device_selector.rs` - Added LineGauge import, `indeterminate_ratio()` method, `has_running_sessions` field, `with_session_state()` constructor, updated loading render to use LineGauge, fixed footer color to Gray, added conditional Esc display, added 5 new tests
- `src/tui/render.rs` - Updated to pass session state via `with_session_state()` for DeviceSelector and EmulatorSelector modes
- `src/app/handler.rs` - Updated `Message::Tick` handler to call `device_selector.tick()` when visible and loading, added 3 new tests
- `src/tui/event.rs` - Changed poll timeout to generate `Message::Tick` instead of `None` for animation updates

### Notable Decisions/Tradeoffs

1. **LineGauge symbols**: Used `filled_symbol()` and `unfilled_symbol()` instead of deprecated `line_set()` method
2. **Bouncing animation**: 60-frame cycle (1 second at 60fps) creates smooth oscillation between 0.0 and 1.0 ratio
3. **Color choices**: Cyan filled / Black unfilled for gauge; Gray footer text on DarkGray background for visibility
4. **Backward compatibility**: `DeviceSelector::new()` preserved with default `has_running_sessions: false`

### Testing Performed

```
cargo check - PASS (no warnings)
cargo test --lib device_selector - PASS (30 tests)
cargo test --lib - PASS (440 tests)
cargo clippy - PASS (only pre-existing warning in tui/mod.rs:390)
cargo fmt - PASS
```

### New Tests Added

In `device_selector.rs`:
- `test_indeterminate_ratio_bounds` - Verifies ratio stays in 0.0-1.0 range
- `test_indeterminate_ratio_oscillates` - Verifies bouncing behavior
- `test_device_selector_with_session_state` - Verifies constructor sets field correctly
- `test_device_selector_render_loading_with_linegauge` - Verifies gauge characters render
- `test_footer_shows_esc_only_with_sessions` - Verifies conditional Esc display

In `handler.rs`:
- `test_tick_advances_device_selector_animation` - Verifies tick advances animation when loading
- `test_tick_does_not_advance_when_not_loading` - Verifies tick is no-op when not loading
- `test_tick_does_not_advance_when_hidden` - Verifies tick is no-op when hidden

### Risks/Limitations

- Animation speed tied to tick rate; if tick rate changes, gauge animation speed changes
- LineGauge requires 1 row height minimum; gauge won't render if layout is too constrained
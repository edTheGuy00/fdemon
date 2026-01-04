## Task: Persistent Session Header

**Objective**: Make the session header row always visible when at least one session exists, displaying device name with status icon instead of only showing tabs when multiple sessions are running.

**Depends on**: None

---

### Scope

#### `src/tui/layout.rs`
- Change `show_tabs = session_count > 1` to `show_tabs = session_count >= 1`
- Update `header_height()` function to return 4 for `session_count >= 1` (not just > 1)
- Update tests to reflect new behavior

#### `src/tui/widgets/tabs.rs`
- Modify `SessionTabs` widget to handle single-session case:
  - When 1 session: render device name with status icon (simplified row, no tab styling)
  - When >1 sessions: render tabs as before
- Keep existing `render_single_session_header()` logic but adapt for standalone tabs area
- Ensure single session row shows: `● Device Name` format with appropriate status icon

#### `src/tui/render.rs`
- Update tabs rendering logic:
  - Current: `if let Some(tabs_area) = areas.tabs { ... }`
  - This will now fire for single sessions too (no code change needed if layout returns tabs area)
- Verify tabs render correctly for both 1 and >1 sessions

---

### Implementation Details

**layout.rs changes:**

```rust
// Before
let show_tabs = session_count > 1;

// After  
let show_tabs = session_count >= 1;
```

**tabs.rs changes:**

The `SessionTabs` widget render should check session count:

```rust
impl Widget for SessionTabs<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.session_manager.is_empty() {
            return;
        }

        if self.session_manager.len() == 1 {
            // Render simplified single-session header
            self.render_single_session(area, buf);
        } else {
            // Render full tabs UI
            self.render_tabs(area, buf);
        }
    }
}
```

Add helper method for single session rendering:
- Display: `● iPhone 15 Pro` (status icon + device name)
- Left-aligned with padding
- Use session.phase for icon color

---

### Acceptance Criteria

1. ✅ Zero sessions: No subheader row visible
2. ✅ One session: Subheader row shows `● Device Name` with status icon
3. ✅ Multiple sessions: Subheader row shows tabs with all sessions
4. ✅ Status icon updates correctly (○ initializing, ● running, ↻ reloading)
5. ✅ Layout heights correct: 3 lines header when 0 sessions, 4 lines when >= 1
6. ✅ All existing tests pass
7. ✅ New tests cover single-session header rendering

---

### Testing

#### Unit Tests

```rust
#[test]
fn test_layout_shows_tabs_for_single_session() {
    let area = Rect::new(0, 0, 80, 24);
    let layout = create_with_sessions(area, 1);
    
    assert!(layout.tabs.is_some());
    assert_eq!(layout.tabs.unwrap().height, 1);
}

#[test]
fn test_session_tabs_renders_single_session() {
    let mut manager = SessionManager::new();
    manager.create_session(&test_device("d1", "iPhone 15")).unwrap();
    
    let backend = TestBackend::new(80, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    
    terminal.draw(|f| {
        let tabs = SessionTabs::new(&manager);
        f.render_widget(tabs, f.area());
    }).unwrap();
    
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    
    assert!(content.contains("iPhone 15"));
    assert!(content.contains("○")); // Initializing icon
}
```

#### Manual Testing

1. Start fdemon in a project directory
2. Select one device → verify header shows device name
3. Add second device → verify tabs appear
4. Close one device → verify single device header returns
5. Close all devices → verify no subheader

---

### Notes

- The existing `render_single_session_header()` function in tabs.rs is designed for use in the main header, not the tabs area. We need a new simpler variant that just shows device info without keybinding hints (those stay in the main header).
- Consider using the same truncation logic for long device names in single-session mode.

---

## Completion Summary

**Status:** ✅ Done

### Files Modified
- `src/tui/layout.rs` - Changed `show_tabs` condition from `> 1` to `>= 1`, updated `header_height()` to return 4 for single sessions
- `src/tui/widgets/tabs.rs` - Added `render_single_session()` and `render_tabs()` helper methods to `SessionTabs` widget, modified `Widget::render` to dispatch based on session count

### Notable Decisions/Tradeoffs
- Single-session subheader uses simplified layout: `● Device Name` format without tab styling
- Reused existing `truncate_name()` function for consistent device name truncation
- Device name truncation adapts to available terminal width (max 8 chars minimum)
- Status icon colors match existing tab icon colors for consistency

### Testing Performed
- `cargo check` - Compilation successful
- `cargo test` - All 447 tests passed
- `cargo fmt` - Code formatted correctly
- `cargo clippy` - No warnings

### New Tests Added
- `test_session_tabs_single_session_renders_device_name` - Verifies single session shows device name with status icon
- `test_session_tabs_single_session_running_status` - Verifies running session shows correct icon

### Updated Tests
- `test_create_layout_single_session` - Updated to expect `tabs.is_some()` for single session
- `test_header_height` - Updated to expect height 4 for single session
- `test_layout_areas_sum_to_total` - Updated to include tabs height for single session case

### Risks/Limitations
- None identified - this is a pure UI change with no business logic impact
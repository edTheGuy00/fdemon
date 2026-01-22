## Task: Add Responsive Mode Labels

**Objective**: Make mode button labels (Debug/Profile/Release) responsive to available width, showing full labels when space allows instead of always using abbreviated text.

**Depends on**: None

**Bug Reference**: Bug 4 - Portrait Layout Content Not Using Full Width

### Scope

- `src/tui/widgets/new_session_dialog/launch_context.rs`: Modify `render_mode_inline()` to check width and use appropriate labels

### Details

**Current State:**

`render_mode_inline()` uses hardcoded abbreviated labels regardless of available space:

```rust
// launch_context.rs:830-869
let mode_str = vec![
    Span::styled("  Mode: ", style_label),
    Span::styled(
        if self.state.mode == FlutterMode::Debug {
            "(●)Dbg"   // Always abbreviated
        } else {
            "(○)Dbg"
        },
        // ...
    ),
    // ... "(●)Prof", "(●)Rel"
];
```

**Width Analysis:**

| Label Type | Debug | Profile | Release | Total (with spacing) |
|------------|-------|---------|---------|---------------------|
| Abbreviated | `(●)Dbg` (6) | `(●)Prof` (7) | `(●)Rel` (6) | ~24 chars |
| Full | `(●) Debug` (10) | `(●) Profile` (13) | `(●) Release` (12) | ~42 chars |

Portrait mode width range: 40-69 columns
Dialog width: 90% of terminal = 36-62 columns

**Threshold:** Use full labels when `area.width >= 48` (gives ~6 char buffer)

**Implementation:**

```rust
fn render_mode_inline(&self, area: Rect, buf: &mut Buffer) {
    let style_label = Style::default().fg(Color::DarkGray);

    // Determine if we have space for full labels
    // Full labels need ~42 chars, abbreviated need ~24 chars
    // Add buffer for "Mode: " prefix (8 chars) and margins
    let use_full_labels = area.width >= 48;

    let (debug_label, profile_label, release_label) = if use_full_labels {
        ("Debug", "Profile", "Release")
    } else {
        ("Dbg", "Prof", "Rel")
    };

    let debug_style = if self.state.mode == FlutterMode::Debug {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let profile_style = if self.state.mode == FlutterMode::Profile {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let release_style = if self.state.mode == FlutterMode::Release {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mode_indicator = |mode: FlutterMode| -> &'static str {
        if self.state.mode == mode { "(●) " } else { "(○) " }
    };

    let mode_str = vec![
        Span::styled("  Mode: ", style_label),
        Span::styled(
            format!("{}{}", mode_indicator(FlutterMode::Debug), debug_label),
            debug_style,
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{}{}", mode_indicator(FlutterMode::Profile), profile_label),
            profile_style,
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{}{}", mode_indicator(FlutterMode::Release), release_label),
            release_style,
        ),
    ];

    let line = Line::from(mode_str);
    buf.set_line(area.x, area.y, &line, area.width);
}
```

**Key Files to Reference:**
- `src/tui/widgets/new_session_dialog/launch_context.rs:806-874` - `render_mode_inline()` to modify
- `src/tui/widgets/new_session_dialog/launch_context.rs:188-224` - `ModeSelector` widget (full mode reference)
- `src/tui/widgets/new_session_dialog/mod.rs:121-135` - Layout thresholds for reference

### Alternative Approaches

**Option A: Fixed threshold (Recommended)**
- Simple: `if area.width >= 48 { full } else { abbreviated }`
- Predictable behavior
- Easy to test

**Option B: Calculate exact fit**
- Measure actual label widths at runtime
- More complex but handles edge cases
- May be overkill for this use case

**Option C: Progressive abbreviation**
- Width >= 48: "Debug", "Profile", "Release"
- Width >= 38: "Debug", "Prof", "Rel" (only abbreviate longest)
- Width < 38: "Dbg", "Prof", "Rel"
- More granular but more complex

**Recommendation:** Use Option A (fixed threshold) for simplicity.

### Acceptance Criteria

1. Mode labels show "Debug", "Profile", "Release" when `area.width >= 48`
2. Mode labels show "Dbg", "Prof", "Rel" when `area.width < 48`
3. Selected mode is still highlighted with correct color (Green/Yellow/Red)
4. Radio button indicators (●/○) remain visible
5. Labels don't overflow or wrap
6. No visual regression in horizontal layout (uses `ModeSelector`, not `render_mode_inline`)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn test_mode_inline_full_labels_wide_area() {
        let mut state = LaunchContextState::default();
        state.mode = FlutterMode::Debug;

        let widget = LaunchContextWithDevice::new(&state, None, false).compact(true);

        // Wide area (>= 48)
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);

        // Call render_mode_inline directly or through render_compact
        widget.render_mode_inline(area, &mut buf);

        let content = buffer_to_string(&buf);
        assert!(content.contains("Debug"), "Should show full 'Debug' label");
        assert!(content.contains("Profile"), "Should show full 'Profile' label");
        assert!(content.contains("Release"), "Should show full 'Release' label");
    }

    #[test]
    fn test_mode_inline_abbreviated_labels_narrow_area() {
        let mut state = LaunchContextState::default();
        state.mode = FlutterMode::Debug;

        let widget = LaunchContextWithDevice::new(&state, None, false).compact(true);

        // Narrow area (< 48)
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);

        widget.render_mode_inline(area, &mut buf);

        let content = buffer_to_string(&buf);
        assert!(content.contains("Dbg"), "Should show abbreviated 'Dbg' label");
        assert!(content.contains("Prof"), "Should show abbreviated 'Prof' label");
        assert!(content.contains("Rel"), "Should show abbreviated 'Rel' label");
        assert!(!content.contains("Debug"), "Should NOT show full 'Debug' label");
    }

    #[test]
    fn test_mode_inline_threshold_boundary() {
        let mut state = LaunchContextState::default();

        // Exactly at threshold (48)
        let area_at_threshold = Rect::new(0, 0, 48, 1);
        let mut buf = Buffer::empty(area_at_threshold);

        let widget = LaunchContextWithDevice::new(&state, None, false).compact(true);
        widget.render_mode_inline(area_at_threshold, &mut buf);

        let content = buffer_to_string(&buf);
        assert!(content.contains("Debug"), "At threshold should use full labels");

        // Just below threshold (47)
        let area_below = Rect::new(0, 0, 47, 1);
        let mut buf = Buffer::empty(area_below);
        widget.render_mode_inline(area_below, &mut buf);

        let content = buffer_to_string(&buf);
        assert!(content.contains("Dbg"), "Below threshold should use abbreviated labels");
    }

    fn buffer_to_string(buf: &Buffer) -> String {
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push(buf.get(x, y).symbol().chars().next().unwrap_or(' '));
            }
        }
        s
    }
}
```

### Notes

- The `render_mode_inline()` method is ONLY used in compact/portrait mode
- Horizontal layout uses the separate `ModeSelector` widget which already shows full labels
- The `area` parameter is passed to `render_mode_inline()` but wasn't being used for width decisions
- Consider extracting the threshold as a constant: `const MODE_FULL_LABEL_MIN_WIDTH: u16 = 48;`
- If Task 01 adds borders, the available width will be reduced by 2 - may need to adjust threshold

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (to be filled after implementation)

**Implementation Details:**

(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` -
- `cargo check` -
- `cargo clippy` -
- `cargo test` -

**Notable Decisions:**
- (to be filled after implementation)

**Risks/Limitations:**
- (to be filled after implementation)

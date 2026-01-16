# Task 05: Add Layout Mode Detection

## Objective

Add layout mode enum and detection logic to determine whether to use horizontal or vertical layout based on terminal size.

## Priority

**Medium** - Enables responsive layout for narrow terminals

## Problem

Current code has a binary check: fits (80x24) or doesn't fit. No intermediate states for responsive layout.

```rust
// Current: src/tui/widgets/new_session_dialog/mod.rs:156-158
fn fits_in_area(area: Rect) -> bool {
    area.width >= MIN_WIDTH && area.height >= MIN_HEIGHT
}
```

## Solution

### Step 1: Define Layout Mode Enum

**File:** `src/tui/widgets/new_session_dialog/mod.rs`

Add layout mode enum:

```rust
/// Layout mode for NewSessionDialog based on terminal size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Two-pane horizontal layout (Target Selector | Launch Context)
    /// Requires width >= 70
    Horizontal,

    /// Stacked vertical layout (Target Selector above Launch Context)
    /// For narrow terminals (width 40-69)
    Vertical,

    /// Terminal too small to render dialog meaningfully
    /// Below 40x20
    TooSmall,
}
```

### Step 2: Define Size Thresholds

Update constants:

```rust
// Minimum for horizontal (two-pane) layout
const MIN_HORIZONTAL_WIDTH: u16 = 70;
const MIN_HORIZONTAL_HEIGHT: u16 = 20;

// Minimum for vertical (stacked) layout
const MIN_VERTICAL_WIDTH: u16 = 40;
const MIN_VERTICAL_HEIGHT: u16 = 20;

// Absolute minimum (show "too small" message)
const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 20;
```

### Step 3: Add Layout Mode Detection

```rust
impl NewSessionDialog<'_> {
    /// Determine the appropriate layout mode for the given area
    pub fn layout_mode(area: Rect) -> LayoutMode {
        if area.width >= MIN_HORIZONTAL_WIDTH && area.height >= MIN_HORIZONTAL_HEIGHT {
            LayoutMode::Horizontal
        } else if area.width >= MIN_VERTICAL_WIDTH && area.height >= MIN_VERTICAL_HEIGHT {
            LayoutMode::Vertical
        } else {
            LayoutMode::TooSmall
        }
    }

    /// Check if area supports at least vertical layout
    pub fn fits_in_area(area: Rect) -> bool {
        Self::layout_mode(area) != LayoutMode::TooSmall
    }
}
```

### Step 4: Update Render Entry Point

Modify the `Widget::render` implementation to use layout mode:

```rust
impl Widget for NewSessionDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match Self::layout_mode(area) {
            LayoutMode::TooSmall => {
                Self::render_too_small(area, buf);
            }
            LayoutMode::Horizontal => {
                self.render_horizontal(area, buf);
            }
            LayoutMode::Vertical => {
                self.render_vertical(area, buf);
            }
        }
    }
}
```

### Step 5: Refactor Existing Render to render_horizontal

Move current rendering logic to `render_horizontal`:

```rust
impl NewSessionDialog<'_> {
    /// Render horizontal (two-pane) layout
    fn render_horizontal(&self, area: Rect, buf: &mut Buffer) {
        // Clear background
        Clear.render(area, buf);

        // Calculate dialog bounds (80% width, 70% height, centered)
        let dialog_area = centered_rect(80, 70, area);

        // ... existing two-pane layout code ...
    }
}
```

### Step 6: Add Stub for Vertical Layout

Add placeholder for Task 06:

```rust
impl NewSessionDialog<'_> {
    /// Render vertical (stacked) layout
    fn render_vertical(&self, area: Rect, buf: &mut Buffer) {
        // TODO: Implement in Task 06
        // For now, fall back to horizontal with reduced margins
        self.render_horizontal(area, buf);
    }
}
```

### Step 7: Update "Too Small" Message

Update the error message with new thresholds:

```rust
fn render_too_small(area: Rect, buf: &mut Buffer) {
    Clear.render(area, buf);

    let message = format!(
        "Terminal too small. Need at least {}x{} (current: {}x{})",
        MIN_WIDTH, MIN_HEIGHT, area.width, area.height
    );

    let paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::Red))
        .alignment(Alignment::Center);

    // Center vertically
    let y = area.height / 2;
    let text_area = Rect::new(area.x, area.y + y, area.width, 1);
    paragraph.render(text_area, buf);
}
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/mod.rs` | Add `LayoutMode` enum, detection function, refactor render |

## Acceptance Criteria

1. `LayoutMode` enum with `Horizontal`, `Vertical`, `TooSmall` variants
2. `layout_mode(area)` function correctly detects mode:
   - width >= 70, height >= 20 → Horizontal
   - width 40-69, height >= 20 → Vertical
   - below 40x20 → TooSmall
3. Existing horizontal layout still works
4. "Too small" message shows for terminals < 40x20
5. `cargo check` passes

## Testing

```bash
cargo check
cargo test layout_mode
cargo test new_session_dialog
```

Add unit tests:

```rust
#[test]
fn test_layout_mode_horizontal() {
    let area = Rect::new(0, 0, 100, 40);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Horizontal);
}

#[test]
fn test_layout_mode_vertical() {
    let area = Rect::new(0, 0, 50, 30);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Vertical);
}

#[test]
fn test_layout_mode_too_small() {
    let area = Rect::new(0, 0, 30, 15);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::TooSmall);
}

#[test]
fn test_layout_mode_boundary_horizontal() {
    let area = Rect::new(0, 0, 70, 20);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Horizontal);
}

#[test]
fn test_layout_mode_boundary_vertical() {
    let area = Rect::new(0, 0, 69, 20);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Vertical);
}
```

## Notes

- This task sets up the infrastructure - Task 06 implements the vertical layout
- The vertical layout stub falls back to horizontal temporarily
- Thresholds (70, 40) can be adjusted based on testing

---

## Completion Summary

**Status:** Not Started

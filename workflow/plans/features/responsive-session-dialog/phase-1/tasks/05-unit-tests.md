## Task: Unit Tests for Height-Based Compact/Expanded Decisions

**Objective**: Add comprehensive unit tests verifying that compact/expanded mode selection is driven by available height rather than layout orientation. Test boundary conditions, hysteresis thresholds, and both layout paths.

**Depends on**: 03-horizontal-height-decision, 04-vertical-height-decision

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`: Add tests to existing `#[cfg(test)] mod tests` block

### Details

Use the `TestTerminal::with_size(w, h)` helper for custom terminal sizes. The existing test infrastructure (from `crate::test_utils`) provides:
- `TestTerminal::with_size(w, h)` — arbitrary terminal dimensions
- `terminal.draw_with(|f| { ... })` — render into the test buffer
- `terminal.buffer_contains("text")` — substring assert on buffer content
- `test_device_full(id, name, platform, emulator)` — create test device fixtures

To distinguish between compact and expanded LaunchContext rendering, use these observable differences:
- **Expanded mode**: No "Launch Context" titled border; fields rendered as 4-row glass blocks with labels above bordered boxes
- **Compact mode**: Has a `" Launch Context "` titled border; fields rendered as single-line `"Label: [value]"` inline rows

To distinguish between compact and full TargetSelector rendering:
- **Full mode**: No outer border; 3-row `TabBar` widget with box drawing; 1-row footer with key hints
- **Compact mode**: Has a `" Target Selector "` titled border; 1-row inline tab text; no footer

#### Test Cases

**Group 1: Horizontal layout with height-based LaunchContext decision**

```rust
#[test]
fn test_horizontal_short_terminal_uses_compact_launch_context() {
    // Wide-but-short: horizontal layout, but not enough height for expanded fields
    // Terminal 100x25 → dialog ~80x17 → content pane ~11 rows → compact
    let mut terminal = TestTerminal::with_size(100, 25);
    // Set up NewSessionDialog state with a connected device
    // Render the dialog
    // Assert: buffer contains "Launch Context" (the compact border title)
}

#[test]
fn test_horizontal_tall_terminal_uses_expanded_launch_context() {
    // Wide-and-tall: horizontal layout with enough height for expanded fields
    // Terminal 100x55 → dialog ~80x38 → content pane ~32 rows → expanded
    let mut terminal = TestTerminal::with_size(100, 55);
    // Render the dialog
    // Assert: buffer does NOT contain "Launch Context" border title
    // Assert: buffer contains field labels that appear in expanded mode (e.g., "Configuration")
}
```

**Group 2: Vertical layout with height-based decisions**

```rust
#[test]
fn test_vertical_short_terminal_uses_compact_both() {
    // Narrow-and-short: vertical layout, compact for both widgets
    // Terminal 50x25 → standard compact behavior (same as current)
    let mut terminal = TestTerminal::with_size(50, 25);
    // Render the dialog
    // Assert: buffer contains "Launch Context" (compact border)
    // Assert: buffer contains "Target Selector" (compact border)
}

#[test]
fn test_vertical_tall_terminal_uses_expanded_launch_context() {
    // Narrow-but-very-tall: vertical layout, expanded LaunchContext
    // Terminal 50x70 → dialog ~45x59 → launch area ~28+ rows → expanded
    let mut terminal = TestTerminal::with_size(50, 70);
    // Render the dialog
    // Assert: buffer does NOT contain "Launch Context" border
    // Assert: buffer contains expanded field layout indicators
}

#[test]
fn test_vertical_medium_tall_uses_full_target_compact_launch() {
    // Narrow-and-medium-tall: full TargetSelector but compact LaunchContext
    // Terminal 50x40 → target area ~12 rows (>= 10) → full; launch area ~15 rows (< 28) → compact
    let mut terminal = TestTerminal::with_size(50, 40);
    // Assert: buffer does NOT contain "Target Selector" border (full mode)
    // Assert: buffer DOES contain "Launch Context" border (compact mode)
}
```

**Group 3: Boundary conditions at thresholds**

```rust
#[test]
fn test_horizontal_at_expanded_threshold_boundary() {
    // Find the exact terminal height where content pane height == MIN_EXPANDED_LAUNCH_HEIGHT
    // Test height-1 → compact, height → expanded
    // This requires computing: terminal_h * 0.70 - 2 (border) - 6 (header/sep/footer) >= 28
    // → terminal_h >= (28 + 8) / 0.70 ≈ 52
    let mut compact_terminal = TestTerminal::with_size(100, 50);
    let mut expanded_terminal = TestTerminal::with_size(100, 55);
    // Render both, assert compact vs expanded
}
```

**Group 4: Regression — standard sizes still work**

```rust
#[test]
fn test_standard_80x24_renders_correctly() {
    // Classic terminal: horizontal layout, likely compact due to short height
    let mut terminal = TestTerminal::with_size(80, 24);
    // Assert: renders without panic, shows dialog content
}

#[test]
fn test_standard_120x40_renders_correctly() {
    // Large terminal: horizontal layout, expanded
    let mut terminal = TestTerminal::with_size(120, 40);
    // Assert: renders without panic, shows dialog content
}
```

#### Test Helper Setup

Each test needs a `NewSessionDialog` with populated state. Create a helper at the top of the test module:

```rust
fn test_dialog_state() -> NewSessionDialogState {
    let mut state = NewSessionDialogState::default();
    // Add a connected device so LaunchContext shows device-dependent fields
    state.target_selector.set_connected_devices(vec![
        test_device_full("iphone15", "iPhone 15", "ios", false),
    ]);
    state.target_selector.loading = false;
    state
}
```

Check if a similar helper already exists in the test module — the existing `test_dialog_renders` test (around line 700+ of mod.rs) likely has fixture setup that can be extracted or reused.

### Acceptance Criteria

1. At least 6 new test functions covering the scenarios above
2. Tests verify the correct compact/expanded mode is chosen based on terminal height
3. Tests cover both horizontal and vertical layout paths
4. Boundary conditions at threshold values are tested
5. Regression tests confirm standard terminal sizes still work
6. All tests pass: `cargo test -p fdemon-tui`
7. No test relies on exact pixel/cell positions — use content-based assertions (`buffer_contains`)

### Testing

```bash
# Run all new tests
cargo test -p fdemon-tui -- test_horizontal_short
cargo test -p fdemon-tui -- test_horizontal_tall
cargo test -p fdemon-tui -- test_vertical_short
cargo test -p fdemon-tui -- test_vertical_tall

# Run full suite to check for regressions
cargo test -p fdemon-tui
```

### Notes

- The `TestTerminal` approach renders into an in-memory buffer. The dialog's `centered_rect` / `centered_rect_custom` logic will compute actual dialog dimensions from the terminal size, exercising the full render pipeline.
- Color/style differences between focused/unfocused states cannot be verified with string assertions. Tests should focus on content presence (field labels, border titles) to distinguish modes.
- If existing test helpers for `NewSessionDialogState` already exist in the test module, prefer reusing them over creating new ones.
- The exact terminal heights that trigger threshold crossings depend on `centered_rect` percentage calculations. Tests should document the math in comments (as shown above) so future maintainers understand why specific sizes were chosen.

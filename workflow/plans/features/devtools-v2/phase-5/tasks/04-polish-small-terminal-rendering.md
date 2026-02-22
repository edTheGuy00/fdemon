## Task: Polish Small Terminal Rendering

**Objective**: Ensure all three DevTools panels (Inspector, Performance, Network) render gracefully at very small terminal sizes (< 60 cols, < 15 rows) and very large terminals (> 200 cols). Fix silent blank rendering, add "terminal too small" messages where needed, and verify state preservation when the terminal is resized.

**Depends on**: None

### Scope

- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`: MODIFIED — Add "too small" message for very small terminals
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`: MODIFIED — Add minimum height guard for tree render path
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: MODIFIED — Add minimum size guard before panel dispatch

### Details

#### 1. Network panel — add "too small" message

**Current behavior** (`network/mod.rs:109`): When `usable.height < 3`, the function returns early with no output — the user sees a blank panel.

**Fix**: Instead of a silent return, render a centered "Terminal too small" message:

```rust
if usable.height < 3 {
    let msg = Line::from(Span::styled(
        "Terminal too small for network view",
        Style::default().fg(Color::DarkGray),
    ));
    let x = usable.x + usable.width.saturating_sub(msg.width() as u16) / 2;
    let y = usable.y + usable.height / 2;
    buf.set_line(x, y, &msg, usable.width);
    return;
}
```

Also handle the case where `usable.width < 20` — the table columns won't fit meaningfully. Show a similar message.

#### 2. Inspector panel — add compact tree guard

**Current behavior**: The `render_tree()` path at `inspector/mod.rs:141` splits the area for tree panel and layout panel but does not guard against very small heights. If the split gives each panel 1-2 rows, tree node rendering may produce garbled output.

**Fix**: Add a minimum height check:

```rust
// Before the tree/layout split:
if inner.height < 4 {
    // Show a compact single-line status instead of the full tree
    let msg = if state.tree.is_empty() {
        "No widget tree"
    } else {
        &format!("{} nodes", state.tree.len())
    };
    let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
    buf.set_line(inner.x, inner.y, &line, inner.width);
    return;
}
```

For the horizontal/vertical split: ensure each half has at least 3 rows. If not enough height for two panels, show only the tree panel (skip the layout panel).

#### 3. DevTools container — minimum size guard

In `devtools/mod.rs`, before dispatching to the active panel, add a global minimum check:

```rust
// If the available area is too small for any panel, show a message
if area.height < 3 || area.width < 20 {
    let msg = Line::from(Span::styled(
        "Resize terminal for DevTools",
        Style::default().fg(Color::DarkGray),
    ));
    let x = area.x + area.width.saturating_sub(msg.width() as u16) / 2;
    let y = area.y;
    buf.set_line(x, y, &msg, area.width);
    return;
}
```

This provides a consistent baseline across all panels.

#### 4. Performance panel — verify existing compact mode

The Performance panel already has 3 explicit tiers:
- `height < 7`: compact single-line summary
- `7–15`: frame chart only
- `>= 16`: full dual-section layout

**Verify** these work correctly at extreme sizes:
- Height = 1: should show at least the compact summary (or truncate gracefully)
- Width = 20: verify FPS summary doesn't overflow
- Width = 200+: verify charts scale properly

No code changes expected here — just add tests if edge cases are found.

#### 5. Cross-panel resize behavior

Verify that resizing the terminal while in DevTools mode does not crash or lose state:
- Selected frame index preserved
- Selected network request preserved
- Inspector tree selection preserved
- Scroll positions preserved

This is primarily a verification task. If issues are found, fix them.

### Acceptance Criteria

1. Network panel shows "Terminal too small" instead of blank when height < 3 or width < 20
2. Inspector panel has a compact fallback for height < 4
3. Inspector panel shows only tree (no layout panel) when split would give < 3 rows each
4. DevTools container has a global minimum size guard
5. Performance panel compact mode works at height = 1 (no crash, no garbled output)
6. All panels render without panics at 20x5, 40x10, 60x15, 200x50 terminal sizes
7. State is preserved across terminal resize events
8. `cargo test -p fdemon-tui -- devtools` passes

### Testing

```bash
cargo test -p fdemon-tui -- devtools
cargo test -p fdemon-tui -- network
cargo test -p fdemon-tui -- inspector
cargo test -p fdemon-tui -- performance
```

Add widget tests for extreme terminal sizes:

```rust
#[test]
fn test_network_monitor_very_small_terminal() {
    // Render at 20x3 — should show "too small" message, not crash
}

#[test]
fn test_inspector_very_small_terminal() {
    // Render at 30x4 — should show compact node count
}

#[test]
fn test_devtools_panel_minimum_size_guard() {
    // Render at 15x2 — should show resize message
}
```

### Notes

- **Test infrastructure**: The existing `TestTerminal` wrapper in `crates/fdemon-tui/src/test_utils.rs` supports creating terminals of arbitrary sizes. Use `TestTerminal::new(width, height)` for small terminal tests.
- **Rendering safety**: Ratatui's `Buffer::set_line()` and `Paragraph` widget already handle overflow gracefully (truncating to the available area). The main risk is not overflow but rather garbled layouts where split percentages produce 0-height areas.
- **Priority**: The Network and Inspector fixes are the most important. Performance panel is already well-handled. The DevTools container guard is a defense-in-depth measure.

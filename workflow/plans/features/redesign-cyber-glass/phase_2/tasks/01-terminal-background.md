## Task: Fill Terminal Background with DEEPEST_BG

**Objective**: Fill the entire terminal area with the `DEEPEST_BG` color before rendering any widgets. This establishes the depth foundation for the Cyber-Glass design — all subsequent widgets render on top of this dark base.

**Depends on**: None (Phase 1 theme module must exist)

### Scope

- `crates/fdemon-tui/src/render/mod.rs` — Add background fill at the start of `view()`

### Details

#### Current State

The `view()` function (line 22) starts by computing layout areas and immediately rendering widgets. No background fill is applied — the terminal's default background color shows through.

#### Change

Add a full-area background fill as the **first rendering operation** in `view()`, before any widget rendering:

```rust
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();

    // Fill entire terminal with deepest background color
    let bg_block = Block::default()
        .style(Style::default().bg(palette::DEEPEST_BG));
    frame.render_widget(bg_block, area);

    // ... existing layout and widget rendering follows ...
}
```

This uses a `Block` with no borders and just a background style. It fills every cell in the terminal with `DEEPEST_BG` (`Color::Black` in Phase 1, `Rgb(10, 12, 16)` when Phase 2 RGB values are applied).

Also apply the same background to `render_loading_screen()` (line 276):

```rust
// Before
.style(Style::default().bg(Color::Black))

// After
.style(Style::default().bg(palette::DEEPEST_BG))
```

### Acceptance Criteria

1. The `view()` function fills the full terminal area with `DEEPEST_BG` before rendering widgets
2. `render_loading_screen()` uses `palette::DEEPEST_BG` instead of `Color::Black`
3. `cargo check -p fdemon-tui` passes
4. `cargo clippy -p fdemon-tui` passes

### Testing

Run the app visually to confirm the background is uniformly dark. No functional test changes expected since the background color is the same as current (`Color::Black`).

### Notes

- This is a trivial change but establishes the visual foundation. All glass containers (header, log panel) will have a lighter `CARD_BG` background that contrasts against this `DEEPEST_BG` base.
- The `Block::default().style(...)` approach is more efficient than iterating cells manually.
- In Phase 1, `DEEPEST_BG == Color::Black`, so there's no visible change. The visible change comes when Phase 2 RGB values are applied (a very dark near-black `Rgb(10, 12, 16)`).

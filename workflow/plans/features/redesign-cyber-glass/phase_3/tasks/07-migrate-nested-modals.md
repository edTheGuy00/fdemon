## Task: Migrate Nested Modals (Fuzzy + Dart Defines) to Theme

**Objective**: Replace all hardcoded RGB colors in `fuzzy_modal.rs` and `dart_defines_modal.rs` with palette constants and apply Cyber-Glass styling consistent with the redesigned dialog.

**Depends on**: 01-migrate-palette-to-rgb, 02-redesign-modal-overlay

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` — Migrate to theme palette
- `crates/fdemon-tui/src/widgets/new_session_dialog/dart_defines_modal.rs` — Migrate to theme palette
- `crates/fdemon-tui/src/theme/palette.rs` — Potentially adjust modal-specific palette constants

### Details

#### Fuzzy Modal — Current Hardcoded Colors

The fuzzy modal uses these palette constants that are already RGB:

```rust
// In palette.rs (already defined)
MODAL_FUZZY_BG: Color::Rgb(40, 40, 50)
MODAL_FUZZY_QUERY_BG: Color::Rgb(60, 60, 70)
```

And these standard palette references:
- `palette::ACCENT` — title, selected item bg
- `palette::TEXT_MUTED` — hints, dimmed overlay
- `palette::TEXT_PRIMARY` — list items
- `palette::CONTRAST_FG` — selected item text
- `palette::STATUS_YELLOW` — "No matches"
- `palette::DEEPEST_BG` — dim overlay bg
- `palette::BORDER_DIM` — separator

**Changes needed:**
1. Replace `MODAL_FUZZY_BG` with `palette::POPUP_BG` (Rgb(28,33,43)) — consistent with the main dialog
2. Replace `MODAL_FUZZY_QUERY_BG` with `palette::SURFACE` (Rgb(22,27,34)) — consistent with field backgrounds
3. Apply `BorderType::Rounded` consistently
4. Update border color to `palette::BORDER_DIM`
5. Add `palette::POPUP_BG` background to the block style
6. Use `render_dim_overlay()` from `modal_overlay.rs` instead of the local `render_dim_overlay()` function

**Specific function changes:**

```rust
// fuzzy_modal.rs — replace local dim_overlay with shared one
// Before:
pub fn render_dim_overlay(area: Rect, buf: &mut Buffer) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = buf.cell_mut((x, y)).expect("in bounds");
            cell.set_style(Style::default().fg(Color::DarkGray));
        }
    }
}

// After: Remove this function, use crate::widgets::modal_overlay::dim_background()
```

Update the caller in `mod.rs::render_fuzzy_modal_overlay()`:

```rust
// Before:
fuzzy_modal::render_dim_overlay(dialog_area, buf);

// After:
modal_overlay::dim_background(buf, dialog_area);
```

**Fuzzy modal block styling:**

```rust
// Before:
let block = Block::default()
    .borders(Borders::ALL)
    .border_set(symbols::border::ROUNDED)
    .style(Style::default().bg(palette::MODAL_FUZZY_BG));

// After:
let block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(styles::border_inactive())
    .style(Style::default().bg(palette::POPUP_BG));
```

**Query field background:**

```rust
// Before:
Style::default().bg(palette::MODAL_FUZZY_QUERY_BG)

// After:
Style::default().bg(palette::SURFACE)
```

#### Dart Defines Modal — Current Hardcoded Colors

The dart defines modal uses these palette constants:

```rust
MODAL_DART_DEFINES_BG: Rgb(30, 30, 40)
MODAL_DART_DEFINES_INPUT_ACTIVE_BG: Rgb(60, 60, 80)
MODAL_DART_DEFINES_INPUT_INACTIVE_BG: Rgb(40, 40, 50)
MODAL_DART_DEFINES_BUTTON_INACTIVE_BG: Rgb(50, 50, 60)
MODAL_DART_DEFINES_CLEAR_BG: Rgb(20, 20, 30)
```

**Migration mapping:**

| Current | Replacement | Rationale |
|---------|-------------|-----------|
| `MODAL_DART_DEFINES_BG` | `palette::POPUP_BG` | Consistent modal background |
| `MODAL_DART_DEFINES_INPUT_ACTIVE_BG` | `palette::SURFACE` | Active input field = elevated surface |
| `MODAL_DART_DEFINES_INPUT_INACTIVE_BG` | `palette::CARD_BG` | Inactive input = card level |
| `MODAL_DART_DEFINES_BUTTON_INACTIVE_BG` | `palette::CARD_BG` | Inactive button = card level |
| `MODAL_DART_DEFINES_CLEAR_BG` | `palette::DEEPEST_BG` | Full clear = deepest background |

**Specific changes:**

1. Replace full-screen clear logic with `modal_overlay::dim_background()`:

```rust
// Before (in dart_defines_modal.rs):
for y in area.top()..area.bottom() {
    for x in area.left()..area.right() {
        let cell = buf.cell_mut((x, y)).expect("in bounds");
        cell.reset();
        cell.set_style(Style::default().bg(palette::MODAL_DART_DEFINES_CLEAR_BG));
    }
}

// After:
modal_overlay::dim_background(buf, area);
```

2. Update outer border from `DOUBLE` to `Rounded` for consistency:

```rust
// Before:
.border_set(symbols::border::DOUBLE)
.border_style(Style::default().fg(palette::ACCENT))

// After:
.border_type(BorderType::Rounded)
.border_style(styles::border_inactive())
```

3. Update inner pane borders to use theme styles:

```rust
// Before: Manual border style selection
let border_color = if is_focused { palette::ACCENT } else { palette::BORDER_DIM };
.border_style(Style::default().fg(border_color))

// After: Use theme style builders
.border_style(if is_focused { styles::border_active() } else { styles::border_inactive() })
```

4. Update input field backgrounds:

```rust
// Active input
Style::default().bg(palette::SURFACE)

// Inactive input
Style::default().bg(palette::CARD_BG)
```

5. Update button styles:

```rust
// Active button
Style::default()
    .fg(palette::CONTRAST_FG)
    .bg(palette::ACCENT)
    .add_modifier(Modifier::BOLD)

// Inactive button
Style::default()
    .fg(palette::TEXT_PRIMARY)
    .bg(palette::CARD_BG)
```

#### Palette Cleanup

After migration, the following palette constants become unused and should be removed or marked `#[allow(dead_code)]`:

- `MODAL_FUZZY_BG`
- `MODAL_FUZZY_QUERY_BG`
- `MODAL_DART_DEFINES_BG`
- `MODAL_DART_DEFINES_INPUT_ACTIVE_BG`
- `MODAL_DART_DEFINES_INPUT_INACTIVE_BG`
- `MODAL_DART_DEFINES_BUTTON_INACTIVE_BG`
- `MODAL_DART_DEFINES_CLEAR_BG`

Remove these constants from `palette.rs` to reduce palette clutter. The design hierarchy now uses only the core tokens (`DEEPEST_BG`, `CARD_BG`, `SURFACE`, `POPUP_BG`).

### Acceptance Criteria

1. `fuzzy_modal.rs` uses only `palette::` and `styles::` references — no local hardcoded colors
2. Local `render_dim_overlay()` in `fuzzy_modal.rs` removed — uses shared `modal_overlay::dim_background()`
3. Fuzzy modal block uses `BorderType::Rounded` + `POPUP_BG` background
4. `dart_defines_modal.rs` uses only `palette::` and `styles::` references
5. Dart defines modal clear effect uses `modal_overlay::dim_background()` instead of manual cell iteration
6. Dart defines outer border changed from `DOUBLE` to `Rounded`
7. Old modal-specific palette constants removed from `palette.rs`
8. All modal functionality preserved (fuzzy search, dart defines editing, keyboard navigation)
9. `cargo check -p fdemon-tui` passes
10. `cargo clippy -p fdemon-tui` passes

### Testing

- Visually verify fuzzy modal: background dimmed, modal has glass styling
- Verify fuzzy search still works: typing, filtering, selection, scrolling
- Verify fuzzy modal for all types: config, flavor, entry point
- Visually verify dart defines modal: consistent with main dialog styling
- Verify dart defines: add/edit/delete variables, save, keyboard navigation
- Verify both modals render correctly in horizontal and vertical layouts
- Run `cargo test -p fdemon-tui` — update any tests that reference removed palette constants

### Notes

- **Gradual consistency**: The goal is to make nested modals visually consistent with the redesigned main dialog. They should feel like they belong to the same design system.
- **DOUBLE border removal**: The dart defines modal currently uses `DOUBLE` borders (`╔╗╚╝`) for emphasis. Changing to `Rounded` makes it consistent with the rest of the UI. If visual distinction is still desired, use `BORDER_ACTIVE` color instead of `BORDER_DIM`.
- **dim_background vs clear**: The fuzzy modal dims the background (content visible but dark), while the dart defines modal completely clears it. With the new overlay system, both should use `dim_background()` for consistency — the dart defines modal renders on top of the dimmed content.
- **Test for removed palette constants**: `cargo test` will catch any remaining references to removed constants as compilation errors.

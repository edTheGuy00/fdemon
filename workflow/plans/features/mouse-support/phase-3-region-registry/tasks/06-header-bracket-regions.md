## Task: Record Header Bracketed-Shortcut Regions

**Objective**: In `widgets/header.rs::render_title_row`, record one click region per bracketed shortcut (`[r] [R] [x] [d] [D] [q]`). Each region covers only the bracket+letter cells (not the trailing label text), so accidental drags over the label do not fire actions. Pass `MouseCtx` into `MainHeader` so the registration happens during render.

**Depends on**: 04

**Estimated Time**: 1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/header.rs`: Add `with_mouse(self, ctx: &mut MouseCtx<'_>)` builder method (or accept `Option<&mut MouseCtx<'_>>` in `render_title_row` directly), record six bracketed-shortcut rects with their respective `Message::*` actions during render, add snapshot tests on the populated registry.
- `crates/fdemon-tui/src/render/mod.rs`: Remove the `drop(mouse_ctx)` placeholder from Task 04 and pass `&mut mouse_ctx` into `MainHeader::new(...).with_mouse(&mut mouse_ctx)`. (Single-line change in the existing `frame.render_widget(header, areas.header);` call site.)

**Files Read (Dependencies):**
- `crates/fdemon-app/src/mouse_regions.rs` (Task 01): `MouseRect`, `MouseAction`.
- `crates/fdemon-tui/src/render/mod.rs` (Task 04): `MouseCtx`, `to_mouse_rect` helper.
- `crates/fdemon-app/src/message.rs`: `HotReload`, `HotRestart`, `CloseCurrentSession`, `EnterDevToolsMode`, `ToggleDap`, `RequestQuit`.

### Details

#### Architectural choice: builder vs. direct parameter

`MainHeader` currently uses `frame.render_widget(header, areas.header)` (the standard ratatui `Widget` trait). Adding region recording requires the widget to mutate the registry during render — which collides with `Widget::render(self, area, &mut Buffer)`'s signature.

Two clean options:

- **Option A — Optional ctx field on the widget.** Add `mouse_ctx: Option<*mut MouseCtx<'_>>` (via a 'a lifetime parameter) and a `.with_mouse(&mut MouseCtx)` builder. The widget consults it inside `render_title_row`. Borrow checker requires the lifetime to match, which means `MainHeader<'a>` becomes `MainHeader<'a, 'b>`. Workable but ugly.
- **Option B — Bypass `Widget::render` for the header.** Replace `frame.render_widget(header, areas.header)` with `widgets::header::render(frame.buffer_mut(), areas.header, &header_args, &mut mouse_ctx)` — a free function that takes the ctx explicitly. Keep the existing `Widget` impl for tests that don't need regions.

**Decision**: Option B — keeps the lifetime story simple and matches the PLAN.md recommendation ("thread it — keeps render functions explicit about side effects").

Concretely, refactor `MainHeader` so that the existing `Widget::render` body delegates to a new free function `render_with_mouse(area, buf, header: &MainHeader, ctx: Option<&mut MouseCtx>)`. The `Widget` impl passes `None`; the new render entry path passes `Some(&mut ctx)`.

```rust
// header.rs (sketch)

pub fn render_main_header(
    area: Rect,
    buf: &mut Buffer,
    header: &MainHeader<'_>,
    mut ctx: Option<&mut crate::render::MouseCtx<'_>>,
) {
    // Move the existing Widget::render body here, threading `ctx.as_deref_mut()`
    // into `render_title_row`.
    // ...
}

impl Widget for MainHeader<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_main_header(area, buf, &self, None);
    }
}
```

In `render::view`:

```rust
let header = widgets::MainHeader::new(state.project_name.as_deref(), icons)
    .with_sessions(&state.session_manager);
widgets::header::render_main_header(areas.header, frame.buffer_mut(), &header, Some(&mut mouse_ctx));
```

#### Region registration in `render_title_row`

The existing shortcut spans (lines 166-185 of `header.rs`) build a `Vec<Span>` literal:

```rust
let shortcuts = vec![
    Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
    Span::styled("r", Style::default().fg(palette::STATUS_YELLOW)),
    Span::styled("] Run  ", Style::default().fg(palette::TEXT_MUTED)),
    // ...repeated for R, x, d, D, q
];
```

Each shortcut is three spans: open bracket `[` (1 cell) + letter (1 cell) + `] Label  ` (variable). The clickable rect should cover the first two cells (the `[X` part) — *not* the closing bracket and label. This matches the PLAN.md guidance "Register only the bracket+letter cells, not the trailing label text".

Registration happens *after* the line is written to the buffer, so we know the start `x` of each shortcut. Compute it from the shortcut order:

```rust
const SHORTCUTS: &[(char, Message)] = &[
    ('r', Message::HotReload),
    ('R', Message::HotRestart),
    ('x', Message::CloseCurrentSession),
    ('d', Message::EnterDevToolsMode),
    ('D', Message::ToggleDap),
    ('q', Message::RequestQuit),
];

// Each entry in the rendered Vec<Span> is `[`, `<letter>`, `] <Label>  ` —
// 3 spans per shortcut. The `[<letter>` portion covers exactly 2 cells.
const SHORTCUT_CLICK_WIDTH: u16 = 2;

// `shortcuts_x` is already computed at line 220 of header.rs as the line's
// origin within `area`. Walk the spans and accumulate widths to find each
// `[<letter>` cell-pair start.
let mut cursor_x = shortcuts_x;
for (label, _msg) in SHORTCUTS {
    // Open bracket span starts here.
    let click_x = cursor_x;

    // Compute width: `[X] Label  ` = 1 + 1 + (3 + label_width + 2) cells
    let label_str = match label {
        'r' => "Run", 'R' => "Restart", 'x' => "Stop",
        'd' => "Debug", 'D' => "DAP", 'q' => "Quit",
        _ => unreachable!(),
    };
    let segment_width = (1 + 1 + 1 + 1 + label_str.len() + 2) as u16; // `[` + letter + `] ` + label + `  `
    cursor_x = cursor_x.saturating_add(segment_width);

    // Skip if this segment overflowed the visible area (we render before
    // total_content_width <= area.width path, so this is a safety net).
    if click_x + SHORTCUT_CLICK_WIDTH > area.x + area.width {
        continue;
    }
    if let Some(ctx) = ctx.as_deref_mut() {
        ctx.click(
            MouseRect::new(click_x, area.y, SHORTCUT_CLICK_WIDTH, 1),
            MouseAction::Emit(_msg.clone()),
        );
    }
}
```

**Important**: this only registers regions in the `total_content_width <= area.width` branch (lines 215-231 of the existing header.rs) — i.e., when shortcuts actually fit. The other two branches (lines 232-245) skip rendering shortcuts; in those cases, no regions should be registered.

To keep the code DRY, hoist the `SHORTCUTS` constant array and the registration loop into a `register_shortcut_clicks(ctx, area, shortcuts_x)` helper so both call sites stay short.

#### Lifetime hazard

Holding `Option<&mut MouseCtx<'_>>` across the existing rendering body may run into borrow-checker complaints. Mitigations:

- Take the registration phase *after* `buf.set_line(shortcuts_x, area.y, ...)`. By then no widget needs to borrow `ctx`.
- Or, register *inside* the `total_content_width <= area.width` branch immediately after `set_line`.

The implementor should pick the simpler arrangement once the borrow checker constrains the choice.

### Acceptance Criteria

1. `widgets::header::render_main_header(area, buf, &header, Some(ctx))` exists and is called from `render::view` instead of `frame.render_widget(header, ...)`.
2. The `Widget for MainHeader` impl delegates to `render_main_header` with `ctx = None`, so existing tests using `term.render_widget(header, term.area())` continue to work.
3. Six regions are registered on the title row whenever shortcuts fit (`total_content_width <= area.width`):
   - `[r]` cells → `Message::HotReload`
   - `[R]` cells → `Message::HotRestart`
   - `[x]` cells → `Message::CloseCurrentSession`
   - `[d]` cells → `Message::EnterDevToolsMode`
   - `[D]` cells → `Message::ToggleDap`
   - `[q]` cells → `Message::RequestQuit`
4. Each region covers exactly 2 cells (`[X`), height 1, at `y == title_row.y`.
5. Regions are NOT registered when shortcuts are clipped (the two narrower branches at lines 232-245 of header.rs).
6. Empty rect protection (Task 01) means out-of-bounds shortcuts at very narrow terminals do not panic.
7. `cargo test --workspace` passes — including the new snapshot test below.
8. `cargo clippy --workspace --all-targets -- -D warnings` passes.

### Testing

Add to `widgets/header.rs::tests`:

```rust
#[test]
fn header_records_six_bracketed_shortcut_regions_at_120x24() {
    use fdemon_app::{MouseRegions, MouseAction, AppState};
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    let actions: Vec<_> = regions
        .iter()
        .filter_map(|e| e.on_left.as_ref().map(|a| (e.rect, a.clone())))
        .collect();

    // Expect at least the 6 bracketed-shortcut regions on the header row.
    assert!(actions.len() >= 6, "expected ≥ 6 shortcut regions, got {}", actions.len());

    // Order is r, R, x, d, D, q — left-to-right.
    let messages: Vec<_> = actions
        .iter()
        .filter_map(|(_, a)| match a {
            MouseAction::Emit(m) => Some(format!("{:?}", m)),
            _ => None,
        })
        .collect();
    assert!(messages.iter().any(|m| m.contains("HotReload")), "no HotReload region");
    assert!(messages.iter().any(|m| m.contains("HotRestart")), "no HotRestart region");
    assert!(messages.iter().any(|m| m.contains("CloseCurrentSession")), "no Close region");
    assert!(messages.iter().any(|m| m.contains("EnterDevToolsMode")), "no DevTools region");
    assert!(messages.iter().any(|m| m.contains("ToggleDap")), "no DAP region");
    assert!(messages.iter().any(|m| m.contains("RequestQuit")), "no Quit region");
}

#[test]
fn header_skips_region_recording_when_shortcuts_clipped() {
    // At 40 cols, the existing header logic falls into the "Only left section
    // fits" branch and does not render shortcuts. No regions should be
    // registered for shortcuts.
    use fdemon_app::AppState;
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(40, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    // No bracketed-shortcut regions at this width.
    let shortcut_count = regions.iter().filter(|e| e.rect.width == 2 && e.rect.height == 1).count();
    assert_eq!(
        shortcut_count, 0,
        "shortcuts not visible at 40 cols → no clickable regions"
    );
}

#[test]
fn header_shortcut_rect_is_two_cells_wide() {
    use fdemon_app::AppState;
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    // Find the HotReload region; it should be exactly 2 cells wide.
    let entry = regions
        .iter()
        .find(|e| matches!(
            e.on_left,
            Some(fdemon_app::MouseAction::Emit(fdemon_app::Message::HotReload))
        ))
        .expect("HotReload region must be registered");
    assert_eq!(entry.rect.width, 2);
    assert_eq!(entry.rect.height, 1);
}
```

### Notes

- The `cursor_x` arithmetic mirrors the rendered span widths exactly. If anyone changes the header label copy (e.g., `"Run "` → `"Reload "`), the snapshot tests above catch the drift. PLAN.md's "Bind drift over time" risk is the motivation.
- Do NOT register regions in multi-session mode's tabs row (`render_title_row(..., false)` branch where `show_device == false`). The tabs row is owned by `SessionTabs::render` (Task 07).
- `MainHeader::with_mouse(...)` is mentioned for ergonomic completeness in Acceptance Criteria 1, but the recommended Option B (free function with explicit ctx) makes it unnecessary. Either approach is acceptable as long as the existing `Widget::render` continues to work for tests.
- The `frame.buffer_mut()` access pattern is well-supported in ratatui 0.30. If unfamiliar, look at how other tests in `render/tests.rs` use the buffer directly.

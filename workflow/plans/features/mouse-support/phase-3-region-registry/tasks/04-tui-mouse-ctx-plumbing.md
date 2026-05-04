## Task: TUI Plumbing — `MouseCtx` and `render::view` Take/Put-back

**Objective**: Wire the `MouseRegions` registry through `render::view`. Define `MouseCtx<'a>` (the borrowed bridge between `MouseRegionsBuilder` and widget render functions), and add the take-clear-render-putback dance to `view()`. Add the `From<ratatui::layout::Rect> for MouseRect` boundary conversion. No widget records anything yet — that lands in Tasks 06 and 07.

**Depends on**: 03

**Estimated Time**: 1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/render/mod.rs`: Add `MouseCtx<'a>` struct, modify `view(frame, state)` to take/clear/put-back the registry around the existing render body, expose the `MouseCtx` to header/tab render paths.
- `crates/fdemon-tui/src/widgets/mod.rs`: Re-export `MouseCtx` for use by widget modules; add `impl From<ratatui::layout::Rect> for fdemon_app::MouseRect` (or a free `to_mouse_rect(r: ratatui::layout::Rect) -> MouseRect` helper if Rust's orphan rule blocks the impl — see Details).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/mouse_regions.rs` (Task 01): `MouseRegionsBuilder`, `MouseRegions`, `MouseRect`, `MouseAction`.
- `crates/fdemon-app/src/state.rs` (Task 03): `AppState::mouse_regions: Cell<MouseRegions>`.
- `crates/fdemon-tui/src/widgets/header.rs` and `tabs.rs`: Existing render code paths — no edits in this task, but understand their signatures so Tasks 06 and 07 can extend them.

### Details

#### Orphan rule for `From<ratatui::layout::Rect> for MouseRect`

`MouseRect` lives in `fdemon-app` and `Rect` lives in `ratatui`. Implementing `From<Rect> for MouseRect` from inside `fdemon-tui` (a third crate) violates Rust's orphan rule. Two clean options:

- **Option A — Free helper.** Add a free function in `fdemon-tui/src/widgets/mod.rs`:
  ```rust
  use fdemon_app::MouseRect;
  use ratatui::layout::Rect;

  pub(crate) fn to_mouse_rect(r: Rect) -> MouseRect {
      MouseRect::new(r.x, r.y, r.width, r.height)
  }
  ```
  Call sites: `to_mouse_rect(area)`. Verbose but obvious.

- **Option B — Newtype.** Wrap `MouseRect` inside `fdemon-tui` and impl `From` on the wrapper. Adds a layer with no benefit. Skip.

**Decision**: Option A. The helper is one line and keeps the conversion site self-documenting.

#### `MouseCtx`

```rust
// crates/fdemon-tui/src/render/mod.rs

use fdemon_app::{MouseAction, MouseRect, MouseRegionsBuilder};

/// Borrowed bridge between [`render::view`] and widgets that record clickable
/// regions during render.
///
/// `MouseCtx` exists so widgets do not need to thread `&mut MouseRegionsBuilder`
/// directly (which collides ergonomically with the `Widget::render` trait that
/// only sees `area` and `buf`). Widgets that need region recording accept an
/// `Option<&mut MouseCtx<'_>>` constructor argument; passing `None` keeps the
/// widget usable in tests that render without a registry.
#[derive(Debug)]
pub struct MouseCtx<'a> {
    builder: MouseRegionsBuilder<'a>,
}

impl<'a> MouseCtx<'a> {
    pub fn new(builder: MouseRegionsBuilder<'a>) -> Self {
        Self { builder }
    }

    /// Register a left-click region at `z_index = 0`.
    pub fn click(&mut self, rect: MouseRect, action: MouseAction) {
        self.builder.click(rect, action);
    }

    /// Register a left-click region at a specific `z_index`. Phase 5
    /// dialogs/overlays use `z_index = 1`.
    pub fn click_at_z(&mut self, rect: MouseRect, action: MouseAction, z: u8) {
        self.builder.click_at_z(rect, action, z);
    }

    /// Register a region with separate left and middle bindings (used by
    /// session tabs: left = select, middle = close).
    pub fn click_left_middle(
        &mut self,
        rect: MouseRect,
        on_left: MouseAction,
        on_middle: MouseAction,
    ) {
        self.builder.click_left_middle(rect, on_left, on_middle);
    }
}
```

Re-export it from `widgets/mod.rs`:

```rust
// crates/fdemon-tui/src/widgets/mod.rs
pub use crate::render::MouseCtx;
```

#### `view()` take/put-back

Modify `crates/fdemon-tui/src/render/mod.rs::view` (around line 54). Existing first lines:

```rust
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();
    // ...existing body...
}
```

New body (delta only):

```rust
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();

    // ── Mouse region registry: take, clear, render, put back ─────────────
    // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
    let mut regions = state.mouse_regions.take();
    regions.clear();

    // ── Existing rendering body (unchanged) ──────────────────────────────
    // The header & tabs widgets in Phase 3 Task 06/07 will accept an
    // `Option<&mut MouseCtx>` and use it to push regions. For now, we
    // construct the ctx but do not yet pass it to any widget.
    let mut mouse_ctx = MouseCtx::new(regions.builder());
    drop(mouse_ctx); // suppress unused warning until Tasks 06/07 land

    // ...existing body verbatim — header, log view, modal overlays, etc.

    // ── Put the populated registry back ──────────────────────────────────
    // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
    state.mouse_regions.set(regions);
}
```

**Important sequencing note**: The `drop(mouse_ctx)` is a placeholder so this task compiles standalone. Tasks 06 and 07 remove the `drop` and pass `&mut mouse_ctx` into `MainHeader::new(...).with_mouse(&mut mouse_ctx)` and `SessionTabs::new(...).with_mouse(&mut mouse_ctx)` respectively. **Document this clearly** in the inline comment so the next implementor knows the placeholder is temporary.

A cleaner alternative is to pass the ctx into a closure and let `MouseCtx::new` return a guard that auto-puts-back on drop:

```rust
// Cleaner alternative — RAII guard
state.mouse_regions.with_builder(|ctx| {
    // ...render...
});
```

If the implementor prefers RAII, add `MouseRegions::with_builder` (or a free function on `Cell<MouseRegions>`). **Either approach is acceptable** — pick whichever results in the shortest call site. The take/put-back pair is the explicit baseline.

#### Why no widget changes here

Tasks 06 and 07 modify `MainHeader` and `SessionTabs` to accept the ctx. Splitting along widget boundaries means Tasks 06 and 07 can run in parallel worktrees (header.rs and tabs.rs are different files). This task only sets up the conduit.

### Acceptance Criteria

1. `MouseCtx<'a>` is defined in `render/mod.rs` and re-exported from `widgets/mod.rs`.
2. `to_mouse_rect(r: ratatui::layout::Rect) -> MouseRect` exists in `widgets/mod.rs` (or wherever the implementor decides — must be reachable from `header.rs` and `tabs.rs`).
3. `view()` takes the registry, clears it, constructs a `MouseCtx`, runs the existing rendering body, and puts the populated registry back.
4. `cargo check --workspace --all-targets` passes.
5. `cargo test --workspace` passes — no existing test should regress because `view()`'s observable output (the rendered buffer) is unchanged.
6. `cargo clippy --workspace --all-targets -- -D warnings` passes.
7. The `state.mouse_regions` field is observably empty after a call to `view()` that does no region recording — Task 06 / 07 will replace this with a populated registry, but until then `view()` should leave the registry empty (verifies the take/clear/put-back invariant).

### Testing

Add a single integration test to `crates/fdemon-tui/src/render/tests.rs` (existing file):

```rust
#[test]
fn test_view_leaves_mouse_regions_empty_when_no_widget_records() {
    use fdemon_app::AppState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = AppState::new();

    terminal.draw(|f| crate::render::view(f, &mut state)).unwrap();

    let regions = state.mouse_regions.take();
    assert!(
        regions.is_empty(),
        "no widget records regions yet (Phase 3 Tasks 06/07 will change this)"
    );
}
```

This test is **fragile by design** — it asserts a placeholder invariant that Tasks 06 and 07 will deliberately break. When those tasks land, this test should be replaced (or strengthened) by their snapshot tests. Mark it with a TODO comment:

```rust
// TODO(phase-3): Tasks 06 (header regions) and 07 (tab regions) will
// change this assertion. Replace with snapshot tests on the populated
// registry contents.
```

### Notes

- The `MouseCtx` indirection is intentional. It would be slightly shorter to pass `&mut MouseRegionsBuilder` directly, but doing so requires every widget that *might* want regions to import `MouseRegionsBuilder` (a `fdemon-app` type). Wrapping in `MouseCtx` (a `fdemon-tui` type) localizes the dependency and gives us a place to add TUI-specific helpers later (e.g., a `click_rect_only(rect, msg)` shortcut that skips the `MouseAction::Emit(...)` boilerplate).
- The `to_mouse_rect` helper avoids the orphan rule. If the implementor really wants `From`-style ergonomics, they can add a method on `MouseCtx`: `fn click_ratatui(&mut self, r: Rect, action: MouseAction)` that does the conversion internally. This is fine — pick whichever reads best at the call site.
- Performance: `Cell::take` is `mem::replace(&self, Default::default())` — a swap of two `Vec` headers (3 pointers each). With `MouseRegions::with_capacity()` pre-allocating the vec backing once at startup, steady-state has zero allocations. Verified by `clear_preserves_capacity` test in Task 01.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/render/mod.rs` | Added `MouseCtx<'a>` struct with `click`, `click_at_z`, `click_left_middle` methods; added `use fdemon_app::{MouseAction, MouseRect, MouseRegionsBuilder};`; modified `view()` to take/clear/put-back the mouse regions registry around the render body |
| `crates/fdemon-tui/src/widgets/mod.rs` | Re-exported `MouseCtx` from `crate::render`; added `to_mouse_rect(r: ratatui::layout::Rect) -> fdemon_app::MouseRect` free function with `#[allow(dead_code)]` pending Tasks 06/07 |
| `crates/fdemon-tui/src/render/tests.rs` | Added `test_view_leaves_mouse_regions_empty_when_no_widget_records` test with TODO comment for Tasks 06/07 |

### Notable Decisions/Tradeoffs

1. **Inner block for builder borrow**: Used a block `{ let _mouse_ctx = ...; }` rather than `drop(mouse_ctx)` to avoid the `clippy::drop_non_drop` lint. The block clearly documents the borrow scope and Tasks 06/07 can restructure it to hoist `mouse_ctx` outside the block when they thread it into widget constructors.

2. **`#[allow(dead_code)]` on `to_mouse_rect`**: The helper is `pub(crate)` so Rust treats it as potentially dead until Tasks 06/07 add call sites. A single targeted allow-attribute with a TODO comment is cleaner than making it `pub` (which would expose it in the crate's external API prematurely).

3. **Re-export path**: `MouseCtx` is defined in `render/mod.rs` and re-exported from `widgets/mod.rs`. Widget modules (header.rs, tabs.rs) import from `super::MouseCtx` or `crate::widgets::MouseCtx`, keeping the render dependency chain correct (widgets use the public path, not the internal one).

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all test suites, new test passes)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Placeholder `{  }` block**: The inner block that creates and immediately drops `_mouse_ctx` is a deliberate placeholder. Tasks 06/07 must restructure this when they pass `&mut mouse_ctx` into widget constructors. The TODO comment in the source makes this visible.
2. **Registry always empty until Tasks 06/07**: The test `test_view_leaves_mouse_regions_empty_when_no_widget_records` deliberately asserts the placeholder invariant and is marked fragile-by-design. Tasks 06/07 must update or replace it.

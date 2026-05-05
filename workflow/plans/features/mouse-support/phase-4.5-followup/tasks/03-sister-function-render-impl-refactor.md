# Task 03: Sister-Function `render_impl` Refactor (3 DevTools Panels)

## Goal

Refactor three DevTools panels to share a single `render_impl(area, buf, ctx: Option<&mut MouseCtx<'_>>)` body between `Widget::render` and `render_with_regions`, eliminating the duplicated background-clear / state-guard / layout dispatch code that currently exists in both paths. Add per-panel tests asserting that both render paths produce byte-identical buffers.

## Background

Phase 4 introduced sister-function pairs: each widget has both `Widget::render(...)` and a free function `render_with_regions(self, area, buf, ctx)`. The intent was to keep `Widget::render` available for non-clickable callers while threading `MouseCtx` into the TEA renderer.

Three panels followed the wrong pattern (duplicate the body) instead of the right pattern (share via `render_impl`):

| Panel | Sister-function file | Lines | Current pattern |
|-------|---------------------|-------|-----------------|
| DevTools (top-level) | `widgets/devtools/mod.rs` | ~386–406 | Duplicated background fill + minimum-size guard |
| Performance | `widgets/devtools/performance/mod.rs` | ~295–388 | Duplicated background fill + disconnected guard + compact-threshold + layout dispatch |
| Inspector | `widgets/devtools/inspector/mod.rs` | ~461–468 | Duplicated state branch dispatch |

**Reference for the correct pattern** — `widgets/devtools/network/mod.rs`, `widgets/devtools/network/request_table.rs`, and `widgets/devtools/network/request_details.rs` all delegate `Widget::render` to `render_impl(area, buf, None)`, and `render_with_regions` to `render_impl(area, buf, ctx)`. Apply the same shape to the three panels above.

## Files

**Modify:**
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`
- `crates/fdemon-tui/src/widgets/devtools/tests.rs` (or wherever DevTools view tests live)
- `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs`
- `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs`

**Read (reference for the desired pattern):**
- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` — `render_impl` pattern
- `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs` — same pattern at table level
- `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs` — same pattern at details level

## Plan

1. **Refactor each panel's render entry points to share `render_impl`**:

   ```rust
   fn render_impl(self, area: Rect, buf: &mut Buffer, mut ctx: Option<&mut MouseCtx<'_>>) {
       // Existing Widget::render body, with click-region sites guarded by `if let Some(c) = ctx.as_deref_mut() { ... }`.
   }

   impl Widget for PanelView<'_> {
       fn render(self, area: Rect, buf: &mut Buffer) {
           self.render_impl(area, buf, None);
       }
   }

   pub fn render_with_regions(view: PanelView<'_>, area: Rect, buf: &mut Buffer, ctx: &mut MouseCtx<'_>) {
       view.render_impl(area, buf, Some(ctx));
   }
   ```

   For `inspector::render_with_regions` and `devtools::render_with_regions` which dispatch on a state branch (loaded vs loading vs empty), the dispatch happens *inside* `render_impl`; the click-region calls are at the leaf branches that need them.

   For `performance::render_with_regions`, the dispatch covers disconnected / compact / frame-only / dual-section paths. Forward `ctx` only into the FrameChart section (which is the only clickable surface). The other branches receive `None` (no regions registered when in a non-clickable mode).

2. **Add a byte-identical-buffer parity test per panel**. Each test renders the same `PanelView` twice — once via `Widget::render`, once via `render_with_regions(... &mut ctx)` with a discarded ctx — and asserts the buffers are equal cell-by-cell:

   ```rust
   #[test]
   fn render_with_regions_matches_widget_render_buffer() {
       let view = build_view(/* fixture for this panel */);
       let area = Rect::new(0, 0, 80, 24);

       let mut buf_a = Buffer::empty(area);
       view.clone().render(area, &mut buf_a);

       let mut buf_b = Buffer::empty(area);
       let mut regions = MouseRegions::default();
       let mut builder = regions.builder();
       let mut ctx = MouseCtx::new(&mut builder);
       render_with_regions(view, area, &mut buf_b, &mut ctx);

       assert_eq!(buf_a, buf_b, "render paths must produce identical buffers");
   }
   ```

   Place each test in the corresponding panel's `tests.rs` file. The fixture should hit a non-trivial state branch (e.g., for inspector: tree loaded with at least one node; for performance: at least one frame in the buffer with `vm_connected = true`; for devtools: a non-empty session with active panel = Inspector).

3. **Verify existing tests pass** — the new structure should be a pure refactor; no behavioral change. Existing tests covering `Widget::render` and `render_with_regions` should still pass without modification.

## Acceptance Criteria

- [ ] Each of the three panels has a private `render_impl(self, area, buf, ctx: Option<&mut MouseCtx<'_>>)` method.
- [ ] `Widget::render` for each panel calls `render_impl(... None)`.
- [ ] `render_with_regions` for each panel calls `render_impl(... Some(ctx))`.
- [ ] No background-clear loop or state-guard block is duplicated between `Widget::render` and `render_with_regions`.
- [ ] One byte-identical-buffer parity test per panel (3 new tests total).
- [ ] All existing devtools / performance / inspector tests still pass.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets` pass.

## Notes

- **Do not touch** `widgets/devtools/network/` or `widgets/devtools/inspector/tree_panel.rs` in this task. The network panel already follows the correct pattern. `tree_panel.rs` is owned by Task 09.
- **Do not touch** `widgets/log_view/` — the log-view widget already correctly uses `render_inner` (Task 06 docstring update is in Task 06's scope, not here).
- The test fixture should be minimal — just enough to render a clickable surface in the non-degenerate state. Fixture builders likely already exist in each panel's `tests.rs`; reuse them.
- If a panel's `Widget::render` already had divergent semantics from `render_with_regions` (e.g., mismatched padding) that the byte-identical test surfaces, that is a real bug — file a follow-up note in the Completion Summary and decide whether to fix in this task or open a separate ticket. Do not paper over a real divergence.

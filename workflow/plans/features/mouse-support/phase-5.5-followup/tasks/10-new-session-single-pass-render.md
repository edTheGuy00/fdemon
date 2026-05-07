# Task 10: NewSessionDialog Single-Pass Render (Drop Duplicate Calls)

## Goal

Refactor `widgets/new_session_dialog/mod.rs` to render the fuzzy modal overlay and the launch-context fields exactly once per frame (Minor #14). Currently, when a `MouseCtx` is present, the modal/context is rendered twice — once for visual pixels, once for region recording.

## Background

`widgets/new_session_dialog/mod.rs::render_horizontal_with_regions` (lines ~797-816) and `render_vertical_with_regions` (lines ~869+) currently call `self.render_fuzzy_modal_overlay(dialog_area, buf)` (paint) and then `fuzzy_modal::fuzzy_modal_render_with_regions(...)` (paint AGAIN + register regions). The comment acknowledges this with "fuzzy_modal_render_with_regions will re-render them (idempotent)", but it doubles the render cost on the hot path.

Same pattern in `launch_context.rs::launch_context_render_with_regions` (lines ~1267, 1296): calls `Widget::render` for pixels, then walks the layout tree again to register regions.

The fix is to ensure that when a `MouseCtx` is present, only the `_render_with_regions` variant is called (it both paints AND registers). When `MouseCtx` is `None`, the no-region path renders once via the regular widget `Widget::render`.

## Files

**Modify:**
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` — drop duplicate `render_fuzzy_modal_overlay` call when ctx is `Some`; same for `launch_context`.

**Read (reference):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` — verify `launch_context_render_with_regions` is a complete render path (calls all the same paint code as `Widget::render`)
- `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` — verify `fuzzy_modal_render_with_regions` is a complete render path

## Plan

1. **Audit `fuzzy_modal_render_with_regions`** to confirm it paints the full modal (Clear, block, search input, separator, list, hints) — not just regions. Read the function body. If any paint step is missing (e.g., the dim background), `render_fuzzy_modal_overlay` may still be needed before the regions call. If complete, the regions call replaces the overlay call.

2. **Update `render_horizontal_with_regions` and `render_vertical_with_regions`**:
   ```rust
   // Before (in current code):
   self.render_fuzzy_modal_overlay(dialog_area, buf);
   if let Some(modal_state) = &self.state.fuzzy_modal {
       let fuzzy_widget = FuzzyModal::new(modal_state, /* ... */);
       fuzzy_modal::fuzzy_modal_render_with_regions(dialog_area, buf, fuzzy_widget, Some(c));
   }

   // After:
   if let Some(modal_state) = &self.state.fuzzy_modal {
       let fuzzy_widget = FuzzyModal::new(modal_state, /* ... */);
       // Single-pass: render_with_regions paints AND registers regions.
       fuzzy_modal::fuzzy_modal_render_with_regions(
           dialog_area,
           buf,
           fuzzy_widget,
           ctx_arg, // Some(c) when ctx is present, else None
       );
   }
   ```
   The trick is that `Widget::render` for `NewSessionDialog` (no ctx path) might still want the `render_fuzzy_modal_overlay` call (which paints) or might want `fuzzy_modal_render_with_regions(_, _, _, None)`. Audit and unify.

3. **Same treatment for `launch_context`** — review `launch_context_render_with_regions` and ensure the call site in `mod.rs` does NOT precede it with a separate `Widget::render` call.

4. **Verify visual identity**: render NewSessionDialog at 100×40 with the fuzzy modal open, both via the new single-pass path and (in a test) via `Widget::render` (no ctx). Compare buffers — they should be byte-identical.

5. **Add a single-pass invariant test** in `widgets/new_session_dialog/tests.rs` (or wherever existing fuzzy modal tests live):
   ```rust
   #[test]
   fn fuzzy_modal_renders_once_per_frame_in_with_regions_path() {
       // Render with Some(ctx) and inspect that the modal area is painted exactly once.
       // Concrete check: buffer contents match the same area rendered via fuzzy_modal_render_with_regions(_, _, _, None).
       // (Idempotence is preserved; this guards against accidental re-introduction of the duplicate call.)
   }
   ```

6. **Quality gates**:
   ```bash
   cargo test -p fdemon-tui widgets::new_session_dialog
   cargo test --workspace
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

## Acceptance Criteria

- [ ] `render_horizontal_with_regions` and `render_vertical_with_regions` no longer call `render_fuzzy_modal_overlay` followed by `fuzzy_modal_render_with_regions` when ctx is present.
- [ ] Same pattern eliminated for `launch_context_render_with_regions`.
- [ ] Buffer output is byte-identical to the pre-fix output (visual regression check).
- [ ] Region count is unchanged (regression check).
- [ ] 1 new test asserting single-pass invariant.
- [ ] Quality gates pass.

## Notes

- This is a perf/cleanup task, not a correctness fix. The pre-fix code was idempotent (no visual regression); we simply avoid paying the cost twice.
- T02 fixes the underflow in `fuzzy_modal.rs` itself. T10 modifies only call sites in `mod.rs`. **No file overlap with T02.**
- If the audit in step 1 reveals that `fuzzy_modal_render_with_regions` does NOT paint the full modal (e.g., relies on a prior `Clear` from `render_fuzzy_modal_overlay`), then T10's scope expands to also update `fuzzy_modal_render_with_regions` to paint everything. In that case, T10 would write `fuzzy_modal.rs` and conflict with T02 — escalate to the orchestrator (sequential T02 → T10) or merge T10 into T02. The cleanest path forward is to keep this task narrow: only re-arrange call sites in `mod.rs`. If `fuzzy_modal_render_with_regions` needs widening, file as a separate task.
- The "double render" cost in absolute terms is small (modal is typically ~30 rows × ~40 cols = ~1200 cells ≈ ~1ms re-paint). At 60fps that's 60ms/sec of needless work. Worth fixing but not urgent.

## Task: Remove `_visible` Placeholder, Consolidate Per-Frame `inspector_rows()` Call, Rename `ExitDevToolsMode → DevToolsEscape`, Extract Shared Test Helper

**Objective**: Clean up the placeholder + duplicate-build debt left over from Phase 1 task 09. Drop the dead `_visible` parameter on `render_tree_panel_inner`; build `inspector_rows()` exactly once per render frame and thread it through the inspector + details renderers; rename the misleading `Message::ExitDevToolsMode` variant; extract the duplicated `collect_buf_text` test helper.

**Depends on**: 04 (same files `tree_panel.rs` + `tests.rs`), 06 (consumes the consolidated row-slice signature)

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs` — remove `_visible` parameter; change signature to accept `rows: &[InspectorRow<'_>]` if it doesn't already; remove the comment block referencing task 09.
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` — build `inspector_rows()` once at the top of `render_impl`; pass the slice to both `render_tree_panel_inner` and `render_details_panel`.
- `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs` — drop the empty-slice argument from test helpers; extract `collect_buf_text` into a shared module (e.g. a private `test_helpers` module accessible to all inspector test modules).
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` — accept `rows: &[InspectorRow<'_>]` instead of calling `visible_nodes()` internally; remove the `refs` re-collect; rename `_tab` → `tab` (if task 03 missed any).
- `crates/fdemon-app/src/message.rs` — rename `ExitDevToolsMode` variant to `DevToolsEscape`; update doc comment to reflect the tiered semantics.
- `crates/fdemon-app/src/handler/keys.rs` — update the rename + fix the misleading Up/Down comment at lines 633-638 (the comment claims Up/Down work in both modes; the handler swallows them when `details_open`).
- `crates/fdemon-app/src/handler/update.rs` — update the match arm dispatching `ExitDevToolsMode` → `DevToolsEscape`.

**Files Read (Dependencies):**
- None.

### Review Items Resolved

- **M2** — `_visible` placeholder param survived Phase 1
- **M5** — `inspector_rows()` rebuilt 3–4× per frame (per-frame consolidation)
- **m4** — `Message::ExitDevToolsMode` variant name misleading
- **m6** — KEYBINDINGS.md ↔ `keys.rs:633-638` comment disagree about Up/Down (this task fixes the comment; task 10 fixes the docs)
- **m11** — `details/mod.rs:95-98` re-collects `visible` into `refs` for no semantic gain
- **m13** — Six identical `collect_buf_text` test helpers across `details/*`, `tests.rs`, `layout_panel_tests.rs`

### Details

#### M2 + M5 — Consolidate `inspector_rows()` per frame

Currently `render_impl` (or whatever the inspector tab's render entrypoint is in `inspector/mod.rs`) computes `visible_nodes()` and `inspector_rows()` separately; `tree_panel.rs` and `details/mod.rs` each rebuild the row list. Switch to:

```rust
// inspector/mod.rs render_impl (or equivalent)
let rows = self.inspector_state.inspector_rows();
// ...
self.render_tree_panel_inner(tree_area, buf, &rows, selected, tree_ctx);
// ...
if let Some(right_area) = layout_area {
    if self.inspector_state.details_open {
        self.render_details_panel(right_area, buf, &rows);
    } else {
        self.render_layout_panel(right_area, buf);
    }
}
```

Change the signatures:

```rust
// tree_panel.rs
pub(super) fn render_tree_panel_inner(
    &self,
    area: Rect,
    buf: &mut Buffer,
    rows: &[InspectorRow<'_>],   // ← replaces _visible
    selected: usize,
    mut ctx: Option<&mut MouseCtx<'_>>,
) {
    // existing body — already uses inspector_rows() internally; remove that
    // internal rebuild and use the passed-in `rows` slice.
}

// details/mod.rs
pub(super) fn render_details_panel(
    &self,
    area: Rect,
    buf: &mut Buffer,
    rows: &[InspectorRow<'_>],   // ← new param replacing visible_nodes() call
) {
    // ... use `rows` to look up the selected row by inspector_state.selected_index
}
```

Inside `render_details_panel`, remove the `let visible = self.inspector_state.visible_nodes();` and the `refs: Vec<...> = visible.iter().map(...).collect()` re-collect (m11). Use the slice directly.

Update test helpers in `tests.rs` to construct an empty row slice or a real one as appropriate. The fixtures should now look like:

```rust
let rows: Vec<InspectorRow<'_>> = Vec::new();
widget.render_tree_panel_inner(buf.area, buf, &rows, selected, None);
```

#### m4 — Rename `Message::ExitDevToolsMode` → `Message::DevToolsEscape`

Mechanical rename. Use `rg "ExitDevToolsMode" crates/` to enumerate all call sites:
- Variant definition in `message.rs`.
- Match arm in `handler/update.rs` (the one routed through `handle_devtools_escape`).
- Producer in `handler/keys.rs:555` (or wherever the Esc keybinding produces it).
- Any tests that reference the variant by name.

Update the variant's doc comment to reflect the actual tiered behaviour:

```rust
/// Escape key pressed while in DevTools mode. The handler routes this
/// through [`handle_devtools_escape`]:
/// - Inspector tab + details open → close details, stay in DevTools.
/// - Otherwise → exit DevTools back to Logs.
DevToolsEscape,
```

After the rename, `cargo check` must be green before moving on within this task.

#### m6 part 1 — Fix `keys.rs:633-638` comment

The comment claims `Up`/`Down`/`j`/`k` "work in both tree and details modes." The handler swallows navigation when `details_open` is true. Update the comment to match reality:

```rust
// Navigation keys (Up/Down/j/k). Emitted in both tree and details modes;
// the handler returns no-op when `details_open == true` (selection
// frozen). See handler/devtools/inspector.rs::handle_inspector_navigate
// for the guard.
```

Docs (KEYBINDINGS.md) are updated in task 10.

#### m13 — Extract shared `collect_buf_text`

Six copies live in:
- `widgets/devtools/inspector/details/mod.rs` (tests module)
- `widgets/devtools/inspector/details/properties_tab.rs` (tests)
- `widgets/devtools/inspector/details/render_object_tab.rs` (tests)
- `widgets/devtools/inspector/details/flex_explorer_tab.rs` (tests)
- `widgets/devtools/inspector/tests.rs`
- `widgets/devtools/inspector/layout_panel_tests.rs` (or wherever it lives)

Extract one canonical copy into a `#[cfg(test)] mod test_helpers` (or `test_utils`) inside `widgets/devtools/inspector/mod.rs`, exposed `pub(super) fn` so all test modules in the subtree can use it. Delete the five duplicates and replace with imports.

If a wider scope is preferred (e.g. `widgets/devtools/test_helpers.rs`), that's acceptable — document the location choice.

### Acceptance Criteria

1. `render_tree_panel_inner` no longer has the `_visible` parameter. Its signature now takes `rows: &[InspectorRow<'_>]`.
2. `render_details_panel` accepts `rows: &[InspectorRow<'_>]` and uses it directly; no `visible_nodes()` call inside.
3. `inspector/mod.rs::render_impl` calls `inspector_rows()` exactly once per frame and threads the slice to both renderers.
4. No `Message::ExitDevToolsMode` references remain in the codebase; all use `Message::DevToolsEscape`.
5. `keys.rs:633-638` comment accurately describes the Up/Down behaviour (emitted by keys, swallowed by handler when details_open).
6. One canonical `collect_buf_text` helper exists; the five duplicates are deleted and replaced with imports.
7. Existing inspector tests pass after argument updates.
8. New tests:
   - `render_inspector_calls_inspector_rows_only_once_per_frame`: instrumented mock (or measurement via a probe field) confirming the count. May be omitted if instrumentation is awkward; substitute with a code-review observation noting the call count is bounded to 1 in `render_impl`.
   - `render_tree_panel_with_empty_rows_slice_does_not_panic`: regression guard for narrow-terminal / first-render-before-fetch scenarios.
9. `cargo test --workspace` passes.
10. `cargo clippy --workspace --all-targets -- -D warnings` passes.

### Testing

The single-call assertion (criterion 8a) is hard to enforce in a unit test without instrumentation. Acceptable alternative: a code-comment in `render_impl` documenting the invariant, plus the structural fact that the slice is passed down (so neither callee rebuilds the list).

### Notes

- This task is the largest of the cleanup wave but is mechanical: signature changes, mechanical rename, helper extraction. No new logic.
- Wave: W5. Sequential with task 04 (same files `tree_panel.rs` + `tests.rs`).
- Sequential with task 06 (06 introduces the wiring this task threads the new row slice through). 06 lives in `handler/devtools/inspector.rs` which 09 does not touch, but the conceptual dependency is real — 09 should be written after 06's handler logic is in place.
- Task 10 (docs) consumes the variant rename and the comment fix.

---

## Completion Summary

**Status:** Not Started
**Branch:** —

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations

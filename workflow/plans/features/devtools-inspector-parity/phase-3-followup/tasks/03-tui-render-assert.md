## Task: Add `debug_assert!` to the renderer's tab-fallback path

**Objective**: Add a `debug_assert!` to the defensive dispatch fallback in `render_details_panel` so handler-side clamp regressions surface in dev/test builds. Renderer remains pure (assert is read-only).

**Depends on**: None

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `InspectorState::visible_tabs`, `details_tab`, `DetailsTab` (signatures only)
- `workflow/reviews/features/devtools-inspector-parity/phase-3/ACTION_ITEMS.md` — m2 spec

### Details

#### Background

The `risks_tradeoffs_analyzer` flagged the silent renderer fallback at `details/mod.rs:142-154` as MEDIUM severity (issue m2):

> When `state.details_tab` is not in `visible_tabs()`, the renderer falls back to `Properties` silently. The user-visible symptom of a missed clamp is "tab strip highlights tab X but content shows Properties" — confusing UX with no signal to developers.

The renderer's purity constraint (TEA pattern) means it must not mutate state — that's correct and not under debate. The fix is to make the invariant violation **observable in dev/test/CI** without breaking purity. `debug_assert!` is compiled out in release builds, so it imposes zero runtime cost in production and zero behavior change for end users.

This complements task 02's m1 fix: even if the timeout-clamp gap were never patched, the assertion would now catch any handler that fails to clamp before the renderer runs.

#### Current code (approximately lines 142–154 of `details/mod.rs`)

```rust
let visible = state.visible_tabs();
let dispatch_tab = if visible.contains(&state.details_tab) {
    state.details_tab
} else {
    visible.first().copied().unwrap_or(DetailsTab::Properties)
};
match dispatch_tab {
    DetailsTab::Properties => self.properties_tab.render(...),
    DetailsTab::RenderObject => self.render_object_tab.render(...),
    DetailsTab::FlexExplorer => self.flex_explorer_tab.render(...),
}
```

#### Proposed change

Add a `debug_assert!` at the top of the block, before the `dispatch_tab` calculation:

```rust
let visible = state.visible_tabs();
debug_assert!(
    visible.contains(&state.details_tab),
    "details_tab {:?} is not in visible_tabs {:?} — a handler missed a \
     clamp_details_tab() call. The renderer will fall back to the first \
     visible tab, but this masks a state inconsistency that should be \
     fixed in the handler layer. See workflow/reviews/features/\
     devtools-inspector-parity/phase-3/ACTION_ITEMS.md item m2.",
    state.details_tab,
    visible
);
let dispatch_tab = if visible.contains(&state.details_tab) {
    state.details_tab
} else {
    visible.first().copied().unwrap_or(DetailsTab::Properties)
};
```

The existing fallback logic remains unchanged — in release builds the assert is compiled out, and the fallback continues to handle the bad state gracefully.

### Acceptance Criteria

1. `render_details_panel` (or whichever function houses the dispatch block at `details/mod.rs:142–154`) contains a `debug_assert!` that fires when `state.details_tab` is not in `visible_tabs()`.
2. The renderer remains pure — no `&mut self`, no state mutation. The existing test `details_panel_falls_back_to_properties_when_active_tab_hidden` continues to pass (the assert message points to the fix-up location; the renderer still falls back correctly).
3. In release builds (`cargo build --release`), the assert is compiled out — no runtime cost.
4. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

The existing test `details_panel_falls_back_to_properties_when_active_tab_hidden` exists specifically to validate the fallback path. With the new `debug_assert!`, **that test will now panic in debug builds** (which is the standard test build profile — `cargo test` runs in debug by default).

Two ways to keep the test running:

**Option A (preferred): rename and refactor the test to exercise both the post-clamp happy path AND the production-only fallback.**

Replace the existing fallback test with two tests:

```rust
#[test]
fn details_panel_renders_active_tab_when_visible() {
    // Happy path: details_tab is in visible_tabs; assert renders it.
    let state = /* state where details_tab = Properties and visible includes it */;
    let buf = render_for_state(&state);
    // Assert Properties content rendered.
}

// Removed: details_panel_falls_back_to_properties_when_active_tab_hidden
// Reason: with the new debug_assert!, this scenario is treated as a bug
// to surface in dev/test rather than silently absorb. The release-build
// fallback is preserved for end-user resilience but is not unit-tested.
```

**Option B: gate the assert behind a release-only check OR adjust the test to suppress the assert.** Not recommended — defeats the purpose of the assert.

Choose Option A. Update the test docstring/comment to explain the change, and reference the task / ACTION_ITEMS.md item m2 for context.

### Notes

- **Why `debug_assert!` and not `tracing::warn!`:** Per cross-cutting constraint #5 in `TASKS.md`: production renderer fallback works correctly; the value of the assert is dev/test/CI feedback, not production observability. `debug_assert!` is the right tool — compiled out in release, screams loudly in test.
- **Why this doesn't make the renderer impure:** `debug_assert!` only reads from `state.details_tab` and the locally computed `visible` — both already read by the dispatch logic itself. No new mutations, no side effects in the rendering data path.
- **Coordination with task 02:** Once task 02 lands the timeout-clamp fix (m1), there is no known handler path that can leave `details_tab` out of `visible_tabs()`. The assert is then a future-regression guard, not a current-bug catcher.

---

## Completion Summary

**Status:** Pending
**Branch:** _to be filled_

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | _to be filled_ |

### Notable Decisions/Tradeoffs

1. _to be filled_

### Testing Performed

- _to be filled_

### Risks/Limitations

1. _to be filled_

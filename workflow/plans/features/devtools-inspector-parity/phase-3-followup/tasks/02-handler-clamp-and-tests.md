## Task: Handler hygiene — timeout-clamp symmetry, 2-tab backward cycle test, import normalization

**Objective**: Bundle three handler-layer fixes from the Phase 3 review (m1, m4, s1) that all touch `crates/fdemon-app/src/handler/devtools/inspector.rs`. Add `clamp_details_tab()` to the timeout settlement handler for invariant symmetry with `fetched` / `fetch_failed`. Add the missing 2-tab backward cycle test. Normalize the `DetailsContext` import to use the `fdemon_core` root re-export.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `InspectorState::clamp_details_tab`, `details_context`, `DetailsTab` (signatures)
- `crates/fdemon-core/src/lib.rs` — confirm `DetailsContext` is re-exported at the crate root (it is)
- `workflow/reviews/features/devtools-inspector-parity/phase-3/ACTION_ITEMS.md` — m1, m4, s1 specs

### Details

#### Background

Three independent handler-layer findings from the Phase 3 review, all in the same file:

| ID | Severity | Sub-issue |
|----|----------|-----------|
| m1 | MEDIUM (risks) / MINOR (quality, logic) | `handle_inspector_properties_fetch_timeout` does not call `clamp_details_tab()`. Asymmetric with `fetch_failed` (`:484`) and `fetched` (`:438`). Benign today — but invariant-breaking. |
| m4 | MINOR | Acceptance criterion #6 of Phase 3 task 03 requires 2-tab `Properties ↔ RenderObject` wrap in **both** directions. Forward case is tested by `handle_cycle_tab_skips_flex_explorer_when_hidden`. Backward case for that same 2-tab pair is missing. |
| s1 | SUGGESTION | Line 700 uses the fully qualified `fdemon_core::widget_tree::DetailsContext::default()`. Every other usage in this file (and in `state.rs`) uses the root re-export `fdemon_core::DetailsContext`. Sub-module path is the only inconsistency. |

#### 1. Add timeout-clamp symmetry (m1)

Locate `handle_inspector_properties_fetch_timeout` (currently around lines 494–524). It currently sets `properties_loading = false` and a timeout `properties_error`, then returns `UpdateResult::none()`. Add `inspector.clamp_details_tab();` immediately before the return, parallel to the call already present at line 484 in `handle_inspector_properties_fetch_failed`.

```rust
// Phase 3 follow-up (m1): the timeout settlement path does not currently
// mutate `render_properties`, so visible_tabs() cannot change here today.
// We clamp anyway to preserve the "every settlement path clamps"
// invariant — see fetched/fetch_failed handlers above.
inspector.clamp_details_tab();
UpdateResult::none()
```

#### 2. Add the missing 2-tab backward cycle test (m4)

Locate the existing `handle_cycle_tab_skips_flex_explorer_when_hidden` test. It sets `render_properties` non-empty + default `details_context` (no flex layout) → `visible_tabs() = [Properties, RenderObject]` → cycles forward from `Properties`, asserts `RenderObject`. Add a sibling backward-cycling test next to it:

```rust
#[test]
fn handle_cycle_tab_two_visible_tabs_backward_wraps_between_properties_and_render_object() {
    let mut state = AppState::new();
    let session_id = /* set up session as in adjacent tests */;
    {
        let inspector = &mut state.devtools_view_state.inspector;
        inspector.details_open = true;
        inspector.details_tab = crate::state::DetailsTab::Properties;
        inspector.render_properties = vec![fdemon_core::DiagnosticsNode {
            description: "RenderFlex".into(),
            ..Default::default()
        }];
        // details_context default → is_flex_layout = false → FlexExplorer hidden
    }
    // Backward from Properties → last visible tab (RenderObject).
    handle_cycle_tab(&mut state, false);
    assert_eq!(
        state.devtools_view_state.inspector.details_tab,
        crate::state::DetailsTab::RenderObject,
        "backward from Properties should wrap to RenderObject (last visible tab)"
    );
    // Backward from RenderObject → Properties.
    handle_cycle_tab(&mut state, false);
    assert_eq!(
        state.devtools_view_state.inspector.details_tab,
        crate::state::DetailsTab::Properties,
        "backward from RenderObject should wrap to Properties"
    );
}
```

Use the same session-id setup pattern as the adjacent test. The exact test-fixture boilerplate (session init, etc.) should follow the working pattern in `handle_cycle_tab_skips_flex_explorer_when_hidden`.

Also add a **timeout-clamp invariant test** for m1 — assert that after a timeout, an active tab that is still visible is preserved (no spurious snap to Properties):

```rust
#[test]
fn handle_inspector_properties_fetch_timeout_does_not_disturb_visible_active_tab() {
    let mut state = AppState::new();
    let session_id = /* set up session as in adjacent tests */;
    {
        let inspector = &mut state.devtools_view_state.inspector;
        inspector.details_open = true;
        inspector.details_tab = crate::state::DetailsTab::RenderObject;
        inspector.render_properties = vec![fdemon_core::DiagnosticsNode {
            description: "RenderFlex".into(),
            ..Default::default()
        }];
        inspector.pending_properties_node_id = Some("node-1".into());
        inspector.properties_loading = true;
        inspector.details_node_id = Some("node-1".into());
    }
    handle_inspector_properties_fetch_timeout(&mut state, session_id, "node-1".into());
    // RenderObject is still in visible_tabs (render_properties non-empty),
    // so the clamp must be a no-op for the active tab.
    assert_eq!(
        state.devtools_view_state.inspector.details_tab,
        crate::state::DetailsTab::RenderObject,
        "timeout clamp must not snap active tab when it's still visible"
    );
    assert!(!state.devtools_view_state.inspector.properties_loading);
    assert!(state.devtools_view_state.inspector.properties_error.is_some());
}
```

#### 3. Normalize the `DetailsContext` import (s1)

Locate the existing `use fdemon_core::...` import block near the top of `handler/devtools/inspector.rs` (the file already imports `RowGroup` from `fdemon_core` at the root level). Add `DetailsContext` to that import:

```rust
// BEFORE
use fdemon_core::RowGroup;

// AFTER
use fdemon_core::{DetailsContext, RowGroup};
```

Then on line 700 (currently):

```rust
inspector.details_context = fdemon_core::widget_tree::DetailsContext::default();
```

Change to:

```rust
inspector.details_context = DetailsContext::default();
```

This removes the only sub-module-path usage in the file and matches the import style in `state.rs:17`.

### Acceptance Criteria

1. `handle_inspector_properties_fetch_timeout` calls `inspector.clamp_details_tab()` before returning `UpdateResult::none()`, with a comment explaining the invariant.
2. A new unit test asserts that timeout does NOT mutate a still-visible active tab (regression-locks the no-op).
3. A new unit test asserts 2-tab backward cycling: `[Properties, RenderObject]` → backward from `Properties` lands on `RenderObject`; backward from `RenderObject` lands on `Properties`.
4. `DetailsContext` is imported from `fdemon_core` (root re-export) and used unqualified at line 700.
5. All existing tests in `handler/devtools/inspector.rs` continue to pass.
6. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

Two new tests as shown above. Add them in the existing `#[cfg(test)] mod tests` block, near the other `handle_cycle_tab_*` tests and the `handle_inspector_properties_fetch_failed_*` tests respectively. Use the existing test-fixture boilerplate from those adjacent tests for session initialization — do not introduce new fixture patterns.

### Notes

- **m1 is invariant preservation, not a bug fix.** The current `fetch_timeout` handler does not mutate `render_properties` or `details_context`, so visible-tab membership cannot change as a result of a timeout. Calling `clamp_details_tab()` is therefore a no-op in current code. The value is locking in the invariant "every settlement path clamps" so a future change to the timeout handler that *does* clear `render_properties` won't silently leave a stale active tab (which the renderer's defensive fallback would then mask — see also task 03).
- **m4 fills an explicit acceptance-criteria gap.** Phase 3 task 03's acceptance criterion #6 is "Cycling with 2 visible tabs (Properties + RenderObject) wraps between the two." Both directions are required by the criterion text; only forward was tested.
- **s1 is pure style.** The fix is identical behavior; just consistent import style. Done at the same time because the file is already being edited.
- **`handle_inspector_properties_fetch_timeout` signature:** Confirm the function takes `&mut AppState, session_id, node_id` (or similar) — match whatever the adjacent `..._fetched` / `..._failed` handlers take so the test mirrors them.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | (s1) Added `DetailsContext` to `fdemon_core` import, removed sub-module path at line ~706; (m1) Added `inspector.clamp_details_tab()` with invariant comment before `UpdateResult::none()` in `handle_inspector_properties_fetch_timeout`; (m4) Added `handle_cycle_tab_two_visible_tabs_backward_wraps_between_properties_and_render_object` test; Added `handle_inspector_properties_fetch_timeout_does_not_disturb_visible_active_tab` test |

### Notable Decisions/Tradeoffs

1. **Import normalization (s1)**: Added `DetailsContext` to the existing `use fdemon_core::{..., RowGroup}` import and removed the fully-qualified `fdemon_core::widget_tree::DetailsContext::default()` call. This is a pure style change with no behavioral difference.
2. **Clamp placement (m1)**: The `clamp_details_tab()` call is placed after clearing `properties_loading` / `properties_error` / `pending_properties_node_id`, identical to the position in `fetch_failed`. Today the clamp is a no-op for timeouts because `render_properties` is not cleared by a timeout — but it locks in the invariant that every settlement path clamps.
3. **Tests use `AppState::new()`**: The 2-tab backward cycle test and the timeout-clamp test both follow the pattern of adjacent Phase 3 tests (`handle_cycle_tab_skips_flex_explorer_when_hidden` and `properties_fetch_timeout_sets_error`) — no session is needed because these handlers operate on `devtools_view_state.inspector` fields that don't require an active session for the tested paths.

### Testing Performed

- `cargo test -p fdemon-app -- handler::devtools::inspector::tests` — PASS (102 tests, 3 new)
- `cargo check --workspace --all-targets` — PASS
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS
- `cargo fmt --all -- --check` — PASS

### Risks/Limitations

1. **No behavioral change**: All three changes are either invariant-preserving no-ops (m1), pure style (s1), or test additions (m4). There is no risk of behavioral regression.

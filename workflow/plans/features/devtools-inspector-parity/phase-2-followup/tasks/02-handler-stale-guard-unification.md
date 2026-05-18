## Task: Unify stale-guard key across properties + layout handlers

**Objective**: Fix the close-details + reopen-on-different-node race condition (C2) in the properties fetch handler, and unify the stale-guard key with the layout fetch handler so both use `state.details_node_id` as the single source of truth (M2). Add regression tests for both scenarios.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `InspectorState` fields (`details_node_id`, `pending_*`, `last_fetched_*`)
- `crates/fdemon-app/src/message.rs` — Message variants for fetched / failed / timeout
- `workflow/reviews/features/devtools-inspector-parity/phase-2/REVIEW.md` — C2, M2 findings

### Details

#### Background

The properties stale-guard at `inspector.rs:409-411` uses `pending_properties_node_id == response.node_id` as its discard predicate. The layout stale-guard at `inspector.rs:312-321` uses `pending_node_id == selected_value_id()`. Both work in isolation but disagree on the comparison key, and one of them (properties) is reachable to a real bug:

**C2 reproduction:**

1. User opens details on node A → `pending_properties_node_id = Some("A")`, `properties_loading = true`. Spawn task starts.
2. User presses Esc. `handle_close_details` clears `details_open` and `details_node_id`, but **leaves `pending_properties_node_id` and `properties_loading` untouched**.
3. User navigates tree to node B and presses Enter. `handle_open_details` checks `need_properties && !properties_loading`. `properties_loading` is still `true` → **no new fetch is dispatched**. State now: `details_node_id = Some("B")`, `pending_properties_node_id = Some("A")`.
4. Background task for A completes, emits `DevToolsInspectorPropertiesFetched { node_id: "A", ... }`.
5. Stale guard checks `pending_properties_node_id ("A") == response.node_id ("A")` → match → response is applied. State: `properties` and `render_properties` populated with A's data, but `details_node_id == Some("B")`.
6. User sees B's details panel populated with A's properties.

**Fix:** Use `state.details_node_id` as the comparison key for both stale guards. `details_node_id` is the field that actually drives the details panel render — it's the right source of truth.

#### 1. Convert properties handlers to use `details_node_id` as the stale-guard key

**Location:** `handler/devtools/inspector.rs` — three handler functions:
- `handle_inspector_properties_fetched(state, session_id, node_id, widget_properties, render_properties)` (~line 392-422)
- `handle_inspector_properties_fetch_failed(state, session_id, node_id, err)` (~lines after fetched)
- `handle_inspector_properties_fetch_timeout(state, session_id, node_id)` (~lines after failed)

**Pattern (apply to all three):**

```rust
pub fn handle_inspector_properties_fetched(
    state: &mut AppState,
    session_id: Uuid,
    node_id: String,
    widget_properties: Vec<DiagnosticsNode>,
    render_properties: Vec<DiagnosticsNode>,
) -> UpdateResult {
    let Some(active_id) = state.session_manager.active_id() else {
        return UpdateResult::none();
    };
    if active_id != session_id {
        return UpdateResult::none();
    }

    let inspector = &mut state.devtools.inspector;

    // Stale-guard: only apply if the response matches the currently-displayed
    // details panel. Discards orphan responses from closed-then-reopened-on-
    // different-node races (Phase 2 follow-up C2).
    if inspector.details_node_id.as_deref() != Some(node_id.as_str()) {
        // Optional: clear pending if it still points to this stale node, but
        // do NOT touch properties/render_properties since they belong to the
        // (no-longer-displayed) prior node.
        if inspector.pending_properties_node_id.as_deref() == Some(node_id.as_str()) {
            inspector.pending_properties_node_id = None;
            inspector.properties_loading = false;
        }
        return UpdateResult::none();
    }

    // Apply the response.
    inspector.properties = widget_properties;
    inspector.render_properties = render_properties;
    inspector.properties_loading = false;
    inspector.properties_error = None;
    inspector.pending_properties_node_id = None;
    inspector.last_fetched_properties_node_id = Some(node_id);

    UpdateResult::none()
}
```

The key change: **comparison is now `details_node_id == response.node_id`**, not `pending_properties_node_id == response.node_id`. The pending-id field is retained (it's still used for the cache logic in `handle_open_details`), but it's no longer authoritative for staleness checks.

Apply the same pattern to `handle_inspector_properties_fetch_failed` and `handle_inspector_properties_fetch_timeout`. In the failed/timeout cases, set `properties_error = Some(...)` instead of mutating `properties`. The stale-guard logic is identical.

#### 2. Convert layout handlers to use `details_node_id` as the stale-guard key

**Location:** `handler/devtools/inspector.rs:301-339` — `handle_layout_data_fetched` (and its sibling failed/timeout handlers).

**Current logic** (paraphrased):
```rust
let selected = state.devtools.inspector.selected_value_id();
if inspector.pending_node_id.as_deref() != selected.as_deref() {
    return UpdateResult::none();
}
```

**New logic (unified):**
```rust
if inspector.details_node_id.as_deref() != Some(node_id.as_str()) {
    if inspector.pending_node_id.as_deref() == Some(node_id.as_str()) {
        inspector.pending_node_id = None;
        inspector.layout_loading = false;
    }
    return UpdateResult::none();
}
```

This is a behavior change: the layout handler previously rejected responses when the tree-selection moved away from the fetched node, even if details was still open on that node. The new behavior accepts a response if `details_node_id` still matches — which is the field that actually drives the rendered layout panel. This is arguably more correct: a user who navigates the tree (changing selection) while details is open should still see the in-flight layout response for the open-details node, not a discarded one.

**Update or remove the test** that asserts the old `selected_value_id()` semantics (likely in `handler/devtools/inspector.rs` tests module). Replace with assertions about `details_node_id`.

#### 3. New regression test for the C2 race

Add this test to the existing `#[cfg(test)] mod tests` block in `handler/devtools/inspector.rs`:

```rust
#[test]
fn properties_response_discarded_when_user_reopened_details_on_different_node() {
    // Reproduces C2: open A → close → open B → A's fetch completes → B's
    // details must not be mutated.
    let mut state = test_state_with_session();

    // Step 1: open details on A. Schedules a fetch, sets pending=A.
    state.devtools.inspector.details_open = true;
    state.devtools.inspector.details_node_id = Some("A".into());
    state.devtools.inspector.pending_properties_node_id = Some("A".into());
    state.devtools.inspector.properties_loading = true;

    // Step 2: user closes details (simulates handle_close_details behavior)
    state.devtools.inspector.details_open = false;
    state.devtools.inspector.details_node_id = None;
    // pending and loading deliberately left as-is — this is the original
    // close-details behavior that opens the race.

    // Step 3: user reopens details on B. Loading is still true so no new
    // fetch dispatches; pending stays at A.
    state.devtools.inspector.details_open = true;
    state.devtools.inspector.details_node_id = Some("B".into());

    // Step 4: A's response arrives.
    let session_id = state.session_manager.active_id().unwrap();
    let widget_props = vec![sample_node("colorA", "Color(0xff0000ff)")];
    let render_props = vec![];
    let result = handle_inspector_properties_fetched(
        &mut state,
        session_id,
        "A".into(),
        widget_props,
        render_props,
    );

    // Step 5: verify B's details were NOT mutated.
    let inspector = &state.devtools.inspector;
    assert!(inspector.properties.is_empty(),
        "properties for B must remain empty; A's response should be discarded");
    assert!(inspector.render_properties.is_empty(),
        "render_properties for B must remain empty");
    assert_eq!(inspector.details_node_id.as_deref(), Some("B"),
        "details_node_id should still point to B");

    // Step 6: pending should be cleared since A's fetch is now resolved
    assert!(inspector.pending_properties_node_id.is_none(),
        "pending should be cleared once A's stale response arrives");
    assert!(!inspector.properties_loading,
        "properties_loading should be cleared so user can refetch for B");

    assert_eq!(result, UpdateResult::none());
}
```

This test fails today and will pass after the fix.

#### 4. Companion test for the unified-key layout handler

```rust
#[test]
fn layout_response_applied_when_details_node_matches_even_if_selection_moved() {
    // Verifies the M2 unification: layout fetch is keyed on details_node_id,
    // not on tree selection. User can navigate the tree while details is open
    // and the in-flight layout response for the open-details node still applies.
    let mut state = test_state_with_session();
    state.devtools.inspector.details_open = true;
    state.devtools.inspector.details_node_id = Some("A".into());
    state.devtools.inspector.pending_node_id = Some("A".into());
    state.devtools.inspector.layout_loading = true;

    // User navigates tree to a different node (selection moves to B), but
    // details panel stays open on A.
    // (simulate via state.devtools.inspector.selected = "B" if applicable)

    let layout = LayoutInfo::default();
    let session_id = state.session_manager.active_id().unwrap();
    let result = handle_layout_data_fetched(
        &mut state, session_id, "A".into(), layout.clone(),
    );

    // Layout for A should be applied because details_node_id is A
    let inspector = &state.devtools.inspector;
    assert_eq!(inspector.layout.as_ref(), Some(&layout));
    assert!(!inspector.layout_loading);
    assert_eq!(result, UpdateResult::none());
}
```

(Adjust the test helper invocation to match the actual `handle_layout_data_fetched` signature in the codebase.)

### Acceptance Criteria

1. Both properties and layout handlers use `state.devtools.inspector.details_node_id` (not `selected_value_id()` or `pending_*_node_id`) as the stale-guard comparison key.
2. The new regression test `properties_response_discarded_when_user_reopened_details_on_different_node` passes.
3. The companion layout test `layout_response_applied_when_details_node_matches_even_if_selection_moved` passes.
4. Existing tests `properties_fetched_discards_stale_response` and similar continue to pass after being adapted to the new key semantics.
5. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

In addition to the two new tests above:
- Verify existing `handler/devtools/inspector.rs` tests covering details-open / details-close / cache-hit paths still pass.
- The cross-session guard tests (`properties_fetched_cross_session_guard` etc.) should still pass — those test session_id mismatch, which is a different layer of the guard.
- Confirm `handle_inspector_properties_fetch_failed` and `handle_inspector_properties_fetch_timeout` apply the same stale-guard pattern (mirror tests if there are existing ones for those).

### Notes

- The `pending_properties_node_id` field is retained because `handle_open_details` still reads it indirectly via the cache predicate (`last_fetched_properties_node_id != Some(node_id) || properties_error.is_some()`). Don't delete the field — only deprioritize it for stale-guard purposes.
- The fix changes layout-handler behavior in one observable way: when details is open on node A and the user moves tree selection to node B (without closing details), an in-flight layout response for A will now apply (previously it was discarded as "stale"). This is the intended new behavior — the field that drives display is `details_node_id`, not `selected`.
- If the codebase has a separate "free-form layout panel" that uses `selected_value_id()` independent of details-open (i.e. a non-modal layout view), the layout handler change may need a fork. Inspect `handle_layout_data_fetched`'s call context to confirm. Per the review, the layout fetch is only spawned from `handle_open_details`, so a single `details_node_id` key should suffice — but verify before changing.
- Do NOT touch `handle_close_details` in this task. The reviewer offered option (b) "clear pending fields on close" as an alternative; we're choosing option (a) "cross-check in fetched handler" per cross-cutting constraint #1 in TASKS.md. Closing details should remain a UI-state-only operation.

---

## Completion Summary

**Status:** Not Started

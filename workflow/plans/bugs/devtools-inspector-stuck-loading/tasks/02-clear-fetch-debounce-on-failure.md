## Task: Clear Fetch Debounce on Failure / Timeout

**Objective**: Ensure pressing `r` after a failed or timed-out widget tree fetch immediately re-issues the request, instead of being silently debounce-blocked for 2 seconds.

**Depends on**: 01-add-diagnostic-instrumentation

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`: Add a `clear_fetch_debounce(&mut self)` method on `InspectorState` that resets `last_fetch_time` (set it to `None`, or a sentinel value far in the past, depending on its type).
- `crates/fdemon-app/src/handler/devtools/inspector.rs`:
  - `handle_widget_tree_fetch_failed` (line ~80): call `inspector.clear_fetch_debounce()` after setting `loading = false` and storing the error.
  - `handle_widget_tree_fetch_timeout` (line ~224): same.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs`: Existing handler shapes.

### Details

Today the debounce flow:

1. `record_fetch_start()` stamps `last_fetch_time = Some(Instant::now())`.
2. `is_fetch_debounced()` (`state.rs:292-300`) returns `true` for 2 s after that stamp.
3. On RPC failure: `handle_widget_tree_fetch_failed` clears `loading = false` but **does not** touch `last_fetch_time`.
4. User presses `r` quickly → `is_fetch_debounced()` returns `true` → request silently discarded.

The fix is mechanical: clear the debounce when the fetch terminates non-successfully. We could do the same on success too, but success already produces a non-zero hit so a near-immediate `r` retry is rare and harmless if it falls through to the debounce.

```rust
// state.rs — new method on InspectorState
pub fn clear_fetch_debounce(&mut self) {
    self.last_fetch_time = None;
}

// handler/devtools/inspector.rs — handle_widget_tree_fetch_failed
pub fn handle_widget_tree_fetch_failed(state: &mut AppState, msg: WidgetTreeFetchFailed) -> UpdateResult {
    if let Some(inspector) = active_inspector_state(state) {
        inspector.loading = false;
        inspector.error = Some(msg.error);
        inspector.clear_fetch_debounce();  // <-- new line
        info!(error = %inspector.error.as_deref().unwrap_or(""), "Inspector: fetch failed, debounce cleared");
    }
    UpdateResult::default()
}
```

### Acceptance Criteria

1. After a `WidgetTreeFetchFailed` message, `is_fetch_debounced()` returns `false` immediately.
2. After a `WidgetTreeFetchTimeout` message, `is_fetch_debounced()` returns `false` immediately.
3. Pressing `r` in the UI immediately after a failed fetch enqueues a new `RequestWidgetTree` and fires a fresh RPC (verifiable via instrumentation logs from task 01).
4. Existing success path is unchanged (success still leaves `last_fetch_time` set to fetch-start).
5. `cargo test --workspace` passes; new unit tests cover both failure paths.

### Testing

Add unit tests in `handler/devtools/inspector.rs` test module:

```rust
#[test]
fn fetch_failed_clears_debounce() {
    let mut state = AppState::test_default();
    let inspector = active_inspector_state(&mut state).unwrap();
    inspector.record_fetch_start();
    assert!(inspector.is_fetch_debounced());

    handle_widget_tree_fetch_failed(&mut state, WidgetTreeFetchFailed { /* ... */ });

    let inspector = active_inspector_state(&mut state).unwrap();
    assert!(!inspector.is_fetch_debounced(), "debounce should clear on failure");
}

#[test]
fn fetch_timeout_clears_debounce() { /* analogous */ }
```

### Notes

- Type of `last_fetch_time` is likely `Option<Instant>` — confirm by reading `state.rs:292-300`.
- Don't forget to clear it in the (rarer) "fetch returned nothing" code path if one exists.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `clear_fetch_debounce()` method on `InspectorState` that sets `last_fetch_time = None` |
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | Called `clear_fetch_debounce()` in `handle_widget_tree_fetch_failed` and `handle_widget_tree_fetch_timeout`; added 4 new unit tests |

### Notable Decisions/Tradeoffs

1. **Success path unchanged**: `handle_widget_tree_fetched` does not clear `last_fetch_time`. Confirmed by `fetch_success_leaves_debounce_intact` test — after a successful fetch the 2-second cooldown remains active, which matches the task requirement ("success path is unchanged").
2. **`loading` guard still works**: After calling `clear_fetch_debounce()`, `loading` is already set to `false` by both handlers, so `is_fetch_debounced()` correctly returns `false` (both the `loading` guard and the `last_fetch_time` check are clear).
3. **No "fetch returned nothing" code path found**: The codebase has no handler that sets loading=false on an empty result without calling `handle_widget_tree_fetched` or `handle_widget_tree_fetch_failed`, so no additional path needed.

### Testing Performed

- `cargo test -p fdemon-app` — Passed (2172 tests)
- `cargo check --workspace` — Passed (no errors)
- `cargo clippy -p fdemon-app` — Passed (no warnings)

New tests added:
- `fetch_failed_clears_debounce` — verifies `is_fetch_debounced()` returns `false` after `handle_widget_tree_fetch_failed`
- `fetch_timeout_clears_debounce` — verifies `is_fetch_debounced()` returns `false` after `handle_widget_tree_fetch_timeout`
- `fetch_failed_no_session_does_not_clear_debounce` — verifies no state change when session_id does not match
- `fetch_success_leaves_debounce_intact` — verifies the success path does NOT clear `last_fetch_time`

### Risks/Limitations

1. **None identified**: The change is mechanical — two additional method calls in two handlers. No cross-cutting concerns or state invariant violations.

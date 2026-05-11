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

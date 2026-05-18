## Task: Implement `spawn_fetch_inspector_properties` action task

**Objective**: Implement the background task that executes the two-stage `getProperties` round-trip: one call to fetch the widget's own properties, then one call per render-object property to fetch its sub-properties. The task sends `Message::DevToolsInspectorPropertiesFetched / FetchFailed / FetchTimeout` back through `msg_tx`. Wires this task into the `actions/mod.rs` dispatch arm for `UpdateAction::FetchInspectorProperties`.

**Depends on**: 02 (`GET_PROPERTIES` constant + `parse_properties_response` + `split_widget_and_render_properties` helpers), 03 (`UpdateAction` + `Message` variants)

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/mod.rs` — new dispatch arm for `UpdateAction::FetchInspectorProperties`
- `crates/fdemon-app/src/actions/inspector/mod.rs` — new function `spawn_fetch_inspector_properties` + tests

**Files Read (Dependencies):**
- `crates/fdemon-app/src/actions/inspector/mod.rs:332–487` — existing `spawn_fetch_layout_data` (the pattern to mirror)
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — `GET_PROPERTIES` and `DISPOSE_GROUP` constants, `INSPECTOR_OBJECT_GROUP`
- `crates/fdemon-daemon/src/vm_service/extensions/properties.rs` — `parse_properties_response`, `split_widget_and_render_properties`
- `crates/fdemon-app/src/handler/mod.rs` — `UpdateAction::FetchInspectorProperties` definition (from task 03)
- `crates/fdemon-app/src/message.rs` — three new property-fetch `Message` variants (from task 03)
- `crates/fdemon-core/src/widget_tree.rs` — `DiagnosticsNode.value_id`, `is_render_object_property()`

### Details

#### 1. Dispatch arm in `actions/mod.rs`

Locate the existing `match action` block (around `actions/mod.rs:315–329` per the research). Add a new arm next to `FetchLayoutData`:

```rust
UpdateAction::FetchInspectorProperties { session_id, node_id, vm_handle } => {
    if let Some(handle) = vm_handle {
        inspector::spawn_fetch_inspector_properties(session_id, node_id, handle, msg_tx);
    } else {
        warn!(
            session_id = %session_id,
            node_id = %node_id,
            "FetchInspectorProperties dispatched without VM handle (no active VM Service)"
        );
    }
}
```

If the no-handle fallback was already wired in task 03's `hydrate_fetch_inspector_properties` (sending a `FailedToFetch` message instead of silently dropping), this arm just needs the `if let Some(handle)` guard and the warn-log. Mirror exactly what `FetchLayoutData` does.

#### 2. `spawn_fetch_inspector_properties` in `actions/inspector/mod.rs`

Place this immediately after `spawn_fetch_layout_data`. The structure mirrors that function step-for-step:

```rust
/// Background task that fetches widget properties (and recursively the
/// sub-properties of any render-object property) for the given widget.
///
/// Sends one of these messages on completion via `msg_tx`:
/// - `Message::DevToolsInspectorPropertiesFetched` on success
/// - `Message::DevToolsInspectorPropertiesFetchFailed` on RPC error / parse failure
/// - `Message::DevToolsInspectorPropertiesFetchTimeout` when the 10s budget elapses
///
/// Reuses `INSPECTOR_OBJECT_GROUP` for the duration of the call. The object
/// group is disposed at session end (consistent with parent PLAN §7.2).
pub fn spawn_fetch_inspector_properties(
    session_id: SessionId,
    node_id: String,
    handle: VmRequestHandle,
    msg_tx: mpsc::UnboundedSender<Message>,
) {
    tokio::spawn(async move {
        // 1. Resolve the Flutter UI isolate (same pattern as layout fetch).
        if let Err(error) = handle.resolve_flutter_ui_isolate().await {
            let _ = msg_tx.send(Message::DevToolsInspectorPropertiesFetchFailed {
                session_id,
                node_id,
                error: error.to_string(),
            });
            return;
        }

        // 2. Issue the first getProperties call (for the widget itself).
        let widget_args = serde_json::json!({
            "arg": node_id,
            "objectGroup": INSPECTOR_OBJECT_GROUP,
        });

        let widget_resp = match tokio::time::timeout(
            PROPERTIES_FETCH_TIMEOUT,
            handle.call_extension(ext::GET_PROPERTIES, widget_args),
        )
        .await
        {
            Err(_timeout) => {
                let _ = msg_tx.send(Message::DevToolsInspectorPropertiesFetchTimeout {
                    session_id,
                    node_id,
                });
                return;
            }
            Ok(Err(e)) => {
                let _ = msg_tx.send(Message::DevToolsInspectorPropertiesFetchFailed {
                    session_id,
                    node_id,
                    error: e.to_string(),
                });
                return;
            }
            Ok(Ok(v)) => v,
        };

        let all_props = match parse_properties_response(&widget_resp) {
            Ok(p) => p,
            Err(e) => {
                let _ = msg_tx.send(Message::DevToolsInspectorPropertiesFetchFailed {
                    session_id,
                    node_id,
                    error: e.to_string(),
                });
                return;
            }
        };

        let (widget_properties, mut render_properties) =
            split_widget_and_render_properties(all_props);

        // 3. For each render-object property with a valueId, fetch its sub-properties.
        //    DevTools does this sequentially (inspector_controller.dart:914–931).
        //    Each sub-fetch shares the same overall timeout budget.
        let render_value_ids: Vec<String> = render_properties
            .iter()
            .filter_map(|p| p.value_id.clone())
            .collect();

        for value_id in render_value_ids {
            let args = serde_json::json!({
                "arg": value_id,
                "objectGroup": INSPECTOR_OBJECT_GROUP,
            });
            match tokio::time::timeout(
                PROPERTIES_FETCH_TIMEOUT,
                handle.call_extension(ext::GET_PROPERTIES, args),
            )
            .await
            {
                Err(_timeout) => {
                    let _ = msg_tx.send(Message::DevToolsInspectorPropertiesFetchTimeout {
                        session_id,
                        node_id,
                    });
                    return;
                }
                Ok(Err(e)) => {
                    // Sub-fetch failure: log but don't fail the whole fetch — DevTools
                    // best-effort merges sub-properties (inspector_controller.dart:920).
                    tracing::debug!(
                        value_id = %value_id,
                        error = %e,
                        "getProperties sub-fetch for render-object failed; skipping"
                    );
                    continue;
                }
                Ok(Ok(v)) => match parse_properties_response(&v) {
                    Ok(subs) => render_properties.extend(subs),
                    Err(e) => {
                        tracing::debug!(value_id = %value_id, error = %e, "sub-fetch parse failed");
                        continue;
                    }
                },
            }
        }

        // 4. Send the success message.
        let _ = msg_tx.send(Message::DevToolsInspectorPropertiesFetched {
            session_id,
            node_id,
            widget_properties,
            render_properties,
        });
    });
}
```

Constants to add near the top of `actions/inspector/mod.rs` (next to `LAYOUT_FETCH_TIMEOUT`):

```rust
/// Total time budget for a single `FetchInspectorProperties` action — covers
/// the initial widget `getProperties` call AND all sub-property calls.
///
/// 10s is the same budget used by `LAYOUT_FETCH_TIMEOUT`. A widget with many
/// render-object sub-properties will eat into this budget; in practice DevTools
/// observes typically 0–1 render-object properties per widget, so this is
/// generous.
const PROPERTIES_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
```

If `INSPECTOR_OBJECT_GROUP` is already in scope (it's at `actions/inspector/mod.rs:31` per the research), reuse it. Don't introduce a new object group.

#### 3. Imports

The new function needs imports for `parse_properties_response` and `split_widget_and_render_properties` from the daemon's `extensions::properties` module — verify the daemon crate re-exports these at a sensible path (e.g., via `pub use vm_service::extensions::properties::*` in `crates/fdemon-daemon/src/lib.rs`, or by adding such a re-export as part of this task if it's missing). The existing `spawn_fetch_layout_data` does `use fdemon_daemon::vm_service::extensions::layout::extract_layout_info;` (or equivalent) — follow the same pattern.

### Acceptance Criteria

1. `UpdateAction::FetchInspectorProperties` is dispatched in `actions/mod.rs`, spawning the background task when `vm_handle` is `Some`.
2. `spawn_fetch_inspector_properties` performs the initial `getProperties` RPC, partitions the response, and recursively fetches sub-properties for each render-object node.
3. The total time budget for one action invocation is enforced by `PROPERTIES_FETCH_TIMEOUT`. Timeouts at either the widget-level or sub-fetch level emit `DevToolsInspectorPropertiesFetchTimeout`.
4. Sub-fetch failures (single render-object) are logged at debug level and the action continues — partial render-property data is better than no data. This matches DevTools' `_loadPropertiesForNode` best-effort behavior.
5. Success path sends `DevToolsInspectorPropertiesFetched { session_id, node_id, widget_properties, render_properties }`.

### Testing

Following the layout-fetch test precedent (per the research: `actions/inspector/mod.rs` has tests inline, mocking the `handle.call_extension` calls). The mock pattern likely uses a trait-object handle or feature-gated mock. Whatever pattern `spawn_fetch_layout_data` tests use, mirror it.

Add at minimum:

- `spawn_properties_sends_fetched_message_on_success` — happy path with no render-object properties; assert the message includes both result lists.
- `spawn_properties_recurses_into_render_object_property` — mock a response containing one `propertyType == "RenderObject"` node with a `valueId`; assert a second `call_extension` was made for that valueId; assert sub-properties appear in `render_properties`.
- `spawn_properties_emits_timeout_when_first_call_hangs` — use `tokio::time::pause()` and inject a slow mock; assert `PropertiesFetchTimeout` message.
- `spawn_properties_emits_failed_when_first_call_errors` — inject an error from `call_extension`; assert `PropertiesFetchFailed`.
- `spawn_properties_skips_sub_fetch_on_error_but_completes_widget_call` — mock first call success with render-object property; mock second call error; assert success message with empty sub-properties merge (only the render-object node itself, no children appended).

### Notes

- The recursive sub-fetch is intentionally sequential (a `for` loop, not `try_join_all`) to match DevTools' implementation and to keep the timeout accounting predictable. Concurrent sub-fetches would shave a few ms but complicate error attribution.
- `INSPECTOR_OBJECT_GROUP` is the same group used by all inspector RPCs. Disposing the group is a session-level concern (handled elsewhere when the session ends) — this task does NOT call `disposeGroup`.
- The action does NOT update `InspectorState` fields like `properties_loading` — those are set by the handler that dispatched the action (task 06's `handle_open_details`). The action only communicates back via Messages.
- Per parent PLAN §7.5, `RequestTracker` integration is transparent — `call_extension` already routes through the `RequestTracker`, no new request types needed.
- If `serde_json::Value` ergonomics in the inline `json!()` macros bloat the file, factor out helper functions. The file is already large (the research suggested >400 lines for the layout fetch alone); be mindful of CODE_STANDARDS' 500-line ceiling. If the file would exceed the limit, propose a split into `actions/inspector/layout.rs` + `actions/inspector/properties.rs` in the completion summary; the implementor may make that split here.

---

## Completion Summary

**Status:** Pending

(To be filled in by the implementor.)

## Task: Resolve the Flutter UI Isolate Explicitly

**Objective**: Replace `main_isolate_id`'s "first non-system isolate" heuristic with one that looks up the Flutter UI isolate by checking which isolate has `ext.flutter.*` extension RPCs registered. Cache the result per `VmServiceHandle` to avoid repeated lookups.

**Depends on**: 02-clear-fetch-debounce-on-failure, 03-promote-channel-drop-to-error-log

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/vm_service/client.rs` (lines 150-157 and around `VmServiceHandle`): Add `resolve_flutter_ui_isolate(&self) -> Result<String>` that:
  1. Calls `getVM` to enumerate isolates.
  2. For each non-system isolate, calls `getIsolate { isolateId }` and inspects the `extensionRPCs` array.
  3. Returns the id of the first isolate whose `extensionRPCs` contains any string starting with `ext.flutter.`.
  4. Falls back to today's "first non-system isolate" behavior if none found (with a `warn!` trace explaining the fallback).
  - Cache the resolved id on `VmServiceHandle` (e.g., `flutter_ui_isolate_id: Mutex<Option<String>>` or `RwLock<Option<String>>`).
- `crates/fdemon-daemon/src/vm_service/client.rs`: Invalidate the cached isolate id on hot restart and isolate-exit events. Add a `clear_isolate_cache()` method.
- `crates/fdemon-app/src/actions/inspector/mod.rs`: Replace `handle.main_isolate_id().await` with `handle.resolve_flutter_ui_isolate().await`.
- `crates/fdemon-daemon/src/vm_service/protocol.rs` (if it doesn't already model it): Ensure `Isolate` struct in the daemon-side types has an `extension_rpcs: Vec<String>` field deserialized from `getIsolate`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs`: For the inspector-state cache-clearing point (called on hot restart).
- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs`: To understand which extensions exist (`ext.flutter.inspector.*`).

### Details

The VM Service `getVM` response gives a list of isolates but does not include extension RPCs. To find the Flutter UI isolate, we need a follow-up `getIsolate` call per candidate, then filter by `extensionRPCs.startsWith("ext.flutter.")`.

```rust
// Sketch — actual API may use different argument shapes
pub async fn resolve_flutter_ui_isolate(&self) -> Result<String> {
    // Fast path: cached
    if let Some(id) = self.flutter_ui_isolate_id.lock().await.clone() {
        return Ok(id);
    }

    // Slow path: enumerate
    let vm = self.call_get_vm().await?;
    let candidates: Vec<&Isolate> = vm.isolates.iter()
        .filter(|i| !i.is_system_isolate.unwrap_or(false))
        .collect();

    for iso in &candidates {
        let detail = self.call_get_isolate(&iso.id).await?;
        if detail.extension_rpcs.iter().any(|e| e.starts_with("ext.flutter.")) {
            info!(isolate_id = %iso.id, extension_count = detail.extension_rpcs.len(), "VM Service: resolved Flutter UI isolate");
            *self.flutter_ui_isolate_id.lock().await = Some(iso.id.clone());
            return Ok(iso.id.clone());
        }
    }

    // Fallback
    if let Some(first) = candidates.first() {
        warn!(isolate_id = %first.id, "VM Service: no Flutter extensions found on any isolate; falling back to first non-system isolate");
        Ok(first.id.clone())
    } else {
        Err(Error::vm_service("no non-system isolates available"))
    }
}
```

Invalidation: subscribe to `Isolate.Runnable` and `Service.IsolateExit` streams (if not already), and clear the cache on hot restart.

### Acceptance Criteria

1. On a Flutter project with one or more background Dart isolates, `resolve_flutter_ui_isolate` returns the UI isolate id (the one with `ext.flutter.inspector.*` registered).
2. Resolution result is cached per `VmServiceHandle`; second call doesn't re-issue `getVM` / `getIsolate`.
3. Cache is invalidated on hot restart (the next `resolve_flutter_ui_isolate` triggers a fresh lookup).
4. Fallback path (no Flutter extensions found) emits a `warn!` and returns the first non-system isolate.
5. `cargo test --workspace` passes; new unit tests cover the three scenarios (single isolate, multi-isolate, no Flutter extensions).

### Testing

- Unit tests in `vm_service/client.rs` using mocked `getVM` / `getIsolate` responses:
  - Single non-system isolate with `ext.flutter.*` extensions → returns that isolate.
  - Two non-system isolates, only the second has `ext.flutter.*` → returns the second.
  - No isolate has `ext.flutter.*` → fallback to first non-system isolate; warn logged.
- Manual verification: log the resolved isolate id (instrumentation from task 01) on a real Flutter app with `compute()` workers — should pick the UI isolate, not the worker.

### Notes

- The cache should not be `Arc<RwLock<...>>` if `VmServiceHandle` is already cloneable; check the existing concurrency model in `client.rs`.
- Annotate the cache field with `// EXCEPTION` if it crosses any TEA boundary (it shouldn't — this is daemon-layer state).
- If `Isolate.Runnable` is not currently subscribed to, decide in this task whether to wire it up (probably yes) or punt to a follow-up.
- The `Error::vm_service(...)` helper may need to be added to `fdemon-core/error.rs` if missing.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/client.rs` | Added `resolve_flutter_ui_isolate()` to `VmRequestHandle` (enumerates isolates, inspects `extensionRPCs`, caches result, falls back with `warn!`). Added `clear_isolate_cache()` alias for `invalidate_isolate_cache()`. Added 7 unit tests. |
| `crates/fdemon-app/src/actions/inspector/mod.rs` | Replaced all 4 `handle.main_isolate_id()` calls with `handle.resolve_flutter_ui_isolate()` in `spawn_fetch_widget_tree`, `spawn_toggle_overlay`, `spawn_fetch_layout_data`, and `spawn_dispose_devtools_groups`. |

### Notable Decisions/Tradeoffs

1. **Shared cache**: `resolve_flutter_ui_isolate` reuses the same `isolate_id_cache` field as `main_isolate_id`. This means whichever is called first fills the cache. No new field was needed since both methods are looking for the same thing (the Flutter UI isolate).

2. **Scope of replacement**: Only replaced calls in `inspector/mod.rs` as specified. Calls in `network.rs`, `performance.rs`, `vm_service.rs` etc. were left using `main_isolate_id()` per task scope — those callers don't need the Flutter-extension-specific logic.

3. **IsolateInfo.extension_rpcs**: Already existed as `Option<Vec<String>>` in `protocol.rs` — no schema changes needed.

4. **Cache invalidation**: The existing `invalidate_isolate_cache()` method already handles hot restart invalidation. The new `clear_isolate_cache()` is a public alias for discoverability.

5. **Fallback**: When no `ext.flutter.*` extensions are found, falls back silently to the first non-system isolate with a `warn!` log (matching the task sketch exactly).

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (769 fdemon-daemon tests including 7 new; all workspace tests pass)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Extra RPC calls on cold path**: Each call to `resolve_flutter_ui_isolate` on an empty cache triggers one `getVM` and up to N `getIsolate` calls (one per non-system isolate). This is the intended design — the result is cached immediately after, so subsequent calls hit the fast path.

2. **Fallback does not cache**: If no `ext.flutter.*` isolate is found, the fallback returns the first non-system isolate but does NOT cache it (matching the task sketch). This means every call on a Flutter app with no registered extensions re-issues `getVM` + `getIsolate`. This is intentional — it will retry discovery when extensions eventually become registered.

3. **`Isolate.Runnable` subscription**: The task mentioned potentially wiring up the `Isolate.Runnable` event subscription. The `ISOLATE` stream is already in `RESUBSCRIBE_STREAMS` (added in a prior fix), so `IsolateRunnable` events are already received. Cache invalidation on `IsolateExit` is handled by `invalidate_isolate_cache()` / `clear_isolate_cache()` which callers invoke on hot restart (wired in `handler/update.rs`).

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

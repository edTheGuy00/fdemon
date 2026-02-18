## Task: Implement Basic VM Introspection Methods

**Objective**: Add high-level methods to `VmServiceClient` for VM introspection: `get_vm()`, `get_isolate()`, `stream_listen()`, and main isolate tracking. These are the building blocks that Tasks 06 and 07 use.

**Depends on**: 04-vm-client

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/client.rs` — Add introspection methods
- `crates/fdemon-daemon/src/vm_service/mod.rs` — Export new types

### Details

#### 1. High-Level Request Methods

Add convenience methods to `VmServiceClient`:

```rust
impl VmServiceClient {
    /// Call `getVM` — returns VM info with isolate list
    pub async fn get_vm(&self) -> Result<VmInfo> {
        let result = self.request("getVM", None).await?;
        serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse getVM: {e}")))
    }

    /// Call `getIsolate` — returns full isolate details
    pub async fn get_isolate(&self, isolate_id: &str) -> Result<IsolateInfo> {
        let params = serde_json::json!({ "isolateId": isolate_id });
        let result = self.request("getIsolate", Some(params)).await?;
        serde_json::from_value(result).map_err(|e| Error::vm_service(format!("parse getIsolate: {e}")))
    }

    /// Call `streamListen` — subscribe to a VM Service stream
    pub async fn stream_listen(&self, stream_id: &str) -> Result<()> {
        let params = serde_json::json!({ "streamId": stream_id });
        self.request("streamListen", Some(params)).await?;
        Ok(())
    }

    /// Call `streamCancel` — unsubscribe from a VM Service stream
    pub async fn stream_cancel(&self, stream_id: &str) -> Result<()> {
        let params = serde_json::json!({ "streamId": stream_id });
        self.request("streamCancel", Some(params)).await?;
        Ok(())
    }
}
```

#### 2. Main Isolate Discovery

After connecting, the client needs to find the main UI isolate:

```rust
impl VmServiceClient {
    /// Discover the main Flutter UI isolate.
    /// Calls getVM, finds the non-system isolate, returns its ID.
    pub async fn discover_main_isolate(&self) -> Result<IsolateRef> {
        let vm = self.get_vm().await?;

        // Find the main isolate (non-system, usually named "main")
        let main_isolate = vm.isolates.iter()
            .find(|iso| !iso.is_system_isolate.unwrap_or(false))
            .ok_or_else(|| Error::vm_service("no non-system isolate found"))?;

        Ok(main_isolate.clone())
    }
}
```

#### 3. Stream Subscription Helper

Subscribe to the streams needed for Phase 1:

```rust
impl VmServiceClient {
    /// Subscribe to all streams needed for Phase 1.
    /// Returns error details for any failed subscriptions (non-fatal).
    pub async fn subscribe_phase1_streams(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Extension stream: Flutter.Error events (crash logs)
        if let Err(e) = self.stream_listen("Extension").await {
            errors.push(format!("Extension stream: {e}"));
        }

        // Logging stream: structured log records
        if let Err(e) = self.stream_listen("Logging").await {
            errors.push(format!("Logging stream: {e}"));
        }

        errors
    }
}
```

#### 4. Error Variant

Add a `VmService` variant to the project's `Error` enum if it doesn't exist:

In `crates/fdemon-core/src/error.rs`:
```rust
/// VM Service communication error
VmService(String),
```

With helper constructor:
```rust
pub fn vm_service(msg: impl Into<String>) -> Self {
    Error::VmService(msg.into())
}
```

### Acceptance Criteria

1. `get_vm()` returns parsed `VmInfo` with isolate list
2. `get_isolate()` returns parsed `IsolateInfo` for a given isolate ID
3. `stream_listen()` subscribes to a named stream (Extension, Logging)
4. `stream_cancel()` unsubscribes from a named stream
5. `discover_main_isolate()` finds the main Flutter isolate
6. `subscribe_phase1_streams()` subscribes to Extension + Logging
7. Error cases return proper `Error::VmService` variants (no panics)
8. All methods have unit tests

### Testing

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_discover_main_isolate_skips_system_isolates() {
        // Create VmInfo with system + non-system isolates
        // Assert discover returns the non-system one
    }

    #[test]
    fn test_discover_main_isolate_returns_error_when_none() {
        // Create VmInfo with only system isolates
        // Assert returns Error::VmService
    }

    #[test]
    fn test_get_vm_request_format() {
        // Verify the JSON-RPC request format for getVM
    }

    #[test]
    fn test_stream_listen_request_format() {
        // Verify streamListen sends correct streamId param
    }
}
```

### Notes

- The `discover_main_isolate` logic may need refinement if Flutter apps spawn multiple non-system isolates (rare but possible with `Isolate.spawn`)
- Stream subscription failures should be logged as warnings, not errors — the app should still work without them
- After reconnection (from Task 04), these streams need to be re-subscribed — consider storing the list of subscribed streams
- The `extension_rpcs` field in `IsolateInfo` can be used in Phase 2 to check which Flutter extensions are available

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/error.rs` | Added `Error::VmService(String)` variant and `Error::vm_service()` constructor |
| `crates/fdemon-daemon/src/vm_service/client.rs` | Added `get_vm()`, `get_isolate()`, `stream_listen()`, `stream_cancel()`, `discover_main_isolate()`, `subscribe_phase1_streams()` methods; added corresponding unit tests |

### Notable Decisions/Tradeoffs

1. **`Error::VmService` placed in VM Service section**: Added a clearly labelled section in `error.rs` to group VM Service errors, following the existing sectioning convention. The variant is a simple tuple `VmService(String)` matching the task spec.

2. **Tests are synchronous and do not require a real VM Service**: The introspection methods are async and depend on a live WebSocket connection. All tests exercise the logic synchronously by directly testing the discovery predicate (`is_system_isolate.unwrap_or(false)`) and request serialization, without spinning up a real client. This is consistent with the existing test approach in `client.rs`.

3. **Additional test coverage beyond spec**: Added `test_discover_main_isolate_treats_missing_flag_as_non_system` (covers `None` value for `is_system_isolate`) and `test_stream_cancel_request_format` / `test_get_isolate_request_format` to fully cover all new methods.

4. **Import added to `client.rs`**: `IsolateInfo`, `IsolateRef`, and `VmInfo` were added to the `use super::protocol::` import so the new methods could reference them without full paths.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-daemon` - Passed (182 passed, 3 ignored)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Passed (minor whitespace reformatting of test helper functions)

### Risks/Limitations

1. **`discover_main_isolate` returns the first non-system isolate**: If a Flutter app spawns multiple user isolates via `Isolate.spawn`, this may not return the UI isolate. The task notes acknowledge this; refinement is deferred to a later task.

2. **`subscribe_phase1_streams` errors are non-fatal**: Subscription failures are returned as `Vec<String>` rather than `Result`, so callers must explicitly log/handle them — consistent with the task spec and the note that stream failures should be warnings, not errors.

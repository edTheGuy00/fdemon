## Task: Service Extension Framework

**Objective**: Create the generic infrastructure for calling Flutter service extensions via the VM Service client, including isolate ID management, response parsing helpers, object group lifecycle, and extension availability error handling.

**Depends on**: Phase 1 complete (VmServiceClient with `request()` method)

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions.rs`: **NEW** — Extension call infrastructure
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Add `pub mod extensions` and re-exports
- `crates/fdemon-daemon/src/vm_service/client.rs`: Add isolate ID caching convenience methods
- `crates/fdemon-core/src/error.rs`: Add `Error::ExtensionNotAvailable` variant (if not already covered by `Error::VmService`)

### Details

#### 1. Extension Call Method

Add a typed `call_extension()` method to `VmServiceClient` that wraps the existing `request()` with extension-specific semantics:

```rust
impl VmServiceClient {
    /// Call a Flutter service extension method.
    ///
    /// Automatically includes `isolateId` in params.
    /// All param values must be strings (VM Service protocol requirement).
    pub async fn call_extension(
        &self,
        method: &str,
        isolate_id: &str,
        args: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value> {
        let mut params = serde_json::Map::new();
        params.insert("isolateId".to_string(), json!(isolate_id));
        if let Some(extra) = args {
            for (k, v) in extra {
                params.insert(k, json!(v));
            }
        }
        self.request(method, Some(serde_json::Value::Object(params))).await
    }
}
```

#### 2. Isolate ID Caching

The `VmServiceClient` already has `discover_main_isolate()` which returns an `IsolateRef`. Add a cached version:

```rust
impl VmServiceClient {
    /// Get the main isolate ID, discovering it if not yet cached.
    /// Returns the isolate ID string (e.g., "isolates/6010531716406367").
    pub async fn main_isolate_id(&self) -> Result<String> {
        // Check cache first, then call discover_main_isolate() and cache
    }
}
```

Implementation note: The cache needs to be invalidated on reconnection. Store as `Arc<RwLock<Option<String>>>` or similar. The background client task should clear this on disconnect/reconnect.

Alternatively, a simpler approach: store the isolate ID in the `VmServiceClient` struct behind an `Arc<Mutex<Option<String>>>` and clear it when the connection state changes.

#### 3. Response Parsing Helpers

Create helper functions in `extensions.rs` for the common response patterns:

```rust
/// Parse a boolean toggle response: {"enabled": "true"|"false"}
pub fn parse_bool_extension_response(result: &Value) -> Result<bool> {
    result.get("enabled")
        .and_then(|v| v.as_str())
        .map(|s| s == "true")
        .ok_or_else(|| Error::protocol("missing 'enabled' field in extension response"))
}

/// Parse a string data response: {"data": "<string>"}
pub fn parse_data_extension_response(result: &Value) -> Result<String> {
    result.get("data")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::protocol("missing 'data' field in extension response"))
}
```

#### 4. Object Group Management

Inspector tree extensions require object group management. References (`valueId`) are only valid while their group exists. Implement:

```rust
/// Manages object groups for the Widget Inspector.
///
/// Each group tracks a set of object references. When a group is disposed,
/// all references fetched under that group become invalid.
pub struct ObjectGroupManager {
    client: ...,  // reference to VmServiceClient
    isolate_id: String,
    active_group: Option<String>,
    group_counter: u32,
}

impl ObjectGroupManager {
    /// Create a new object group and return its name.
    /// Disposes the previous active group if one exists.
    pub async fn create_group(&mut self) -> Result<String> {
        if let Some(old) = self.active_group.take() {
            self.dispose_group(&old).await?;
        }
        self.group_counter += 1;
        let name = format!("fdemon-inspector-{}", self.group_counter);
        self.active_group = Some(name.clone());
        Ok(name)
    }

    /// Dispose a named object group via ext.flutter.inspector.disposeGroup.
    pub async fn dispose_group(&self, group_name: &str) -> Result<()> { ... }

    /// Get the current active group name.
    pub fn active_group(&self) -> Option<&str> { ... }
}
```

The `disposeGroup` extension call:
```json
{
    "method": "ext.flutter.inspector.disposeGroup",
    "params": {
        "isolateId": "isolates/...",
        "objectGroup": "<group_name>"
    }
}
```

#### 5. Extension Availability Error Handling

When an extension is not available (profile mode, release mode, or extension not registered), the VM Service returns an error response. Create typed handling:

```rust
/// Check if a VM Service error indicates an unavailable extension.
pub fn is_extension_not_available(error: &VmServiceError) -> bool {
    // Check error code or message for "method not found" / extension not available
}
```

The typed extension wrappers in subsequent tasks should return `Result<T>` where the error case includes `ExtensionNotAvailable` so callers can distinguish "extension doesn't exist in this build mode" from "connection error."

#### 6. Extension Constants

Define constants for all extension method names:

```rust
pub mod ext {
    // Debug overlays
    pub const REPAINT_RAINBOW: &str = "ext.flutter.repaintRainbow";
    pub const DEBUG_PAINT: &str = "ext.flutter.debugPaint";
    pub const SHOW_PERFORMANCE_OVERLAY: &str = "ext.flutter.showPerformanceOverlay";
    pub const INSPECTOR_SHOW: &str = "ext.flutter.inspector.show";

    // Widget inspector
    pub const GET_ROOT_WIDGET_TREE: &str = "ext.flutter.inspector.getRootWidgetTree";
    pub const GET_ROOT_WIDGET_SUMMARY_TREE: &str = "ext.flutter.inspector.getRootWidgetSummaryTree";
    pub const GET_DETAILS_SUBTREE: &str = "ext.flutter.inspector.getDetailsSubtree";
    pub const GET_SELECTED_WIDGET: &str = "ext.flutter.inspector.getSelectedWidget";
    pub const DISPOSE_GROUP: &str = "ext.flutter.inspector.disposeGroup";

    // Layout explorer
    pub const GET_LAYOUT_EXPLORER_NODE: &str = "ext.flutter.inspector.getLayoutExplorerNode";

    // Debug dumps
    pub const DEBUG_DUMP_APP: &str = "ext.flutter.debugDumpApp";
    pub const DEBUG_DUMP_RENDER_TREE: &str = "ext.flutter.debugDumpRenderTree";
    pub const DEBUG_DUMP_LAYER_TREE: &str = "ext.flutter.debugDumpLayerTree";
}
```

### Acceptance Criteria

1. `call_extension(method, isolate_id, args)` sends correct JSON-RPC with `isolateId` in params
2. `main_isolate_id()` discovers and caches the main isolate ID
3. `parse_bool_extension_response()` correctly parses `{"enabled": "true"|"false"}`
4. `parse_data_extension_response()` correctly parses `{"data": "..."}`
5. `ObjectGroupManager` creates, tracks, and disposes groups
6. Extension-not-available errors are distinguishable from connection errors
7. All extension method name constants are defined
8. Module re-exported from `fdemon_daemon::vm_service`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bool_response_true() {
        let json = json!({"enabled": "true", "type": "_extensionType"});
        assert_eq!(parse_bool_extension_response(&json).unwrap(), true);
    }

    #[test]
    fn test_parse_bool_response_false() {
        let json = json!({"enabled": "false", "type": "_extensionType"});
        assert_eq!(parse_bool_extension_response(&json).unwrap(), false);
    }

    #[test]
    fn test_parse_bool_response_missing_field() {
        let json = json!({"other": "value"});
        assert!(parse_bool_extension_response(&json).is_err());
    }

    #[test]
    fn test_parse_data_response() {
        let json = json!({"data": "Widget tree dump..."});
        assert_eq!(parse_data_extension_response(&json).unwrap(), "Widget tree dump...");
    }

    #[test]
    fn test_parse_data_response_empty() {
        let json = json!({"data": ""});
        assert_eq!(parse_data_extension_response(&json).unwrap(), "");
    }

    #[test]
    fn test_extension_constants_are_correct() {
        assert!(ext::REPAINT_RAINBOW.starts_with("ext.flutter."));
        assert!(ext::GET_ROOT_WIDGET_TREE.starts_with("ext.flutter.inspector."));
    }
}
```

### Notes

- The `VmServiceClient.request()` method already handles JSON-RPC request/response correlation via `VmRequestTracker`. The `call_extension()` method is a thin wrapper adding `isolateId` handling.
- Isolate ID caching must be invalidated on reconnection — coordinate with the existing reconnection logic in `client.rs` (the background task's `run_io_loop`).
- The `ObjectGroupManager` will be consumed by Task 04 (Widget Tree Extensions). Keep it generic enough for both inspector and layout explorer use.
- Consider whether `ObjectGroupManager` should be a standalone struct or methods on `VmServiceClient`. A standalone struct is more testable and composable.

---

## Completion Summary

**Status:** Not started

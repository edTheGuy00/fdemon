## Task: Fix ServiceExtensionAdded empty-string RPC name

**Objective**: Change `ServiceExtensionAdded` parsing to return `None` instead of producing an event with an empty `extension_rpc` string when the `extensionRPC` field is absent.

**Depends on**: 01-fix-serde-flatten-bug (Task 01 changes the `parse_isolate_event` signature, and this task modifies code inside that function)

**Review Issues**: #5

### Scope

- `crates/fdemon-daemon/src/vm_service/debugger_types.rs`:
  - In the `ServiceExtensionAdded` arm of `parse_isolate_event` (currently around lines 526-536)
  - Change `unwrap_or("").to_string()` to `.map(str::to_owned)?` so that a missing or non-string `extensionRPC` returns `None`

### Details

**Current code (post Task 01 — reading from `event.data`):**
```rust
"ServiceExtensionAdded" => {
    let extension_rpc = event.data
        .get("extensionRPC")
        .and_then(|v| v.as_str())
        .unwrap_or("")       // ← produces empty string on missing field
        .to_string();
    Some(IsolateEvent::ServiceExtensionAdded {
        isolate,
        extension_rpc,
    })
}
```

**Fixed code:**
```rust
"ServiceExtensionAdded" => {
    let extension_rpc = event.data
        .get("extensionRPC")
        .and_then(|v| v.as_str())
        .map(str::to_owned)?;   // ← returns None if extensionRPC is absent
    Some(IsolateEvent::ServiceExtensionAdded {
        isolate,
        extension_rpc,
    })
}
```

### Acceptance Criteria

1. `parse_isolate_event` returns `None` when `extensionRPC` is absent from a `ServiceExtensionAdded` event
2. `parse_isolate_event` returns `Some(IsolateEvent::ServiceExtensionAdded { ... })` with the correct `extension_rpc` when present
3. Existing tests updated to reflect the new behavior (any test expecting an empty string should expect `None` instead)
4. `cargo test --workspace` passes

### Testing

```rust
#[test]
fn test_parse_service_extension_added_missing_rpc_returns_none() {
    let event = StreamEvent {
        kind: "ServiceExtensionAdded".to_string(),
        isolate: Some(test_isolate_ref()),
        timestamp: None,
        data: json!({}),  // no extensionRPC field
    };
    assert!(parse_isolate_event(&event).is_none());
}
```

### Notes

- This is a small, focused change. The only risk is that downstream code might rely on receiving `ServiceExtensionAdded` events with empty `extension_rpc`, but no such code exists in Phase 1.

---

## Completion Summary

**Status:** Not Started

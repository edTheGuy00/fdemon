## Task: Clean Up get_version Dead Code and VersionInfo Serde

**Objective**: Either move `get_version()` to `VmRequestHandle` so the heartbeat can use the typed API, or remove the dead method. Also add the missing `#[serde(rename_all = "camelCase")]` to `VersionInfo` for consistency.

**Depends on**: None

**Review Reference**: Phase-3 Review Issues #3, #4

### Scope

- `crates/fdemon-daemon/src/vm_service/client.rs`: Move or remove `get_version()` (~line 463)
- `crates/fdemon-daemon/src/vm_service/protocol.rs`: Add serde attribute to `VersionInfo` (~line 135)
- `crates/fdemon-app/src/actions.rs`: Update heartbeat probe call (~line 1083) if `get_version()` is moved

### Details

#### Problem 1: Dead `get_version()` method

`VmServiceClient::get_version()` (client.rs:463-467) is never called. The heartbeat in `forward_vm_events` uses `heartbeat_handle.request("getVersion", None)` directly on a `VmRequestHandle` (actions.rs:1083), bypassing the typed wrapper entirely.

**Current heartbeat code** (actions.rs:1082-1083):
```rust
_ = heartbeat.tick() => {
    let probe = heartbeat_handle.request("getVersion", None);
```

`heartbeat_handle` is a `VmRequestHandle` obtained at line 989 via `client.request_handle()`. `VmRequestHandle` does not expose `get_version()` — it only has the generic `request(method, params)` method.

#### Recommended approach: Move `get_version()` to `VmRequestHandle`

Add `get_version()` as a convenience method on `VmRequestHandle`, matching the pattern of other typed methods like `call_extension()`:

```rust
// client.rs — add to VmRequestHandle impl block
/// Send a `getVersion` RPC and parse the response into [`VersionInfo`].
pub async fn get_version(&self) -> Result<VersionInfo> {
    let result = self.request("getVersion", None).await?;
    serde_json::from_value(result)
        .map_err(|e| Error::vm_service(format!("parse getVersion response: {e}")))
}
```

Then update the heartbeat to use it:

```rust
// actions.rs — heartbeat probe (~line 1083)
let probe = heartbeat_handle.get_version();
```

And remove the now-redundant `VmServiceClient::get_version()` at line 463.

#### Alternative approach: Remove `get_version()` entirely

If the typed response is not needed (the heartbeat only cares about success/failure, not the version values), simply remove `VmServiceClient::get_version()` and leave the heartbeat using the raw `request()` call. This is simpler but less self-documenting.

**Recommendation**: Move to `VmRequestHandle`. The heartbeat implicitly expects a `VersionInfo`-shaped response, and having a typed method makes this explicit. It also validates the response structure, catching protocol changes early.

#### Problem 2: Missing serde attribute on `VersionInfo`

Every other multi-field struct in `protocol.rs` uses `#[serde(rename_all = "camelCase")]`:
- `StreamEventParams` (line 86)
- `StreamEvent` (line 95)
- `VmInfo` (line 148) — immediately after `VersionInfo`

`VersionInfo` (line 135) is missing the attribute. It works today because `major` and `minor` are already lowercase, but it creates an inconsistency that could break silently if fields are added later.

**Fix:**
```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
}
```

### Acceptance Criteria

1. `get_version()` exists on `VmRequestHandle` (or is removed from `VmServiceClient` if taking the alternative approach)
2. No dead/uncalled `get_version()` method exists anywhere
3. The heartbeat probe uses `heartbeat_handle.get_version()` (or remains as `request()` if taking the alternative)
4. `VersionInfo` has `#[serde(rename_all = "camelCase")]`
5. All existing `VersionInfo` deserialization tests pass
6. `cargo check --workspace` passes
7. `cargo clippy --workspace -- -D warnings` clean
8. `cargo test -p fdemon-daemon` passes
9. `cargo test -p fdemon-app` passes

### Notes

- `VmRequestHandle` already has `call_extension()` (line 226) as a precedent for typed convenience methods
- If moving `get_version()`, the `VersionInfo` import may need to be added to `VmRequestHandle`'s scope (check if `protocol.rs` types are already imported)
- The `re-export` in `vm_service/mod.rs` (line ~27) already exports `VersionInfo` publicly

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/protocol.rs` | Added `#[serde(rename_all = "camelCase")]` to `VersionInfo` struct for consistency with all other multi-field structs in the module |
| `crates/fdemon-daemon/src/vm_service/client.rs` | Added `get_version()` typed convenience method to `VmRequestHandle` impl block; removed dead `VmServiceClient::get_version()` method |
| `crates/fdemon-app/src/actions.rs` | Updated heartbeat probe from raw `heartbeat_handle.request("getVersion", None)` to typed `heartbeat_handle.get_version()` |

### Notable Decisions/Tradeoffs

1. **Moved to `VmRequestHandle` (recommended approach)**: Added `get_version()` to `VmRequestHandle` rather than taking the alternative of leaving the raw `request()` call. The typed method makes the heartbeat's intent explicit — it expects a `VersionInfo`-shaped response — and validates the response structure, catching protocol changes early.

2. **No import changes needed**: `VersionInfo` was already imported in `client.rs` at the top-level use statement, so no additional imports were required for the `VmRequestHandle` impl block.

3. **The heartbeat return value is discarded**: The heartbeat's `Ok(Ok(_))` arm discards the typed `VersionInfo` value, which is correct — only success/failure matters for the liveness probe. The typed return means the response is validated before being discarded, providing stronger protocol correctness guarantees.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon` - Passed (383 tests)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1141 tests + 1 doc-test)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **Heartbeat validation tightened**: The heartbeat now validates the `VersionInfo` shape before treating a probe as successful. If the VM Service response for `getVersion` is malformed, the heartbeat will now count it as a failure where previously the raw `request()` call would have succeeded. This is the desired behavior but represents a subtle behavioral change.

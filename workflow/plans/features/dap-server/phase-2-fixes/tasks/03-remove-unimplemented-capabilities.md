## Task: Remove Unimplemented Capabilities from fdemon_defaults()

**Objective**: Stop advertising DAP capabilities that the adapter cannot handle. Per the DAP spec, advertising a capability means the adapter MUST handle the corresponding request.

**Depends on**: None

**Priority**: MEDIUM (pre-merge)

**Review Source**: REVIEW.md Issue #3 (Risks & Tradeoffs Analyzer)

### Scope

- `crates/fdemon-dap/src/protocol/types.rs`: Trim `fdemon_defaults()` to only implemented capabilities

### Background

`Capabilities::fdemon_defaults()` at `types.rs:316-328` currently advertises 7 capabilities as `Some(true)`:

| Capability | Wire Name | Implemented? |
|---|---|---|
| `supports_configuration_done_request` | `supportsConfigurationDoneRequest` | **Yes** — `configurationDone` handler exists in `session.rs` |
| `support_terminate_debuggee` | `supportTerminateDebuggee` | No — Phase 3 |
| `supports_evaluate_for_hovers` | `supportsEvaluateForHovers` | No — Phase 3 |
| `supports_exception_info_request` | `supportsExceptionInfoRequest` | No — Phase 3 |
| `supports_loaded_sources_request` | `supportsLoadedSourcesRequest` | No — Phase 3 |
| `supports_log_points` | `supportsLogPoints` | No — Phase 3 |
| `supports_delayed_stack_trace_loading` | `supportsDelayedStackTraceLoading` | No — Phase 3 |

When a DAP client (e.g., VS Code) sees `supportsEvaluateForHovers: true`, it will send `evaluate` requests during hover. Since there's no handler, the client gets either no response or an error, degrading the user experience.

### Details

Change `fdemon_defaults()` from:

```rust
pub fn fdemon_defaults() -> Self {
    Self {
        supports_configuration_done_request: Some(true),
        support_terminate_debuggee: Some(true),
        supports_evaluate_for_hovers: Some(true),
        supports_exception_info_request: Some(true),
        supports_loaded_sources_request: Some(true),
        supports_log_points: Some(true),
        supports_delayed_stack_trace_loading: Some(true),
        ..Default::default()
    }
}
```

to:

```rust
pub fn fdemon_defaults() -> Self {
    Self {
        supports_configuration_done_request: Some(true),
        ..Default::default()
    }
}
```

All other fields remain `None` (via `Default`) and are omitted from serialization (via `skip_serializing_if = "Option::is_none"`).

**Do NOT remove the struct fields.** The fields should remain defined so Phase 3 can re-enable them as handlers are implemented. Only change the defaults method.

### Acceptance Criteria

1. `fdemon_defaults()` only sets `supports_configuration_done_request: Some(true)`
2. All other capability fields default to `None`
3. Serialized initialize response contains only `"supportsConfigurationDoneRequest": true` (no other capabilities)
4. The `Capabilities` struct still has all 10 fields (unchanged)
5. Existing tests pass — update any test that asserts specific capability values
6. `cargo test -p fdemon-dap` passes

### Testing

Check existing tests in `types.rs` that reference `fdemon_defaults()`. The test at line 576 checks `supportTerminateDebuggee` — if it asserts `Some(true)`, update it to assert `None` / absence from JSON. Run the full test suite to catch any other assertions on removed capabilities.

### Notes

- This is a targeted change to `fdemon_defaults()` only — ~6 lines removed.
- The struct fields, serde attributes, and `Default` derive all remain unchanged.
- When Phase 3 implements a handler for a capability, re-add it to `fdemon_defaults()` in the same task that implements the handler.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/protocol/types.rs` | Trimmed `fdemon_defaults()` to only set `supports_configuration_done_request: Some(true)`; updated `test_capabilities_fdemon_defaults` to assert `None` for all Phase 3 capabilities |

### Notable Decisions/Tradeoffs

1. **Struct fields preserved**: All 10 `Capabilities` fields remain defined per task instructions. Only the `fdemon_defaults()` method body changed. This allows Phase 3 to re-enable capabilities one at a time as handlers are implemented without any struct changes.

2. **Test updated instead of deleted**: `test_capabilities_fdemon_defaults` was updated to assert `is_none()` for the 6 removed capabilities rather than deleted. This keeps coverage of the contract that these capabilities remain `None` until Phase 3 implements their handlers.

### Testing Performed

- `cargo test -p fdemon-dap` - Passed (78 tests)
- `cargo clippy -p fdemon-dap -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Phase 3 re-enablement**: Each Phase 3 capability must be re-added to `fdemon_defaults()` in the same task that implements its handler, to keep the implementation and advertisement in sync. No risk for the current phase.

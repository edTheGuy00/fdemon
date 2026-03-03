## Task: Fix doc comments and task reference comment

**Objective**: Address review observations #6 and #7 — replace the workflow task reference comment with a descriptive comment, and improve the `find_active_by_device_id` doc comment to state the positive contract.

**Depends on**: 01-remove-find-by-device-id (the current doc comment references the deleted method)

### Scope

- `crates/fdemon-app/src/session_manager.rs`: Update `find_active_by_device_id` doc comment
- `crates/fdemon-app/src/handler/new_session/launch_context.rs`: Replace task reference comment

### Details

#### 1. Update `find_active_by_device_id` doc comment (session_manager.rs, lines 317–321)

Current:

```rust
/// Find an active (non-stopped) session by device_id.
///
/// Unlike `find_by_device_id`, this skips sessions in `Stopped` or `Quitting`
/// phases. Used by the new-session launch guard to allow device reuse after
/// a session exits.
```

After task 01 deletes `find_by_device_id`, the "Unlike" reference is stale. Replace with a positive contract:

```rust
/// Find an active session by device_id.
///
/// Returns `Some(id)` for sessions in `Initializing`, `Running`, or `Reloading`
/// phase. Returns `None` for sessions in `Stopped` or `Quitting` phase, or if
/// no session matches the device_id.
```

#### 2. Replace task reference comment (launch_context.rs, lines 1280–1282)

Current:

```rust
// ─────────────────────────────────────────────────────────────────────────
// Phase 4 Task 04: Device Reuse Tests for handle_launch
// ─────────────────────────────────────────────────────────────────────────
```

Replace with:

```rust
// ─────────────────────────────────────────────────────────────────────────
// Device reuse guard tests — verify stopped sessions allow reuse, active sessions block
// ─────────────────────────────────────────────────────────────────────────
```

### Acceptance Criteria

1. `find_active_by_device_id` doc comment no longer references deleted `find_by_device_id`
2. Doc comment states the positive contract (which phases return `Some`)
3. Task reference comment in `launch_context.rs` replaced with descriptive comment
4. `cargo test -p fdemon-app` passes (doc comments don't affect compilation, but verify)

### Testing

No new tests — documentation changes only. Run `cargo check -p fdemon-app` to confirm no issues.

### Notes

- This must run after task 01 because the doc comment update depends on `find_by_device_id` being deleted first.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session_manager.rs` | Replaced stale `find_active_by_device_id` doc comment with positive contract; removed reference to deleted `find_by_device_id` |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Replaced task reference section header with descriptive comment |

### Notable Decisions/Tradeoffs

1. **No behaviour changes**: Both edits are documentation-only; no runtime or test logic was touched.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1160 passed, 0 failed, 4 ignored)

### Risks/Limitations

1. **None**: Documentation-only changes carry no behavioural risk.

## Task: Fix Import Paths and Doc Comments

**Objective**: Fix the submodule path access in `actions.rs` to use re-exported paths, and fix any broken doc comments.

**Depends on**: None

**Estimated Time**: 0.5 hours

### Scope

- `crates/fdemon-app/src/actions.rs`: Fix inline path for `enable_frame_tracking`

### Details

#### Fix 1: Submodule Path Access (MAJOR #9)

**Location:** `actions.rs:610-614`

Currently uses the deep submodule path:
```rust
let _ = fdemon_daemon::vm_service::timeline::enable_frame_tracking(
    &client.request_handle(),
    &isolate_id,
)
.await;
```

`enable_frame_tracking` is re-exported from `vm_service/mod.rs:86-89`:
```rust
pub use timeline::{
    enable_frame_tracking, flutter_extension_kind, is_frame_event, parse_frame_timing,
    parse_str_u64,
};
```

**Fix:** Either add `enable_frame_tracking` to the `use` block at the top of `actions.rs` (lines 17-23) alongside the other vm_service imports, or change the inline path:

```rust
// Option A: Add to use block (preferred)
use fdemon_daemon::{
    vm_service::{
        enable_frame_tracking,  // ADD THIS
        flutter_error_to_log_entry, parse_flutter_error, parse_frame_timing, parse_gc_event,
        parse_log_record, vm_log_to_log_entry, VmRequestHandle, VmServiceClient,
    },
    CommandSender, DaemonCommand, Device, FlutterProcess, RequestTracker, ToolAvailability,
};

// Then at call site:
let _ = enable_frame_tracking(&client.request_handle(), &isolate_id).await;
```

```rust
// Option B: Fix inline path (acceptable)
let _ = fdemon_daemon::vm_service::enable_frame_tracking(
    &client.request_handle(),
    &isolate_id,
)
.await;
```

Option A is preferred — it's consistent with how other `vm_service` functions are imported in the same file.

#### Fix 2: Broken Doc Comment (MINOR #10)

The codebase researcher found **no broken doc comment** (`/ Returns` vs `/// Returns`) in the current `feat/devtools` branch. This issue may have been fixed during implementation or was a false positive. Verify with:

```bash
grep -rn '^\s*/ [A-Z]' crates/ --include='*.rs' | grep -v '///'
```

If found, fix by adding the missing `/` to make it `///`.

### Acceptance Criteria

1. `enable_frame_tracking` imported via `fdemon_daemon::vm_service::` (not `::timeline::`)
2. No broken doc comments in new Phase 3 code
3. `cargo check -p fdemon-app` passes
4. `cargo clippy -p fdemon-app -- -D warnings` passes

### Testing

No new tests needed — this is a pure import path change with no behavioral impact.

### Notes

- This is a quick fix that can be done first to warm up
- The import style should match the existing pattern in `actions.rs` where all vm_service items are imported from the flat re-export surface

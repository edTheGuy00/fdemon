## Task: Extract Inspector Module from actions/mod.rs

**Objective**: Move all DevTools inspector-related functions (widget tree, overlay toggle, layout explorer, group disposal) into `actions/inspector.rs`.

**Depends on**: 04-extract-performance-module

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Remove inspector functions
- `crates/fdemon-app/src/actions/inspector.rs` — **NEW**

### Details

#### Functions to move

| Function | Current Lines (approx) | Purpose |
|----------|----------------------|---------|
| `spawn_fetch_widget_tree` | ~1157-1237 | Fetch root widget tree with readiness polling and API fallback |
| `poll_widget_tree_ready` | ~1247-1326 | Poll `isWidgetTreeReady` until true or exhausted |
| `try_fetch_widget_tree` | ~1338-1400 | Fetch with API fallback (`getRootWidgetTree` → `getRootWidgetSummaryTree`) |
| `is_transient_error` | ~1406-1411 | Helper: check if error is retryable |
| `is_method_not_found` | ~1416-1423 | Helper: check for -32601 error code |
| `spawn_toggle_overlay` | ~1433-1522 | Toggle debug overlay extensions via VM Service |
| `spawn_fetch_layout_data` | ~1532-1655 | Fetch layout explorer data for a widget node |
| `spawn_dispose_devtools_groups` | ~1666-1709 | Dispose inspector + layout object groups |

**Estimated size**: ~440 lines (under 500-line limit)

#### Imports for inspector.rs

```rust
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::message::{DebugOverlayKind, Message};
use crate::session::SessionId;
use fdemon_core::Error;
use fdemon_daemon::vm_service::{
    ext, extract_layout_info, parse_bool_extension_response,
    parse_diagnostics_node_response, VmRequestHandle,
};
```

#### Update mod.rs

1. Add `mod inspector;`
2. Update `handle_action` arms:
   - `FetchWidgetTree` → `inspector::spawn_fetch_widget_tree(...)`
   - `FetchLayoutData` → `inspector::spawn_fetch_layout_data(...)`
   - `ToggleOverlay` → `inspector::spawn_toggle_overlay(...)`
   - `DisposeDevToolsGroups` → `inspector::spawn_dispose_devtools_groups(...)`
3. Remove moved functions from `mod.rs`

### Acceptance Criteria

1. All 8 functions listed above live in `actions/inspector.rs`
2. `inspector.rs` has a `//!` module doc header
3. `inspector.rs` is ≤500 lines
4. Helper functions (`is_transient_error`, `is_method_not_found`) remain private to the module
5. `cargo check --workspace` passes
6. `cargo test --workspace` passes
7. `cargo clippy --workspace -- -D warnings` clean

### Testing

No new tests needed — pure move refactoring. All existing tests must pass.

### Notes

- All 8 functions are private (`fn`, not `pub fn`) — they only need to be `pub(super)` or called from `mod.rs` via the module path.
- The `spawn_*` functions are called from `handle_action` in `mod.rs`, so they need `pub(super)` visibility.
- The helper functions (`is_transient_error`, `is_method_not_found`, `poll_widget_tree_ready`, `try_fetch_widget_tree`) are only called within the inspector module — they can stay private.
- Grouping all inspector actions together mirrors `handler/devtools/inspector.rs` which handles the corresponding TEA messages.

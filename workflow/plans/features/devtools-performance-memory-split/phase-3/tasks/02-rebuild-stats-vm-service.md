## Task: Rebuild Stats VM Service RPCs

**Objective**: Add the VM Service constants, the `widget_location_id_map()` RPC wrapper, and the `set_profile_widget_builds(enabled)` toggle wrapper that the app layer needs to drive widget-rebuild profiling. No subscription logic — the `Flutter.RebuiltWidgets` event stream is already wired via `RESUBSCRIBE_STREAMS` (`Extension` stream). T04 will add the event-dispatch branch.

**Depends on**: T01 (`fdemon-core::rebuild_stats::{LocationMap, parse_rebuilt_widgets_event}`)

**Agent:** implementor

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs`: add `pub async fn widget_location_id_map(client: &VmServiceClient, isolate_id: &str) -> Result<LocationMap>` plus unit tests.
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs`:
  - Add two constants inside the existing `pub mod ext { ... }` block:
    - `pub const PROFILE_WIDGET_BUILDS: &str = "ext.flutter.profileWidgetBuilds";` (in the `// Debug overlays` or new `// Performance flags` section)
    - `pub const WIDGET_LOCATION_ID_MAP: &str = "ext.flutter.inspector.widgetLocationIdMap";` (in the `// Widget inspector` section, after `GET_PROPERTIES`)
  - Add `pub mod performance;` declaration alongside existing `pub mod overlays;`, `pub mod inspector;`, etc.
- `crates/fdemon-daemon/src/vm_service/extensions/performance.rs` (NEW): wraps `toggle_bool_extension(client, ext::PROFILE_WIDGET_BUILDS, ...)` as `pub async fn set_profile_widget_builds(client, isolate_id, enabled: Option<bool>) -> Result<bool>` and a sibling `pub async fn get_profile_widget_builds(client, isolate_id) -> Result<bool>` shorthand.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/vm_service/extensions/overlays.rs`: reference pattern for `toggle_bool_extension` + `parse_bool_extension_response`.
- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs:171–175`: reference pattern for `get_root_widget_tree` — same shape for `widget_location_id_map`.
- `crates/fdemon-daemon/src/vm_service/client.rs:390–398`: `call_extension` signature.
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs:62–159`: existing `pub mod ext` constants block (alphabetical insertion).
- `crates/fdemon-daemon/src/vm_service/timeline.rs:141`: existing `enable_frame_tracking` — **DO NOT modify** in this task; T04 calls `set_profile_widget_builds` from the app layer for explicit toggle control, while `enable_frame_tracking` stays as the session-start best-effort enabler.
- T01 outputs (`LocationMap`, `merge_parallel_arrays`).

### Details

#### `extensions/performance.rs` (NEW)

```rust
//! # Performance Extension Toggles
//!
//! Boolean service extensions controlling the Flutter performance profilers.
//! Currently: widget-rebuild profiling (`ext.flutter.profileWidgetBuilds`).

use crate::error::Result;
use crate::vm_service::client::VmServiceClient;
use crate::vm_service::extensions::ext;
use crate::vm_service::extensions::overlays::toggle_bool_extension;

/// Toggle (or query) the `ext.flutter.profileWidgetBuilds` extension.
///
/// * `enabled = Some(true)` — enable rebuild tracking.
/// * `enabled = Some(false)` — disable.
/// * `enabled = None` — query current state without changing it.
///
/// Returns the new (or current) state. The extension's effect is to emit
/// `Flutter.RebuiltWidgets` Extension events for each frame; the subscription
/// lives on the already-active `Extension` stream.
pub async fn set_profile_widget_builds(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::PROFILE_WIDGET_BUILDS, isolate_id, enabled).await
}

/// Convenience: query the current state.
pub async fn get_profile_widget_builds(
    client: &VmServiceClient,
    isolate_id: &str,
) -> Result<bool> {
    set_profile_widget_builds(client, isolate_id, None).await
}
```

> **Note**: `toggle_bool_extension` already exists in `overlays.rs:50–63` and is the established pattern. Re-use it; do not introduce a parallel helper.

#### `extensions/inspector.rs` — `widget_location_id_map`

Add to the existing module (after `get_selected_widget` at line 261, before the `WidgetInspector` struct at line 308):

```rust
/// Fetch the engine's widget location map (id → (file:line:column, name)).
///
/// Used as a one-shot fallback when fdemon connects after `Flutter.RebuiltWidgets`
/// events have already been emitted with location data that we missed. The
/// response shape is identical to the `locations` sub-object inside
/// `Flutter.RebuiltWidgets` events: parallel arrays per file URI.
///
/// Returns an empty `LocationMap` if the Flutter app hasn't built any
/// instrumented widgets yet.
pub async fn widget_location_id_map(
    client: &VmServiceClient,
    isolate_id: &str,
) -> Result<LocationMap> {
    let response = client
        .call_extension(ext::WIDGET_LOCATION_ID_MAP, isolate_id, None)
        .await?;

    // Response is an Object whose keys are file URIs (filter out the type
    // marker key "type" which VM Service responses always carry).
    let obj = response.as_object().ok_or_else(|| {
        Error::protocol("widgetLocationIdMap response was not a JSON object")
    })?;

    let mut map = LocationMap::default();
    for (key, value) in obj {
        if key == "type" {
            continue;
        }
        map.merge_parallel_arrays(key, value)?;
    }
    Ok(map)
}
```

Add a use statement at the top of the file:

```rust
use fdemon_core::rebuild_stats::LocationMap;
```

#### `extensions/mod.rs` — Constant additions

Inside the `pub mod ext { ... }` block (lines 62–159). Match the existing comment-grouping convention. Suggested insertions:

```rust
pub mod ext {
    // Debug overlays
    pub const REPAINT_RAINBOW: &str = "ext.flutter.repaintRainbow";
    pub const DEBUG_PAINT: &str = "ext.flutter.debugPaint";
    pub const SHOW_PERFORMANCE_OVERLAY: &str = "ext.flutter.showPerformanceOverlay";
    pub const INSPECTOR_SHOW: &str = "ext.flutter.inspector.show";

    // Performance flags
    pub const PROFILE_WIDGET_BUILDS: &str = "ext.flutter.profileWidgetBuilds";  // NEW

    // Widget inspector
    pub const GET_ROOT_WIDGET_TREE: &str = "ext.flutter.inspector.getRootWidgetTree";
    // ...
    pub const GET_PROPERTIES: &str = "ext.flutter.inspector.getProperties";
    pub const WIDGET_LOCATION_ID_MAP: &str = "ext.flutter.inspector.widgetLocationIdMap";  // NEW

    // ... (existing layout/dumps/network unchanged)
}
```

Outside the `ext` block, add the module declaration alongside existing ones (verify file's current top-of-file `pub mod` order):

```rust
pub mod overlays;
pub mod inspector;
pub mod performance;  // NEW
// ... etc
```

### Acceptance Criteria

1. `crates/fdemon-daemon/src/vm_service/extensions/performance.rs` exists with `set_profile_widget_builds` and `get_profile_widget_builds` — both `pub async fn`.
2. `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` exports `pub async fn widget_location_id_map` that returns a `fdemon_core::rebuild_stats::LocationMap`.
3. `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` exposes `ext::PROFILE_WIDGET_BUILDS` and `ext::WIDGET_LOCATION_ID_MAP` constants and declares `pub mod performance;`.
4. `enable_frame_tracking` in `timeline.rs:141` is **unchanged** (the session-start best-effort enabler stays — T04 layers explicit toggle on top).
5. `cargo check -p fdemon-daemon` passes.
6. `cargo test -p fdemon-daemon` includes the new unit tests below — all green.
7. `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` is clean.
8. All `pub` items have `///` doc comments.

### Testing

`crates/fdemon-daemon/src/vm_service/extensions/performance.rs` — `#[cfg(test)] mod tests`:

- `set_profile_widget_builds_uses_correct_extension_name` — mock client, capture the extension method string, assert it equals `"ext.flutter.profileWidgetBuilds"`.
- `set_profile_widget_builds_passes_enabled_arg_true`.
- `set_profile_widget_builds_passes_enabled_arg_false`.
- `set_profile_widget_builds_with_none_passes_no_args` — verifies query-only mode.
- `set_profile_widget_builds_round_trips_enabled_true` — mock returns `{ "enabled": "true" }`, asserts `Ok(true)`.

> Re-use the mock-client pattern already established in `overlays.rs` tests.

`crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` — add to existing `mod tests`:

- `widget_location_id_map_parses_single_file` — mock response:
  ```json
  {
    "type": "Map",
    "package:foo/main.dart": {
      "ids": [1, 2],
      "lines": [10, 20],
      "columns": [3, 4],
      "names": ["A", "B"]
    }
  }
  ```
  Verify `LocationMap.by_id.len() == 2`, IDs 1 and 2 present with correct fields.
- `widget_location_id_map_parses_multi_file` — two file-URI keys, four locations total.
- `widget_location_id_map_handles_empty_response` — only the `"type"` key → empty `LocationMap`.
- `widget_location_id_map_propagates_parse_errors` — malformed parallel arrays (length mismatch) returns `Err`.
- `widget_location_id_map_skips_type_key` — verifies the `"type"` marker isn't treated as a file URI.

### Notes

- **No new dependencies.** All work uses existing `serde_json`, `crate::error`, and the already-imported `VmServiceClient`.
- **`enable_frame_tracking` stays untouched.** T04 will call `set_profile_widget_builds` from the app layer when the user presses `R` (or when `auto_enable_rebuild_tracking == true`). The session-start `enable_frame_tracking` continues to fire-and-forget at connection time so any historical events are captured even when the user opens the panel later.
- **Re-uses `toggle_bool_extension`** — do not duplicate its retry/error logic. The single source of truth is `overlays.rs:50–63`.
- **`widget_location_id_map` shape** matches the `locations` sub-object inside `Flutter.RebuiltWidgets` events, allowing T04's accumulator to use the same merge code path for both event-side and RPC-side location data.
- **Future refactor (deferred):** `enable_frame_tracking` could be re-implemented to call `set_profile_widget_builds(client, isolate_id, Some(true))` for consistency. Not in scope for Phase 3 — leaving as-is avoids a file overlap with T03's `timeline.rs` edits and keeps the blast radius small.

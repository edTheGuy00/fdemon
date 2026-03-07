## Task: Extract Event Handling into events.rs

**Objective**: Move all event-handling methods from the `DapAdapter` impl block in `mod.rs` into a fresh `events.rs` module.

**Depends on**: 01-delete-stale-extract-types-backend

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs`: Remove event methods from impl block, add `mod events;`
- `crates/fdemon-dap/src/adapter/events.rs`: **CREATE** with extracted event methods

### Details

Extract these methods from the `DapAdapter<B>` impl block in `mod.rs`:

| Method | Approx. Lines | Visibility |
|--------|---------------|------------|
| `handle_debug_event` | ~911–1324 | `pub async` |
| `emit_output` | ~1326–1340 | `pub async` |
| `interpolate_log_message` | ~1342–1402 | `async` (private) |
| `on_resume` | ~1404–1416 | `pub` |
| `on_hot_restart` | ~1418–1436 | `pub` |
| `send_event` | ~1438–1449 | `async` (private) |

Also extract this free function:
| Function | Approx. Lines | Visibility |
|----------|---------------|------------|
| `pause_reason_to_dap_str` | ~2831–2841 | `pub(crate)` |

**File structure for `events.rs`:**

```rust
//! # Debug Event Handling
//!
//! DapAdapter methods for handling VM Service debug events.

use crate::adapter::backend::DebugBackend;
use crate::adapter::breakpoints;
use crate::adapter::types::{...};
use crate::adapter::DapAdapter;
// ... other necessary imports

impl<B: DebugBackend> DapAdapter<B> {
    pub async fn handle_debug_event(&mut self, event: DebugEvent) { ... }
    pub async fn emit_output(&self, category: &str, output: &str) { ... }
    async fn interpolate_log_message(&self, isolate_id: &str, template: &str) -> String { ... }
    pub fn on_resume(&mut self) { ... }
    pub fn on_hot_restart(&mut self) { ... }
    async fn send_event(&self, event: &str, body: Option<serde_json::Value>) { ... }
}

pub(crate) fn pause_reason_to_dap_str(reason: &PauseReason) -> &'static str { ... }
```

**Update `mod.rs`:**
- Add `mod events;` declaration (private — only needs `pub(crate)` if other crates need it)
- Remove the extracted methods from the `impl<B: DebugBackend> DapAdapter<B>` block
- Remove the `pause_reason_to_dap_str` free function
- If `pause_reason_to_dap_str` is used in `mod.rs` tests, add `use events::pause_reason_to_dap_str;` in the test module

### Acceptance Criteria

1. `events.rs` contains all 6 methods + 1 free function listed above
2. Methods removed from `mod.rs` impl block
3. `mod events;` declaration added to `mod.rs`
4. All existing tests pass without modification (tests call through `DapAdapter` which picks up the impl from `events.rs`)
5. `cargo check --workspace` — Pass
6. `cargo test --workspace` — Pass
7. `cargo clippy --workspace -- -D warnings` — Pass

### Notes

- The `handle_debug_event` method accesses private fields (`thread_map`, `thread_names`, `paused_isolates`, `breakpoint_state`, `desired_breakpoints`, `exception_mode`, `source_reference_store`, `vm_disconnected`). Since `events.rs` is a submodule of `adapter`, it has access to all private fields of `DapAdapter` — no visibility changes needed on the struct fields.
- `send_event` is used by other methods (in `handlers.rs` later) — it should be `pub(crate)` or `pub(super)` in `events.rs` so sibling modules can call it. Alternatively, keep it private in `events.rs` if only event methods use it, and add a separate `send_event` in handlers if needed.
- Verify that `interpolate_log_message` doesn't need to be called from handlers — if it does, adjust visibility.

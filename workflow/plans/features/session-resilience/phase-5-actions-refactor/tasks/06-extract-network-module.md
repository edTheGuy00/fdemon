## Task: Extract Network Module from actions/mod.rs

**Objective**: Move all network monitoring functions and the browser utility into `actions/network.rs`.

**Depends on**: 05-extract-inspector-module

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Remove network functions
- `crates/fdemon-app/src/actions/network.rs` — **NEW**

### Details

#### Functions to move

| Function | Current Lines (approx) | Purpose |
|----------|----------------------|---------|
| `spawn_network_monitoring` | ~1731-1893 | Periodic HTTP profile polling via VM Service |
| `spawn_fetch_http_request_detail` | ~1901-1953 | One-shot HTTP request detail fetch |
| `spawn_clear_http_profile` | ~1959-1998 | One-shot HTTP profile clear |
| `open_url_in_browser` | ~2006-2046 | Cross-platform browser launch utility |

#### Constants to move

| Constant | Value | Purpose |
|----------|-------|---------|
| `NETWORK_POLL_MIN_MS` | 500ms | Minimum HTTP profile polling interval |

**Estimated size**: ~340 lines (well under 500-line limit)

#### Imports for network.rs

```rust
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::vm_service::{ext, VmRequestHandle};
```

#### Update mod.rs

1. Add `mod network;`
2. Update `handle_action` arms:
   - `StartNetworkMonitoring` → `network::spawn_network_monitoring(...)`
   - `FetchHttpRequestDetail` → `network::spawn_fetch_http_request_detail(...)`
   - `ClearHttpProfile` → `network::spawn_clear_http_profile(...)`
   - `OpenBrowserDevTools` → `network::open_url_in_browser(...)`
3. Remove moved functions and constant from `mod.rs`

### Acceptance Criteria

1. All 4 functions and `NETWORK_POLL_MIN_MS` live in `actions/network.rs`
2. `network.rs` has a `//!` module doc header
3. `network.rs` is ≤500 lines
4. `cargo check --workspace` passes
5. `cargo test --workspace` passes
6. `cargo clippy --workspace -- -D warnings` clean

### Testing

No new tests needed — pure move refactoring. All existing tests must pass.

### Notes

- `open_url_in_browser` is a utility function that could live anywhere, but it's only used for opening DevTools in a browser — placing it in `network.rs` alongside the other network/DevTools actions keeps the module cohesive and avoids creating a tiny standalone file.
- `spawn_network_monitoring` has the same `watch::channel` pattern as `spawn_performance_polling` — they follow the same lifecycle conventions but are in separate modules to match the handler decomposition.
- After this task, `mod.rs` should contain only: `handle_action`, module declarations, re-exports, and any remaining constants/types. It should be ~350 lines.

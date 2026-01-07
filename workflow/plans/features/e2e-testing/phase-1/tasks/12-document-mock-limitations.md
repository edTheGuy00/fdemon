## Task: Document Mock Daemon Limitations

**Objective**: Add comprehensive documentation to `mock_daemon.rs` explaining the mock's limitations, design decisions, and known constraints.

**Depends on**: None (can be done independently)

**Priority**: Major (documentation requirement from review)

**Source**: [REVIEW.md](../../../REVIEW.md) - Risks & Tradeoffs Review, Recommendations #1

### Scope

- `tests/e2e/mock_daemon.rs`: Module-level documentation

### Details

The mock daemon has several intentional limitations that should be documented to prevent confusion and help future maintainers understand the design decisions.

**Add module-level documentation:**

```rust
//! Mock Flutter daemon for integration testing
//!
//! Simulates the Flutter daemon's JSON-RPC protocol without
//! requiring an actual Flutter installation.
//!
//! # Design Philosophy
//!
//! This mock operates at the **channel level**, not the process level.
//! It simulates the JSON-RPC messages that would flow through stdin/stdout
//! of a real Flutter daemon process, but does not spawn any processes.
//!
//! # Limitations
//!
//! This mock has intentional limitations to keep it simple and fast:
//!
//! ## Protocol Limitations
//!
//! - **Fixed request ID**: Uses ID `1` for all requests (real daemon uses incrementing IDs)
//! - **No app_id validation**: Does not verify that commands reference a valid app_id
//! - **Sequential processing**: Commands are processed one at a time
//! - **Limited command coverage**: Only implements 6 of 20+ daemon commands:
//!   - `app.restart` (hot reload/restart)
//!   - `app.stop`
//!   - `daemon.shutdown`
//!   - `device.getDevices`
//!   - `device.enable`
//!   - Custom responses via `set_response()`
//!
//! ## Timing Limitations
//!
//! - **Fixed timeouts**: `recv_event()` uses a 1-second timeout (may need adjustment for CI)
//! - **Simulated reload time**: Hot reload completes in 50ms (real varies widely)
//!
//! ## Architecture Limitations
//!
//! - **No TUI integration**: Does not test the actual event loop routing
//! - **No process lifecycle**: Does not simulate process spawn/exit signals
//! - **No file I/O**: Does not read/write any files
//!
//! # What This Mock IS Good For
//!
//! - Testing handler state transitions in response to daemon events
//! - Verifying hot reload/restart command flows
//! - Testing session management logic
//! - Fast, deterministic CI tests (no Flutter installation required)
//!
//! # What This Mock IS NOT Good For
//!
//! - Testing actual Flutter compilation
//! - Testing real device communication
//! - Testing process management (spawn, signals, exit codes)
//! - Testing TUI rendering (use widget tests instead)
//!
//! # Usage Example
//!
//! ```ignore
//! use crate::e2e::mock_daemon::{MockFlutterDaemon, MockScenarioBuilder};
//!
//! #[tokio::test]
//! async fn test_hot_reload() {
//!     let (daemon, mut handle) = MockScenarioBuilder::new()
//!         .with_app_id("my-app")
//!         .with_app_started()
//!         .build();
//!
//!     tokio::spawn(daemon.run());
//!
//!     // Skip daemon.connected
//!     handle.recv_event().await;
//!
//!     // Send reload command
//!     handle.send(DaemonCommand::Reload { app_id: "my-app".into() }).await.unwrap();
//!
//!     // Verify progress events
//!     let event = handle.recv_event().await;
//!     assert!(matches!(event, Some(DaemonEvent::Stdout(s)) if s.contains("app.progress")));
//! }
//! ```
//!
//! # Future Improvements (Phase 2+)
//!
//! - Expanded command coverage
//! - App_id validation option
//! - Configurable timeouts
//! - Full TUI event loop integration tests
```

### Acceptance Criteria

1. Module-level doc comment added with all limitations documented
2. Design philosophy explained
3. Usage example included
4. Future improvements noted
5. `cargo doc --test e2e` generates documentation without warnings

### Testing

```bash
cargo doc --document-private-items
cargo test --test e2e
```

### Notes

- Documentation is crucial for maintainability
- Helps new contributors understand what the mock does and doesn't test
- Links to Phase 2 planning for future improvements

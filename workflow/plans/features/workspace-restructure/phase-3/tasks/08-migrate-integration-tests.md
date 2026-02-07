## Task: Migrate Integration Tests

**Objective**: Update integration tests in `tests/` to work in the workspace context. Ensure dev-dependencies are properly configured for each crate that needs them, and that workspace-level integration tests can import from all crates.

**Depends on**: 07-update-binary-and-headless

**Estimated Time**: 2-3 hours

### Scope

- `tests/discovery_integration.rs`: Update imports
- `tests/` (any other integration tests): Update imports
- Root `Cargo.toml` `[dev-dependencies]`: Ensure workspace integration tests compile
- Crate-level `[dev-dependencies]`: Audit and fix

### Details

#### 1. Audit Existing Integration Tests

Current integration tests:
- `tests/discovery_integration.rs` - Tests Flutter project discovery

These tests previously imported from `flutter_demon::*`. They need to import from specific crates.

#### 2. Update Integration Test Imports

```rust
// tests/discovery_integration.rs
// Old:
// use flutter_demon::core::{discover_flutter_projects, is_runnable_flutter_project};
// New:
use fdemon_core::{discover_flutter_projects, is_runnable_flutter_project};
```

#### 3. Workspace-Level Integration Tests

Integration tests at the workspace root (`tests/`) are compiled against the binary crate's dependencies. Since the binary depends on all 4 internal crates, the integration tests can import from any of them.

If integration tests need to create `Engine` instances or test cross-crate interactions:
```rust
use fdemon_app::Engine;
use fdemon_core::prelude::*;
use fdemon_daemon::Device;
```

#### 4. Audit Dev-Dependencies Per Crate

Ensure each crate has the right dev-dependencies for its unit tests:

| Crate | Dev-Dependencies Needed |
|-------|------------------------|
| `fdemon-core` | `tempfile` (for discovery tests) |
| `fdemon-daemon` | `tempfile`, `tokio-test` |
| `fdemon-app` | `tempfile`, `tokio-test`, `mockall` |
| `fdemon-tui` | `tempfile`, `insta`, `fdemon-daemon` (for Device test construction) |
| Binary (root) | `tempfile`, `tokio-test`, `serial_test`, `expectrl` |

#### 5. Handle E2E Tests

If there are E2E tests (using `expectrl` for PTY-based testing), they remain at the workspace root level since they test the binary:

```bash
cargo test --test e2e
```

These tests import the binary's dependencies.

#### 6. Verify Snapshot Files

If `insta` snapshot tests exist in `tui/render/`, verify the `.snap` files are at the correct path relative to `crates/fdemon-tui/`. Run snapshot tests to regenerate if needed:

```bash
cargo test -p fdemon-tui -- render
cargo insta review  # If snapshots changed
```

### Acceptance Criteria

1. All integration tests in `tests/` compile and pass
2. Each crate's unit tests pass when run in isolation (`cargo test -p <crate>`)
3. `cargo test --workspace` passes (runs all unit + integration tests)
4. No unused dev-dependencies (verify with `cargo clippy`)
5. Snapshot files (if any) are in correct locations

### Testing

```bash
# Run all workspace tests
cargo test --workspace

# Run integration tests specifically
cargo test --test '*'

# Run each crate's tests in isolation
cargo test -p fdemon-core
cargo test -p fdemon-daemon
cargo test -p fdemon-app
cargo test -p fdemon-tui

# Run with verbose output to see test counts
cargo test --workspace -- --nocapture 2>&1 | tail -5
```

### Notes

- Integration tests at workspace root compile against the binary crate. They can access all 4 internal crates because the binary depends on all of them.
- If a test file imports types from multiple crates, all those crates must be in the binary's `[dependencies]` or `[dev-dependencies]`.
- The `serial_test` crate is used for E2E tests that need exclusive terminal access. These stay at the workspace root.
- `mockall` is used in `fdemon-app` tests for mocking service traits. Verify these still work after the crate split.
- Some tests may use `#[cfg(test)]` modules that reference private types. These tests must stay within their crate since cross-crate tests can only use public APIs.

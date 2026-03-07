## Task: Move Mock Backends to test_helpers Module

**Objective**: Move the 10 top-level mock backend structs from the `mod tests` block in `mod.rs` into the existing `test_helpers.rs` module, reducing the test block by ~600-700 lines.

**Depends on**: 04-extract-variables

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs`: Remove mock struct definitions from `mod tests`
- `crates/fdemon-dap/src/adapter/test_helpers.rs`: Add mock backend structs

### Details

The `test_helpers.rs` module already contains the `MockTestBackend` trait with default no-op implementations and a blanket `impl<T: MockTestBackend> DebugBackend for T`. Move these 10 mock structs (which implement `MockTestBackend`) from `mod tests` in `mod.rs` to `test_helpers.rs`:

| Mock Struct | Purpose | Approx. Lines |
|-------------|---------|---------------|
| `MockBackend` | No-op default; `get_source` returns synthetic snippet | ~10 |
| `MockBackendWithUri` | Returns known ws_uri, device_id, build_mode | ~30 |
| `AttachMockBackend` | Returns two named isolates from `get_vm()` | ~15 |
| `FailingVmBackend` | All operations fail; simulates disconnected VM | ~70 |
| `StackMockBackend` | Returns realistic 3-frame stack | ~35 |
| `VarMockBackend` | Returns variables + object expansion for List/Map/PlainInstance | ~85 |
| `NotConnectedBackend` | Every method returns `BackendError::NotConnected` | ~90 |
| `HotOpMockBackend` | Configurable hot_reload/hot_restart results | ~30 |
| `CondMockBackend` | Configurable evaluate_in_frame; tracks resume_calls | ~35 |
| `LogpointMockBackend` | Expression map for logpoint interpolation; tracks resume_calls | ~75 |

**Do NOT move** the 4 inline struct mocks (`ErrorEvalBackend`, `TrackingBackend`, `StopTrackingBackend`, `StopTrackingBackend2`) — they are small, defined inside specific test functions, and coupled tightly to their tests.

**Update `test_helpers.rs`:**
- Add the 10 mock structs with their `impl MockTestBackend` blocks
- Make them `pub(crate)` so the test module in `mod.rs` can use them

**Update `mod.rs` test module:**
- Remove the 10 mock struct definitions
- Add `use super::test_helpers::{MockBackend, MockBackendWithUri, ...};` at the top of `mod tests`
- Also add imports for the `FailingResumeBackend` (from task 08) if it exists as a top-level mock

### Acceptance Criteria

1. 10 mock structs moved to `test_helpers.rs`
2. Mock struct definitions removed from `mod.rs` test module
3. Test module imports mocks via `use super::test_helpers::*`
4. `test_helpers.rs` remains under 800 lines
5. All existing tests pass unchanged
6. `cargo check --workspace` — Pass
7. `cargo test --workspace` — Pass
8. `cargo clippy --workspace -- -D warnings` — Pass

### Notes

- Some mocks use `Arc<Mutex<u32>>` for tracking calls (e.g., `CondMockBackend.resume_calls`) — ensure these imports are available in `test_helpers.rs`
- `VarMockBackend` builds complex JSON responses — the serde_json import will be needed
- `FailingVmBackend` overrides ALL methods to return errors, not just a few — verify the full impl is moved
- After this task, the `mod.rs` test block should be ~4,500 lines (down from ~5,178)

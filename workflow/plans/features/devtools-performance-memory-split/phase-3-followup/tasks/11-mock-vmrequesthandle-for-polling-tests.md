# 11 — Mock VmRequestHandle for `spawn_timeline_polling` Integration Tests

**Wave:** 4
**Depends On:** 06
**Agent:** implementor
**Estimated Hours:** 3–5h
**Addresses:** M7

## Context

Phase 3 Task 04 explicitly skipped acceptance criterion 12: *"`spawn_timeline_polling` integration test (using a mock `VmRequestHandle`) verifies pause/resume/shutdown semantics within 100ms wall-clock."* The reason given was that `VmRequestHandle` requires a live WebSocket. Multiple reviewer agents flagged this — the loop's pause/resume/shutdown timing and watermark drift are non-trivial and regressions would be invisible.

The reviewer recommended extracting a trait abstraction over `VmRequestHandle`'s relevant methods (`request` and `call_extension`) so test code can substitute a mock. This unlocks integration testing for `spawn_timeline_polling` AND, longer-term, `spawn_performance_polling` / `spawn_allocation_polling` / `spawn_network_polling`. For this task, focus on `spawn_timeline_polling` only — keep the trait minimal and well-scoped.

This is the largest task in the followup phase. It introduces internal infrastructure that will be reused but does not change external behavior.

## Acceptance Criteria

1. **Trait definition.** `crates/fdemon-daemon/src/vm_service/client.rs` (or a new sibling file) defines:
   ```rust
   #[cfg(any(test, feature = "test-util"))]
   #[async_trait::async_trait]
   pub trait VmRequestApi: Send + Sync {
       async fn request(&self, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value>;
       async fn call_extension(&self, method: &str, isolate_id: &str, args: Option<HashMap<String, String>>) -> Result<serde_json::Value>;
   }
   ```
   (Adjust signatures to match the real `VmRequestHandle` methods exactly. Use `#[async_trait]` if it's already in the dep tree; otherwise prefer the GAT-based hand-rolled async-trait pattern.)
2. **Real impl.** `VmRequestHandle` implements `VmRequestApi` for production use. The impl simply delegates to the existing methods.
3. **`spawn_timeline_polling` refactor.** The function in `crates/fdemon-app/src/actions/performance.rs` accepts `impl VmRequestApi + 'static` instead of the concrete `VmRequestHandle`. All call sites (production) pass the concrete `VmRequestHandle` and continue to work unchanged.
4. **Mock impl.** A new test-only `MockVmRequestApi` struct in the test module:
   - Records every `request` and `call_extension` call (method, params, isolate_id).
   - Returns canned `Ok` / `Err` responses configured by the test.
   - Exposes a method like `call_log() -> Vec<MockCall>` for assertions.
5. **Three new integration tests** in the test module of `actions/performance.rs`:
   - **`test_timeline_pause_stops_rpcs`** — start polling, fire `pause_tx.send(true)`, wait 1.5 s, assert NO new `getVMTimeline` calls in the mock log during the pause window.
   - **`test_timeline_resume_restarts`** — after pause, fire `pause_tx.send(false)`, assert a new `getVMTimeline` call lands within one poll interval (~1.2 s).
   - **`test_timeline_shutdown_exits_within_100ms`** — fire `shutdown_tx.send(true)`, await the spawned task's `JoinHandle`, assert the task exits in `< 100ms` wall-clock.
6. **Polling-tasks unchanged otherwise.** No production behavior changes — pause/resume/shutdown semantics are preserved.
7. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Files Modified (Write)

- `crates/fdemon-daemon/src/vm_service/client.rs` (or new sibling, e.g., `client_api.rs`) — `VmRequestApi` trait definition and `VmRequestHandle` impl.
- `crates/fdemon-app/src/actions/performance.rs` — `spawn_timeline_polling` accepts `impl VmRequestApi`; tests module gets the mock + 3 new integration tests.

## Files Read (Dependencies)

- T06 outputs — `spawn_timeline_polling`'s body changes from T06 (watermark, constant, retry) must be in place; T11 layers the trait abstraction on top.
- `crates/fdemon-daemon/src/vm_service/client.rs` — read-only: current `VmRequestHandle` signature for `request` and `call_extension`.

## Approach Hints

- **Trait visibility.** Gate behind `#[cfg(any(test, feature = "test-util"))]` initially. If subsequent followup tasks need to use the trait from production code (e.g., for dependency injection), the gate can be relaxed later. Starting tight keeps the public surface small.
- **async_trait.** Check `Cargo.toml` for `async-trait` — if absent, weigh adding it as a dev-dep vs. hand-rolling. `async-trait` is a one-line cost; preferred.
- **Mock design.** The mock should be minimal: a `Mutex<Vec<MockCall>>` for the log + an `Arc<Mutex<HashMap<String, MockResponse>>>` for canned responses keyed by RPC method. Keep it simple — extending later is easy.
- **Timing-sensitive assertions.** For the shutdown-within-100ms test, use `tokio::time::timeout(Duration::from_millis(100), join_handle).await.expect("...")`.
- **Tokio test runtime.** Use `#[tokio::test]` (or `#[tokio::test(flavor = "multi_thread")]` if needed) and ensure the test runtime is consistent with the production behavior.
- **One trait, two tasks scope.** Only `spawn_timeline_polling` is refactored. `spawn_performance_polling`, `spawn_allocation_polling`, `spawn_network_polling` are intentionally left for follow-up work. Note this in the task's completion summary.

## Out of Scope

- Refactoring `spawn_performance_polling`, `spawn_allocation_polling`, or `spawn_network_polling` to use the trait. Separate follow-up tasks if needed.
- Adding `VmRequestApi` methods beyond `request` and `call_extension`. Minimal surface only.
- Making the trait `pub` in the `fdemon-daemon` library API. Internal `pub(crate)` is sufficient, gated by `#[cfg]`.
- Adding a derive macro or proc-macro for the trait — manual impl is fine.
- Testing other code paths (e.g., session lifecycle, error recovery) — only the three pause/resume/shutdown tests.
- Wiring this into CI as a separate test suite — the existing `cargo test --workspace` is sufficient.

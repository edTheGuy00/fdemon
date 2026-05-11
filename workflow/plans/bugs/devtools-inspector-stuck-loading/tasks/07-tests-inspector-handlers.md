## Task: End-to-End Tests for Inspector Handler Flow

**Objective**: Lock in the fixes from tasks 02-06 with integration-style unit tests covering the full inspector fetch lifecycle: open, success, failure, timeout, refresh, multi-isolate resolution.

**Depends on**: 05-shrink-readiness-poll-budget, 06-bypass-readiness-poll-on-refresh

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs` (tests module at end of file): Add new tests covering the failure/timeout/debounce-clear scenarios.
- `crates/fdemon-app/src/actions/inspector/widget_tree.rs` (tests module): Add tests covering the new poll-budget + exhaustion-doesn't-error contract.
- `crates/fdemon-daemon/src/vm_service/client.rs` (tests module): Add tests covering the multi-isolate Flutter-UI-resolution + cache invalidation.

**Files Read (Dependencies):**
- All code modified in tasks 01-06.

### Details

Test matrix:

| # | Scenario | Expected |
|---|----------|----------|
| 1 | Initial inspector open → success | `loading=false`, `root=Some(_)`, `error=None` |
| 2 | Initial open → `WidgetTreeFetchFailed` | `loading=false`, `error=Some(_)`, `last_fetch_time=None` (debounce cleared) |
| 3 | Initial open → `WidgetTreeFetchTimeout` | `loading=false`, `error=Some(_)`, `last_fetch_time=None` |
| 4 | After failure → `r` press → new RPC fires immediately | `RequestWidgetTree` is not debounced |
| 5 | Initial open → success → `r` press | `FetchTrigger::Refresh` (skip readiness poll) |
| 6 | Initial open → `r` press before first render | `FetchTrigger::Initial` (poll still applies) |
| 7 | `getVM` returns 2 non-system isolates, only one has `ext.flutter.*` | resolver returns Flutter UI isolate |
| 8 | Cache hit → second resolve_flutter_ui_isolate doesn't re-issue RPCs | mock RPC count == 1 |
| 9 | Hot restart event → cache cleared → next resolve re-issues RPCs | mock RPC count increments |
| 10 | Readiness poll exhausted → `try_fetch_widget_tree` still runs | RPC fired |

### Acceptance Criteria

1. All 10 scenarios above have at least one named test that asserts the expected outcome.
2. `cargo test --workspace` passes with the new tests green.
3. Test names follow `docs/CODE_STANDARDS.md` — descriptive snake_case (`test_inspector_open_then_fail_clears_debounce`).
4. Tests do not rely on real timing — use injected `Clock` / mock timeout (or assert state without sleeps).
5. Tests do not require a live Flutter process; mock `VmServiceHandle` is used.

### Testing

(This task IS the testing.)

### Notes

- If a mock `VmServiceHandle` does not exist, add a minimal trait-based mock under `#[cfg(test)]` in `vm_service/client.rs`. Keep production API surface unchanged.
- Coverage targets are in `CLAUDE.md` (~3,209 unit tests across crates). Adding 10-15 tests here is healthy; don't over-extend.
- Skip flaky-time tests — use deterministic mock clocks.

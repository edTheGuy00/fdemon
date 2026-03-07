## Task: Wire or remove #[allow(dead_code)] items

**Objective**: Remove `#[allow(dead_code)]` annotations from 7 public items by either wiring them into active code paths or deleting them until needed.

**Depends on**: 02-split-adapter-mod

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/handler/dap_backend.rs`: `DapSessionMetadata::new` (line 489), `session_metadata_slot` (line 592)
- `crates/fdemon-dap/src/adapter/types.rs` (post-split; currently `adapter/mod.rs:697-718`): `REQUEST_TIMEOUT`, `ERR_NOT_CONNECTED`, `ERR_NO_DEBUG_SESSION`, `ERR_THREAD_NOT_FOUND`, `ERR_EVAL_FAILED`

### Details

**Decision criteria:** If an item has a clear, near-term usage path, wire it. If it's speculative, delete it — it can always be re-added from git history.

**Recommendation — delete all 7:**

1. `DapSessionMetadata::new` — commented as "Phase 4, Task 08 follow-up"; not wired. Delete.
2. `session_metadata_slot` — same comment. Delete.
3. `REQUEST_TIMEOUT` — no backend call timeout logic exists. Delete until timeout support is added.
4. `ERR_NOT_CONNECTED` through `ERR_EVAL_FAILED` — error responses currently use string messages, not numeric codes. Delete until DAP error code support is added.

Note: `ERR_VM_DISCONNECTED = 1005` (line 721) is actively used and should NOT be removed.

### Acceptance Criteria

1. No `#[allow(dead_code)]` annotations remain on the 7 identified items
2. Items are either wired into active code or deleted
3. `cargo check --workspace` — Pass (no dead code warnings)
4. `cargo test --workspace` — Pass

### Notes

- Prefer deletion over keeping dead code with suppressed warnings
- If any of these constants are needed later, they're trivial to re-add

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/dap_backend.rs` | Deleted `DapSessionMetadata::new` method and `session_metadata_slot` method; removed empty `impl DapSessionMetadata {}` block |
| `crates/fdemon-dap/src/adapter/types.rs` | Removed 5 dead constants: `REQUEST_TIMEOUT`, `ERR_NOT_CONNECTED`, `ERR_NO_DEBUG_SESSION`, `ERR_THREAD_NOT_FOUND`, `ERR_EVAL_FAILED` |
| `crates/fdemon-dap/src/adapter/mod.rs` | Updated re-export to remove the 5 deleted constants; replaced test referencing `ERR_NOT_CONNECTED` with literal `1000`; removed `test_request_timeout_constant_is_10_seconds` test; trimmed `test_error_code_constants_are_defined` to only check `ERR_TIMEOUT` and `ERR_VM_DISCONNECTED` |
| `crates/fdemon-app/src/engine.rs` | Fixed pre-existing `unused_mut` warning: `let mut state` → `let state` (assignment to `state.dap_debug_senders` was already removed by prior task 03 work) |
| `crates/fdemon-app/src/handler/devtools/debug.rs` | Added missing `UpdateAction` import; added `#[cfg(test)]` to `forward_dap_event` (now only used in tests after TEA-purity refactor); fixed `test_isolate_start_tracks_isolate` assertion (removed stale `result.action.is_none()` assertion since `IsolateStart` now produces a `ForwardDapDebugEvents` action); replaced large block of old-style tests that referenced the removed `AppState::dap_debug_senders` field with new action-based tests |

### Notable Decisions/Tradeoffs

1. **Pre-existing compilation failure fixed as prerequisite**: The workspace failed to compile due to an incomplete Task 03 (TEA purity) implementation — `AppState.dap_debug_senders` was removed from `state.rs` in prior work but `debug.rs` tests still referenced it. This had to be fixed before Task 10's acceptance criteria (`cargo check --workspace` passing) could be met. The fix replaced old-style tests (using `state.dap_debug_senders` receivers) with new action-based tests checking `result.action` for `UpdateAction::ForwardDapDebugEvents`.

2. **`forward_dap_event` marked `#[cfg(test)]` rather than deleted**: The function is still useful for testing the forwarding utility in isolation (pruning stale senders, handling None events, empty registries). Marking it test-only avoids a dead_code warning without losing the test coverage it enables.

3. **`session_metadata` field kept in `VmBackendFactory`**: Only the `new()` constructor method and `session_metadata_slot()` accessor were dead. The `session_metadata` field itself is actively used in `VmBackendFactory::create()` and was not removed.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed (no warnings)
- `cargo test --workspace` — Passed (1317 + 360 + 460 + 581 + 796 + 80 + others = all passing; 0 failures)
- `cargo clippy --workspace -- -D warnings` — Passed

### Risks/Limitations

1. **Tests now action-based**: Tests that previously used channel receivers to verify DAP event forwarding now inspect `result.action`. This is the correct pattern after the TEA-purity refactor (Task 03) and matches the new architecture, but reviewers should be aware that the test style changed substantially.

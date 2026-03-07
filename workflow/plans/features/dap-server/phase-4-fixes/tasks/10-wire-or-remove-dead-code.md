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

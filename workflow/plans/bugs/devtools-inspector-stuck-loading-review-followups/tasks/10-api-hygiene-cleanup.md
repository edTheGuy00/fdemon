## Task: API Hygiene Cleanup (Phase 3, Bundle)

**Objective**: Tighten the public API surface and update stale docstrings introduced by `fix/devtools-improvements`. Bundles four minor cleanups: narrow `FetchTrigger` visibility, remove redundant `clear_isolate_cache` alias, fix `isolate_id_cache` docstring, and document the `has_ever_rendered_tree` non-reset in `InspectorState::reset()`.

**Depends on**: None (Phase 2 must merge first per plan, but task 10 has no direct file overlap with 06-09)

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/lib.rs` — remove `FetchTrigger` from public re-exports
- `crates/fdemon-app/src/handler/mod.rs` — change `pub enum FetchTrigger` to `pub(crate)`
- `crates/fdemon-daemon/src/vm_service/client.rs` — remove `pub fn clear_isolate_cache(&self)` alias; update test-only callers to use `invalidate_isolate_cache`; update `isolate_id_cache` field docstring (around line 66) to describe dual use
- `crates/fdemon-app/src/state.rs` — add an inline comment in `InspectorState::reset()` documenting the intentional non-reset of `has_ever_rendered_tree` (around lines 282-298)

**Files Read (Dependencies):**
- None

### Details

**4a. Narrow `FetchTrigger` visibility:**

`crates/fdemon-app/src/handler/mod.rs` declares `pub enum FetchTrigger`. `lib.rs` re-exports it. Neither `fdemon-tui` nor `main.rs` reference it. Tighten to `pub(crate)` and drop the `lib.rs` re-export:

```rust
// handler/mod.rs
pub(crate) enum FetchTrigger {
    Initial,
    Refresh,
}
```

Remove the `pub use crate::handler::FetchTrigger;` line (or equivalent) from `lib.rs`.

**4b. Remove `clear_isolate_cache` alias:**

`crates/fdemon-daemon/src/vm_service/client.rs:354-356` defines `pub fn clear_isolate_cache(&self)` as an alias for `invalidate_isolate_cache`. Only test code (`client.rs:1748`, `1870`) calls it. Remove the alias and update the test callers:

```bash
git grep -n "clear_isolate_cache" crates/
# Update each call site to invalidate_isolate_cache
```

**4c. Update `isolate_id_cache` docstring:**

At `client.rs:66`:
```rust
// Before:
/// Cached main isolate ID. Cleared by the background task on reconnection.
isolate_id_cache: Arc<Mutex<Option<String>>>,

// After:
/// Cached Flutter UI isolate ID. Populated by either `main_isolate_id()`
/// (first non-system isolate heuristic) or `resolve_flutter_ui_isolate()`
/// (first isolate with `ext.flutter.*` extension RPCs). First caller wins.
/// Cleared by:
///  - the background task on reconnection,
///  - `invalidate_isolate_cache()` calls from the handler layer,
///  - hot restart (`Message::SessionRestartCompleted`),
///  - isolate exit (`IsolateEvent::IsolateExit`).
isolate_id_cache: Arc<Mutex<Option<String>>>,
```

**4d. Inline comment in `InspectorState::reset()`:**

`crates/fdemon-app/src/state.rs:282-298`:
```rust
pub fn reset(&mut self) {
    self.root = None;
    self.expanded.clear();
    self.loading = false;
    self.error = None;
    // has_ever_rendered_tree intentionally NOT reset — sticky for session lifetime.
    // Cleared on hot restart (handler/update.rs::SessionRestartCompleted) and
    // session drop.
    self.has_object_group = false;
    self.last_fetch_time = None;
    // ... layout fields ...
}
```

### Acceptance Criteria

1. `FetchTrigger` is `pub(crate)`. `git grep -n "FetchTrigger" crates/ | grep -v "src/handler/\|src/actions/inspector/\|src/process\|src/lib.rs"` returns no external references.
2. `clear_isolate_cache` is removed; `git grep "clear_isolate_cache" crates/` returns no matches.
3. `isolate_id_cache` field docstring accurately describes both writers and all invalidation sources.
4. `InspectorState::reset()` carries an inline comment explaining the `has_ever_rendered_tree` non-reset.
5. All CI quality gates pass.

### Testing

No behavior change — existing tests should pass. Verify by:
- `cargo check --workspace --all-targets`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

### Notes

- After task 02 removes `AutoRehydrate`, the `FetchTrigger` re-export may already be unnecessary. Verify the actual export state before editing.
- The `clear_isolate_cache` alias was added in task 04 of the original fix because the implementor wanted "two names for the same thing for clarity". The reviewer flagged this as API surface duplication; `invalidate_isolate_cache` is the canonical name (used in production code).

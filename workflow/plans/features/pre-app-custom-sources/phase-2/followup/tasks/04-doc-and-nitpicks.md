## Task: Fix ARCHITECTURE.md Inaccuracy and Address Nitpicks

**Objective**: Fix the documentation inaccuracy about shared source modes in ARCHITECTURE.md, demote unused public helpers to `pub(crate)`, and add `#[derive(Debug)]` to `CustomSourceHandle` for parity with `SharedSourceHandle`.

**Depends on**: None

**Severity**: MINOR (doc fix) + NITPICK (helpers, derive)

**Review Reference**: [REVIEW.md](../../../../reviews/features/pre-app-custom-sources-phase-2/REVIEW.md) — Issues 4, 5, 6

### Scope

- `docs/ARCHITECTURE.md`: Fix inaccurate statement about shared sources (~line 1181)
- `crates/fdemon-app/src/config/types.rs`: Demote `has_shared_sources()` and `shared_sources()` to `pub(crate)` (~lines 919-927)
- `crates/fdemon-app/src/session/handle.rs`: Add `#[derive(Debug)]` to `CustomSourceHandle` (~line 17)

### Details

#### Fix 1: ARCHITECTURE.md (line ~1181)

**Current text:**
```
Shared sources are started as part of the pre-app source flow (they require `start_before_app = true`) and are shut down during `AppState::shutdown_shared_sources()` when fdemon exits — after all per-session sources have been stopped.
```

**Replace with:**
```
Shared sources can be started either as pre-app sources (`start_before_app = true`) or as post-app sources (`start_before_app = false`). They are shut down during `AppState::shutdown_shared_sources()` when fdemon exits — after all per-session sources have been stopped.
```

This matches the actual implementation: `spawn_custom_sources` in `native_logs.rs:283+` handles shared post-app sources, and `CONFIGURATION.md` already documents both modes correctly.

#### Fix 2: Demote unused public helpers (config/types.rs lines ~919-927)

The following methods on the native logs settings struct have no production callers — they are only used in their own `#[cfg(test)]` module:

```rust
// Change from pub to pub(crate)
pub(crate) fn has_shared_sources(&self) -> bool {
    self.custom_sources.iter().any(|s| s.shared)
}

pub(crate) fn shared_sources(&self) -> impl Iterator<Item = &CustomSourceConfig> {
    self.custom_sources.iter().filter(|s| s.shared)
}
```

This prevents external crates from depending on these convenience methods that may change or be removed. The test module within `types.rs` can still use them since `pub(crate)` is visible within the crate.

#### Fix 3: Add `#[derive(Debug)]` to `CustomSourceHandle` (session/handle.rs line ~17)

**Current:**
```rust
pub struct CustomSourceHandle {
```

**New:**
```rust
#[derive(Debug)]
pub struct CustomSourceHandle {
```

`SharedSourceHandle` (line 39) already has `#[derive(Debug)]`. The two structs are structurally identical (same four fields, same types). `SessionHandle` implements `Debug` manually and only prints `custom_source_count`, but `CustomSourceHandle` itself should still be formattable with `{:?}` for debugging purposes.

### Acceptance Criteria

1. ARCHITECTURE.md accurately states that shared sources support both `start_before_app = true` and `start_before_app = false`.
2. `has_shared_sources()` and `shared_sources()` are `pub(crate)`, not `pub`.
3. `CustomSourceHandle` has `#[derive(Debug)]`.
4. All existing tests pass. No compilation errors from the visibility change.

### Testing

No new tests needed. Run the standard quality gate:

```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

The `pub(crate)` change should compile cleanly since no external crate calls these methods.

### Notes

- If `has_shared_sources()` or `shared_sources()` are found to have callers outside `config/types.rs` tests (e.g., added by a concurrent branch), keep them `pub` and note the finding in the completion summary.
- The `has_shared_pre_app_sources()` method at line ~930 was not flagged in the review but may also be unused in production. Check during implementation and demote if appropriate.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Replaced inaccurate statement at line 1181: shared sources now correctly described as supporting both `start_before_app = true` and `start_before_app = false` |
| `crates/fdemon-app/src/config/types.rs` | Demoted `has_shared_sources()`, `shared_sources()`, and `has_shared_pre_app_sources()` from `pub` to `pub(crate)` and added `#[cfg(test)]` since all callers are in the test module |
| `crates/fdemon-app/src/session/handle.rs` | Added `#[derive(Debug)]` to `CustomSourceHandle` for parity with `SharedSourceHandle` |

### Notable Decisions/Tradeoffs

1. **`#[cfg(test)]` in addition to `pub(crate)`**: Simply changing `pub` to `pub(crate)` triggered a `-D dead_code` error under `cargo clippy --workspace -- -D warnings` because the methods are only called from `#[cfg(test)]` blocks. Adding `#[cfg(test)]` to the method definitions themselves resolves this cleanly and makes the test-only intent explicit. The task notes anticipated external callers might exist; none were found — all three methods are purely test helpers.

2. **`has_shared_pre_app_sources()` also demoted**: The task asked to check this method and demote if appropriate. It had no production callers (only test module usage), so it was demoted alongside the other two.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed (no warnings)
- `cargo test --workspace` - Passed (all test suites green, consistent with pre-change counts)
- `cargo clippy --workspace -- -D warnings` - Passed

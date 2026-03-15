## Task: Add `shared` Field to `CustomSourceConfig`

**Objective**: Add `shared: bool` (default: false) to `CustomSourceConfig` and helper methods on `NativeLogsSettings` so the system can distinguish shared vs. per-session custom sources.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add field, serde, defaults, helpers, validation

### Details

#### 1. Add Field

In `CustomSourceConfig` struct, add after `start_before_app`:

```rust
/// Whether this source is shared across all sessions (spawned once).
///
/// When `true`, the source is spawned on first session launch and persists
/// until fdemon quits. Logs are broadcast to all active sessions.
/// When `false` (default), the source is per-session.
#[serde(default)]
pub shared: bool,
```

#### 2. Add Helpers on `NativeLogsSettings`

```rust
/// Returns `true` if any custom source has `shared = true`.
pub fn has_shared_sources(&self) -> bool {
    self.custom_sources.iter().any(|s| s.shared)
}

/// Returns an iterator over shared custom sources.
pub fn shared_sources(&self) -> impl Iterator<Item = &CustomSourceConfig> {
    self.custom_sources.iter().filter(|s| s.shared)
}
```

#### 3. Add Helpers on `CustomSourceConfig`

Extend `has_pre_app_sources()` to optionally filter by `shared`:

```rust
/// Returns `true` if any custom source has `start_before_app = true` AND `shared = true`.
pub fn has_shared_pre_app_sources(&self) -> bool {
    self.custom_sources.iter().any(|s| s.start_before_app && s.shared)
}
```

#### 4. Validation

No new validation constraints — `shared = true` is valid with any combination of `start_before_app` and `ready_check`. The existing rule (`ready_check` requires `start_before_app = true`) still applies independently.

### Acceptance Criteria

1. `shared: bool` field exists on `CustomSourceConfig` with `#[serde(default)]`
2. `has_shared_sources()` and `shared_sources()` helpers exist on `NativeLogsSettings`
3. `has_shared_pre_app_sources()` helper exists
4. Existing configs without `shared` deserialize unchanged (default false)
5. All existing tests pass
6. New tests for `shared` field deserialization and helper methods

### Testing

```rust
#[test]
fn test_shared_field_defaults_to_false() { ... }

#[test]
fn test_shared_field_parses_true() { ... }

#[test]
fn test_has_shared_sources() { ... }

#[test]
fn test_has_shared_pre_app_sources() { ... }
```

### Notes

- The `shared` field has no validation interdependency — it's purely additive
- This is a data-only change; no behavioral changes yet

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/types.rs` | Added `shared: bool` field to `CustomSourceConfig` struct; added `has_shared_sources()`, `shared_sources()`, `has_shared_pre_app_sources()` helpers on `NativeLogsSettings`; added `shared: false` to all 20 existing test struct literals; added 11 new tests |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Added `shared: false` to `pre_app_source` test helper struct literal |
| `crates/fdemon-app/src/handler/tests.rs` | Added `shared: false` to 5 test struct literals |
| `crates/fdemon-app/src/actions/native_logs.rs` | Added `shared: false` to `make_source_config` test helper struct literal |

### Notable Decisions/Tradeoffs

1. **Field ordering**: `shared` was placed between `start_before_app` and `ready_check` per the task spec, matching the conceptual grouping of lifecycle-related boolean flags.
2. **No Default impl added**: `CustomSourceConfig` does not derive `Default` (by design — `name` and `command` have no sensible defaults). All struct literals were updated explicitly rather than relying on spread syntax.
3. **Pre-existing clippy errors**: The repo has 75 pre-existing clippy errors across pre-existing changed files on this branch; no new errors were introduced in my changed files (`types.rs` has zero clippy errors).

### Testing Performed

- `cargo check -p fdemon-app --tests` - Passed
- `cargo test -p fdemon-app` - Passed (1668 tests, 0 failed)
- `cargo fmt -p fdemon-app` - Applied (no formatting changes needed)
- New tests added: 11 tests covering `shared` field deserialization (default false, explicit true, explicit false), `has_shared_sources` (empty, none shared, one shared), `shared_sources` iterator, and `has_shared_pre_app_sources` (empty, shared-not-pre-app, pre-app-not-shared, both true)

### Risks/Limitations

1. **Pre-existing clippy errors**: The quality gate `cargo clippy -- -D warnings` fails due to 75 pre-existing errors on this branch, none related to this task. The task spec only requires `cargo check` and `cargo test` for verification.

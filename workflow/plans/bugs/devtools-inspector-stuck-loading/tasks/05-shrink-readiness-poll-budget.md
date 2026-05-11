## Task: Shrink Readiness-Poll Budget + Make It Configurable

**Objective**: Reduce the readiness-poll budget so it can no longer consume the entire `fetch_timeout_secs` window. Expose `readiness_poll_attempts` and `readiness_poll_interval_ms` as config keys with conservative new defaults.

**Depends on**: 04-resolve-flutter-ui-isolate

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/inspector/widget_tree.rs` (`poll_widget_tree_ready`, lines 22-103):
  - Replace hard-coded `8` attempts and `500 ms` interval with values read from settings.
  - Reduce default attempts from 8 → **2**, default interval from 500 ms → **250 ms**, per-call timeout from 2 s → **1 s**.
  - On exhaustion, log `warn!` and **return `Ok(())` instead of an error** — let the subsequent RPC speak for itself.
- `crates/fdemon-app/src/config/settings.rs` (or equivalent): Add config keys:
  ```toml
  [devtools.inspector]
  readiness_poll_attempts = 2          # default 2
  readiness_poll_interval_ms = 250     # default 250
  readiness_poll_call_timeout_ms = 1000 # default 1000
  ```
  Wire these into `Settings` struct and into the action dispatch path.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/actions/inspector/mod.rs`: Reads `fetch_timeout_secs` already — adjust to pass new poll-related fields.

### Details

Current code (effectively):
```rust
for _ in 0..8 {  // 8 attempts
    tokio::time::timeout(Duration::from_secs(2), handle.call_extension(...)).await ...
    tokio::time::sleep(Duration::from_millis(500)).await;
}
```

Worst-case budget: 8 × (2 s per-call + 500 ms sleep) = **20 s**. With a default `fetch_timeout_secs` of (check config — likely 10 s or less), the outer `tokio::time::timeout` fires deep inside the poll loop.

New defaults:
```rust
for _ in 0..config.readiness_poll_attempts {  // 2 attempts
    tokio::time::timeout(
        Duration::from_millis(config.readiness_poll_call_timeout_ms),
        handle.call_extension(...),
    ).await ...
    tokio::time::sleep(Duration::from_millis(config.readiness_poll_interval_ms)).await;
}
```

Worst-case budget: 2 × (1 s + 250 ms) = **2.5 s**. Leaves the outer timeout plenty of room.

Crucially, **don't error out on exhaustion** — `warn!` and proceed. The browser DevTools doesn't poll at all; the framework's own error reply is more useful than a synthetic readiness timeout.

```rust
if !ready_after_polls {
    warn!(
        attempts = config.readiness_poll_attempts,
        "Inspector: readiness poll exhausted; proceeding with fetch anyway"
    );
    // Fall through; do not return Err.
}
Ok(())
```

### Acceptance Criteria

1. Default readiness budget is ≤ 2.5 s (config-overridable).
2. Poll exhaustion no longer aborts the fetch — `try_fetch_widget_tree` runs even when `isWidgetTreeReady` never returned `true`.
3. New config keys parse correctly from `.fdemon/config.toml`; defaults applied when keys are absent.
4. Unit tests cover: default values, custom values, exhaustion-doesn't-error.
5. `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` pass.

### Testing

Unit tests in `actions/inspector/widget_tree.rs`:

```rust
#[tokio::test]
async fn poll_exhaustion_returns_ok_not_error() {
    let handle = mock_handle_returning_not_ready();
    let result = poll_widget_tree_ready(&handle, "isolate-1", session_id, &test_config()).await;
    assert!(result.is_ok(), "poll exhaustion should not propagate as error");
}

#[tokio::test]
async fn poll_respects_custom_attempts_and_interval() { /* ... */ }
```

Settings test in `config/settings.rs`:
```rust
#[test]
fn settings_readiness_poll_defaults_to_2_attempts() { /* ... */ }
```

### Notes

- The instrumentation from task 01 should now produce `warn!` lines whenever a real production session hits exhaustion — that signal helps future-tuning.
- If the existing config module has a `[devtools]` or `[devtools.inspector]` table, reuse it. Otherwise add it under the existing pattern.
- Per CLAUDE.md, default constants should be named (e.g., `DEFAULT_READINESS_POLL_ATTEMPTS: usize = 2`) with doc comments explaining derivation.

## Task: Remove Debug Scaffolding from Production Code

**Objective**: Downgrade or remove all `[native-logs-debug]` tracing calls from `info!` to `debug!` level.

**Depends on**: None

**Review Issue**: #5 (MAJOR)

### Scope

- `crates/fdemon-app/src/actions/native_logs.rs`: 1 occurrence (~line 62-66)
- `crates/fdemon-app/src/handler/session.rs`: 3 occurrences (~lines 304-307, 322-326, 343-346)

### Details

Four `tracing::info!("[native-logs-debug] ...")` calls are development artifacts that appear in every user's log file for every session start. They should be downgraded to `tracing::debug!` and have the `[native-logs-debug]` prefix removed.

**All 4 occurrences:**

1. `actions/native_logs.rs:62-66`:
   ```rust
   tracing::info!(
       "[native-logs-debug] spawn_native_log_capture called, {} custom sources configured, project_path={}",
       settings.custom_sources.len(),
       project_path.display()
   );
   ```

2. `handler/session.rs:304-307`:
   ```rust
   tracing::info!(
       "[native-logs-debug] Skipping: already running for session {}",
       session_id
   );
   ```

3. `handler/session.rs:322-326`:
   ```rust
   tracing::info!(
       "[native-logs-debug] platform={}, needs_platform={}, has_tools={}, custom_sources={}, enabled={}",
       platform, needs_platform_capture, has_platform_tools, has_custom_sources,
       state.settings.native_logs.enabled
   );
   ```

4. `handler/session.rs:343-346`:
   ```rust
   tracing::info!(
       "[native-logs-debug] Emitting StartNativeLogCapture for session {}",
       session_id
   );
   ```

**Fix:** For each, change `tracing::info!` to `tracing::debug!` and remove the `[native-logs-debug]` prefix. The messages themselves are reasonable debug output and can be kept.

### Acceptance Criteria

1. No `[native-logs-debug]` strings remain in any `tracing::info!` calls
2. All four calls are either `tracing::debug!` or removed entirely
3. `grep -r "native-logs-debug" crates/` returns no matches
4. Existing tests pass

### Testing

No new tests needed — this is a log level change. Verify with:
```bash
grep -r "native-logs-debug" crates/
```

### Notes

- The messages are useful for debugging native log startup — keeping them at `debug!` level is fine
- If task 02 modifies the guard message at session.rs:304-307, coordinate to avoid conflicts

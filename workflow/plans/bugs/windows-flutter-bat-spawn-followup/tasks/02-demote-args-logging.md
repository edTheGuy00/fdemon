## Task: Demote `args` logging from `info!` to `debug!` in `process.rs`

**Objective**: Restore the original Wave-1 plan's spec (BUG.md Track 4) which specified `debug!` for spawn args. The implementation upgraded the level to `info!`, which writes user-supplied dart-define values — potentially API keys, OAuth client IDs, or Sentry DSNs — to the persistent log file at `%TEMP%\fdemon\fdemon-*.log`. BUG.md Track 3 explicitly invites users to share these log files for verification, creating a real disclosure path.

**Depends on**: nothing — Wave A

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/process.rs`:
  - Split the existing `info!` log at lines 63-68 into two log statements: an `info!` for non-sensitive fields (`binary`, `cwd`) and a `debug!` for the args list.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/launch.rs:306-329` (read-only — to confirm where `args` are constructed and that they include user-supplied dart-defines).

### Details

Current code at `crates/fdemon-daemon/src/process.rs:63-68`:

```rust
info!(
    binary = %flutter.path().display(),
    args = ?args,
    cwd = %project_path.display(),
    "Spawning flutter session"
);
```

Replacement:

```rust
info!(
    binary = %flutter.path().display(),
    cwd = %project_path.display(),
    "Spawning flutter session"
);
debug!(
    binary = %flutter.path().display(),
    args = ?args,
    cwd = %project_path.display(),
    "Spawning flutter session (with args)"
);
```

Ensure `tracing::debug` is in scope. `process.rs` already imports `tracing` macros at the top of the file — verify `debug` is among them; if not, add it to the import list.

### Acceptance Criteria

1. The single `info!(args = ?args)` call at `process.rs:63-68` is split into:
   - An `info!` log emitting only `binary` and `cwd` (and the message `"Spawning flutter session"`).
   - A `debug!` log emitting `binary`, `args`, and `cwd` (with a distinct message like `"Spawning flutter session (with args)"`).
2. With `RUST_LOG=info` the args do not appear in the trace output. With `RUST_LOG=debug` (or `RUST_LOG=fdemon_daemon::process=debug`) the args appear.
3. `cargo test -p fdemon-daemon` passes (no regressions).
4. `cargo clippy -p fdemon-daemon` exits clean (no new warnings).

### Testing

```bash
cargo test -p fdemon-daemon process
cargo clippy -p fdemon-daemon
# Manual: run a short fdemon session and confirm args do not appear in the info-level log file
RUST_LOG=info cargo run -- /tmp/some-flutter-project &  # or however the project is started
```

### Notes

- Do NOT redact dart-defines at the log site. We considered redaction but it is fragile (must handle `--dart-define KEY=VALUE`, `--dart-define=KEY=VALUE`, and `--dart-define-from-file=...` forms) and adds maintenance burden. Demoting the level is simpler and equally safe.
- The two log statements share most fields. Some readers may consider this duplication; it is intentional — the `info!` line is the "this happened" diagnostic that should always be retained, the `debug!` line is the "what was passed" detail useful only for active debugging.
- Do NOT move the args to `trace!`. `debug!` is the correct level — it's the level developers actually enable when investigating spawn issues.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/process.rs` | Split `info!(args = ?args)` at lines 63-68 into two log statements: an `info!` for `binary` and `cwd` only, and a `debug!` for `binary`, `args`, and `cwd` with distinct message "Spawning flutter session (with args)" |

### Notable Decisions/Tradeoffs

1. **`debug!` macro already in scope**: The prelude (`fdemon_core::prelude::*`) already re-exports `tracing::debug`, so no additional import was needed.
2. **No redaction**: Per task notes, args are demoted by level rather than redacted — simpler and equally safe against dart-define disclosure.

### Testing Performed

- `cargo fmt --all` - Passed (no formatting changes needed)
- `cargo test -p fdemon-daemon` - Passed (736 tests, 0 failures, 3 ignored)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (clean, no warnings)

### Risks/Limitations

1. **None**: This is a pure log-level demotion with no behavioral changes.

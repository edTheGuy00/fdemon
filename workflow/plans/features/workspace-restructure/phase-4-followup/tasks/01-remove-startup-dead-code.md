## Task: Remove startup.rs Dead Code and `devices_stub` Module

**Objective**: Remove 260+ lines of dead code from `crates/fdemon-tui/src/startup.rs`, including the `devices_stub` module with `unimplemented!()` stubs that would panic at runtime if ever called.

**Depends on**: None

**Severity**: CRITICAL (runtime panic risk)

**Source**: Code Quality Inspector (ACTION_ITEMS.md Critical #1)

### Scope

- `crates/fdemon-tui/src/startup.rs`: Remove dead code, keep only `startup_flutter()` and tests

### Details

The file is 409 lines, but only ~60 lines are live. After the phase-4 restructure, startup logic moved to the Engine and runner, leaving behind dead functions that reference a `devices_stub` module with `unimplemented!()` stubs.

**Items to remove:**

| Lines | Symbol | Why Dead |
|-------|--------|----------|
| 25-47 | `devices_stub` module + `use devices_stub as devices` | Fake module with `unimplemented!()` -- would panic if called |
| 57-61 | `StartupAction::AutoStart` variant | Never constructed; runner ignores `StartupAction` return |
| 64-96 | `animate_during_async()` | Only called by `auto_start_session` (also dead) |
| 118-210 | `auto_start_session()` | Never called from outside startup.rs |
| 213-249 | `try_auto_start_config()` | Only called by `auto_start_session` |
| 251-265 | `launch_with_validated_selection()` | Only called by `auto_start_session` |
| 267-313 | `launch_session()` | Only called by dead functions above |
| 315-325 | `enter_normal_mode_disconnected()` | Never called |
| 327-376 | `cleanup_sessions()` | Never called; Engine::shutdown() handles this now |

**Imports that become unnecessary after removal:**
- `tokio::sync::{mpsc, watch}`
- `tracing::{info, warn}`
- `fdemon_app::message::Message`
- `fdemon_app::session::SessionId`
- `fdemon_app::spawn`
- `fdemon_app::Device`
- `fdemon_app::SessionTaskMap`
- `fdemon_app::UpdateAction`
- `fdemon_core::LogSource`
- `fdemon_app::config::{get_first_auto_start, get_first_config, load_last_selection, validate_last_selection, LaunchConfig, ValidatedSelection}`
- `crate::render`

**What stays:**
- Module doc comment
- `use std::path::Path`
- `use fdemon_app::config::{self, load_all_configs, LoadedConfigs, Settings}`
- `use fdemon_app::state::{AppState, UiMode}`
- `StartupAction::Ready` variant only
- `startup_flutter()` function (~15 lines)
- Two unit tests at the bottom

**The only external caller** is `crates/fdemon-tui/src/runner.rs:30-31`:
```rust
let _startup_result =
    startup::startup_flutter(&mut engine.state, &engine.settings, &engine.project_path);
```
Note: the result is already ignored (`_startup_result`).

### Acceptance Criteria

1. No `unimplemented!()` calls anywhere in the codebase
2. No `devices_stub` module exists
3. `startup.rs` is ~60 lines (down from 409)
4. `cargo check -p fdemon-tui` passes
5. `cargo test -p fdemon-tui --lib` passes (the two remaining unit tests still work)
6. `cargo clippy -p fdemon-tui` has no new warnings

### Testing

```bash
# Verify no unimplemented!() calls remain
rg 'unimplemented!' crates/

# Verify compilation
cargo check -p fdemon-tui

# Verify tests
cargo test -p fdemon-tui --lib
```

### Notes

- The `StartupAction` enum may simplify to just `Ready` -- consider whether it should remain an enum or become a unit struct
- The runner already ignores the return value, so changing `StartupAction` shape is safe
- Do NOT remove `startup_flutter()` itself -- it handles config loading and initial state setup

---

## Completion Summary

**Status:** Not started

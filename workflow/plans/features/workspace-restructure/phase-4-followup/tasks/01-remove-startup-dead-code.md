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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/startup.rs` | Removed 341 lines of dead code (410 → 69 lines). Deleted `devices_stub` module with `unimplemented!()` stubs, `StartupAction::AutoStart` variant, and 8 dead functions (`animate_during_async`, `auto_start_session`, `try_auto_start_config`, `launch_with_validated_selection`, `launch_session`, `enter_normal_mode_disconnected`, `cleanup_sessions`). Removed all unnecessary imports. Kept only `startup_flutter()`, `StartupAction::Ready`, and two unit tests. |

### Notable Decisions/Tradeoffs

1. **Kept `StartupAction` as enum**: Although it now has only one variant (`Ready`), keeping it as an enum preserves the API contract and allows for potential future variants without breaking changes. The alternative (converting to a unit struct or removing entirely) would require changing the function signature and updating the caller in `runner.rs`.

2. **Removed all dead imports**: Cleaned up imports that were only used by dead code, reducing the dependency surface. The file now only imports what it actually needs: `Path`, config utilities (`load_all_configs`), and state types (`AppState`, `UiMode`).

### Testing Performed

- `rg 'unimplemented!' crates/` - No results (all `unimplemented!()` calls removed from codebase)
- `cargo check -p fdemon-tui` - Passed with no errors
- `cargo test -p fdemon-tui --lib` - Passed (438 tests, including the 2 tests in startup.rs)
- `cargo clippy -p fdemon-tui` - Passed with no new warnings (existing warnings are in fdemon-app, not fdemon-tui)

### Risks/Limitations

1. **None identified**: The removed code was entirely dead and never executed in the current codebase. The only external caller (`runner.rs`) already ignores the return value, so this change has zero runtime impact.

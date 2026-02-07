## Task: Lock Down fdemon-app Public API

**Objective**: Define a clean public API for `fdemon-app` -- the largest and most visibility-leaky crate. This involves making handler submodules internal, privatizing Engine struct fields, cleaning up config wildcard re-exports, and internalizing process/signals/actions modules.

**Depends on**: 01-lock-down-fdemon-core, 02-lock-down-fdemon-daemon

**Estimated Time**: 4-6 hours

### Scope

- `crates/fdemon-app/src/lib.rs`: Change module visibility, update re-exports
- `crates/fdemon-app/src/handler/mod.rs`: Make submodules `pub(crate)`
- `crates/fdemon-app/src/engine.rs`: Make fields private, add accessor methods
- `crates/fdemon-app/src/config/mod.rs`: Replace wildcard re-export with explicit list
- `crates/fdemon-app/src/actions.rs`: Make `pub(crate)`
- `crates/fdemon-app/src/process.rs`: Make `pub(crate)`
- `crates/fdemon-app/src/signals.rs`: Make `pub(crate)`
- `src/headless/runner.rs`: Update to use Engine methods instead of direct field access
- `crates/fdemon-tui/src/runner.rs`: Update to use Engine methods instead of direct field access
- `crates/fdemon-tui/src/startup.rs`: Update to use Engine methods instead of direct field access

### Details

#### 1. Make Handler Submodules `pub(crate)`

The handler module has 11 submodules, all `pub mod`, exposing ~100+ internal dispatch functions. Only `update()`, `UpdateAction`, `Task`, and `UpdateResult` should be public.

**In `handler/mod.rs`**, change all submodules from `pub mod` to `pub(crate) mod`:

```rust
// BEFORE:
pub mod daemon;
pub mod helpers;
pub mod keys;
pub mod log_view;
pub mod new_session;
pub mod scroll;
pub mod session;
pub mod session_lifecycle;
pub mod settings;
pub mod settings_handlers;
pub mod update;

// AFTER:
pub(crate) mod daemon;
pub(crate) mod helpers;
pub(crate) mod keys;
pub(crate) mod log_view;
pub(crate) mod new_session;
pub(crate) mod scroll;
pub(crate) mod session;
pub(crate) mod session_lifecycle;
pub(crate) mod settings;
pub(crate) mod settings_handlers;
pub(crate) mod update;
```

Keep the existing re-exports at the bottom of `handler/mod.rs`:
```rust
pub use update::update;
pub use helpers::detect_raw_line_level;
pub use keys::handle_key;
```

Note: `detect_raw_line_level` and `handle_key` are currently `pub use`'d. Check if they are used outside `fdemon-app`. If only used by tests or internally, change to `pub(crate) use`.

#### 2. Privatize Engine Struct Fields

Engine fields are currently `pub`, but accessor methods already exist for most. Direct field access allows bypassing Engine's orchestration guarantees.

**In `engine.rs`**, change field visibility and add new accessor methods:

| Field | Current | New | Accessor |
|-------|---------|-----|----------|
| `state` | `pub` | `pub` | Keep pub -- TUI needs `&mut state` for rendering. Consider `pub(crate)` with `state()` / `state_mut()` methods |
| `msg_tx` | `pub` | `pub(crate)` | `msg_sender()` already exists (clones) |
| `msg_rx` | `pub` | `pub(crate)` | Add `recv_message()` async method |
| `session_tasks` | `pub` | `pub(crate)` | Not needed externally |
| `shutdown_tx` | `pub` | `pub(crate)` | Not needed externally (Engine.shutdown() exists) |
| `shutdown_rx` | `pub` | `pub(crate)` | `shutdown_receiver()` already exists |
| `settings` | `pub` | `pub` | Keep pub -- TUI startup reads directly |
| `project_path` | `pub` | `pub` | Keep pub -- TUI startup reads directly |

**Add new method for headless runner:**

```rust
/// Receive the next message from the channel.
///
/// Returns None if the channel is closed.
pub async fn recv_message(&mut self) -> Option<Message> {
    self.msg_rx.recv().await
}
```

**Decision on `state` field**: The TUI runner calls `engine.state` in multiple places for rendering and startup. Making it `pub(crate)` would require `state()` / `state_mut()` accessors on Engine, but since `fdemon-tui` is a different crate, those would need to be `pub`. The simplest approach is to keep `state` as `pub` but add a comment documenting that direct mutation should go through `process_message()`:

```rust
/// TEA application state (the Model).
///
/// Read access is public for rendering. State mutations should go through
/// `process_message()` to maintain Engine invariants (event emission,
/// SharedState sync). Direct `&mut` access is provided for TUI startup
/// only -- do not mutate outside of the TEA cycle in normal operation.
pub state: AppState,
```

#### 3. Update Headless Runner for Private Fields

The headless runner currently accesses Engine fields directly. Update to use methods:

**In `src/headless/runner.rs`:**

| Current Usage | Replacement |
|--------------|-------------|
| `engine.msg_rx.recv().await` | `engine.recv_message().await` |
| `engine.msg_tx.clone()` | `engine.msg_sender()` |
| `engine.session_tasks.clone()` | Remove -- move `handle_action` call into Engine |
| `engine.shutdown_rx.clone()` | `engine.shutdown_receiver()` |

The biggest change: the headless runner calls `handle_action()` directly with `engine.session_tasks`, `engine.shutdown_rx`, and `engine.project_path`. This should be routed through Engine instead.

**Option A (preferred)**: Add `Engine::dispatch_action(action)` method that wraps `handle_action()`:

```rust
/// Dispatch an UpdateAction (same as what process_message does internally).
///
/// Used by headless runner for auto-start session spawning.
pub fn dispatch_action(&self, action: UpdateAction) {
    handle_action(
        action,
        self.msg_tx.clone(),
        None,
        Vec::new(),
        self.session_tasks.clone(),
        self.shutdown_rx.clone(),
        &self.project_path,
        Default::default(),
    );
}
```

Then the headless runner replaces the direct `handle_action()` call with `engine.dispatch_action(action)`.

#### 4. Make Internal Modules `pub(crate)`

Several modules in `fdemon-app` are only used internally or by the Engine:

**In `lib.rs`**, change module visibility:

| Module | Current | New | Reason |
|--------|---------|-----|--------|
| `actions` | `pub mod` | `pub(crate) mod` | `handle_action()` + `SessionTaskMap` only used by Engine and headless runner |
| `process` | `pub mod` | `pub(crate) mod` | `process_message()` only used by Engine |
| `signals` | `pub mod` | `pub(crate) mod` | `spawn_signal_handler()` only used by Engine |
| `input_key` | `pub mod` | `pub(crate) mod` | Key mapping helpers -- only used by handler |

Keep these as `pub mod`:
- `config` -- settings/launch config types used by TUI
- `engine` -- Engine is the primary public API
- `engine_event` -- EngineEvent used by subscribers
- `handler` -- exports UpdateAction, Task, UpdateResult (submodules now `pub(crate)`)
- `message` -- Message type used everywhere
- `session` -- Session types used by TUI for rendering
- `session_manager` -- SessionManager used by TUI for rendering
- `state` -- AppState used by TUI for rendering
- `services` -- service traits are the extension point
- `watcher` -- WatcherConfig/WatcherEvent may be useful externally
- `spawn` -- spawn functions used by TUI startup
- `editor` -- open_in_editor used by handler but could be useful externally
- `log_view_state` -- LogViewState used by TUI rendering
- `hyperlinks` -- LinkHighlightState used by TUI rendering
- `confirm_dialog` -- ConfirmDialogState used by TUI rendering
- `new_session_dialog` -- dialog state used by TUI rendering
- `settings_items` -- settings items used by TUI settings panel

#### 5. Clean Up Config Wildcard Re-export

**In `config/mod.rs`**, replace the wildcard `pub use types::*` with an explicit list of types that external crates need:

```rust
// BEFORE:
pub use types::*;

// AFTER:
pub use types::{
    FlutterMode, LaunchConfig, LoadedConfigs, Settings, WatcherSettings,
    EditorSettings, ParentIde,
};
```

Items to keep internal (only used within fdemon-app):
- `WindowPrefs` -- internal UI state
- `DevToolsSettings` -- not yet used externally
- `SettingValue`, `SettingItem`, `SettingsTab` -- used by settings_items.rs and TUI settings panel
- `ConfigSource` -- internal tracking
- `ResolvedLaunchConfig` -- internal launch resolution

**Important**: Check which types are actually used by `fdemon-tui` before finalizing the list. The TUI settings panel may need `SettingItem`, `SettingValue`, `SettingsTab`. If so, keep them in the export list.

#### 6. Update lib.rs Re-exports

Remove the re-exports of daemon types -- let consumers import from `fdemon-daemon` directly:

```rust
// REMOVE this block:
// Re-export daemon types for TUI
pub use fdemon_daemon::{AndroidAvd, Device, IosSimulator, SimulatorState, ToolAvailability};
```

**Check first**: Grep for `fdemon_app::Device`, `fdemon_app::AndroidAvd` etc. in `fdemon-tui` and `src/`. If they exist, update those imports to use `fdemon_daemon::` directly. If the TUI doesn't depend on `fdemon-daemon`, this re-export may need to stay.

Note: `fdemon-tui` only depends on `fdemon-core` and `fdemon-app` (not `fdemon-daemon`). If TUI code uses `Device`, it must come through `fdemon-app`. In that case, keep the re-export but document it:

```rust
/// Re-exported from `fdemon-daemon` for crates that depend on `fdemon-app`
/// but not `fdemon-daemon` directly.
pub use fdemon_daemon::{Device, IosSimulator, SimulatorState, ToolAvailability, AndroidAvd};
```

### Acceptance Criteria

1. Handler submodules are `pub(crate)` -- external crates cannot reach `handler::scroll::handle_scroll_up()` etc.
2. `Engine.msg_tx`, `Engine.msg_rx`, `Engine.session_tasks`, `Engine.shutdown_tx`, `Engine.shutdown_rx` are not `pub`
3. `Engine::recv_message()` method exists for headless runner
4. `Engine::dispatch_action()` method exists for headless runner
5. Headless runner uses Engine methods, not direct field access
6. `actions`, `process`, `signals` modules are `pub(crate)`
7. Config module does not use wildcard `pub use types::*`
8. `cargo check -p fdemon-app` passes
9. `cargo test -p fdemon-app` passes
10. `cargo check --workspace` passes
11. `cargo test --workspace` passes

### Testing

```bash
# Crate-level verification
cargo check -p fdemon-app
cargo test -p fdemon-app

# Test downstream crates
cargo check -p fdemon-tui
cargo test -p fdemon-tui

# Binary crate (headless runner)
cargo check
cargo test

# Full workspace verification
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
```

### Notes

- This is the largest task in Phase 4 -- it touches the most files and has the most cross-crate impact
- The headless runner is in the binary crate (`src/headless/runner.rs`), so `pub(crate)` items in `fdemon-app` are NOT accessible to it -- it must use `pub` methods
- Be careful with the config re-export cleanup: the TUI settings panel widget renders `SettingItem` values, so some config types may need to stay public
- The `handle_key` and `detect_raw_line_level` re-exports from handler need checking -- if they're only used internally, make them `pub(crate) use`
- Do NOT make `state` field private in this task -- too disruptive. Document the intended usage pattern instead
- The `spawn` module functions (`spawn_device_discovery`, `spawn_tool_availability_check`, etc.) are used by TUI startup -- keep the module public

# Plan: Cargo Workspace Restructure for Pro Version Architecture

## TL;DR

Restructure Flutter Demon from a single Rust crate into a **Cargo workspace** of 5 crates (`fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`, `fdemon` binary) to enable a **pro version** in a separate repository. The pro repo will include this repo as a **git submodule** and add premium features (MCP server, remote SSH, desktop app) by depending on the core crates and implementing extension traits. This requires fixing 6 dependency violations, extracting shared types into correct layers, wiring up the dormant services layer, and creating a clean public API surface.

---

## Background

### Current Situation

Flutter Demon is a single Rust crate (`flutter_demon`) producing one library and one binary (`fdemon`). While the architecture document describes clean layered separation, the actual dependency graph has significant violations:

```
Intended:                          Actual:

  core -> (nothing)                  core -> daemon (!!!)
  daemon -> core                     daemon -> core, config
  config -> (nothing)                config -> common
  common -> (nothing)                common -> app (!!!)
  services -> core, daemon           services -> core, daemon, common
  app -> core, daemon, tui           app -> core, daemon, tui (!!!)
  tui -> app, core, daemon           tui -> app, core, daemon, watcher
  headless -> app                    headless -> app, tui (!!!)
  watcher -> (nothing)               watcher -> app (!!!)
```

**6 dependency violations** prevent clean separation:
1. `core/events.rs` imports `DaemonMessage` from `daemon/`
2. `common/signals.rs` imports `Message` from `app/`
3. `app/state.rs`, `app/session.rs` import TUI widget types (`LogViewState`, `LinkHighlightState`, `ConfirmDialogState`) from `tui/`
4. `app/handler/*.rs` imports functions/types from `tui/` (editor, fuzzy_filter, SettingsPanel)
5. `watcher/mod.rs` imports `Message` from `app/`
6. `headless/runner.rs` imports `SessionTaskMap` and `process` from `tui/`

Additionally, the `services/` layer (with well-designed traits: `FlutterController`, `LogService`, `StateService`) is **completely dormant** -- neither TUI nor headless mode instantiates or uses these abstractions.

### Business Goal

- **Core repo** (this one): Public, open-source (BSL-1.1), contains all free features
- **Pro repo** (separate): Private, includes core as a **git submodule**, adds premium features
- **Pro features planned**: MCP server, remote SSH, desktop app (Tauri/similar)
- **Architecture**: Pro repo defines traits in core, pro implements on top

### Why Workspace Split (Not Just Fixing Coupling)

A single crate with fixed coupling would work for a git dependency approach, but the submodule strategy benefits greatly from workspace splitting because:

1. **Pro repo becomes a workspace** that includes the core submodule as workspace members
2. Pro crates can depend on **specific layers** (e.g., `fdemon-daemon` without `fdemon-tui`)
3. A desktop app only needs `fdemon-core` + `fdemon-daemon` + `fdemon-app`, not `fdemon-tui`
4. MCP server needs `fdemon-app` + `fdemon-daemon`, not the TUI
5. Compile times improve -- changing TUI code doesn't recompile daemon
6. Clean API surfaces prevent accidental coupling in the future

---

## Research Findings

### Services Layer Status

Three async trait abstractions exist but are **not wired into any runner**:

| Trait | Implementation | Status |
|-------|---------------|--------|
| `FlutterController` | `DaemonFlutterController`, `CommandSenderController` | Defined, tested in isolation, not used by TUI or headless |
| `LogService` | `SharedLogService` | Defined, tested in isolation, not used |
| `StateService` | `SharedStateService` | Defined, tested in isolation, not used |

`SharedState` (Arc<RwLock<...>> with broadcast channels) is designed for multi-consumer access but disconnected from the actual `AppState` used by the TEA loop.

### AppState/Message Coupling

`AppState` embeds TUI-specific types as direct fields:
- `ui_mode: UiMode` -- screen/modal state
- `confirm_dialog_state: Option<ConfirmDialogState>` -- imported from `tui::widgets`
- `settings_view_state: SettingsViewState` -- tab/edit state
- `loading_state: Option<LoadingState>` -- animation frames
- `new_session_dialog_state: NewSessionDialogState` -- mixed domain + UI

`Session` embeds:
- `log_view_state: LogViewState` -- imported from `tui::widgets`
- `link_highlight_state: LinkHighlightState` -- imported from `tui::hyperlinks`

`Message` enum has ~75 variants, roughly 50/50 split between domain events and TUI navigation.

### Headless Mode Duplication

The headless runner (`headless/runner.rs`, 605 lines) reimplements the entire orchestration lifecycle instead of sharing an engine with TUI. It imports `tui::SessionTaskMap` and `tui::process::process_message`, creating a headless->tui dependency.

### Current Public API

All modules are `pub mod` with no `pub(crate)` boundaries anywhere. The library surface is effectively "everything." An external crate can reach into any internal type.

---

## Target Architecture

### Workspace Layout

```
flutter-demon/                        (this repo)
  Cargo.toml                          (workspace root)
  crates/
    fdemon-core/                      (domain types, no deps on other crates)
      Cargo.toml
      src/lib.rs
      src/types.rs                    (LogEntry, LogLevel, AppPhase, etc.)
      src/events.rs                   (DaemonEvent, DaemonMessage - moved FROM daemon)
      src/discovery.rs                (Flutter project detection)
      src/stack_trace.rs              (Stack trace parsing)
      src/ansi.rs                     (ANSI handling)
      src/ui_state.rs                 (LogViewState, CollapseState, FilterState, SearchState - moved FROM tui)
      src/error.rs                    (Error enum, Result alias)
      src/prelude.rs                  (Common imports)

    fdemon-daemon/                    (Flutter process management)
      Cargo.toml                      (depends on fdemon-core)
      src/lib.rs
      src/process.rs                  (FlutterProcess)
      src/protocol.rs                 (JSON-RPC parsing)
      src/commands.rs                 (CommandSender, RequestTracker)
      src/devices.rs                  (Device discovery)
      src/emulators.rs                (Emulator management)

    fdemon-app/                       (TEA state machine + engine)
      Cargo.toml                      (depends on fdemon-core, fdemon-daemon)
      src/lib.rs
      src/state.rs                    (AppState - domain fields only)
      src/message.rs                  (Message enum - domain + TUI variants)
      src/handler/                    (TEA update function)
      src/session.rs                  (Session, SessionHandle)
      src/session_manager.rs          (SessionManager)
      src/engine.rs                   (NEW: shared Engine struct)
      src/config/                     (Settings, LaunchConfig, loaders)
      src/services/                   (traits: FlutterController, LogService, StateService)
      src/watcher.rs                  (FileWatcher - moved from top-level)
      src/signals.rs                  (Signal handling - moved from common)
      src/new_session_dialog/         (NewSessionDialog state - domain parts)

    fdemon-tui/                       (Terminal UI - ratatui)
      Cargo.toml                      (depends on fdemon-core, fdemon-app)
      src/lib.rs
      src/runner.rs                   (TUI event loop)
      src/render/                     (State -> UI rendering)
      src/layout.rs                   (Layout calculations)
      src/event.rs                    (Terminal event polling)
      src/terminal.rs                 (Terminal setup/restore)
      src/selector.rs                 (Project selector)
      src/widgets/                    (All TUI widgets)
      src/process.rs                  (process_message - moved to be TUI-specific wrapper)
      src/actions.rs                  (handle_action, SessionTaskMap)
      src/hyperlinks.rs               (Link highlight - TUI only)

  src/
    main.rs                           (fdemon binary - thin CLI)
    headless/                         (headless mode - uses Engine, not TUI)

  tests/                              (integration tests)
  docs/
  website/
  example/
```

### Dependency Graph (Target)

```
                fdemon (binary)
               /       \
              v         v
         fdemon-tui   headless
              |         |
              v         v
           fdemon-app
           /         \
          v           v
    fdemon-daemon   (config, services,
          |          watcher, signals)
          v
      fdemon-core
```

**Clean invariants:**
- `fdemon-core` depends on nothing internal (only external crates: serde, chrono, thiserror, regex)
- `fdemon-daemon` depends only on `fdemon-core`
- `fdemon-app` depends on `fdemon-core` + `fdemon-daemon`
- `fdemon-tui` depends on `fdemon-core` + `fdemon-app` (NOT on `fdemon-daemon` directly)
- Binary depends on all, routes between TUI and headless

### Pro Repo Integration

```
flutter-demon-pro/                    (pro repo)
  Cargo.toml                          (workspace root)
  core/                               (git submodule -> this repo)
    Cargo.toml                        (the workspace with fdemon-* crates)
    crates/
      fdemon-core/
      fdemon-daemon/
      fdemon-app/
      fdemon-tui/

  crates/
    fdemon-mcp/                       (MCP server - pro feature)
      Cargo.toml                      (depends on fdemon-app, fdemon-core)
      src/
        server.rs
        tools.rs
        resources.rs

    fdemon-ssh/                       (Remote SSH - pro feature)
      Cargo.toml                      (depends on fdemon-daemon, fdemon-core)
      src/

    fdemon-desktop/                   (Desktop app - pro feature)
      Cargo.toml                      (depends on fdemon-app, fdemon-core)
      src/

  src/
    main.rs                           (fdemon-pro binary)
```

```toml
# flutter-demon-pro/Cargo.toml
[workspace]
members = [
    "crates/fdemon-mcp",
    "crates/fdemon-ssh",
    "crates/fdemon-desktop",
]

[workspace.dependencies]
# Path deps pointing into the submodule
fdemon-core = { path = "core/crates/fdemon-core" }
fdemon-daemon = { path = "core/crates/fdemon-daemon" }
fdemon-app = { path = "core/crates/fdemon-app" }
fdemon-tui = { path = "core/crates/fdemon-tui" }
```

---

## Affected Modules

### Files That Move Between Crates

| Current Location | Destination Crate | Reason |
|-----------------|-------------------|--------|
| `src/core/types.rs` | `fdemon-core` | Domain types are the foundation |
| `src/core/events.rs` | `fdemon-core` | DaemonEvent/DaemonMessage belong in core (not daemon) |
| `src/core/discovery.rs` | `fdemon-core` | Project detection is domain logic |
| `src/core/stack_trace.rs` | `fdemon-core` | Stack trace parsing is domain logic |
| `src/core/ansi.rs` | `fdemon-core` | ANSI handling is domain logic |
| `src/common/error.rs` | `fdemon-core` | Error types are foundational |
| `src/common/prelude.rs` | `fdemon-core` | Common imports |
| `src/common/logging.rs` | `fdemon-core` | File-based logging setup |
| `src/daemon/*` | `fdemon-daemon` | Flutter process management |
| `src/app/*` | `fdemon-app` | TEA state machine |
| `src/config/*` | `fdemon-app` | Config belongs with app logic |
| `src/services/*` | `fdemon-app` | Service traits live with the app engine |
| `src/watcher/*` | `fdemon-app` | Watcher is app-level infrastructure |
| `src/common/signals.rs` | `fdemon-app` | Signal handling needs Message type |
| `src/tui/*` | `fdemon-tui` | Terminal UI |
| `src/headless/*` | `fdemon` (binary) or own crate | Headless mode |

### Types That Move Layer

| Type | Current Location | New Location | Reason |
|------|-----------------|-------------|--------|
| `DaemonMessage` | `daemon/events.rs` | `fdemon-core/events.rs` | Core type referenced by core/events.rs |
| `LogViewState` | `tui/widgets/log_view/state.rs` | `fdemon-core/ui_state.rs` | Used by Session (domain layer) |
| `CollapseState` | `core/stack_trace.rs` | stays in `fdemon-core` | Already correct |
| `FilterState`, `SearchState` | `core/types.rs` | stays in `fdemon-core` | Already correct |
| `LinkHighlightState` | `tui/hyperlinks.rs` | `fdemon-tui` (remove from Session) | TUI-only concern |
| `ConfirmDialogState` | `tui/widgets/` | `fdemon-tui` (remove from AppState) | TUI-only concern |
| `SettingsViewState` | `app/state.rs` | `fdemon-tui` (extract from AppState) | TUI-only concern |
| `LoadingState` | `app/state.rs` | `fdemon-tui` (extract from AppState) | TUI-only concern |
| `SessionTaskMap` | `tui/actions.rs` | `fdemon-app/engine.rs` | Shared between TUI and headless |

---

## Development Phases

### Phase 1: Fix Dependency Violations (Foundation)

**Goal**: Eliminate all 6 dependency violations so module boundaries are clean, without changing the single-crate structure yet. This is the prerequisite for splitting.

#### Steps

1. **Move `DaemonMessage` to `core/`**
   - Move the `DaemonMessage` enum from `daemon/events.rs` to `core/events.rs`
   - `core/events.rs` already defines `DaemonEvent` which wraps `DaemonMessage` -- put them together
   - Update all `use crate::daemon::DaemonMessage` to `use crate::core::DaemonMessage`
   - `daemon/` now imports from `core/` (correct direction)

2. **Move signal handler to `app/`**
   - Move `common/signals.rs` to `app/signals.rs`
   - It needs `Message` type which lives in `app/` -- this is the correct layer
   - Update imports across the codebase
   - `common/` becomes a true leaf module

3. **Extract shared UI state types to `core/`**
   - Create `core/ui_state.rs` containing:
     - `LogViewState` (currently in `tui/widgets/log_view/state.rs`)
     - Move only the **data** fields, not rendering logic
   - Update `Session` to import from `core::ui_state` instead of `tui::widgets`
   - The TUI widget still implements rendering against these types

4. **Remove TUI types from `AppState`**
   - Extract `ConfirmDialogState`, `SettingsViewState`, `LoadingState` from `AppState`
   - Create a `TuiState` struct in `tui/` that wraps these TUI-specific fields
   - `AppState` contains only domain state; `TuiState` contains UI state
   - TUI runner owns `(AppState, TuiState)` as a pair
   - Headless runner owns only `AppState`

5. **Remove `LinkHighlightState` from `Session`**
   - Move to a `TuiSessionState` wrapper in `tui/`
   - TUI runner maintains a parallel `HashMap<SessionId, TuiSessionState>`
   - The `Session` struct becomes pure domain state

6. **Extract `SessionTaskMap` and `process_message` from `tui/`**
   - Move `SessionTaskMap` type alias to `app/engine.rs`
   - Move `process_message()` to `app/process.rs` (it's already pure state logic)
   - TUI and headless both import from `app/` (correct direction)
   - Remove headless -> tui dependency

7. **Remove handler -> tui dependencies**
   - `app/handler/log_view.rs` imports `open_in_editor` from `tui/editor` -- move editor logic to `app/`
   - `app/handler/update.rs` imports `fuzzy_filter` from `tui/widgets` -- extract filtering logic to `app/` or `core/`
   - `app/handler/settings_handlers.rs` imports `ConfirmDialogState`, `SettingsPanel` from `tui/` -- use the extracted types
   - `app/new_session_dialog/state.rs` imports `TargetSelectorState` from `tui/` -- extract data portion to `app/`

8. **Make watcher generic**
   - Change `FileWatcher` to accept a generic `Sender<T>` or callback instead of requiring `Message`
   - Move watcher instantiation logic that creates `Message::FilesChanged` to `app/`
   - `watcher/` becomes a pure infrastructure module with no `app/` dependency

**Milestone**: `cargo test` passes. All module imports flow downward. The single crate builds cleanly with the correct dependency direction.

---

### Phase 2: Extract Engine and Wire Services

**Goal**: Create a shared `Engine` abstraction that both TUI and headless runners use, and wire the services layer into the actual runtime.

#### Steps

1. **Create `Engine` struct in `app/engine.rs`**
   ```rust
   pub struct Engine {
       pub state: AppState,
       pub msg_tx: mpsc::Sender<Message>,
       pub msg_rx: mpsc::Receiver<Message>,
       pub session_tasks: SessionTaskMap,
       pub shutdown_tx: watch::Sender<bool>,
       pub shutdown_rx: watch::Receiver<bool>,
       pub file_watcher: Option<FileWatcher>,
       pub settings: Settings,
       pub project_path: PathBuf,
   }

   impl Engine {
       pub fn new(project_path: PathBuf, settings: Settings) -> Self;
       pub fn process_pending_messages(&mut self) -> Vec<UpdateAction>;
       pub fn send_message(&self, msg: Message);
       pub async fn shutdown(self);
   }
   ```

2. **Refactor TUI runner to use Engine**
   - TUI runner creates `Engine`, then adds its own terminal/rendering loop
   - `(engine, tui_state)` pair replaces current flat state
   - Rendering reads `engine.state` + `tui_state`

3. **Refactor headless runner to use Engine**
   - Headless runner creates `Engine`, adds its own NDJSON event emission
   - Remove all duplicated orchestration code from `headless/runner.rs`
   - Headless event loop: `engine.process_pending_messages()` + emit events

4. **Wire services layer into Engine**
   - Engine creates and manages `SharedState`
   - Synchronize `AppState` changes to `SharedState` after each message batch
   - Engine exposes `pub fn flutter_controller(&self) -> Arc<dyn FlutterController>`
   - Engine exposes `pub fn log_service(&self) -> Arc<dyn LogService>`
   - Engine exposes `pub fn state_service(&self) -> Arc<dyn StateService>`

5. **Add event broadcasting to Engine**
   - Engine provides `pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent>`
   - This is the extension point for pro features (MCP server subscribes here)
   - `EngineEvent` wraps domain events (daemon messages, state changes, log entries)

**Milestone**: TUI and headless both run via `Engine`. Services layer is live. External code can create an `Engine` and subscribe to events.

---

### Phase 3: Cargo Workspace Split

**Goal**: Split the single crate into a Cargo workspace with 4 library crates and 1 binary.

#### Steps

1. **Create workspace structure**
   ```
   flutter-demon/
     Cargo.toml              (workspace root)
     crates/
       fdemon-core/Cargo.toml
       fdemon-daemon/Cargo.toml
       fdemon-app/Cargo.toml
       fdemon-tui/Cargo.toml
     src/main.rs              (binary, depends on all)
   ```

2. **Set up workspace `Cargo.toml`**
   ```toml
   [workspace]
   members = ["crates/*"]
   resolver = "2"

   [workspace.package]
   version = "0.1.0"
   edition = "2021"
   license = "BSL-1.1"

   [workspace.dependencies]
   # Shared external deps
   tokio = { version = "1", features = ["full"] }
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   chrono = { version = "0.4", features = ["serde"] }
   tracing = "0.1"
   thiserror = "2"
   # Internal crate deps
   fdemon-core = { path = "crates/fdemon-core" }
   fdemon-daemon = { path = "crates/fdemon-daemon" }
   fdemon-app = { path = "crates/fdemon-app" }
   fdemon-tui = { path = "crates/fdemon-tui" }
   ```

3. **Create `fdemon-core`**
   - Move: `core/*`, `common/error.rs`, `common/prelude.rs`, `common/logging.rs`
   - Dependencies: `serde`, `chrono`, `thiserror`, `tracing`, `regex`
   - No internal crate dependencies (true leaf)

4. **Create `fdemon-daemon`**
   - Move: `daemon/*`
   - Dependencies: `fdemon-core`, `tokio`, `serde_json`
   - Does NOT depend on config (pass args directly to `FlutterProcess::spawn`)

5. **Create `fdemon-app`**
   - Move: `app/*`, `config/*`, `services/*`, `watcher/*`, `signals`
   - Dependencies: `fdemon-core`, `fdemon-daemon`, `tokio`, `toml`, `clap`, `notify`, `dirs`
   - Exports: `Engine`, `AppState`, `Message`, service traits

6. **Create `fdemon-tui`**
   - Move: `tui/*`
   - Dependencies: `fdemon-core`, `fdemon-app`, `ratatui`, `crossterm`
   - Does NOT depend on `fdemon-daemon` directly
   - Owns: `TuiState`, `TuiSessionState`, rendering, widgets, terminal

7. **Update binary `src/main.rs`**
   - Dependencies: `fdemon-core`, `fdemon-app`, `fdemon-tui`
   - CLI routing between TUI and headless modes

8. **Move integration tests**
   - `tests/` directory stays at workspace root
   - Tests depend on whichever crates they need

9. **Verify clean build**
   - `cargo build` compiles all crates
   - `cargo test` passes across workspace
   - `cargo clippy` is clean
   - `cargo fmt` is consistent

**Milestone**: Workspace builds successfully. Each crate has a clean, minimal dependency set. Binary produces the same `fdemon` executable.

---

### Phase 4: Public API Surface and Visibility

**Goal**: Define clean, documented public APIs for each crate. Add `pub(crate)` boundaries. Enable pro repo consumption.

#### Steps

1. **Define `fdemon-core` public API**
   - Export domain types, error types, prelude
   - Mark internal helpers as `pub(crate)`
   - Add `//!` crate-level documentation

2. **Define `fdemon-daemon` public API**
   - Export: `FlutterProcess`, `CommandSender`, `Device`, device/emulator discovery
   - Internalize: protocol parsing details, raw JSON types
   - Ensure `FlutterProcess::spawn()` takes owned args (not `LaunchConfig`)

3. **Define `fdemon-app` public API**
   - Export: `Engine`, `AppState`, `Message`, `UpdateAction`, `SessionManager`
   - Export: `FlutterController`, `LogService`, `StateService` traits + SharedState
   - Export: `Settings`, `LaunchConfig`, configuration loaders
   - Mark handler internals as `pub(crate)` where possible

4. **Define `fdemon-tui` public API**
   - Export: `run_tui(engine: Engine)` entry point
   - Export: Widget types for potential reuse
   - Internalize: rendering details, layout math

5. **Add extension traits for pro features**
   - `Engine` should have a plugin/extension mechanism:
     ```rust
     // In fdemon-app
     pub trait EnginePlugin: Send + Sync {
         fn name(&self) -> &str;
         fn on_start(&self, engine: &Engine) -> Result<()>;
         fn on_message(&self, msg: &Message, state: &AppState) -> Result<()>;
         fn on_shutdown(&self) -> Result<()>;
     }

     impl Engine {
         pub fn register_plugin(&mut self, plugin: Box<dyn EnginePlugin>);
     }
     ```
   - Pro MCP server implements `EnginePlugin`

6. **Document the extension API**
   - Add `docs/EXTENSION_API.md` describing how pro features hook in
   - Document which traits to implement, which events to subscribe to
   - Provide examples of creating a custom plugin

**Milestone**: Clean public APIs. Pro repo can `depend on fdemon-app` and build an MCP server using the Engine and service traits.

---

### Phase 5: Pro Repo Scaffolding and Validation

**Goal**: Create the pro repo skeleton, validate the submodule approach works, and confirm end-to-end compilation.

#### Steps

1. **Create pro repo with submodule**
   ```bash
   mkdir flutter-demon-pro && cd flutter-demon-pro
   git init
   git submodule add https://github.com/<org>/flutter-demon.git core
   ```

2. **Set up pro workspace**
   ```toml
   # flutter-demon-pro/Cargo.toml
   [workspace]
   members = ["crates/*"]
   resolver = "2"

   [workspace.dependencies]
   fdemon-core = { path = "core/crates/fdemon-core" }
   fdemon-daemon = { path = "core/crates/fdemon-daemon" }
   fdemon-app = { path = "core/crates/fdemon-app" }
   fdemon-tui = { path = "core/crates/fdemon-tui" }
   tokio = { version = "1", features = ["full"] }
   ```

3. **Create stub `fdemon-mcp` crate**
   ```toml
   # flutter-demon-pro/crates/fdemon-mcp/Cargo.toml
   [package]
   name = "fdemon-mcp"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   fdemon-core.workspace = true
   fdemon-app.workspace = true
   ```

4. **Create `fdemon-pro` binary**
   - Imports `Engine` from `fdemon-app`
   - Registers MCP plugin
   - Calls `fdemon-tui::run_tui(engine)` or headless mode

5. **Validate end-to-end**
   - `cargo build` in pro repo compiles everything (core submodule + pro crates)
   - `cargo test` runs tests from both core and pro
   - Pro binary starts, MCP stub initializes

6. **CI/CD for pro repo**
   ```yaml
   steps:
     - uses: actions/checkout@v4
       with:
         submodules: recursive
     - run: cargo build --release
     - run: cargo test
   ```

**Milestone**: Pro repo builds and runs with core as a submodule. The architecture is validated end-to-end.

---

## Edge Cases & Risks

### Crate Splitting Risks

- **Risk**: Moving files between crates breaks `#[cfg(test)]` tests that rely on `use super::*` for private access
- **Mitigation**: Tests that need private access stay in their crate. Integration tests that cross crate boundaries use public APIs only. Plan test migration per-crate.

- **Risk**: Circular dependency between crates discovered late (e.g., `fdemon-app` needs a type from `fdemon-tui`)
- **Mitigation**: Phase 1 explicitly fixes all violations first. Phase 3 only moves files along the already-clean dependency lines.

- **Risk**: Performance regression from trait dispatch (`dyn FlutterController`) vs direct calls
- **Mitigation**: Use generics where performance-critical. Trait objects only at the plugin boundary, not in hot loops.

### Submodule Risks

- **Risk**: Contributors forget `git submodule update --init --recursive`
- **Mitigation**: Add a build script or Makefile that checks submodule status. Document prominently.

- **Risk**: Submodule version drift -- pro repo pins old core version
- **Mitigation**: Use tagged versions in the submodule. CI checks for version compatibility.

- **Risk**: Pro workspace Cargo.lock conflicts with core workspace Cargo.lock
- **Mitigation**: Only the pro workspace root Cargo.lock is used. Core's Cargo.lock is for standalone builds only.

### API Stability Risks

- **Risk**: Core crate API changes break pro repo
- **Mitigation**: Semantic versioning for core crates. Pro repo pins to minor versions. Breaking changes go through RFC process.

- **Risk**: `Engine` plugin API is too restrictive or too loose
- **Mitigation**: Start with a minimal plugin trait. Expand based on actual pro feature needs. Don't over-design.

### Services Layer Risks

- **Risk**: `SharedState` synchronization with `AppState` causes subtle bugs or race conditions
- **Mitigation**: Synchronize in one direction only (AppState -> SharedState, never reverse). Use `watch` channel for state change notification.

- **Risk**: Services layer adds overhead to every message processing cycle
- **Mitigation**: Only sync to SharedState when state actually changes (dirty flag). Benchmark before/after.

### Test Migration Risks

- **Risk**: Tests that import private types break when crate boundaries change
- **Mitigation**: Audit all `#[cfg(test)]` blocks during Phase 1. Identify which tests need refactoring.

- **Risk**: Integration test coverage gaps after split
- **Mitigation**: Keep workspace-level integration tests. Add cross-crate integration tests as needed.

---

## Configuration Additions

No new configuration files. Existing `.fdemon/config.toml` and `.fdemon/launch.toml` continue to work unchanged.

The workspace root `Cargo.toml` structure is shown in Phase 3 steps.

---

## Success Criteria

### Phase 1 Complete When:
- [ ] All module imports flow downward (no upward/circular dependencies)
- [ ] `cargo test` passes with no regressions
- [ ] `DaemonMessage` lives in `core/`, not `daemon/`
- [ ] `AppState` contains no TUI widget types
- [ ] `Session` contains no TUI widget types
- [ ] `headless/` does not import from `tui/`
- [ ] `common/` does not import from `app/`
- [ ] `watcher/` does not import from `app/`

### Phase 2 Complete When:
- [ ] `Engine` struct exists and encapsulates shared orchestration
- [ ] TUI runner uses `Engine` (no duplicated setup code)
- [ ] Headless runner uses `Engine` (no duplicated setup code)
- [ ] Services layer (`FlutterController`, `LogService`, `StateService`) is wired into `Engine`
- [ ] `Engine.subscribe()` returns a broadcast receiver for events

### Phase 3 Complete When:
- [ ] Workspace has 4 library crates + 1 binary
- [ ] `fdemon-core` has zero internal crate dependencies
- [ ] `fdemon-daemon` depends only on `fdemon-core`
- [ ] `fdemon-app` depends only on `fdemon-core` + `fdemon-daemon`
- [ ] `fdemon-tui` depends only on `fdemon-core` + `fdemon-app`
- [ ] `cargo build` succeeds at workspace root
- [ ] `cargo test` passes across all crates
- [ ] `cargo clippy` is clean
- [ ] Binary produces identical `fdemon` behavior

### Phase 4 Complete When:
- [ ] Each crate has a documented public API
- [ ] Internal types use `pub(crate)` where appropriate
- [ ] `EnginePlugin` trait exists with at least `on_start`/`on_message`/`on_shutdown`
- [ ] Extension API documentation exists
- [ ] An example plugin can compile against the API

### Phase 5 Complete When:
- [ ] Pro repo exists with core as a git submodule
- [ ] Pro workspace builds successfully with `cargo build`
- [ ] Stub MCP crate compiles against `fdemon-app` traits
- [ ] `fdemon-pro` binary starts and runs the TUI
- [ ] CI workflow works with `submodules: recursive`

---

## Impact on Existing Plans

### DevTools Integration (PLAN.md)

The DevTools plan adds `src/vmservice/` as a new module. After workspace restructure:
- `vmservice/` would live in `fdemon-app` (or its own `fdemon-vmservice` crate)
- WebSocket client depends on `fdemon-core` types
- VM Service logging integrates with `Engine` event system
- TUI DevTools panels live in `fdemon-tui`
- No conflicts -- DevTools implementation should happen AFTER this restructure

### MCP Server (PLAN.md)

The MCP plan becomes much cleaner after restructure:
- MCP server lives in **pro repo** as `fdemon-mcp` crate
- Depends on `fdemon-app` for `Engine`, `FlutterController`, `LogService`, `StateService`
- Uses `EnginePlugin` trait to hook into the runtime
- No need for the service layer extraction described in MCP Phase 1 -- it's already done
- HTTP server (axum) only in pro repo, not in core

### Key Sequencing

```
1. Workspace Restructure (this plan)  ←  DO THIS FIRST
2. DevTools Integration               ←  builds on clean Engine
3. MCP Server (pro repo)              ←  builds on Engine + Services
```

---

## Future Enhancements

1. **Publish core crates to crates.io** -- Once APIs stabilize, publish `fdemon-core` and `fdemon-daemon` for community use
2. **Plugin discovery** -- Dynamic plugin loading via shared libraries (`.so`/`.dylib`)
3. **Watcher as separate crate** -- If other projects want file-watching with debounce
4. **Config crate** -- Separate config parsing if the format becomes reusable
5. **Desktop app crate** -- Tauri/similar using `fdemon-app` engine with a web UI instead of terminal

---

## References

- [Cargo Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Cargo Features](https://doc.rust-lang.org/cargo/reference/features.html)
- [Git Submodules](https://git-scm.com/book/en/v2/Git-Tools-Submodules)
- [Tantivy/Quickwit Pattern](https://github.com/quickwit-oss/tantivy) -- Open core library + commercial product
- [TiKV Workspace](https://github.com/tikv/tikv) -- Large Rust workspace example
- [BSL-1.1 License](https://mariadb.com/bsl11/) -- Business Source License used by this project

## Task: Verify Clean Architecture and Update Documentation

**Objective**: Final verification that the Engine abstraction is working correctly across both TUI and headless runners, services are wired, event broadcasting works, and documentation accurately reflects the new architecture.

**Depends on**: Task 06 (all Engine features implemented)

**Estimated Time**: 2-4 hours

### Scope

- `docs/ARCHITECTURE.md`: Update to reflect Engine-based architecture
- Full test suite verification
- Headless E2E test verification
- Code quality audit (clippy, fmt, dead code)

### Details

#### Verification Steps

1. **Build verification**:
   ```bash
   cargo fmt -- --check
   cargo check
   cargo build
   cargo clippy -- -D warnings
   ```

2. **Test verification**:
   ```bash
   cargo test                    # All tests
   cargo test --lib              # Unit tests only
   cargo test engine             # Engine-specific tests
   cargo test headless           # Headless tests
   cargo test handler            # Handler tests
   cargo test --test e2e         # E2E tests (if available)
   ```

3. **Manual TUI verification**:
   - `cargo run -- /path/to/flutter/project`
   - Verify startup, device selection, session spawning
   - Verify hot reload (press `r`)
   - Verify auto-reload (edit a .dart file)
   - Verify session switching (if multi-session)
   - Verify quit (press `q`)
   - Verify Ctrl+C handling

4. **Manual headless verification**:
   - `cargo run -- --headless /path/to/flutter/project`
   - Verify NDJSON events on stdout
   - Verify stdin commands (`r`, `q`)
   - Verify Ctrl+C handling

#### Dead Code Audit

Check for code that became dead after the Engine refactor:

```bash
# Look for unused imports
cargo clippy -- -W unused-imports

# Look for dead code warnings
cargo clippy -- -W dead-code
```

Specifically check:
- `tui/startup.rs::cleanup_sessions()` -- may be partially dead if Engine::shutdown() replaced it
- Old signal handler in `headless/runner.rs` -- should be removed
- Old `spawn_headless_session()` -- should be removed
- Any `#[allow(dead_code)]` annotations that are no longer needed

#### Architecture Documentation Update

Update `docs/ARCHITECTURE.md` to reflect the Engine-based architecture:

**Add Engine section**:
```markdown
### Engine (`app/engine.rs`)

The Engine is the shared orchestration core used by both TUI and headless runners.
It encapsulates:
- Application state (AppState)
- Message channel (send/receive)
- Session task management
- File watcher with auto-reload
- Signal handling (SIGINT/SIGTERM)
- Shared state for service layer
- Event broadcasting for external consumers

```
Binary (main.rs)
       │
       ├──► TUI Runner ◄─── crossterm events
       │        │
       │        ▼
       │    ┌─────────┐
       │    │  Engine  │ ◄─── signal handler
       │    │         │ ◄─── file watcher
       │    │ state   │ ◄─── session tasks
       │    │ channels│
       │    │ services│ ──► broadcast events
       │    └─────────┘
       │        │
       └──► Headless Runner ◄─── stdin commands
                │
                ▼
            ┌─────────┐
            │  Engine  │ (same struct, different frontend)
            └─────────┘
```
```

**Update data flow diagram** to show Engine in the pipeline:
```
Events → Message Channel → Engine.process_message() → handler::update()
                                                     → handle_action()
                                                     → emit_events()
                                                     → sync_shared_state()
```

**Update services section** to show they are now live:
```markdown
### Services Layer (wired via Engine)

The services layer provides trait-based abstractions for Flutter control
operations. These are instantiated and managed by the Engine:

- `FlutterController`: Hot reload/restart operations via CommandSender
- `LogService`: Log buffer access via SharedState
- `StateService`: App run state access via SharedState

External consumers (future MCP server) access services through Engine:
```rust
let controller = engine.flutter_controller().unwrap();
controller.reload().await?;

let logs = engine.log_service();
let entries = logs.get_logs(100).await;
```
```

**Update module reference table** to include new files:
```markdown
| `app/engine.rs` | Engine struct -- shared orchestration core |
| `app/engine_event.rs` | EngineEvent enum for broadcast events |
```

### Acceptance Criteria

1. `cargo fmt -- --check` passes (no formatting issues)
2. `cargo check` passes (no compilation errors)
3. `cargo test` passes (all tests, no regressions)
4. `cargo clippy -- -D warnings` passes (no clippy warnings)
5. No dead code warnings from the refactor
6. `docs/ARCHITECTURE.md` accurately describes the Engine
7. `docs/ARCHITECTURE.md` shows the Engine in the data flow diagram
8. `docs/ARCHITECTURE.md` shows services as wired (not dormant)
9. `docs/ARCHITECTURE.md` documents `EngineEvent` and `subscribe()`
10. Manual TUI test passes (startup, reload, quit)
11. Manual headless test passes (NDJSON, stdin, quit)

### Testing

```bash
# Full verification pipeline
cargo fmt -- --check && cargo check && cargo test && cargo clippy -- -D warnings
```

### Notes

- This is the final task in Phase 2. After this, the codebase has:
  - A clean Engine abstraction shared by TUI and headless
  - Live services layer accessible through the Engine
  - Event broadcasting ready for pro feature consumers
  - Documented architecture matching the actual code
- Phase 3 (Cargo Workspace Split) can now proceed because the Engine provides clean crate boundaries:
  - `fdemon-core`: domain types (no Engine dependency)
  - `fdemon-daemon`: Flutter process management (no Engine dependency)
  - `fdemon-app`: Engine + state + handlers + services
  - `fdemon-tui`: TUI runner (creates Engine, adds terminal)

---

## Completion Summary

**Status:** Not Started

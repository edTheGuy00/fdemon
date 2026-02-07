## Task: Verify Clean Dependencies and Update Documentation

**Objective**: Final verification that all dependency violations are fixed, plus update `docs/ARCHITECTURE.md` to reflect the new module structure.

**Depends on**: Task 07 (all code changes must be complete)

**Estimated Time**: 1-2 hours

### Scope

- Verify all 6 dependency violations are resolved
- Run full test suite and lint checks
- Update `docs/ARCHITECTURE.md`

### Details

#### Step 1: Verify dependency directions

Run these grep checks. Each must return **zero results** (or only acceptable results as noted):

```bash
# 1. core/ must not import from daemon/
grep -rn "crate::daemon" src/core/ --include="*.rs"
# Expected: ZERO results

# 2. common/ must not import from app/
grep -rn "crate::app" src/common/ --include="*.rs"
# Expected: ZERO results

# 3. watcher/ must not import from app/
grep -rn "crate::app" src/watcher/ --include="*.rs"
# Expected: ZERO results

# 4. app/ must not import from tui/ (except app/mod.rs entry point)
grep -rn "crate::tui" src/app/ --include="*.rs"
# Expected: ONLY src/app/mod.rs (calling tui::run_with_project)

# 5. headless/ must not import from tui/
grep -rn "crate::tui" src/headless/ --include="*.rs"
# Expected: ZERO results

# 6. daemon/ must not import from app/ or tui/
grep -rn "crate::app\|crate::tui" src/daemon/ --include="*.rs"
# Expected: ZERO results

# 7. services/ must not import from tui/
grep -rn "crate::tui" src/services/ --include="*.rs"
# Expected: ZERO results
```

#### Step 2: Verify the target dependency graph

The module dependency flow should now be:

```
core/     -> (nothing internal)
common/   -> (nothing internal)
daemon/   -> core
config/   -> common
services/ -> core, daemon, common
watcher/  -> (nothing internal -- uses WatcherEvent, not Message)
app/      -> core, daemon, config, services, watcher, common
tui/      -> core, daemon, config, app, common
headless/ -> core, daemon, config, app, common
```

For each module, verify its imports only go to allowed dependencies.

#### Step 3: Run full verification suite

```bash
cargo fmt --check     # Formatting
cargo build           # Compilation
cargo clippy          # Lints
cargo test            # All tests
cargo test --lib      # Unit tests only
cargo test --test '*' # Integration tests only
```

All must pass cleanly.

#### Step 4: Update `docs/ARCHITECTURE.md`

Update the following sections:

##### 4a. Update the Project Structure tree

Reflect new files:
```
src/
├── app/
│   ├── mod.rs
│   ├── state.rs
│   ├── message.rs
│   ├── signals.rs              # Moved from common/
│   ├── process.rs              # Moved from tui/
│   ├── actions.rs              # Moved from tui/
│   ├── spawn.rs                # Moved from tui/
│   ├── editor.rs               # Moved from tui/
│   ├── settings_items.rs       # Extracted from tui/widgets/settings_panel/
│   ├── log_view_state.rs       # Moved from tui/widgets/log_view/
│   ├── hyperlinks.rs           # Moved from tui/hyperlinks
│   ├── confirm_dialog.rs       # Moved from tui/widgets/confirm_dialog
│   ├── handler/
│   ├── session.rs
│   ├── session_manager.rs
│   └── new_session_dialog/
│       ├── state.rs
│       ├── fuzzy.rs            # Extracted from tui/widgets/
│       ├── target_selector_state.rs  # Moved from tui/widgets/
│       └── device_groups.rs    # Moved from tui/widgets/
│
├── core/
│   ├── types.rs
│   ├── events.rs               # Now contains DaemonMessage + event structs
│   ├── discovery.rs
│   ├── stack_trace.rs
│   └── ansi.rs
│
├── daemon/
│   ├── protocol.rs             # DaemonMessage::parse() still here
│   ├── process.rs
│   ├── commands.rs
│   ├── devices.rs
│   └── emulators.rs
```

##### 4b. Update the Layer Dependencies table

Update the table at line ~74:

| Layer | Responsibility | Dependencies |
|-------|----------------|--------------|
| **Binary** | CLI, entry point | All |
| **App** | State, orchestration, TEA, action dispatch | Core, Daemon, Config, Services, Watcher, Common |
| **Services** | Reusable controllers | Core, Daemon, Common |
| **TUI** | Presentation | Core, App |
| **Headless** | NDJSON event output | Core, App |
| **Daemon** | Flutter process I/O | Core |
| **Config** | Configuration parsing | Common |
| **Watcher** | File system watching | None (emits WatcherEvent) |
| **Core** | Domain types, events | None |
| **Common** | Utilities, error types | None |

##### 4c. Update the Module Reference section

- **`core/events.rs`**: Now contains `DaemonEvent`, `DaemonMessage`, and all 9 event structs (moved from daemon/)
- **`app/process.rs`**: TEA message processing loop (moved from tui/)
- **`app/actions.rs`**: Action dispatch, `SessionTaskMap` (moved from tui/)
- **`app/signals.rs`**: Signal handling (moved from common/)
- **`app/hyperlinks.rs`**: Link detection and state (moved from tui/)
- **`app/log_view_state.rs`**: Scroll/viewport state (moved from tui/)
- **`watcher/mod.rs`**: Now emits `WatcherEvent` (no longer depends on `Message`)

##### 4d. Add a "Phase 1 Changes" section (optional)

A brief note explaining the restructuring for contributors:

```markdown
### Restructuring Notes (Phase 1)

Several types and functions were relocated to enforce clean layer boundaries:

- **Event types** (`DaemonMessage`, event structs) moved from `daemon/` to `core/` -- core is now a true leaf module
- **State types** (`LogViewState`, `LinkHighlightState`, `ConfirmDialogState`) moved from `tui/` to `app/` -- app no longer depends on tui for state
- **Logic functions** (`process_message`, `handle_action`, `open_in_editor`, `fuzzy_filter`) moved from `tui/` to `app/` -- headless no longer depends on tui
- **Signal handler** moved from `common/` to `app/` -- common is now a true leaf module
- **File watcher** emits its own `WatcherEvent` instead of constructing `Message` -- watcher is now independent of app
```

### Acceptance Criteria

1. All 7 grep checks in Step 1 pass (clean dependency directions)
2. `cargo fmt --check` passes
3. `cargo build` succeeds
4. `cargo clippy` is clean
5. `cargo test` passes with no regressions
6. `docs/ARCHITECTURE.md` accurately reflects the new structure
7. The dependency table in ARCHITECTURE.md matches the actual imports

### Testing

```bash
cargo fmt --check     # Formatting
cargo build           # Compilation
cargo clippy          # Lints
cargo test            # All tests
```

### Notes

- This task is primarily verification and documentation. If any grep checks fail, the issue must be traced back to the relevant task (01-07) and fixed before proceeding.
- After this task, Phase 1 is complete. The codebase is ready for Phase 2 (Engine extraction) and Phase 3 (workspace split).

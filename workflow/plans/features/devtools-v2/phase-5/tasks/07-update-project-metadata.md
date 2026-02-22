## Task: Update CLAUDE.md Project Metadata

**Objective**: Update the `CLAUDE.md` file at the project root to reflect the post-DevTools-V2 state: add DevTools modules to the architecture description, update the test count, document the `[devtools]` configuration section, and add DevTools-related key patterns.

**Depends on**: 01 (needs final config fields)

### Scope

- `CLAUDE.md`: MODIFIED — Update Architecture, Testing, Configuration, and Key Patterns sections

### Details

#### 1. Update Architecture section

The current Architecture section shows a crate dependency diagram and lists the 5 crates. Add DevTools-specific modules to each crate description:

In the `fdemon-core` bullet:
```markdown
- **`fdemon-core`** (`crates/fdemon-core/`): Domain types (`LogEntry`, `LogLevel`, `AppPhase`), performance types (`FrameTiming`, `MemorySample`, `RingBuffer`), network types (`HttpProfileEntry`, `NetworkTiming`), widget tree types (`DiagnosticsNode`, `LayoutInfo`), project discovery, error handling. **Zero internal dependencies.**
```

In the `fdemon-daemon` bullet:
```markdown
- **`fdemon-daemon`** (`crates/fdemon-daemon/`): Flutter process management, JSON-RPC protocol parsing (`--machine` mode), device/emulator discovery, VM Service WebSocket client (`vm_service/`) with extensions for inspector, performance, and network profiling. Depends on `fdemon-core`.
```

In the `fdemon-app` bullet:
```markdown
- **`fdemon-app`** (`crates/fdemon-app/`): TEA implementation - `AppState` (model), `Message` (events), `handler::update()` (state transitions), Engine orchestration, services, config, watcher. DevTools handlers in `handler/devtools/` with per-session state (`PerformanceState`, `NetworkState`). Depends on `fdemon-core` + `fdemon-daemon`.
```

In the `fdemon-tui` bullet:
```markdown
- **`fdemon-tui`** (`crates/fdemon-tui/`): Ratatui-based terminal UI with widgets. DevTools panels in `widgets/devtools/` (Inspector, Performance, Network) with sub-component decomposition. Depends on `fdemon-core` + `fdemon-app`.
```

#### 2. Update Testing section

Update the test count listing. The current listing shows:

```
- `crates/fdemon-core/src/` - 243 unit tests
- `crates/fdemon-daemon/src/` - 136 unit tests
- `crates/fdemon-app/src/handler/tests.rs` - 726 unit tests (state transitions)
- `crates/fdemon-tui/src/widgets/` - 427 unit tests (rendering)
- `tests/` directory - Integration tests (binary crate)

Total: 1,532 unit tests across 4 crates
```

Run `cargo test --workspace 2>&1 | tail -5` to get the actual current test count and update accordingly. The total will be significantly higher after DevTools V2 (~500+ new tests across phases 1-5).

#### 3. Update Configuration section

The current section lists:
```
- `.fdemon/config.toml` - Global settings (watcher paths, debounce, UI options, editor)
- `.fdemon/launch.toml` - Launch configurations (device, mode, flavor, dart-defines)
- `.vscode/launch.json` - Auto-imported VSCode Dart configurations
```

Add a note about DevTools configuration:
```
- `.fdemon/config.toml` - Global settings (watcher paths, debounce, UI options, editor, DevTools settings)
```

#### 4. Update Key Patterns section

Add DevTools-related patterns:

```markdown
- **VM Service client**: `fdemon-daemon/vm_service/` provides WebSocket-based communication with the Dart VM Service for inspector, performance monitoring, and network profiling
- **DevTools handler decomposition**: `fdemon-app/handler/devtools/` splits DevTools message handling into `inspector.rs`, `performance.rs`, `network.rs` sub-modules
- **Per-session DevTools state**: `session/performance.rs` and `session/network.rs` hold ring-buffered telemetry per Flutter session
```

### Acceptance Criteria

1. Architecture section mentions VM Service client, DevTools handlers, and DevTools widgets
2. Test count reflects actual current numbers (run `cargo test --workspace` to verify)
3. Configuration section mentions DevTools settings
4. Key Patterns section includes DevTools-related patterns
5. No existing information removed — only added/updated
6. All file paths referenced exist in the codebase

### Testing

No code tests — documentation-only task.

Verification: Run `cargo test --workspace 2>&1 | grep "test result"` to get accurate test counts for each crate.

### Notes

- **Minimal changes**: CLAUDE.md serves as a concise project overview for Claude Code. Keep additions brief — point to ARCHITECTURE.md for detailed DevTools documentation.
- **Test count accuracy**: The test count should be exact. Run the actual test suite rather than estimating.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `CLAUDE.md` | Updated Architecture, Testing, Configuration, and Key Patterns sections with DevTools V2 information |

### Notable Decisions/Tradeoffs

1. **Test counts from `cargo test --lib`**: Used `--lib` flag per crate to isolate unit test counts, excluding doc tests and integration tests. Actual counts: fdemon-core 357, fdemon-daemon 375, fdemon-app 1,037, fdemon-tui 754 — total 2,523 unit tests (up from 1,532).
2. **fdemon-app path updated**: Changed `crates/fdemon-app/src/handler/tests.rs` to `crates/fdemon-app/src/` since tests are now spread across handler, session, and other modules — more accurate after DevTools V2 additions.

### Testing Performed

- `cargo test --lib -p fdemon-core` — 357 tests, Passed
- `cargo test --lib -p fdemon-daemon` — 375 tests, Passed
- `cargo test --lib -p fdemon-app` — 1,037 tests, Passed
- `cargo test --lib -p fdemon-tui` — 754 tests, Passed
- All referenced file paths verified to exist in the codebase

### Risks/Limitations

1. **Integration test failures**: The `flutter-demon` binary e2e tests have 25 failures, but these are pre-existing and unrelated to this documentation task (no code was changed).
2. **Path specificity**: The fdemon-app test path is now `crates/fdemon-app/src/` rather than the specific `handler/tests.rs` file, which is more accurate given the distributed test structure post-DevTools V2.

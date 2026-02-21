## Task: Final Test and Cleanup

**Objective**: Run the full quality gate across the workspace, verify all Phase 4 success criteria, fix any remaining issues, and ensure no regressions in existing functionality.

**Depends on**: Task 05 (wire-network-monitoring-engine), Task 08 (wire-network-monitor-panel)

### Scope

- All crates in the workspace
- Focus on integration between the new network components

### Details

#### Quality gate

Run the full verification sequence:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

All four must pass clean.

#### Cross-crate integration verification

1. **Core → Daemon**: Verify that `fdemon-daemon/src/vm_service/network.rs` correctly uses types from `fdemon-core/src/network.rs`. Run `cargo test -p fdemon-daemon` and confirm all network parser tests pass.

2. **Core → App**: Verify that `fdemon-app/src/session/network.rs` correctly uses core types. Run `cargo test -p fdemon-app` and confirm all network state and handler tests pass.

3. **App → TUI**: Verify that `fdemon-tui/src/widgets/devtools/network/` correctly accesses session network state via `AppState`. Run `cargo test -p fdemon-tui` and confirm all network widget tests pass.

4. **Full message flow**: Trace the complete flow for a network poll cycle:
   - `StartNetworkMonitoring` action → `actions.rs` spawns polling task
   - Polling task calls `get_http_profile()` → `VmServiceHttpProfileReceived` message
   - Handler merges entries into `NetworkState`
   - TUI renders `NetworkMonitor` from `session.network`

   Verify each link in this chain compiles and the types match.

5. **Detail fetch flow**: Trace:
   - User navigates to request → `NetworkNavigate(Down)` message
   - Handler selects request → `FetchHttpRequestDetail` action
   - Action spawns task → `VmServiceHttpRequestDetailReceived` message
   - Handler stores detail → TUI renders `RequestDetails`

#### Existing test regression check

Run existing test suites and verify no regressions:

```bash
# Core: all existing tests still pass
cargo test -p fdemon-core

# Daemon: all existing tests still pass
cargo test -p fdemon-daemon

# App: all existing tests still pass (including devtools handler tests)
cargo test -p fdemon-app

# TUI: all existing tests still pass (including devtools widget tests)
cargo test -p fdemon-tui

# Integration tests
cargo test --test '*'
```

#### Verify all `DevToolsPanel` match arms

The addition of `DevToolsPanel::Network` requires exhaustive match updates. Verify no unhandled match arms:

1. `handler/devtools/mod.rs` — `handle_switch_panel()` has `Network` arm
2. `handler/keys.rs` — `handle_key_devtools()` handles `in_network` guards
3. `widgets/devtools/mod.rs` — `render()` has `Network` arm
4. `widgets/devtools/mod.rs` — `render_footer()` has `Network` arm
5. `widgets/devtools/mod.rs` — `render_tab_bar()` includes `Network` tab
6. `state.rs` — `DevToolsViewState::reset()` handles Network state (if applicable)
7. Any other match on `DevToolsPanel` found via grep

#### Test count verification

Count new tests added across Phase 4:

```bash
# Count tests per file
cargo test -p fdemon-core -- network 2>&1 | grep "test result"
cargo test -p fdemon-daemon -- network 2>&1 | grep "test result"
cargo test -p fdemon-app -- network 2>&1 | grep "test result"
cargo test -p fdemon-tui -- network 2>&1 | grep "test result"
```

Target: **30+ new tests** across all crates.

#### Visual spot-check (manual)

If possible, run the application and verify:
1. `d` enters DevTools mode
2. `n` switches to Network tab
3. Tab bar shows `[i] Inspector  [p] Performance  [n] Network`
4. Disconnected state shows appropriate message
5. When VM connects, recording indicator appears
6. (If Flutter app with HTTP calls): requests appear in the table
7. Navigation with Up/Down highlights rows
8. Space toggles recording indicator
9. Switching between i/p/n tabs preserves state

#### Fix any issues found

Common issues to check for:
- Missing imports after adding new modules
- Lifetime issues in widget structs borrowing from `NetworkState`
- `match` exhaustiveness after adding `DevToolsPanel::Network`
- Clippy warnings about unused imports, dead code, or naming
- Test helper construction sites not updated for new struct fields

### Acceptance Criteria

1. `cargo fmt --all` — no formatting changes
2. `cargo check --workspace` — clean compilation
3. `cargo test --workspace` — all tests pass (0 failures)
4. `cargo clippy --workspace -- -D warnings` — no warnings
5. 30+ new tests added across all crates
6. No regressions in existing test suites
7. All `DevToolsPanel` match arms handle `Network` variant
8. All network message variants wired in `update.rs`
9. All hydration functions in `process.rs` chain
10. Tab bar shows Network tab
11. Footer hints appropriate for Network panel state

### Testing

This task IS the testing task. No new tests are written here — it verifies that all tests from Tasks 01-08 pass together.

### Notes

- **No documentation updates in this task**: Documentation updates (KEYBINDINGS.md, ARCHITECTURE.md) are deferred to Phase 5, which is the polish/documentation phase.
- **E2E testing limitations**: Full end-to-end testing of the network monitoring flow requires a running Flutter app making HTTP requests. Unit tests cover the parsing and rendering logic. Integration testing of the full poll → message → render pipeline would require mocking the VM service, which is complex. Manual spot-checking is the pragmatic approach for the initial implementation.
- **Snapshot test updates**: If the project uses snapshot tests (the `.snap.new` file in git status suggests this), the startup screen snapshot may need updating if the DevTools tab bar is visible in the snapshot. Update the snapshot if needed.

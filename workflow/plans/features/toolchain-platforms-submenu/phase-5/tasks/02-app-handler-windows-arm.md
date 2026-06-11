## Task: App handler — fold `PlatformWindows` into the guided-only arm

**Objective**: Replace the `PlatformWindows` "Available in a later phase" placeholder arm in
`handle_run_selected_step` with the guided-only behaviour already used by `PlatformWeb` /
`PlatformIos` / `PlatformMacos`, and replace the obsolete placeholder test. `actions.rs` only.

**Depends on**: Task 01 (merged — workspace compiles with `ComponentKind::VisualStudioCpp`).
Runs in parallel with Task 03 (write-disjoint: this task touches only
`handler/install_wizard/actions.rs`; Task 03 touches only `install_wizard/state.rs`).

**Agent:** implementor

**Complexity:** low

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/install_wizard/actions.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/state.rs` — `selected_step()` / `guided_commands` (runtime
  read; no compile dependency on Task 03's builder).
- `crates/fdemon-app/src/install_wizard/types.rs` — `WizardStepKind`.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name/variant.

#### 1. `handle_run_selected_step` — merge the arms

Delete the placeholder arm (~`actions.rs:401`):

```rust
WizardStepKind::PlatformWindows => {
    // Windows platform support is implemented in Phase 5.
    state.install_wizard_state.status_message =
        Some("Available in a later phase".to_string());
    UpdateResult::none()
}
```

and add `WizardStepKind::PlatformWindows` to the existing guided-only `|`-arm (~`actions.rs:385`):

```rust
WizardStepKind::PlatformIos
| WizardStepKind::PlatformMacos
| WizardStepKind::PlatformWeb
| WizardStepKind::PlatformWindows => {
    let has_guided = …; // unchanged body
    …
}
```

Behaviour is identical to the other guided leaves: when the selected step has guided commands, set the
`"Run the listed command(s), then press r to re-check."` status message; always return
`UpdateResult::none()`; **never** `begin_step` / `RunWizardStep`.

#### 2. `handle_step_completed` — no change

Guided-only leaves never complete through this path (the chain handles only `FlutterSdk`,
`PlatformAndroid`, `PathConfig`). Do not add a `PlatformWindows` arm.

#### 3. Tests

- **Delete/replace** `test_windows_still_shows_later_phase` (~`actions.rs:3545`) — the placeholder
  message no longer exists.
- Add the Windows equivalents of the existing iOS/Web guided-arm tests:
  - `test_run_selected_step_windows_with_guided_commands_shows_recheck_message` — a wizard state whose
    selected step is a `PlatformWindows` leaf carrying a non-empty `guided_commands` vec (construct the
    `WizardStep` inline, as the iOS tests do — no dependency on Task 03's builder) → status message is
    the re-check text; result is `none()` (no `UpdateAction`).
  - `test_run_selected_step_windows_without_guided_commands_is_noop` — empty `guided_commands` → no
    status message change, result `none()`.
- Sweep for any other test asserting the `"Available in a later phase"` string for Windows
  (`grep -n "later phase"` across `fdemon-app` and `fdemon-tui`) and update what this file owns; report
  any hit in files owned by Tasks 03/04 in the completion summary instead of editing them.

### Acceptance Criteria

1. `handle_run_selected_step` has no `PlatformWindows`-specific arm; the kind is part of the shared
   guided-only `|`-pattern; behaviour matches Web/iOS/macOS exactly.
2. No `"Available in a later phase"` references remain in `actions.rs` (code or tests).
3. `cargo test -p fdemon-app --lib` green; `cargo fmt --all` + `cargo clippy --workspace -- -D warnings`
   clean.

### Testing

```bash
cargo test -p fdemon-app --lib handler::install_wizard
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

### Notes

- **Write-disjointness is the contract** — do not touch `install_wizard/state.rs` (Task 03 owns it this
  wave), even for test helpers; construct test steps inline.
- The guided arm reads `guided_commands` at runtime, so this task is correct whether it merges before
  or after Task 03 (before: the leaf has no commands yet, so Enter is a silent no-op — same as iOS
  between its Tasks 02/03).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a505aa95f07b3d6a2 (synced from feat/toolchain-platforms-submenu)

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Merged `PlatformWindows` into the shared guided-only arm; deleted obsolete placeholder test; added two new tests for Windows with/without guided commands |

### Notable Decisions/Tradeoffs

1. **Guided-only unification**: PlatformWindows follows the exact same pattern as PlatformWeb/PlatformIos/PlatformMacos — checks for `guided_commands` at runtime and shows the re-check message only when present. This ensures consistency across all platform leaves and keeps the code maintainable.

2. **Test helper functions**: Created helper functions `make_windows_report()`, `state_with_windows_step_and_guided_command()`, and `state_with_windows_step_no_guided_commands()` to mirror the existing iOS/macOS test patterns, ensuring consistency and avoiding duplication.

3. **Write-disjointness preserved**: All changes are confined to `actions.rs`; no modifications to `install_wizard/state.rs`, maintaining the parallel-task contract with Task 03.

### Testing Performed

- `cargo test -p fdemon-app --lib handler::install_wizard` - Passed (134 tests)
- `cargo test -p fdemon-app --lib test_run_selected_step_windows_*` - Passed (2 new Windows tests)
- `cargo test -p fdemon-app --lib` - Passed (3015 tests)
- `cargo fmt --all -- --check` - Passed (no formatting issues)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)
- `cargo check --workspace --all-targets` - Passed (all crates compile)

### Risks/Limitations

1. **No pre-existing failures introduced**: One pre-existing failure exists in fdemon-daemon (`test_run_preflight_nonexistent_sdk_path_does_not_panic`), which is unrelated to this task and remains unchanged.

2. **Task 03 merge order independence**: The implementation correctly handles both merge orders (before or after Task 03 populates guided_commands). If this task merges first, Windows has no guided commands yet, so Enter is a silent no-op. After Task 03, guided commands are present and the re-check message appears. This is identical to how iOS was tested between its Tasks 02 and 03.

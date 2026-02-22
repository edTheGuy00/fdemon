## Task: Final Test and Cleanup

**Objective**: Run the full quality gate across the workspace, verify cross-panel interactions, confirm documentation accuracy, and ensure no regressions from Phase 5 changes. This is the final gating task before Phase 5 is considered complete.

**Depends on**: 01, 02, 03, 04, 05, 06, 07

### Scope

- Workspace-wide verification (all 4 crates + binary)
- Cross-panel interaction verification
- Documentation accuracy check
- Dead code audit

### Details

#### 1. Full quality gate

Run the complete verification pipeline:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

All four commands must pass cleanly.

#### 2. Cross-panel state preservation

Verify the following scenarios work correctly by examining the handler and state code:

| Scenario | Expected Behavior |
|----------|-------------------|
| Switch Inspector → Performance → Network → Inspector | Each panel's state preserved (tree selection, frame selection, request selection) |
| Resize terminal while in Performance panel | Frame chart and memory chart adapt; selected frame preserved |
| VM Service disconnect while in Network panel | Recording stops, entries remain visible, "Disconnected" shown |
| VM Service reconnect after disconnect | Monitoring resumes automatically when tab is reactivated |
| Enter DevTools → exit → re-enter | `DevToolsViewState` resets (active panel to default), but session state (`PerformanceState`, `NetworkState`) persists |
| Close session while in DevTools mode | DevTools exits gracefully, no panic |

Write handler tests for any scenarios not already covered.

#### 3. Dead code audit

Verify no `#[allow(dead_code)]` annotations remain on DevTools-related code that should now be wired:

```bash
# Should find zero results in devtools files after Phase 5
grep -r "allow(dead_code)" crates/fdemon-app/src/session/performance.rs
grep -r "allow(dead_code)" crates/fdemon-app/src/session/network.rs
grep -r "allow(dead_code)" crates/fdemon-app/src/handler/devtools/
```

The `AllocationSortColumn` and `allocation_sort` annotations should be gone after Task 02.

Note: `#[allow(dead_code)]` in other files (e.g., `config/vscode.rs` for deserialized-but-unused fields, `theme/` for pre-built styles) are acceptable and out of scope.

#### 4. Configuration round-trip test

Verify the generated default config loads correctly:

1. Delete `.fdemon/config.toml` (or use a temp directory)
2. Run `cargo run -- --help` or trigger config generation
3. Load the generated config
4. Verify all `[devtools]` fields have correct defaults
5. Modify each network config field, save, reload, verify

This can be a manual verification step or an automated test.

#### 5. Documentation accuracy check

Verify each documentation file against the code:

| Document | Check |
|----------|-------|
| `docs/KEYBINDINGS.md` | Every key in `handler/keys.rs` DevTools section has a corresponding row |
| `docs/ARCHITECTURE.md` | Every file path in the DevTools section exists |
| `CLAUDE.md` | Test count matches `cargo test --workspace` output |
| Generated `config.toml` | Every `[devtools]` field matches `DevToolsSettings` struct |

#### 6. Clippy and formatting

Ensure no new warnings introduced by Phase 5 changes:

```bash
cargo clippy --workspace -- -D warnings 2>&1 | grep -i "warning\|error"
```

Should produce zero output.

#### 7. Test count verification

Record the final test count for the project:

```bash
cargo test --workspace 2>&1 | grep "test result"
```

Update the TASKS.md success criteria with the actual numbers.

### Acceptance Criteria

1. `cargo fmt --all --check` passes (no formatting issues)
2. `cargo check --workspace` passes (no compilation errors)
3. `cargo test --workspace` passes (all tests green)
4. `cargo clippy --workspace -- -D warnings` passes (zero warnings)
5. No `#[allow(dead_code)]` on `AllocationSortColumn` or `allocation_sort`
6. Cross-panel state preservation verified
7. VM disconnect/reconnect handling verified across all panels
8. Documentation matches implementation
9. Generated config template is valid TOML and includes all `[devtools]` fields
10. No regressions in any existing functionality

### Testing

```bash
# Full quality gate
cargo fmt --all --check && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings

# Per-crate devtools tests
cargo test -p fdemon-core -- network
cargo test -p fdemon-core -- performance
cargo test -p fdemon-core -- widget_tree
cargo test -p fdemon-daemon -- vm_service
cargo test -p fdemon-app -- devtools
cargo test -p fdemon-app -- network
cargo test -p fdemon-app -- performance
cargo test -p fdemon-tui -- devtools
cargo test -p fdemon-tui -- network
cargo test -p fdemon-tui -- inspector
cargo test -p fdemon-tui -- performance
```

### Notes

- **This is a verification task, not an implementation task.** The primary work is running commands and checking results. Code changes should be minimal (only fixes for issues discovered during verification).
- **If issues are found**: Fix them inline in this task rather than creating new tasks. Phase 5 is the final phase — any remaining issues should be resolved here.
- **Test count**: The total test count should be recorded in the completion summary. This number will be used to update CLAUDE.md (Task 07) if it wasn't already accurate.

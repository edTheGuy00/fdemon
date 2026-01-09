## Task: Update Snapshot Tests

**Objective**: Update all snapshot tests that contain "Waiting for Flutter..." to reflect the new "Not Connected" message.

**Depends on**: Phase 1 and Phase 2 complete

### Scope

- `src/tui/render/tests.rs`: May need test adjustments
- `src/tui/render/snapshots/`: Snapshot files to regenerate

### Details

**Affected snapshot files (from grep results):**

1. `flutter_demon__tui__render__tests__confirm_dialog_quit.snap`
2. `flutter_demon__tui__render__tests__normal_initializing.snap`
3. `flutter_demon__tui__render__tests__normal_stopped.snap`
4. `flutter_demon__tui__render__tests__normal_reloading.snap`
5. `flutter_demon__tui__render__tests__search_input_mode.snap`
6. `flutter_demon__tui__render__tests__normal_running.snap`
7. `flutter_demon__tui__render__tests__long_project_name.snap`
8. `flutter_demon__tui__render__tests__compact_normal.snap`
9. `flutter_demon__tui__render__tests__no_project_name.snap`
10. `flutter_demon__tui__render__tests__confirm_dialog_quit_multiple.snap`

**Expected changes in snapshots:**

Before:
```
│                            Waiting for Flutter...                            │
│                                                                              │
│                Make sure you're in a Flutter project directory               │
```

After:
```
│                              Not Connected                                   │
│                                                                              │
│                     Press + to start a new session                           │
```

**Regeneration process:**

1. Run tests to see failures:
```bash
cargo test render::tests -- --nocapture
```

2. Review the diff to ensure changes are as expected

3. Update snapshots:
```bash
# Using insta (if installed)
cargo insta review

# Or manually update
UPDATE_EXPECT=1 cargo test render::tests
```

**Note on test setup:**

Some tests may create sessions or set specific UI states. Review each failing test to ensure:
- Tests that should show "Not Connected" don't create sessions
- Tests that should show running state do create sessions

### Acceptance Criteria

1. All snapshot tests pass
2. Snapshots show "Not Connected" and "Press + to start a new session"
3. No snapshots contain "Waiting for Flutter..."
4. Tests with active sessions still show appropriate state
5. All render tests pass: `cargo test render::tests`

### Testing

```bash
# Run snapshot tests
cargo test render::tests -- --nocapture

# If using insta for snapshot management
cargo insta test
cargo insta review

# Full test run
cargo test --lib
```

### Notes

- The insta crate is used for snapshot testing (see Cargo.toml dev-dependencies)
- Snapshots are stored in `src/tui/render/snapshots/`
- Some E2E snapshots in `tests/e2e/snapshots/` may also need updates (handled in task 03)
- If tests create sessions, they should still show the old "Running" state behavior

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**
(To be filled after implementation)

**Testing Performed:**
- `cargo test render::tests` - Pending
- Snapshot review - Pending

## Task: Fix Loading State Not Set Before Scan Begins

**Objective**: Ensure the Flutter Version panel shows "Scanning..." immediately when opened, instead of briefly flashing "No versions found" before the scan result arrives.

**Depends on**: None

**Severity**: MAJOR — visual flash/flicker on every panel open

### Scope

- `crates/fdemon-app/src/flutter_version/state.rs`: Change `VersionListState::default()` to initialize `loading: true`

### Details

#### The Bug

**File:** `crates/fdemon-app/src/flutter_version/state.rs`, lines 99-109

```rust
impl Default for VersionListState {
    fn default() -> Self {
        Self {
            installed_versions: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            loading: false,   // <-- BUG: should be true
            error: None,
            last_known_visible_height: Cell::new(0),
        }
    }
}
```

The flow when the panel opens:
1. `handle_show()` calls `state.show_flutter_version()`
2. `show_flutter_version()` creates `FlutterVersionState::new(...)` which uses `VersionListState::default()` → `loading: false`
3. `handle_show()` returns `UpdateAction::ScanInstalledSdks` which triggers an async scan
4. **During the gap** between steps 2 and 3 completing, the TUI renders with `loading: false` and empty `installed_versions` → shows "No versions found"
5. When the scan completes, `loading` is set to `false` (already was) and `installed_versions` is populated

#### The Fix

Change `loading: false` to `loading: true` in `VersionListState::default()`:

```rust
impl Default for VersionListState {
    fn default() -> Self {
        Self {
            installed_versions: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            loading: true,    // Start in loading state — scan is always triggered on panel open
            error: None,
            last_known_visible_height: Cell::new(0),
        }
    }
}
```

This is the simplest and safest fix. Every construction of `VersionListState` (via `Default` or via `FlutterVersionState::new()`) is immediately followed by a scan action, so starting in `loading: true` is always correct.

#### Alternative Considered

Setting `loading = true` explicitly in `handle_show()` after calling `show_flutter_version()`. This was rejected because:
- It requires remembering to set it in every call site
- The `Default` should represent the correct initial state for the widget
- There's no scenario where `VersionListState` is created without a subsequent scan

#### Test Impact

The existing test at `navigation.rs:146` manually resets `state.flutter_version_state.version_list.loading = false` before checking. With this fix, that test setup line becomes redundant but harmless — it explicitly sets a value that was already `true` to `false` for its specific test scenario.

The test at `state.rs` that checks `assert!(!state.version_list.loading)` in `test_flutter_version_state_default` needs to be updated to `assert!(state.version_list.loading)`.

### Acceptance Criteria

1. `VersionListState::default()` initializes `loading: true`
2. Panel shows "Scanning..." immediately on open (not "No versions found")
3. Existing tests are updated to reflect the new default
4. `cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

Update the existing default test:

```rust
#[test]
fn test_version_list_state_default() {
    let state = VersionListState::default();
    assert!(state.installed_versions.is_empty());
    assert_eq!(state.selected_index, 0);
    assert_eq!(state.scroll_offset, 0);
    assert!(state.loading);  // Changed: starts in loading state
    assert!(state.error.is_none());
    assert_eq!(state.last_known_visible_height.get(), 0);
}
```

Also verify any tests that assert `loading == false` after default construction are updated.

### Notes

- This is a 1-line fix with minimal risk.
- The TUI widget already has a "Scanning..." rendering path when `loading: true` — no widget changes needed.
- After the scan completes, the handler sets `loading: false` and populates `installed_versions`, which is unchanged.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/flutter_version/state.rs` | Changed `loading: false` to `loading: true` in `VersionListState::default()`; updated `test_flutter_version_state_default` assertion from `assert!(!state.version_list.loading)` to `assert!(state.version_list.loading)` |

### Notable Decisions/Tradeoffs

1. **Single-site fix in `Default` impl**: Fixing the default in `VersionListState::default()` rather than at each call site ensures all future construction paths are correct without needing changes at every consumer. The existing test at `navigation.rs:146` that sets `loading = false` explicitly is harmless and left untouched, as it intentionally establishes a pre-test state.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1771 unit tests, 0 failed)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **None**: This is a one-line change with no downstream risk. Every code path that constructs `VersionListState` immediately triggers a scan (`UpdateAction::ScanInstalledSdks`), so initializing to `loading: true` is always correct.

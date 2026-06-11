## Task: App — caveat note on the Xcode-select guided command (Md2)

**Objective**: Add a `note` to the "Select Xcode & accept license" `GuidedCommand` in `xcode_guided_commands`
warning that the hardcoded `/Applications/Xcode.app` path is an assumption to adjust for non-standard installs
(versioned bundles, `Xcode-beta.app`, external volumes). The probe already accepts those non-standard
locations, but the remediation command points at the canonical path with `note: None`, so a power-user who
copy-pastes it (`c` to copy) could misconfigure `xcode-select`.

**Depends on**: Phase 4 (merged). Independent of Task 01 (different crate/file).

**Agent:** implementor

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs` — the `xcode_guided_commands` builder; update/add a unit
  test.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/types.rs` — `GuidedCommand` (`label`, `command`, `note`).

### Details

> Locate by symbol — `fn xcode_guided_commands`.

In `xcode_guided_commands`, the second pushed command (the
`sudo xcode-select -s /Applications/Xcode.app/Contents/Developer && … -runFirstLaunch && … -license accept`
entry) currently has `note: None`. Change it to carry a caveat, e.g.:

```rust
note: Some(
    "Adjust the path if Xcode is not in /Applications (e.g. a versioned or beta bundle).".to_string(),
),
```

Keep the command string itself unchanged (canonical path is the right default). Do **not** touch the iOS-only
`xcodebuild -downloadPlatform iOS` command, the CocoaPods command, or the `status != Partial` early-out.

> **Out of scope (deferred nitpick):** conditionally gating the `-downloadPlatform iOS` guided command on a
> detected Xcode ≥ 16 — leave as-is.

### Acceptance Criteria

1. The "Select Xcode & accept license" guided command has a non-empty `note` mentioning that the
   `/Applications/Xcode.app` path is an assumption to adjust for non-standard installs.
2. No other guided command, status logic, or the `Partial`-gate is changed; the `include_ios_platform`
   behavior is unchanged.
3. `cargo test -p fdemon-app --lib` green; `cargo fmt --all` + `cargo clippy -p fdemon-app -- -D warnings`
   clean.

### Testing

- Extend an existing `xcode_guided_commands` / iOS-leaf test (or add
  `test_xcode_select_command_has_path_caveat_note`) to assert the select-Xcode command's `note` is `Some(..)`
  and contains the path-adjustment guidance. Keep the existing assertions
  (`-downloadPlatform iOS` present for iOS, absent for macOS; `brew install cocoapods` only when CocoaPods
  not Ok) intact.

### Notes

- Pure data change in a pure builder — `build_steps` stays pure-on-report. No handler/TUI change.
- Write-disjoint from Task 01 (`state.rs` vs `ios.rs`) → runs in a parallel worktree.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ae79deadbdffc0671

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | Added `note: Some(...)` to the "Select Xcode & accept license" GuidedCommand in `xcode_guided_commands` function (line 1125-1128); added unit test `test_xcode_select_command_has_path_caveat_note` to verify note presence and content (lines 4972-5012) |

### Notable Decisions/Tradeoffs

1. **Note Text**: Used "Adjust the path if Xcode is not in /Applications (e.g. a versioned or beta bundle)." to match the exact format suggested in the task spec. The note references both the canonical path and provides concrete examples of non-standard locations.

2. **Test Placement**: Added the test right after `test_ios_leaf_includes_download_platform_macos_does_not()` and before `test_cocoapods_only_missing_emits_only_cocoapods_command()` to keep related xcode/iOS tests grouped together.

3. **Test Assertions**: The test validates that:
   - The note is present (`Some(..)`)
   - It mentions the `/Applications` path
   - It mentions adjusting the path
   - It provides examples (versioned or beta bundles)

### Testing Performed

- `cargo test -p fdemon-app --lib` - Passed (3014 tests)
- `cargo fmt --all -- --check` - Passed (no formatting issues)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo check --workspace --all-targets` - Passed

### Risks/Limitations

None. This is a pure data change to a builder function with corresponding test coverage. The change is minimal, scoped, and does not affect other guided commands, status logic, or the Partial-gating mechanism.

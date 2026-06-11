## Task: App handler — split placeholder iOS/macOS arm into guided-only arms

**Objective**: In `handle_run_selected_step`, split the current placeholder
`PlatformIos | PlatformMacos | PlatformWindows` arm so that `PlatformIos` and `PlatformMacos` become
**guided-only** arms identical to the live `PlatformWeb` arm (show a "run the listed command(s)" hint
when the leaf has guided commands, return `none()`, never `begin_step`/`RunWizardStep`). `PlatformWindows`
keeps the "Available in a later phase" placeholder (graduated in Phase 5).

**Depends on**: Task 01 (merged) — for a clean compiling base. (The arm split itself does not reference
the new daemon variants; it reads `selected_step().guided_commands` at runtime.)

**Agent:** implementor

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — split the placeholder arm in
  `handle_run_selected_step`; add unit tests.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/types.rs` — `WizardStepKind` variants.
- `crates/fdemon-app/src/install_wizard/state.rs` — `selected_step()` / `WizardStep.guided_commands`
  (read at runtime only — no compile dependency on Task 03).

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/variant.

#### The current placeholder arm (in `handle_run_selected_step`)

```rust
WizardStepKind::PlatformIos
| WizardStepKind::PlatformMacos
| WizardStepKind::PlatformWindows => {
    // Placeholder leaves: not yet implemented.
    state.install_wizard_state.status_message =
        Some("Available in a later phase".to_string());
    UpdateResult::none()
}
```

#### The live `PlatformWeb` arm (the analog to mirror)

```rust
WizardStepKind::PlatformWeb => {
    let has_guided = state
        .install_wizard_state
        .selected_step()
        .map(|s| !s.guided_commands.is_empty())
        .unwrap_or(false);
    if has_guided {
        state.install_wizard_state.status_message =
            Some("Run the listed command(s), then press r to re-check.".to_string());
    }
    UpdateResult::none()
}
```

#### Phase 4 change — split into a shared guided-only arm + the remaining placeholder

```rust
// iOS/macOS are guided-only (mirror PlatformWeb). Identical logic → one shared arm.
WizardStepKind::PlatformIos | WizardStepKind::PlatformMacos | WizardStepKind::PlatformWeb => {
    let has_guided = state
        .install_wizard_state
        .selected_step()
        .map(|s| !s.guided_commands.is_empty())
        .unwrap_or(false);
    if has_guided {
        state.install_wizard_state.status_message =
            Some("Run the listed command(s), then press r to re-check.".to_string());
    }
    UpdateResult::none()
}

// Windows stays a placeholder until Phase 5.
WizardStepKind::PlatformWindows => {
    state.install_wizard_state.status_message =
        Some("Available in a later phase".to_string());
    UpdateResult::none()
}
```

> Folding `PlatformWeb` into the shared arm is optional but tidy (identical bodies). If the existing
> `PlatformWeb` arm is left standalone, add two new `PlatformIos` / `PlatformMacos` arms with the same
> body instead — either is acceptable as long as iOS/macOS get the `has_guided`-guarded message and
> return `none()`.

#### No other handler changes

- **`actions/mod.rs` executor**: `PlatformIos`/`PlatformMacos` remain in the non-executable catch-all
  that emits `WizardStepFailed`. This is **unreachable** for them because the handler never dispatches
  `RunWizardStep` for guided-only leaves (verified: no `begin_step`, no token, no action). Leave it.
- **`handle_step_completed`**: iOS/macOS fall through to `UpdateResult::none()` — correct (guided-only
  steps never produce a `WizardStepCompleted`). No change.
- **`navigation.rs` / `handle_show` / `handle_rerun_preflight`**: no new `RunToolchainPreflight` field
  (no iOS/macOS config). No change.
- **`selected_command_index` / `[`/`]` cycling / `c` copy**: fully data-driven, already works for any
  multi-command leaf. No change.

### Acceptance Criteria

1. Pressing `Enter` on `PlatformIos` or `PlatformMacos` sets the "Run the listed command(s)…" status
   message **only when** the leaf has guided commands, and otherwise leaves the message unchanged.
2. Neither arm calls `begin_step`, mints a `CancellationToken`, nor dispatches
   `UpdateAction::RunWizardStep`; both return `UpdateResult::none()`. `is_step_running()` stays `false`.
3. `WizardStepFailed` is never reached for `PlatformIos`/`PlatformMacos`.
4. `PlatformWindows` still shows "Available in a later phase".
5. `cargo test -p fdemon-app --lib` green; `cargo fmt --all` + `cargo clippy -p fdemon-app -- -D warnings`
   clean.

### Testing

New tests in `actions.rs` (mirror `test_run_selected_flutter_step_dispatches_install_action`, inverted):
- `test_run_selected_ios_step_is_guided_only` — build a state with a `PlatformIos` leaf carrying ≥1
  guided command selected; assert no action dispatched, `status_message` set, `is_step_running()` false.
- `test_run_selected_macos_step_is_guided_only` — same for `PlatformMacos`.
- `test_run_selected_ios_step_with_no_guided_commands_sets_no_message` — `PlatformIos` leaf with empty
  `guided_commands`; assert `status_message` is unchanged (None or prior value) and no action dispatched.

### Notes

- **The `has_guided` guard matters.** When Xcode/CocoaPods are already `Ok`, Task 03 produces an empty
  `guided_commands` list — without the guard the handler would mislead the user with "run the listed
  command" on a healthy system. `PlatformWeb` already guards this; iOS/macOS must too.
- This task is **write-disjoint** from Task 03 (`actions.rs` vs `state.rs`) and reads
  `guided_commands` at runtime, so it is correct whether it merges before or after Task 03.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Split `PlatformIos \| PlatformMacos \| PlatformWindows` placeholder arm: folded iOS/macOS into `PlatformWeb` shared guided-only arm; isolated Windows as a standalone "later phase" placeholder; updated existing `test_ios_macos_windows_still_show_later_phase` → `test_windows_still_shows_later_phase`; added three new iOS/macOS tests |

### Notable Decisions/Tradeoffs

1. **Fold PlatformWeb into shared arm**: The task said folding Web into the shared iOS/macOS arm was "optional but tidy" since the bodies are identical. I folded all three into one arm (`PlatformIos | PlatformMacos | PlatformWeb`) which removes code duplication and matches the stated intent. The existing PlatformWeb tests still pass.

2. **Existing test renamed**: `test_ios_macos_windows_still_show_later_phase` was checking that all three iOS/macOS/Windows variants show "later phase". After the split, iOS/macOS are guided-only. The test was renamed to `test_windows_still_shows_later_phase` and updated to use a Windows report (so `PlatformWindows` appears in the step list), verifying just the Windows "later phase" behaviour.

3. **macOS report needed for iOS/macOS tests**: `PlatformIos` and `PlatformMacos` are host-gated to `HostPlatform::MacOs` in `build_steps`. Tests use a `make_macos_report()` helper that sets `platform: HostPlatform::MacOs` so those steps appear in the expanded step list.

4. **Guided commands injected directly**: Task 03 (which populates real guided commands for iOS/macOS) hasn't landed yet. Tests inject `GuidedCommand` entries directly onto the step after `apply_report`, mirroring the same pattern used by the PlatformWeb tests.

### Testing Performed

- `cargo test -p fdemon-app --lib` — 3005 passed, 0 failed
- `cargo fmt --all` — clean
- `cargo clippy -p fdemon-app -- -D warnings` — clean

### Risks/Limitations

1. **No runtime change until Task 03 lands**: The `PlatformIos`/`PlatformMacos` steps currently have empty `guided_commands` (set in `build_steps`). The `has_guided` guard means Enter on those steps is silently a no-op until Task 03 populates real commands — which is the correct behaviour per the task notes.
